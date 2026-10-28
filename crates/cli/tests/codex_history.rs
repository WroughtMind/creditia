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

fn paginated(f: &Fixture, owner: &str, base: Option<Value>, rows: &[Value]) -> String {
    let start = base
        .as_ref()
        .and_then(|b| b["end_ordinal_exclusive"].as_u64())
        .unwrap_or(0);
    let mut meta: Value =
        serde_json::from_str(f.transcript(false).lines().next().unwrap()).unwrap();
    meta["payload"]["id"] = json!(owner);
    meta["payload"]["history_mode"] = json!("paginated");
    meta["payload"]["history_base"] = base.unwrap_or(Value::Null);
    let mut all = vec![meta];
    all.extend_from_slice(rows);
    all.into_iter()
        .enumerate()
        .map(|(i, mut r)| {
            r["ordinal"] = json!(start + i as u64);
            r.to_string() + "\n"
        })
        .collect()
}

fn original_rows(f: &Fixture) -> Vec<Value> {
    f.transcript(true)
        .lines()
        .skip(1)
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}

fn base_ref(id: &str, text: &str) -> Value {
    let last: Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    json!({"thread_id":id,"end_ordinal_exclusive":last["ordinal"].as_u64().unwrap()+1,"end_byte_offset":text.len()})
}

#[test]
fn segmented_success_receipt_joins_its_own_prefix_and_rechecks_changed_ancestors() {
    for indexed in [false, true] {
        let f = Fixture::new(indexed, false);
        f.write(&f.transcript(true));
        f.run("collect", false);
        assert_eq!(f.evidence(), 1, "seed the old single-file cache");
        let rows = original_rows(&f);
        let source = f
            .root
            .join("codex/sessions/rollout-2026-10-26T12-00-00-codex-history.jsonl");
        let prefix = paginated(&f, "codex-history", None, &rows[..2]);
        fs::write(&source, &prefix).unwrap();
        f.write(&paginated(
            &f,
            "codex-history",
            Some(base_ref("codex-history", &prefix)),
            &rows[2..],
        ));
        assert_eq!(f.verified(), 1);
        f.run("collect", false);
        f.run("collect", false);
        assert_eq!(f.evidence(), 1, "inherited evidence is not duplicated");
        assert_eq!(
            fs::read_to_string(&source).unwrap(),
            prefix,
            "prefix stays read-only"
        );
        fs::write(&source, prefix.replace("= 42", "= 99")).unwrap();
        assert_eq!(
            f.verified(),
            0,
            "unchanged leaf cannot keep a stale ancestor cache"
        );
        fs::write(&source, &prefix).unwrap();
        assert_eq!(f.verified(), 1);
        fs::remove_file(&source).unwrap();
        f.run("collect", false);
        assert_eq!(
            f.evidence(),
            1,
            "missing fragments do not retract saved evidence"
        );
        fs::write(&source, &prefix).unwrap();
        assert_eq!(f.verified(), 1);
    }
}

#[test]
fn revert_uses_immutable_rollout_id_and_excludes_records_after_the_cutoff() {
    for indexed in [false, true] {
        let f = Fixture::new(indexed, false);
        let rows = original_rows(&f);
        let source = f
            .root
            .join("codex/sessions/rollout-2026-10-26T12-00-00-codex-history_previous.jsonl.zst");
        let complete = paginated(&f, "codex-history", None, &rows);
        let prefix = paginated(&f, "codex-history", None, &rows[..2]);
        fs::write(
            &source,
            zstd::stream::encode_all(complete.as_bytes(), 0).unwrap(),
        )
        .unwrap();
        let mut failed = rows[2].clone();
        failed["payload"]["output"] = json!("Failed to apply patch");
        f.write(&paginated(
            &f,
            "codex-history",
            Some(base_ref("previous", &prefix)),
            &[failed],
        ));
        assert_eq!(
            f.verified(),
            0,
            "the reverted successful receipt is outside the selected lineage"
        );
    }
}

