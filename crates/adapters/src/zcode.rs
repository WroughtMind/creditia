//! ZCode 的实际会话库。只读 session/message/part，按完成的工具调用提取编辑；
//! model_usage/tool_usage 是诊断与用量记录，不把同一次调用再算一遍。

use crate::{any_ts, new_text, open_ro, path_arg, Activity, Health, Stats};
use creditia_core::model::{FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_local;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn database() -> PathBuf {
    crate::source_dir(Src::Zcode).join("cli/db/db.sqlite")
}

struct ZSession {
    id: String,
    cwd: String,
    start: i64,
    end: i64,
}

fn sessions(c: &Connection) -> rusqlite::Result<Vec<ZSession>> {
    let mut q = c.prepare(
        "SELECT id, directory, time_created, time_updated FROM session ORDER BY time_created, id",
    )?;
    let rows = q.query_map([], |r| {
        let start: i64 = r.get(2)?;
        let end: i64 = r.get(3)?;
        Ok(ZSession {
            id: r.get(0)?,
            cwd: r.get(1)?,
            start: any_ts(&Value::from(start)).unwrap_or(0),
            end: any_ts(&Value::from(end)).unwrap_or(0),
        })
    })?;
    rows.collect()
}

fn total_tokens(v: &Value) -> i64 {
    v["total"].as_i64().filter(|n| *n > 0).unwrap_or_else(|| {
        ["input", "output", "reasoning"]
            .iter()
            .map(|k| v[*k].as_i64().unwrap_or(0))
            .sum::<i64>()
            + v["cache"]["read"].as_i64().unwrap_or(0)
            + v["cache"]["write"].as_i64().unwrap_or(0)
    })
}

fn parse(c: &Connection, s: &ZSession, pm: &PathMap) -> rusqlite::Result<FileGroup> {
    let mut g = FileGroup::default();
    let mut models: HashMap<String, usize> = HashMap::new();
    let mut providers: HashMap<String, usize> = HashMap::new();
    let (mut tokens, mut prompts) = (0, 0);
    let mut messages = c.prepare("SELECT id, data, time_created FROM message WHERE session_id = ?1 ORDER BY time_created, id")?;
    let mut parts = c.prepare("SELECT data, time_created FROM part WHERE message_id = ?1 AND session_id = ?2 ORDER BY time_created, id")?;
    let rows = messages.query_map([&s.id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (mid, data, created) = row?;
        let Ok(m) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        if m["role"] == "user" {
            if m["synthetic"] != true {
                prompts += 1;
            }
            continue;
        }
        if m["role"] != "assistant" {
            continue;
        }
        let model = m["modelId"]
            .as_str()
            .or(m["modelID"].as_str())
            .unwrap_or("");
        let provider = m["providerId"]
            .as_str()
            .or(m["providerID"].as_str())
            .unwrap_or("");
        if !model.is_empty() {
            *models.entry(model.into()).or_default() += 1;
        }
        if !provider.is_empty() {
            *providers.entry(provider.into()).or_default() += 1;
        }
        tokens += total_tokens(&m["tokens"]);
        let cwd = m["path"]["cwd"].as_str().unwrap_or(&s.cwd);
        for row in parts.query_map(rusqlite::params![mid, s.id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })? {
            let (data, at) = row?;
            let Ok(p) = serde_json::from_str::<Value>(&data) else {
                continue;
            };
            if p["type"] != "tool" || p["state"]["status"] != "completed" {
                continue;
            }
            let name = p["tool"].as_str().unwrap_or("");
            let input = &p["state"]["input"];
            let ts = any_ts(&p["state"]["time"]["end"])
                .or_else(|| any_ts(&Value::from(at)))
                .or_else(|| any_ts(&Value::from(created)))
                .unwrap_or(s.start);
            let absolute = |path: &str| crate::harness_paths::target(path, cwd);
            if name.eq_ignore_ascii_case("bash") {
                // This native serialization does not retain an exit code or
                // actual file changes. Completed shell transport alone cannot
                // certify which script lines were successfully written.
                continue;
            } else if name.eq_ignore_ascii_case("write") {
                let display = &p["state"]["metadata"]["display"];
                if display["kind"] != "file_diff" || display["truncated"] == true {
                    continue;
                }
                let Some(path) = display["filePath"]
                    .as_str()
                    .or_else(|| input["file_path"].as_str())
                else {
                    continue;
                };
                let added = crate::added_from_patch(&display["structuredPatch"]);
                crate::push_text(
                    &mut g.evs,
                    pm,
                    &absolute(path),
                    &added,
                    Src::Zcode,
                    model,
                    &s.id,
                    ts,
                );
            } else if let (Some(path), Some(text)) = (
                path_arg(input),
                name.eq_ignore_ascii_case("edit")
                    .then(|| new_text(name, input))
                    .flatten(),
            ) {
                crate::push_text(
                    &mut g.evs,
                    pm,
                    &absolute(&path),
                    &text,
                    Src::Zcode,
                    model,
                    &s.id,
                    ts,
                );
            }
        }
    }
    let most = |m: HashMap<String, usize>| {
        m.into_iter()
            .max_by_key(|(k, n)| (*n, std::cmp::Reverse(k.clone())))
            .map(|(k, _)| k)
    };
    g.sessions.push(Session {
        src: Src::Zcode,
        id: s.id.clone(),
        model: most(models).unwrap_or_default(),
        provider: most(providers),
        start: s.start,
        end: s.end,
        tokens,
        prompts,
        branch: None,
        reviewer: false,
        has_content: true,
    });
    Ok(g)
}

fn fingerprint(p: &Path) -> (u64, u64) {
    let stat = |p: &Path| {
        std::fs::metadata(p)
            .ok()
            .map(|m| {
                (
                    m.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(0),
                    m.len(),
                )
            })
            .unwrap_or_default()
    };
    let a = stat(p);
    let b = stat(&PathBuf::from(format!("{}-wal", p.display())));
    (a.0.max(b.0), a.1 + b.1)
}

pub fn scan(store: &mut Store, pm: &PathMap, st: &mut Stats) {
    let db = database();
    if !db.is_file() {
        return;
    }
    let result = (|| -> rusqlite::Result<usize> {
        let c = open_ro(&db)?;
        let fp = fingerprint(&db);
        let mut count = 0;
        for s in sessions(&c)?.into_iter().filter(|s| pm.in_project(&s.cwd)) {
            let key = format!("zcode:{}:{}", db.display(), s.id);
            if store.groups.get(&key).is_none_or(|g| g.fp != fp) {
                let mut g = parse(&c, &s, pm)?;
                g.fp = fp;
                store.groups.insert(key, g);
            }
            count += 1;
        }
        Ok(count)
    })();
    match result {
        Ok(n) if n > 0 => st.notes.push(creditia_core::tr!(
            "zcode: 项目相关会话 {n} 个",
            "zcode: {n} sessions for this project"
        )),
        Ok(_) => {}
        Err(e) => st.warnings.push(creditia_core::tr!(
            "zcode: 会话库读取失败：{e}",
            "zcode: couldn't read the session database: {e}"
        )),
    }
}

pub fn discover() -> Vec<Activity> {
    let all = open_ro(&database())
        .and_then(|c| sessions(&c))
        .unwrap_or_default();
    let mut by: HashMap<String, Activity> = HashMap::new();
    for s in all.into_iter().filter(|s| !s.cwd.is_empty()) {
        let a = by.entry(s.cwd.clone()).or_insert(Activity {
            src: Src::Zcode,
            cwd: s.cwd,
            remote: None,
            sessions: 0,
            last: 0,
        });
        a.sessions += 1;
        a.last = a.last.max(s.end);
    }
    by.into_values().collect()
}

pub fn doctor() -> Health {
    let db = database();
    let result = open_ro(&db).and_then(|c| sessions(&c));
    let mut h = Health {
        source: "zcode".into(),
        path: db.display().to_string(),
        format: "SQLite session / message / part".into(),
        readable: result.is_ok(),
        ..Default::default()
    };
    match result {
        Ok(all) => {
            h.records = all.len() as u64;
            h.earliest = all
                .iter()
                .map(|s| s.start)
                .filter(|t| *t > 0)
                .min()
                .map(date_local);
            h.latest = all
                .iter()
                .map(|s| s.end)
                .filter(|t| *t > 0)
                .max()
                .map(date_local);
            h.note = creditia_core::tr!(
                "读取完成的 Edit 与 Write 实际差分，保留消息模型；没有实际差分的覆盖写与缺执行码/文件结果的 shell 不产生新增行；用量表不重复计数",
                "reads completed Edit and actual Write diffs with message models; overwrites without diffs and shell calls without execution/file receipts do not produce added lines; usage tables are not counted twice"
            );
        }
        Err(e) if db.exists() => h.note = e.to_string(),
        Err(_) => {}
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use creditia_core::hash::{line_key, normalize};
    use serde_json::json;

    #[test]
    fn completed_edits_use_each_messages_model_and_exclude_rejected_calls() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE message(id TEXT, session_id TEXT, data TEXT, time_created INTEGER); CREATE TABLE part(id TEXT, message_id TEXT, session_id TEXT, data TEXT, time_created INTEGER);").unwrap();
        let pm = PathMap {
            known: ["src/a.ts".into()].into_iter().collect(),
            markers: vec!["/demo/".into()],
            excludes: vec![],
        };
        let s = ZSession {
            id: "s1".into(),
            cwd: "/work/demo".into(),
            start: 10,
            end: 30,
        };
        for (id, model, time) in [("m1", "glm-a", 10), ("m2", "glm-b", 20)] {
            c.execute("INSERT INTO message VALUES(?1,'s1',?2,?3)", rusqlite::params![id, json!({"role":"assistant","modelId":model,"providerId":"bigmodel","tokens":{"total":15,"input":10,"output":5}}).to_string(), time]).unwrap();
        }
        for (id, mid, status, tool, input) in [
            (
                "p1",
                "m1",
                "completed",
                "Write",
                json!({"file_path":"src/a.ts","content":"export const original = 1;"}),
            ),
            (
                "p2",
                "m2",
                "completed",
                "Edit",
                json!({"file_path":"src/a.ts","old_string":"export const original = 1;","new_string":"export const original = 1;\nexport const fresh = 2;"}),
            ),
            (
                "p3",
                "m2",
                "error",
                "Write",
                json!({"file_path":"src/a.ts","content":"export const rejected = 3;"}),
            ),
            (
                "p4",
                "m2",
                "running",
                "Write",
                json!({"file_path":"src/a.ts","content":"export const pending = 4;"}),
            ),
            (
                "p5",
                "m2",
                "completed",
                "Write",
                json!({"file_path":"src/a.ts","content":"human_original_line\nmodel_unknown_overwrite"}),
            ),
            (
                "p6",
                "m2",
                "completed",
                "Bash",
                json!({"command":"cat > src/a.ts <<'EOF'\nhuman_original_line\nunknown_shell_line\nEOF"}),
            ),
            (
                "p7",
                "m2",
                "completed",
                "Edit",
                json!({"file_path":"../other/src/a.ts","old_string":"old line","new_string":"outside generated line"}),
            ),
        ] {
            let mut data = json!({"type":"tool","tool":tool,"state":{"status":status,"input":input,"time":{"end":20}}});
            if id == "p1" {
                data["state"]["metadata"] = json!({"display":{"kind":"file_diff","filePath":"src/a.ts","truncated":false,"structuredPatch":[{"lines":["-human_original_line","+human_original_line","+export const original = 1;"]}]}});
            }
            c.execute(
                "INSERT INTO part VALUES(?1,?2,'s1',?3,20)",
                rusqlite::params![id, mid, data.to_string()],
            )
            .unwrap();
        }
        let g = parse(&c, &s, &pm).unwrap();
        assert_eq!(g.evs.len(), 2);
        assert_eq!(g.evs[0].model, "glm-a");
        assert_eq!(g.evs[1].model, "glm-b");
        assert_eq!(
            g.evs[1].key,
            line_key("src/a.ts", &normalize("export const fresh = 2;", "ts"))
        );
        assert_eq!(g.sessions[0].tokens, 30);
        assert_eq!(g.sessions[0].provider.as_deref(), Some("bigmodel"));
    }
}
