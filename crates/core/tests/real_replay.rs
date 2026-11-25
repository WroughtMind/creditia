#[path = "support/replay.rs"]
mod replay;
use creditia_core::model::Src;
use replay::checks::{assert_pruned_replay, public_input, public_json, team_authors};
use serde_json::json;

const INPUT: &str = include_str!("fixtures/real/multi-tool.json");
const REVIEWED_KEYS: &[&str] = &[
    "claude/",
    "codex/",
    "cursor/",
    "grok/",
    "kimi/",
    "codex@openai.com",
    "codex@users.noreply.github.com",
    "cursoragent@cursor.com",
    "noreply@anthropic.com",
];
const REVIEWED_LABELS: &[&str] = &[
    "claude",
    "claude-code",
    "codex",
    "cursor",
    "grok",
    "kimi",
    "anthropic/claude-fable-5.1",
    "gpt-6-astra",
    "gpt-6.1-sol",
    "codex/branch-3",
    "codex/branch-4",
    "41898282+github-actions[bot]@users.noreply.github.com",
    "claude opus 5.5 <noreply@anthropic.com>",
];

fn multi_tool() -> replay::Input {
    public_input(INPUT, REVIEWED_KEYS, REVIEWED_LABELS).unwrap()
}

#[test]
fn real_multi_tool_snapshot_replays_every_position_and_level() {
    let expected = public_json(
        include_str!("fixtures/real/multi-tool.expected.json"),
        &[
            "rows",
            "counts",
            "claude-code:Declared",
            "claude-code:Inferred",
            "claude-code:Verified",
            "codex:Verified",
            "unattributed:Unattributed",
        ],
        &[
            "a:claude-code/claude-fable-5-1",
            "a:claude-code/claude-opus-5-5",
            "a:codex/gpt-6-astra",
            "a:codex/gpt-6.1-sol",
            "declared",
            "inferred",
            "unattributed",
            "unattributed:1",
            "verified",
        ],
    )
    .unwrap();
    assert_eq!(replay::replay(&multi_tool()), expected);
}

