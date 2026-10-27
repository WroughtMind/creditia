//! An incomplete native rollout must not retract previously collected evidence.
use serde_json::{json, Value};
use std::{fs, path::PathBuf, process::Command};

struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    rollout: PathBuf,
}

impl Fixture {
    fn new(indexed: bool, compressed: bool) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "creditia-codex-history-{}-{}-{id}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let repo = root.join("repo");
        let logs = root.join("codex/sessions");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&logs).unwrap();
        let rollout = logs.join(if compressed {
            "rollout.jsonl.zst"
        } else {
            "rollout.jsonl"
        });
        let f = Self {
            root,
            repo,
            rollout,
        };
        f.git(&["init", "-q", "-b", "main"]);
        fs::write(f.repo.join("a.rs"), "let verified_codex_line = 42;\n").unwrap();
        f.git(&["add", "."]);
        f.git(&["commit", "-qm", "fixture", "--date", "2026-10-26T12:01:00Z"]);
        if indexed {
            let db = rusqlite::Connection::open(f.root.join("codex/state_5.sqlite")).unwrap();
            db.execute_batch("create table threads(id text,rollout_path text,created_at integer,updated_at integer,tokens_used integer,model text,git_branch text,cwd text,git_origin_url text);").unwrap();
            db.execute("insert into threads values('codex-history',?1,1793016000,1793016001,0,'codex-test','main',?2,'')", rusqlite::params![f.rollout.display().to_string(), f.repo.display().to_string()]).unwrap();
        }
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
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap())
            .env("HOME", &self.root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_COMMITTER_DATE", "2026-10-26T12:01:00Z")
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }

    fn transcript(&self, receipt: bool) -> String {
        let mut rows = vec![
            json!({"type":"session_meta","timestamp":"2026-10-26T12:00:00Z","payload":{"id":"codex-history","cwd":self.repo,"timestamp":"2026-10-26T12:00:00Z","originator":"codex_cli_rs","cli_version":"0.149.1"}}),
            json!({"type":"turn_context","timestamp":"2026-10-26T12:00:00Z","payload":{"model":"codex-test"}}),
            json!({"type":"response_item","timestamp":"2026-10-26T12:00:00Z","payload":{"type":"custom_tool_call","call_id":"write-a","name":"apply_patch","input":"*** Begin Patch\n*** Add File: a.rs\n+let verified_codex_line = 42;\n*** End Patch"}}),
        ];
        if receipt {
            rows.push(json!({"type":"response_item","timestamp":"2026-10-26T12:00:01Z","payload":{"type":"custom_tool_call_output","call_id":"write-a","output":"Success. Updated the following files:\nA a.rs\n"}}));
        }
        rows.into_iter().map(|v| v.to_string() + "\n").collect()
    }

    fn write(&self, text: &str) {
        let data = if self.rollout.extension().is_some_and(|ext| ext == "zst") {
            zstd::stream::encode_all(text.as_bytes(), 0).unwrap()
        } else {
            text.as_bytes().to_vec()
        };
        fs::write(&self.rollout, data).unwrap();
    }

    fn run(&self, command: &str, fresh: bool) {
        let before = fs::read(&self.rollout).unwrap();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_creditia-core"));
        cmd.args([command, "--repo"])
            .arg(&self.repo)
            .arg("--json")
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap())
            .env("HOME", &self.root)
            .env("CREDITIA_HOME", self.root.join("library"))
            .env("CODEX_HOME", self.root.join("codex"));
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
        assert_eq!(
            fs::read(&self.rollout).unwrap(),
            before,
            "native source is read-only"
        );
    }

    fn evidence(&self) -> i64 {
        let db = rusqlite::Connection::open(self.root.join("library/library.sqlite")).unwrap();
        db.query_row("select count(*) from evidence where src='codex'", [], |r| {
            r.get(0)
        })
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

fn truncated_rollout(indexed: bool) {
    let f = Fixture::new(indexed, false);
    f.write(&f.transcript(true));
    f.run("collect", false);
    assert_eq!(f.evidence(), 1);
    f.write(
        &(f.transcript(false)
            + "{\"timestamp\":\"2026-10-26T12:00:01Z\",\"type\":\"response_item\""),
    );
    f.run("collect", false);
    assert_eq!(
        f.evidence(),
        1,
        "an interrupted final row cannot retract durable evidence"
    );
    assert_eq!(
        f.verified(),
        1,
        "fresh scans restore evidence while the source is incomplete"
    );
    f.write(&f.transcript(false));
    f.run("collect", false);
    assert_eq!(
        f.evidence(),
        0,
        "a complete rebuilt history can retract a missing receipt"
    );
    assert_eq!(f.verified(), 0);
}

#[test]
fn truncated_indexed_rollout_preserves_previous_evidence() {
    truncated_rollout(true);
}

#[test]
fn truncated_unindexed_rollout_preserves_previous_evidence() {
    truncated_rollout(false);
}

#[test]
fn unreadable_compressed_rollout_does_not_suppress_saved_evidence() {
    let f = Fixture::new(true, true);
    f.write(&f.transcript(true));
    f.run("collect", false);
    assert_eq!(f.evidence(), 1);
    let raw = fs::read(&f.rollout).unwrap();
    fs::write(&f.rollout, &raw[..raw.len() / 2]).unwrap();
    f.run("collect", false);
    assert_eq!(f.evidence(), 1);
    assert_eq!(
        f.verified(),
        1,
        "a decoding failure is not an empty live history"
    );
}
