//! Native snapshot ordering must survive copied files, retractions and interrupted appends.
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
    repo: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "creditia-native-snapshot-{}-{}-{id}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let repo = root.join("repo");
        fs::create_dir_all(&repo).unwrap();
        let f = Self { root, repo };
        f.git(&["init", "-q", "-b", "main"]);
        for name in ["first", "second", "third"] {
            fs::write(
                f.repo.join(format!("{name}.rs")),
                format!("let proven_{name}_line = 42;\n"),
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
    fn run(&self, command: &str) -> Value {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_creditia-core"));
        cmd.args([command, "--repo"])
            .arg(&self.repo)
            .arg("--json")
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap())
            .env("CREDITIA_HOME", self.root.join("library"))
            .env("CREDITIA_POCHI_DIR", self.root.join("pochi"))
            .env("CREDITIA_MUX_DIR", self.root.join("mux"));
        if command == "analyze" {
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
    fn evidence(&self, source: &str) -> Vec<String> {
        let db = rusqlite::Connection::open(self.root.join("library/library.sqlite")).unwrap();
        let mut q = db
            .prepare("select rel from evidence where src=? order by rel,occurrence")
            .unwrap();
        q.query_map([source], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }
    fn verified(&self) -> i64 {
        self.run("analyze");
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
    fn pochi(&self, file: &str, edits: &[(&str, bool, i64)], progress: i64, cwd: bool) -> PathBuf {
        let path = self.root.join("pochi").join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("create table tasks(id text,cwd text,parentId text,createdAt integer,updatedAt integer); create table messages(taskId text,data text);").unwrap();
        db.execute(
            "insert into tasks values('shared-session',?,null,100,?)",
            rusqlite::params![if cwd { self.repo.to_str().unwrap() } else { "" }, progress],
        )
        .unwrap();
        for (name, success, ts) in edits {
            let file = self.repo.join(format!("{name}.rs"));
            let row = json!({"role":"assistant","metadata":{"startedAt":ts,"finishedAt":ts},"parts":[{"state":"output-available","output":{"success":success},"toolCallId":format!("call-{name}"),"type":"tool-writeToFile","input":{"path":file,"content":fs::read_to_string(&file).unwrap()}}]});
            db.execute(
                "insert into messages values('shared-session',?)",
                [row.to_string()],
            )
            .unwrap();
        }
        // Progress belongs to the native conversation even when this turn has no edits.
        let row = json!({"role":"user","metadata":{"startedAt":progress},"parts":[]});
        db.execute(
            "insert into messages values('shared-session',?)",
            [row.to_string()],
        )
        .unwrap();
        path
    }
    fn mux_rows(&self, names: &[&str], ts: Option<&str>) -> String {
        names.iter().map(|name| json!({"id":format!("msg-{name}"),"role":"assistant","metadata":{"model":"actual-model","timestamp":ts},"parts":[{"type":"dynamic-tool","state":"output-available","toolName":"file_edit_insert","input":{"path":self.repo.join(format!("{name}.rs"))},"output":{"success":true,"diff":format!("@@ -0,0 +1 @@\n+let proven_{name}_line = 42;\n")}}]}).to_string() + "\n").collect()
    }
    fn mux(&self, directory: &str, rows: &str) -> PathBuf {
        let path = self
            .root
            .join("mux/sessions")
            .join(directory)
            .join("same-session/chat.jsonl");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, rows).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn newer_mtime(path: &Path) {
    fs::File::open(path)
        .unwrap()
        .set_times(
            fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(4_102_444_800)),
        )
        .unwrap();
}
fn warned(result: &Value, text: &str) -> bool {
    result["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|v| v.as_str().is_some_and(|s| s.contains(text)))
}

#[test]
fn copied_database_mtime_cannot_replace_native_progress_or_prevent_retraction() {
    let f = Fixture::new();
    f.pochi(
        "current.db",
        &[("first", true, 200), ("second", true, 300)],
        310,
        true,
    );
    f.run("collect");
    assert_eq!(f.evidence("pochi"), ["first.rs", "second.rs"]);
    let copied = f.pochi("old-copy.db", &[("first", true, 200)], 210, true);
    newer_mtime(&copied);
    for _ in 0..3 {
        f.run("collect");
        assert_eq!(
            f.evidence("pochi"),
            ["first.rs", "second.rs"],
            "a newer filesystem copy is still an older native snapshot"
        );
    }
    f.pochi(
        "current.db",
        &[("first", true, 200), ("second", false, 400)],
        410,
        true,
    );
    f.run("collect");
    assert_eq!(
        f.evidence("pochi"),
        ["first.rs"],
        "later native progress retracts the former success receipt"
    );
    f.pochi("current.db", &[], 510, false);
    f.run("collect");
    assert!(
        f.evidence("pochi").is_empty(),
        "an empty snapshot still carries native progress without a displayed Session"
    );
    f.run("collect");
    assert!(f.evidence("pochi").is_empty());
}

#[test]
fn equal_or_unknown_progress_conflicts_preserve_evidence_until_one_source_remains() {
    for ts in [Some("2026-10-26T12:00:00Z"), None] {
        let f = Fixture::new();
        let current = f.mux("current", &f.mux_rows(&["first", "second"], ts));
        f.run("collect");
        assert_eq!(f.evidence("mux"), ["first.rs", "second.rs"]);
        f.mux("backup", &f.mux_rows(&["first", "second"], ts));
        let identical = f.run("collect");
        assert!(!warned(&identical, "conflicting snapshots"));
        assert_eq!(
            f.evidence("mux"),
            ["first.rs", "second.rs"],
            "identical copies do not multiply occurrences"
        );
        f.mux("backup", &f.mux_rows(&["first"], ts));
        let result = f.run("collect");
        assert!(warned(&result, "conflicting snapshots"), "{result}");
        assert_eq!(f.evidence("mux"), ["first.rs", "second.rs"]);
        assert_eq!(
            f.verified(),
            2,
            "conflicting live Sessions must not hide durable evidence"
        );
        fs::remove_file(current).unwrap();
        for _ in 0..2 {
            f.run("collect");
            assert_eq!(
                f.evidence("mux"),
                ["first.rs"],
                "a single complete source can rebuild even without a timestamp"
            );
        }
        f.mux("backup", "");
        f.run("collect");
        assert!(
            f.evidence("mux").is_empty(),
            "one complete empty source can retract without displayed Session or native timestamp"
        );
    }
}

#[test]
fn truncated_jsonl_does_not_promote_old_copy_and_recovery_resumes_collection() {
    let f = Fixture::new();
    let current = f.mux(
        "current",
        &f.mux_rows(&["first", "second"], Some("2026-10-26T12:00:10Z")),
    );
    f.run("collect");
    f.mux(
        "backup",
        &f.mux_rows(&["first"], Some("2026-10-26T12:00:00Z")),
    );
    fs::write(
        &current,
        f.mux_rows(&["first"], Some("2026-10-26T12:00:10Z")) + "{\"id\":\"interrupted",
    )
    .unwrap();
    f.pochi("independent.db", &[("third", true, 200)], 210, true);
    let result = f.run("collect");
    assert!(warned(&result, "chat.jsonl:2:"), "{result}");
    assert_eq!(
        f.evidence("mux"),
        ["first.rs", "second.rs"],
        "an unreadable candidate cannot make an older copy authoritative"
    );
    assert_eq!(
        f.evidence("pochi"),
        ["third.rs"],
        "another source continues collecting"
    );
    assert_eq!(
        f.verified(),
        2,
        "invalid live groups do not suppress saved Mux evidence"
    );
    // A valid final row need not have a newline. Recovery immediately advances the session.
    fs::write(
        &current,
        f.mux_rows(&["first", "second", "third"], Some("2026-10-26T12:00:20Z"))
            .trim_end(),
    )
    .unwrap();
    for _ in 0..2 {
        let result = f.run("collect");
        assert!(!warned(&result, "chat.jsonl:"), "{result}");
        assert_eq!(f.evidence("mux"), ["first.rs", "second.rs", "third.rs"]);
    }
}
