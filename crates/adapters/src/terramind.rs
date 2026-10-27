//! Nucleus 0.2.177: local native tool execution journal, joined to its owning chat.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    if let Some(p) = std::env::var_os("CREDITIA_TERRAMIND_DIR") {
        return p.into();
    }
    #[cfg(target_os = "macos")]
    let base = crate::home().join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default());
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".config"));
    // APP_IDENTITY.stable keeps Electron's original package-name directory;
    // changing the visible app name to Nucleus does not migrate stored history.
    base.join("nucleus-desktop")
}
fn paths() -> Vec<PathBuf> {
    let r = root();
    let p = if r.is_file() {
        r
    } else {
        r.join("data/agents.db")
    };
    p.is_file().then_some(p).into_iter().collect()
}
fn added(registration: &str, args: &Value, receipt: &str) -> String {
    let path = n::text(&args["file_path"]);
    match registration {
        "native:Write"
            if args["content"].is_string()
                && receipt
                    == format!(
                        "Successfully wrote {} bytes to {path}",
                        n::text(&args["content"]).len()
                    ) =>
        {
            n::text(&args["content"]).into()
        }
        "native:Edit"
            if args["old_string"].is_string()
                && args["new_string"].is_string()
                && args["replace_all"] != true
                && receipt == format!("Successfully edited {path}") =>
        {
            crate::added_lines(n::text(&args["old_string"]), n::text(&args["new_string"]))
        }
        _ => String::new(),
    }
}
fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    db.execute_batch("begin").map_err(|e| e.to_string())?;
    let mut sessions=db.prepare("select s.id,s.messages,coalesce(c.worktree_path,p.path),s.created_at,s.updated_at from sub_chats s join chats c on c.id=s.chat_id join projects p on p.id=c.project_id where s.execution_env='local'").map_err(|e|e.to_string())?;
    let rows = sessions
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut q=db.prepare("select step_id,registration_id,normalized_arguments,output,started_at,settled_at from harness_tool_executions where sub_chat_id=?1 and location='local' and state='settled' and registration_id in ('native:Write','native:Edit') order by started_at,id").map_err(|e|e.to_string())?;
    let mut g = FileGroup::default();
    for row in rows {
        let (id, raw, cwd, start, end) = row.map_err(|e| e.to_string())?;
        let messages: Vec<Value> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let rows = q
            .query_map([&id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let before = g.evs.len();
        for row in rows {
            let (step, registration, raw, receipt, started, settled) =
                row.map_err(|e| e.to_string())?;
            let args: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            let text = added(&registration, &args, receipt.as_deref().unwrap_or(""));
            let dest = n::text(&args["file_path"]);
            // Native file tools require absolute paths. Do not guess a cloud/worktree mapping.
            if !(Path::new(dest).is_absolute() || dest.get(1..2) == Some(":")) {
                continue;
            }
            let model = messages
                .iter()
                .find(|m| m["id"] == step && m["role"] == "assistant")
                .map(|m| n::text(&m["metadata"]["model"]))
                .unwrap_or("");
            imp::push_scoped(
                &mut g,
                pm,
                "",
                dest,
                &text,
                Src::Terramind,
                model,
                &id,
                settled.unwrap_or(started) / 1000,
            );
        }
        n::session(
            &mut g,
            pm,
            Src::Terramind,
            &id,
            &cwd,
            start,
            end,
            messages.iter().filter(|m| m["role"] == "user").count() as i64,
            before,
        );
    }
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(Src::Terramind, store, pm, st, paths(), read);
}
pub fn discover() -> Vec<Activity> {
    let mut out = vec![];
    for p in paths() {
        if let Ok(db) = crate::open_ro(&p) {
            if let Ok(mut q)=db.prepare("select distinct coalesce(c.worktree_path,p.path) from sub_chats s join chats c on c.id=s.chat_id join projects p on p.id=c.project_id where s.execution_env='local'"){if let Ok(rows)=q.query_map([],|r|r.get::<_,String>(0)){out.extend(rows.filter_map(Result::ok).map(|cwd|imp::activity(Src::Terramind,cwd,None,0)));}}
        }
    }
    out
}
pub fn doctor() -> Health {
    imp::health(Src::Terramind,&root(),"data/agents.db: harness_tool_executions + sub_chats",paths().len(),"Nucleus settled local native Write/Edit receipts, absolute paths and per-step model. Cloud/provider tools, uncertain outcomes, replace-all edits and message-only previews are excluded.",false)
}
