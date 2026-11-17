//! Bounded real native excerpts, through the actual CLI and its evidence library.
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

const INPUT: &str = include_str!("fixtures/real-native/input.json");
const EXPECTED: &str = include_str!("fixtures/real-native/expected.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    path: String,
    versions: Vec<Version>,
    records: Vec<Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Version {
    author_time: i64,
    commit_time: i64,
    content: String,
}

struct Replay {
    root: PathBuf,
    log: PathBuf,
    commits: Vec<String>,
}
impl Replay {
    fn new(case: &Case, reject_receipt: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "creditia-real-native-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("repo")).unwrap();
        let log = root.join(match case.name.as_str() {
            "codex" => "codex/sessions/rollout-session-1.jsonl",
            "claude" => "claude/projects/project/session-1.jsonl",
            _ => panic!("unreviewed native source"),
        });
        assert!(matches!(
            case.path.as_str(),
            "docs/file-1.md" | "file-2.json"
        ));
        let mut replay = Self {
            root,
            log,
            commits: vec![],
        };
        replay.git(&["init", "-q", "-b", "main"], None);
        for (i, version) in case.versions.iter().enumerate() {
            let path = replay.root.join("repo").join(&case.path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, &version.content).unwrap();
            replay.git(&["add", &case.path], None);
            replay.git(
                &["commit", "-qm", &format!("sample commit {}", i + 1)],
                Some(version),
            );
            let hash = replay.git(&["rev-parse", "HEAD"], None);
            replay.commits.push(hash);
        }
        let mut records = case.records.clone();
        if reject_receipt {
            // Controlled negative variants, not observations of failed original writes.
            match case.name.as_str() {
                "codex" => records[2]["payload"]["item"]["status"] = json!("failed"),
                "claude" => records[1]["message"]["content"][0]["is_error"] = json!(true),
                _ => unreachable!(),
            }
        }
        fs::create_dir_all(replay.log.parent().unwrap()).unwrap();
        fs::write(
            &replay.log,
            records.iter().map(|r| format!("{r}\n")).collect::<String>(),
        )
        .unwrap();
        fs::write(replay.root.join("config.json"), json!({
            "name":"native replay", "repo":"", "branch":"main",
            "directors":[{"id":"h:owner", "name":"Person 1", "emails":["person-1@example.invalid"]}],
            "paths":["/creditia-native-fixture"]
        }).to_string()).unwrap();
        replay
    }

    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        // Only child processes receive this isolated profile. No real logs,
        // global Git hooks, credentials or evidence libraries are consulted.
        command
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap())
            .env("HOME", self.root.join("home"))
            .env("USERPROFILE", self.root.join("home"))
            .env("APPDATA", self.root.join("appdata"))
            .env("CREDITIA_HOME", self.root.join("library"))
            .env("CODEX_HOME", self.root.join("codex"))
            .env("CLAUDE_CONFIG_DIR", self.root.join("claude"))
            .env("CREDITIA_DUMP_LINES", self.root.join("lines.json"))
            .env("CREDITIA_TZ", "0")
            .env("CREDITIA_LANG", "en")
            .env("LANG", "en_US.UTF-8")
            .env("GIT_CONFIG_NOSYSTEM", "1");
        command
    }

    fn git(&self, args: &[&str], version: Option<&Version>) -> String {
        let mut command = self.command("git");
        command
            .arg("-C")
            .arg(self.root.join("repo"))
            .args([
                "-c",
                "user.name=Person 1",
                "-c",
                "user.email=person-1@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.autocrlf=false",
                "-c",
            ])
            .arg(format!(
                "core.hooksPath={}",
                self.root.join("no-hooks").display()
            ))
            .args(args);
        if let Some(version) = version {
            command
                .env("GIT_AUTHOR_DATE", format!("{} +0000", version.author_time))
                .env(
                    "GIT_COMMITTER_DATE",
                    format!("{} +0000", version.commit_time),
                );
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
    }

    fn analyze(&self) -> Value {
        let output = self
            .command(env!("CARGO_BIN_EXE_creditia-core"))
            .args(["analyze", "--repo"])
            .arg(self.root.join("repo"))
            .arg("--config")
            .arg(self.root.join("config.json"))
            .arg("--out")
            .arg(self.root.join("credits.json"))
            .args(["--fresh", "--json"])
            .output()
            .unwrap();
        assert!(
            matches!(output.status.code(), Some(0 | 2)),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let mut result = json!({
            "report": serde_json::from_slice::<Value>(&fs::read(self.root.join("credits.json")).unwrap()).unwrap(),
            "lines": serde_json::from_slice::<Value>(&fs::read(self.root.join("lines.json")).unwrap()).unwrap()
        });
        result["report"]
            .as_object_mut()
            .unwrap()
            .remove("generatedAt");
        self.normalize(&mut result);
        result
    }

    fn normalize(&self, value: &mut Value) {
        match value {
            Value::String(s) => {
                for (i, hash) in self.commits.iter().enumerate() {
                    for length in [40, 12, 10, 8, 7] {
                        *s = s.replace(&hash[..length], &format!("commit-{}", i + 1));
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|v| self.normalize(v)),
            Value::Object(items) => items.values_mut().for_each(|v| self.normalize(v)),
            _ => {}
        }
    }
}
impl Drop for Replay {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn verified(result: &Value) -> u64 {
    result["report"]["breakdown"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap())
        .sum()
}

#[test]
fn real_native_excerpts_replay_complete_reports_and_surviving_lines() {
    let cases: Vec<Case> = serde_json::from_str(INPUT).unwrap();
    let expected: Vec<Value> = serde_json::from_str(EXPECTED).unwrap();
    assert_eq!(cases.len(), expected.len());
    for (case, expected) in cases.iter().zip(expected) {
        let replay = Replay::new(case, false);
        let result = replay.analyze();
        assert_eq!(
            result["report"]["totals"]["sessions"], 1,
            "reader must actually ingest the excerpt"
        );
        assert!(verified(&result) > 0);
        assert_eq!(result, expected, "{} native replay changed", case.name);
        // --fresh drops scan caches; the native logs have gone, but the evidence
        // library must preserve the exact report and historical/survival rows.
        fs::remove_file(&replay.log).unwrap();
        assert_eq!(replay.analyze(), result, "{} controlled cleanup", case.name);
    }
}

#[test]
fn failed_receipt_variants_cannot_produce_verified_lines() {
    for case in serde_json::from_str::<Vec<Case>>(INPUT).unwrap() {
        let replay = Replay::new(&case, true);
        assert_eq!(
            verified(&replay.analyze()),
            0,
            "{} failed receipt",
            case.name
        );
    }
}

#[test]
fn native_fixture_contains_only_reviewed_fields_and_opaque_content() {
    fn visit(value: &Value) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    assert!(
                        [
                            "name",
                            "path",
                            "versions",
                            "author_time",
                            "commit_time",
                            "content",
                            "records",
                            "type",
                            "timestamp",
                            "payload",
                            "id",
                            "cwd",
                            "model",
                            "item",
                            "status",
                            "changes",
                            "/creditia-native-fixture/docs/file-1.md",
                            "message",
                            "input",
                            "file_path",
                            "tool_use_id",
                            "toolUseResult",
                            "filePath",
                            "sessionId"
                        ]
                        .contains(&key.as_str()),
                        "unreviewed field: {key}"
                    );
                    visit(value);
                }
            }
            Value::Array(values) => values.iter().for_each(visit),
            Value::String(s) => {
                let label = [
                    "codex",
                    "claude",
                    "docs/file-1.md",
                    "file-2.json",
                    "session_meta",
                    "session-1",
                    "/creditia-native-fixture",
                    "turn_context",
                    "gpt-6-astra",
                    "event_msg",
                    "item_completed",
                    "FileChange",
                    "call-1",
                    "completed",
                    "add",
                    "assistant",
                    "message-1",
                    "anthropic/claude-fable-5.1",
                    "tool_use",
                    "Write",
                    "/creditia-native-fixture/file-2.json",
                    "user",
                    "tool_result",
                    "create",
                ]
                .contains(&s.as_str());
                let time = s.starts_with("2000-")
                    && s.ends_with('Z')
                    && s.chars().all(|c| c.is_ascii_digit() || "-T:.Z".contains(c));
                let opaque = s.ends_with('\n')
                    && s.lines().all(|line| {
                        let token = line.trim_end_matches(' ');
                        token.is_empty()
                            || token.strip_prefix("opaque-").is_some_and(|n| {
                                n.len() == 4 && n.bytes().all(|b| b.is_ascii_digit())
                            })
                            || token.len() == 3 && token.bytes().all(|b| b.is_ascii_digit())
                    });
                assert!(label || time || opaque, "unreviewed native fixture string");
            }
            Value::Number(_) => {}
            _ => panic!("unreviewed native fixture value"),
        }
    }
    visit(&serde_json::from_str::<Value>(INPUT).unwrap());
}
