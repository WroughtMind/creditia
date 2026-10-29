#[path = "support/replay.rs"]
mod replay;
use creditia_core::model::Src;

const INPUT: &str = include_str!("fixtures/real/multi-tool.json");

#[test]
fn real_multi_tool_snapshot_replays_every_position_and_level() {
    let input: replay::Input = serde_json::from_str(INPUT).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/real/multi-tool.expected.json")).unwrap();
    assert_eq!(replay::replay(&input), expected);
}

#[test]
fn removing_codex_evidence_cannot_leave_verified_codex_lines() {
    let mut input: replay::Input = serde_json::from_str(INPUT).unwrap();
    input.events.retain(|e| e.src != Src::Codex);
    input.sessions.retain(|s| s.src != Src::Codex);
    let result = replay::replay(&input);
    assert!(result["counts"].get("codex:Verified").is_none());
    assert_eq!(result["rows"].as_array().unwrap().len(), 151);
    assert!(result["counts"]["claude-code:Verified"].as_u64().unwrap() > 0);
    // This subset retains a full Codex receipt for 48 lines. Removing its log
    // must downgrade those rows to declared, rather than erasing the receipt.
    assert_eq!(result["counts"]["codex:Declared"], 48);
    input.events.clear();
    input.sessions.clear();
    let empty = replay::replay(&input);
    assert!(empty["rows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row[1] != "verified"));
}

#[test]
fn committed_fixture_contains_only_reviewed_aliases_and_labels() {
    let input: replay::Input = serde_json::from_str(INPUT).unwrap();
    assert_eq!(input.commits.len(), 3);
    assert_eq!(input.events.len(), 234);
    assert_eq!(input.sessions.len(), 6);
    let alias_path = |s: &str| {
        s.strip_prefix("file-")
            .and_then(|s| s.strip_suffix(".data"))
            .is_some_and(|s| s.len() == 5 && s.bytes().all(|b| b.is_ascii_digit()))
    };
    for p in input
        .commits
        .iter()
        .flat_map(|c| &c.files)
        .map(|f| f.rel.as_str())
        .chain(input.events.iter().map(|e| e.rel.as_str()))
    {
        assert!(alias_path(p));
    }
    for session in &input.sessions {
        assert!(session.provider.is_none());
        assert_eq!(session.tokens, 0);
        assert_eq!(session.prompts, 0);
    }
    // Recursive allowlist: adding an unexpected string anywhere requires review,
    // including map keys (where private config/remote values can otherwise hide).
    fn visit(value: &serde_json::Value) {
        match value {
            serde_json::Value::String(s) => {
                let alias = s.starts_with("file-")
                    || s.starts_with("session-")
                    || s.starts_with("Person ")
                    || s.starts_with("person-")
                    || s.starts_with("h:owner-")
                    || s.starts_with("00000000000000000000000000000000000000");
                let reviewed = [
                    "",
                    "main",
                    "research sample",
                    "sample commit",
                    "claude",
                    "claude-code",
                    "codex",
                    "cursor",
                    "grok",
                    "kimi",
                    "anthropic/claude-fable-5.1",
                    "gpt-6-astra",
                    "gpt-6.1-sol",
                    "branch-1",
                    "codex/branch-3",
                    "codex/branch-4",
                    "100+person-1@users.noreply.github.com",
                    "41898282+github-actions[bot]@users.noreply.github.com",
                    "claude opus 5.5 <noreply@anthropic.com>",
                ];
                assert!(
                    alias || reviewed.contains(&s.as_str()),
                    "unreviewed fixture string: {s}"
                );
                assert!(!s.contains("/Users/") && !s.contains("://"));
            }
            serde_json::Value::Array(v) => v.iter().for_each(visit),
            serde_json::Value::Object(v) => {
                for (key, value) in v {
                    let fields = [
                        "agentEmails",
                        "assisted",
                        "at",
                        "author_ts",
                        "binary",
                        "botEmails",
                        "branch",
                        "branchPrefixes",
                        "coauthors",
                        "commit_ts",
                        "commits",
                        "config",
                        "directors",
                        "display",
                        "email",
                        "emails",
                        "end",
                        "events",
                        "excludeMarkers",
                        "files",
                        "has_content",
                        "hash",
                        "hunks",
                        "id",
                        "integrations",
                        "joins",
                        "key",
                        "keys",
                        "lines_stamp",
                        "markers",
                        "model",
                        "name",
                        "next",
                        "paths",
                        "privacy",
                        "prompts",
                        "provider",
                        "rel",
                        "release",
                        "remotes",
                        "repo",
                        "reviewer",
                        "segments",
                        "session",
                        "sessions",
                        "short",
                        "single_use",
                        "src",
                        "stamps",
                        "start",
                        "subject",
                        "tab",
                        "tokens",
                        "ts",
                        "tz",
                        "version",
                        "web",
                    ];
                    let labels = [
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
                    let hash = key.len() == 40
                        && key.starts_with("00000000000000000000000000000000000000")
                        && key.bytes().all(|b| b.is_ascii_hexdigit());
                    assert!(
                        hash || fields.contains(&key.as_str()) || labels.contains(&key.as_str()),
                        "unreviewed fixture key: {key}"
                    );
                    visit(value);
                }
            }
            _ => {}
        }
    }
    visit(&serde_json::from_str(INPUT).unwrap());
}