#[test]
fn removing_codex_evidence_cannot_leave_verified_codex_lines() {
    let before = multi_tool();
    let mut after = multi_tool();
    after.events.retain(|e| e.src != Src::Codex);
    after.sessions.retain(|s| s.src != Src::Codex);
    let result = assert_pruned_replay(&before, &after);
    assert!(result["counts"].get("codex:Verified").is_none());
    assert_eq!(result["rows"].as_array().unwrap().len(), 151);
    assert!(result["counts"]["claude-code:Verified"].as_u64().unwrap() > 0);
    // This subset retains a full Codex receipt for 48 lines. Removing its log
    // must downgrade those rows to declared, rather than erasing the receipt.
    assert_eq!(result["counts"]["codex:Declared"], 48);
    after.events.clear();
    after.sessions.clear();
    let empty = assert_pruned_replay(&before, &after);
    assert!(empty["rows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row[1] != "verified"));
}

#[test]
fn committed_fixture_contains_only_reviewed_aliases_and_labels() {
    let input = multi_tool();
    assert_eq!(input.commits.len(), 3);
    assert_eq!(input.events.len(), 234);
    assert_eq!(input.sessions.len(), 6);
}

const LOCAL_COMMIT: &str = "0000000000000000000000000000000000000001";
const PEER_COMMIT: &str = "0000000000000000000000000000000000000002";
const STAMPED_COMMIT: &str = "0000000000000000000000000000000000000003";

// Constructed in the test to exercise the scaffold without external records.
fn synthetic_input() -> replay::Input {
    let raw = json!({
        "commits": [
            {"hash": LOCAL_COMMIT, "email": "person-1@example.invalid", "name": "Person 1",
             "subject": "sample commit", "author_ts": 100, "commit_ts": 100,
             "files": [{"rel": "file-1.data", "keys": [11, 12]}]},
            {"hash": PEER_COMMIT, "email": "person-2@example.invalid", "name": "Person 2",
             "subject": "sample commit", "author_ts": 200, "commit_ts": 200,
             "files": [{"rel": "file-2.data", "keys": [21]}]},
            {"hash": STAMPED_COMMIT, "email": "person-2@example.invalid", "name": "Person 2",
             "subject": "sample commit", "author_ts": 300, "commit_ts": 300,
             "files": [{"rel": "file-3.data", "keys": [31, 32, 33]}]}
        ],
        "stamps": {STAMPED_COMMIT: ["codex", null]},
        "events": [
            {"rel": "file-1.data", "key": 11, "src": "codex", "model": "",
             "session": "session-1", "ts": 90},
            {"rel": "file-3.data", "key": 31, "src": "codex", "model": "",
             "session": "session-2", "ts": 280},
            {"rel": "file-3.data", "key": 32, "src": "codex", "model": "",
             "session": "session-3", "ts": 290}
        ],
        "sessions": [],
        "config": {"name": "research sample", "repo": "", "branch": "main", "level": "strict",
                   "directors": [{"id": "h:owner-1", "name": "Person 1", "emails": ["person-1@example.invalid"]}]}
    });
    public_input(&raw.to_string(), &[], &["codex"]).unwrap()
}

#[test]
fn synthetic_team_replay_separates_commit_authors_from_line_credit() {
    let input = synthetic_input();
    assert_eq!(
        team_authors(&input),
        json!({
            LOCAL_COMMIT: {"name": "Person 1", "me": true, "by": {"a:codex": 1, "unattributed": 1}},
            PEER_COMMIT: {"name": "Person 2", "me": false, "by": {"author": 1}},
            STAMPED_COMMIT: {"name": "Person 2", "me": false, "by": {"a:codex": 3}}
        })
    );
}

#[test]
fn synthetic_pruned_replay_preserves_receipts_and_retained_evidence() {
    let before = synthetic_input();
    let mut after = synthetic_input();
    after.events.retain(|e| e.session == "session-3");
    let output = assert_pruned_replay(&before, &after);
    assert_eq!(output["counts"]["codex:Declared"], 2);
    assert_eq!(output["counts"]["codex:Verified"], 1);
    let rows = output["rows"].as_array().unwrap();
    assert_eq!(rows[0][1], "unattributed");
    assert_eq!(rows[2][1], "declared");
    assert_eq!(rows[3], json!(["a:codex", "declared"]));
    assert_eq!(rows[4], json!(["a:codex", "verified"]));
}

#[test]
#[should_panic(expected = "pruned evidence contains a new or duplicated event")]
fn synthetic_pruned_replay_rejects_an_added_duplicate_occurrence() {
    let before = synthetic_input();
    let mut after = synthetic_input();
    after.events = vec![before.events[2].clone(), before.events[2].clone()];
    assert_pruned_replay(&before, &after);
}

#[test]
fn fixture_guard_rejects_unreviewed_strings_and_dictionary_keys() {
    let fields = &["name", "stamps", "session"];
    for raw in [
        r#"{"name":"Person 1 private suffix"}"#,
        r#"{"session":"session-private-id"}"#,
        r#"{"name":"file-1.data/absolute/path"}"#,
        r#"{"name":"https://fixture.invalid/private-repo"}"#,
    ] {
        assert_eq!(
            public_json(raw, fields, &[]),
            Err("unreviewed fixture string")
        );
    }
    assert_eq!(
        public_json(
            r#"{"stamps":{"private-user@fixture.invalid":1}}"#,
            fields,
            &[]
        ),
        Err("unreviewed fixture key")
    );
    assert_eq!(
        public_json(r#"{"name":"private name","name":"Person 1"}"#, fields, &[]),
        Err("duplicate keys in fixture JSON")
    );
    assert!(public_json(
        r#"{"name":"Person \u0031","session":"session-3"}"#,
        fields,
        &[]
    )
    .is_ok());
}

#[test]
fn fixture_guard_rejects_private_metadata_even_with_reviewed_labels() {
    let mut private = json!({
        "commits": [], "stamps": {}, "events": [],
        "sessions": [{"id": "session-1", "src": "codex", "model": "", "start": 100, "end": 110,
                      "tokens": 1, "branch": null, "prompts": 0, "reviewer": false, "has_content": false}],
        "config": {"name": "research sample", "repo": "", "branch": "main"}
    });
    assert_eq!(
        public_input(&private.to_string(), &[], &["codex"]).err(),
        Some("fixture contains session provider or usage metadata")
    );
    private["sessions"][0]["tokens"] = json!(0);
    private["config"]["remotes"] = json!(["branch-3"]);
    assert_eq!(
        public_input(&private.to_string(), &[], &["codex"]).err(),
        Some("fixture contains private project or display metadata")
    );
}