#[test]
fn segmented_receipts_keep_the_call_directory_instead_of_the_current_header() {
    for foreign in [false, true] {
        let f = Fixture::new(false, false);
        let rows = original_rows(&f);
        let source = f
            .root
            .join("codex/sessions/rollout-2026-10-26T12-00-00-codex-history.jsonl");
        let mut prefix: Vec<Value> = paginated(&f, "codex-history", None, &rows[..2])
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        if foreign {
            prefix[0]["payload"]["cwd"] = json!(f.root.join("other-project"));
        }
        let prefix: String = prefix.into_iter().map(|r| r.to_string() + "\n").collect();
        fs::write(&source, &prefix).unwrap();
        let mut leaf: Vec<Value> = paginated(
            &f,
            "codex-history",
            Some(base_ref("codex-history", &prefix)),
            &rows[2..],
        )
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
        leaf[0]["payload"]["cwd"] = json!(f.repo.join("subdirectory"));
        f.write(
            &leaf
                .into_iter()
                .map(|r| r.to_string() + "\n")
                .collect::<String>(),
        );
        assert_eq!(
            f.verified(),
            if foreign { 0 } else { 1 },
            "receipt must use the call's directory"
        );
    }
}

#[test]
fn fork_does_not_inherit_parent_edits_or_complete_a_parent_call() {
    let f = Fixture::new(false, false);
    let rows = original_rows(&f);
    let mut parent: Vec<Value> = paginated(&f, "parent", None, &rows[..2])
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    parent[0]["payload"]["cwd"] = json!(f.root.join("foreign-project"));
    let prefix: String = parent.into_iter().map(|r| r.to_string() + "\n").collect();
    let source = f
        .root
        .join("codex/sessions/rollout-2026-10-26T12-00-00-parent.jsonl");
    fs::write(&source, &prefix).unwrap();
    f.write(&paginated(
        &f,
        "codex-history",
        Some(base_ref("parent", &prefix)),
        &rows[2..],
    ));
    assert_eq!(
        f.verified(),
        0,
        "a child's receipt cannot confirm a foreign parent's call"
    );
    f.write(&paginated(
        &f,
        "codex-history",
        Some(base_ref("parent", &prefix)),
        &rows,
    ));
    assert_eq!(
        f.verified(),
        1,
        "the child's own successful execution counts"
    );
    assert_eq!(f.evidence(), 1);
}

#[test]
fn malformed_segment_boundaries_and_cycles_never_count_as_completed_edits() {
    let f = Fixture::new(false, false);
    let rows = original_rows(&f);
    let prefix = paginated(&f, "parent", None, &rows[..2]);
    let source = f
        .root
        .join("codex/sessions/rollout-2026-10-26T12-00-00-parent.jsonl");
    fs::write(&source, &prefix).unwrap();
    for field in ["end_ordinal_exclusive", "end_byte_offset"] {
        let mut base = base_ref("parent", &prefix);
        base[field] = json!(base[field].as_u64().unwrap() + 1);
        f.write(&paginated(&f, "codex-history", Some(base), &rows));
        assert_eq!(f.verified(), 0);
    }
    let circular = paginated(&f, "parent", Some(base_ref("parent", &prefix)), &rows);
    fs::write(&source, &circular).unwrap();
    f.write(&paginated(
        &f,
        "codex-history",
        Some(base_ref("parent", &circular)),
        &rows,
    ));
    assert_eq!(f.verified(), 0);
}

#[test]
fn copied_subagent_prefix_is_context_not_new_execution() {
    let f = Fixture::new(false, false);
    let rows = original_rows(&f);
    let mut copied: Vec<Value> = paginated(&f, "codex-history", None, &rows)
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    copied[0]["payload"]["subagent_history_start_ordinal"] = json!(3);
    f.write(
        &copied
            .into_iter()
            .map(|r| r.to_string() + "\n")
            .collect::<String>(),
    );
    assert_eq!(f.verified(), 0);
}
