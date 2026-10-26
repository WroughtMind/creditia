//! Main and subagent history must remain independent after durable collection.
use serde_json::{json, Value};
use std::{fs, path::PathBuf, process::Command};

struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    logs: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "creditia-claude-history-{}-{}-{id}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let repo = root.join("repo");
        let logs = root.join("claude/projects/test");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&logs).unwrap();
        let f = Self { root, repo, logs };
        f.git(&["init", "-q", "-b", "main"]);
        for name in ["parent", "child", "missing"] {
            fs::write(
                f.repo.join(format!("{name}.rs")),
                format!("let verified_{name}_line = 42;\n"),
            )
            .unwrap();
        }
        f.git(&["add", "."]);
        f.git(&["commit", "-qm", "fixture", "--date", "2026-10-26T12:01:00Z"]);
        f
    }
    fn git(&self, args: &[&str]) {
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.repo)
            .args([
                "-c",
                "user.name=Review",
                "-c",
                "user.email=review@example.test",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .env("GIT_COMMITTER_DATE", "2026-10-26T12:01:00Z")
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
    fn transcript(&self, name: &str, receipt: bool) -> String {
        let path = self.repo.join(format!("{name}.rs"));
        let text = fs::read_to_string(&path).unwrap();
        let mut rows = vec![
            json!({"type":"assistant","sessionId":"parent-session","cwd":self.repo,"timestamp":"2026-10-26T12:00:00Z","message":{"id":format!("msg_{name}"),"model":"claude-test","content":[{"type":"tool_use","id":format!("write-{name}"),"name":"Write","input":{"file_path":path,"content":text}}]}}),
        ];
        if receipt {
            rows.push(json!({"type":"user","sessionId":"parent-session","cwd":self.repo,"timestamp":"2026-10-26T12:00:01Z","message":{"content":[{"type":"tool_result","tool_use_id":format!("write-{name}"),"is_error":false,"content":"success"}]},"toolUseResult":{"type":"create","content":text}}));
        }
        rows.into_iter().map(|v| v.to_string() + "\n").collect()
    }
    fn write(&self, name: &str, receipt: bool) -> PathBuf {
        let path = self.logs.join(if name == "parent" {
            "main.jsonl".into()
        } else {
            format!("agent-{name}.jsonl")
        });
        fs::write(&path, self.transcript(name, receipt)).unwrap();
        path
    }
    fn run(&self, command: &str, fresh: bool) -> Value {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_creditia-core"));
        cmd.args([command, "--repo"])
            .arg(&self.repo)
            .arg("--json")
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap())
            .env("CREDITIA_HOME", self.root.join("library"))
            .env("CLAUDE_CONFIG_DIR", self.root.join("claude"));
        if fresh {
            cmd.arg("--fresh");
        }
        let o = cmd.output().unwrap();
        assert!(
            matches!(o.status.code(), Some(0 | 2)),
            "{}\n{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        serde_json::from_slice(&o.stdout).unwrap()
    }
    fn db(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.root.join("library/library.sqlite")).unwrap()
    }
    fn evidence(&self) -> Vec<(String, String)> {
        let db = self.db();
        let mut q = db.prepare("select rel,session from evidence where src='claude-code' order by rel,session,occurrence").unwrap();
        q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }
    fn verified(&self) -> i64 {
        self.run("analyze", true);
        let credits: Value =
            serde_json::from_slice(&fs::read(self.repo.join(".creditia/credits.json")).unwrap())
                .unwrap();
        credits["breakdown"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                r["byEvidence"]["verified"]["linesAdded"]
                    .as_i64()
                    .unwrap_or(0)
            })
            .sum()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn missing_or_partial_subagent_history_preserves_only_its_own_durable_evidence() {
    let f = Fixture::new();
    let main = f.write("parent", true);
    let child = f.write("child", true);
    f.run("collect", false);
    assert_eq!(
        f.evidence(),
        vec![
            ("child.rs".into(), "parent-session:agent-child".into()),
            ("parent.rs".into(), "parent-session".into())
        ]
    );
    fs::write(&child, f.transcript("child", false) + "{interrupted\n").unwrap();
    f.run("collect", false);
    assert_eq!(
        f.evidence().len(),
        2,
        "malformed source cannot retract its previous snapshot"
    );
    f.write("child", false);
    f.run("collect", false);
    assert_eq!(
        f.evidence(),
        vec![("parent.rs".into(), "parent-session".into())],
        "a complete source retracts only its own missing receipt"
    );
    f.write("child", true);
    f.run("collect", false);
    fs::remove_file(&child).unwrap();
    assert_eq!(
        f.verified(),
        2,
        "fresh cache and a live parent still restore the missing child"
    );
    fs::remove_file(&main).unwrap();
    assert_eq!(
        f.verified(),
        2,
        "all native logs can disappear after collection"
    );
    assert_eq!(f.evidence().len(), 2);
}

#[test]
fn shared_parent_legacy_rows_migrate_by_exact_occurrence_without_losing_missing_siblings() {
    let f = Fixture::new();
    let main = f.write("parent", true);
    let child = f.write("child", true);
    let missing = f.write("missing", true);
    f.run("collect", false);
    // Recreate the immediately preceding evidence schema and parent-only IDs.
    // One extra identical old occurrence belongs to an unavailable sibling.
    f.db().execute_batch("alter table evidence drop column claude_legacy;
        update evidence set session='parent-session' where src='claude-code';
        insert into evidence(project,rel,key,src,model,session,ts,tab,next,single_use,occurrence)
            select project,rel,key,src,model,session,ts,tab,next,single_use,3 from evidence where rel='parent.rs';
        delete from sessions where id<>'parent-session';").unwrap();
    fs::remove_file(&missing).unwrap();
    for _ in 0..3 {
        f.run("collect", false);
        assert_eq!(
            f.evidence().len(),
            4,
            "migration retries must not consume or multiply an old occurrence"
        );
    }
    let (_, legacy): (i64, i64) = f
        .db()
        .query_row(
            "select count(*),sum(claude_legacy) from evidence",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        legacy, 2,
        "only the readable main and child claimed one matching row each"
    );
    assert_eq!(
        f.verified(),
        3,
        "unclaimed missing-sibling evidence survives a live parent"
    );
    fs::remove_file(main).unwrap();
    fs::remove_file(child).unwrap();
    assert_eq!(f.verified(), 3);
    assert_eq!(f.evidence().len(), 4);
}

#[test]
fn main_schema_upgrade_preserves_unavailable_children_until_their_exact_history_returns() {
    let f = Fixture::new();
    let main = f.write("parent", true);
    let child = f.write("child", true);
    let missing = f.write("missing", true);
    f.run("collect", false);
    // origin/main's table, including its separate `next` column migration.
    f.db().execute_batch("begin;
        create table previous_evidence(
            project text not null, rel text not null, key integer not null, src text not null, model text not null,
            session text not null, ts integer not null, tab integer not null default 0,
            primary key(project,rel,key,src,session));
        alter table previous_evidence add column next integer not null default 0;
        insert into previous_evidence(project,rel,key,src,model,session,ts,tab,next)
            select project,rel,key,src,model,'parent-session',ts,tab,next from evidence;
        drop table evidence;
        alter table previous_evidence rename to evidence;
        create index idx_ev_project on evidence(project);
        delete from sessions where id<>'parent-session';
        commit;").unwrap();
    fs::remove_file(&child).unwrap();
    fs::remove_file(&missing).unwrap();
    assert_eq!(
        f.verified(),
        3,
        "old parent rows from missing children remain usable beside the live main"
    );
    let legacy: i64 = f
        .db()
        .query_row("select sum(claude_legacy) from evidence", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        legacy, 2,
        "the rebuild-table migration must mark old shared rows"
    );
    f.write("child", true);
    for _ in 0..3 {
        assert_eq!(f.verified(), 3);
        assert_eq!(
            f.evidence().len(),
            3,
            "a returning child must claim its old row, not add another copy"
        );
    }
    let legacy: i64 = f
        .db()
        .query_row("select sum(claude_legacy) from evidence", [], |r| r.get(0))
        .unwrap();
    assert_eq!(legacy, 1);
    fs::remove_file(main).unwrap();
    fs::remove_file(child).unwrap();
    assert_eq!(f.verified(), 3);
}
