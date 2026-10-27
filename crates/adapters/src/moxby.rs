//! Moxby Desktop 3.2.0's native file_edit receipts in moxby_chats.db.
//! Pinned schema, dispatch and derivation evidence: SUPPORT_AGENT8.md.
use crate::{native_history as n, trace_import as imp, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Src, Store};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn roots() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("CREDITIA_MOXBY_DIR") {
        return vec![p.into()];
    }
    if let Some(p) = std::env::var_os("MOXBY_STATE_DIR") {
        return vec![p.into()];
    }
    #[cfg(target_os = "macos")]
    let base = crate::home().join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default());
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".local/share"));
    vec![
        base.join("com.moxby.agent"),
        base.join("com.moxby.agent.dev"),
    ]
}
fn paths() -> Vec<PathBuf> {
    roots()
        .iter()
        .flat_map(|p| n::files(p, |p| p.file_name().is_some_and(|s| s == "moxby_chats.db")))
        .collect()
}

fn edit(v: &Value) -> Option<(&str, String)> {
    if v["status"] != "applied" || v["replaced"].as_u64()? != 1 {
        return None;
    }
    let path = v["path"].as_str()?;
    if !(Path::new(path).is_absolute() || path.as_bytes().get(1) == Some(&b':')) {
        return None;
    }
    // This is the post-write structured output, not the requested input or
    // model-facing summary. v3.2.0 emits exact replacement snippets here.
    Some((
        path,
        crate::added_lines(v["oldText"].as_str()?, v["newText"].as_str()?),
    ))
}

fn read(path: &Path, pm: &PathMap) -> Result<FileGroup, String> {
    let db = crate::open_ro(path).map_err(|e| e.to_string())?;
    db.execute_batch("begin").map_err(|e| e.to_string())?;
    let mut threads = db
        .prepare("select id,chat_id,created_at,updated_at from chat_threads order by id")
        .map_err(|e| e.to_string())?;
    let rows = threads
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    // Only the terminal result currently referenced by the call can contribute.
    // Local built-in edits are distinct from delegated CLI or mod tool results.
    let mut q = db.prepare("select r.result_json,r.created_at,coalesce(t.model,'')
        from tool_calls c join tool_results r on r.tool_call_id=c.id and r.event_id=c.result_event_id and r.chat_id=c.chat_id and r.thread_id=c.thread_id
        left join chat_turns t on t.id=r.turn_id and t.chat_id=r.chat_id and t.thread_id=r.thread_id
        where c.thread_id=?1 and c.chat_id=?2 and c.tool_id='file_edit'
          and c.latest_event_id=c.result_event_id and r.status='done' and r.truncated=0
          and r.render_contract_id='file.edit.result' and r.render_contract_version='1'
          and coalesce(c.module_id,'')='' and coalesce(c.harness_id,'') not in ('claude-cli','codex-cli')
        order by r.created_at,c.id").map_err(|e| e.to_string())?;
    let mut g = FileGroup::default();
    for row in rows {
        let (id, chat, start, end) = row.map_err(|e| e.to_string())?;
        if id.is_empty() || chat.is_empty() {
            return Err("Moxby thread has no identity".into());
        }
        let before = g.evs.len();
        let edits = q
            .query_map([&id, &chat], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in edits {
            let (raw, time, model) = row.map_err(|e| e.to_string())?;
            let v: Value =
                serde_json::from_str(&raw).map_err(|e| format!("invalid Moxby receipt: {e}"))?;
            if let Some((path, text)) = edit(&v) {
                imp::push_scoped(
                    &mut g,
                    pm,
                    "",
                    path,
                    &text,
                    Src::Moxby,
                    &model,
                    &id,
                    crate::any_ts(&Value::from(time)).unwrap_or(0),
                );
            }
        }
        n::session(
            &mut g,
            pm,
            Src::Moxby,
            &id,
            "",
            crate::any_ts(&Value::from(start)).unwrap_or(0),
            crate::any_ts(&Value::from(end)).unwrap_or(0),
            0,
            before,
        );
    }
    Ok(g)
}
pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    n::scan(Src::Moxby, store, pm, st, paths(), read);
}
// Native receipt paths are files, not repository roots. Do not infer a project
// from a chat's mutable workspace setting or a common parent of edited files.
pub fn discover() -> Vec<Activity> {
    vec![]
}
pub fn doctor() -> Health {
    imp::health(Src::Moxby,&roots()[0],"moxby_chats.db: tool_calls + tool_results",paths().len(),"Desktop 3.2.0 terminal native file_edit receipts with absolute destinations and per-turn model. Single exact replacements only; truncated, failed, pending, delegated CLI, mod, shell and replace-all results are excluded.",false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_completed_exact_single_replacements_supply_text() {
        let mut v = json!({"status":"applied","replaced":1,"path":"/demo/src/main.rs","oldText":"fn old() {}","newText":"fn saved() {}"});
        assert_eq!(edit(&v).unwrap().1, "fn saved() {}\n");
        for field in ["status", "path", "oldText", "newText", "replaced"] {
            let mut missing = v.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(edit(&missing).is_none(), "{field}");
        }
        v["replaced"] = json!(2);
        assert!(edit(&v).is_none());
        v["replaced"] = json!(1);
        v["status"] = json!("proposed");
        assert!(edit(&v).is_none());
    }
}
