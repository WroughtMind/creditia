use creditia_core::{
    config::Config,
    engine::Engine,
    model::{Commit, CommitAttr, Ev, FileGroup, Index, Session, Store},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub commits: Vec<Commit>,
    pub stamps: HashMap<String, (String, Option<String>)>,
    pub events: Vec<Ev>,
    pub sessions: Vec<Session>,
    pub config: Config,
}

fn replay_commits(input: &Input, mut visit: impl FnMut(&Commit, &Engine<'_>, CommitAttr)) {
    let mut store = Store::default();
    store.groups.insert(
        "fixture".into(),
        FileGroup {
            evs: input.events.clone(),
            sessions: input.sessions.clone(),
            ..Default::default()
        },
    );
    let idx = Index::build(&store);
    let mut engine = Engine::new(
        &input.config,
        &idx,
        &input.sessions,
        &[],
        input.stamps.clone(),
    );
    engine.set_history(&input.commits);
    let mut commits: Vec<_> = input.commits.iter().collect();
    commits.sort_by_key(|c| (c.commit_ts, c.author_ts));
    for commit in commits {
        let result = engine.attribute(commit);
        visit(commit, &engine, result);
    }
}

/// Engine-only replay. This intentionally does not claim coverage of native log
/// ingestion, Git parsing, survival or the complete product report.
pub fn replay(input: &Input) -> Value {
    let mut rows = Vec::new();
    let mut counts = BTreeMap::<String, usize>::new();
    replay_commits(input, |_, engine, result| {
        for file in result.lines {
            for attr in file {
                let who = &engine.reg.list[attr.who];
                // Stable comparison of identities after anonymization. Agent IDs
                // keep tool/model semantics; private human IDs are not output.
                let label = if who.kind == "agent" {
                    who.id.clone()
                } else {
                    format!("{}:{}", who.kind, attr.who)
                };
                rows.push(json!([label, attr.level]));
                *counts
                    .entry(format!(
                        "{}:{:?}",
                        who.harness.as_deref().unwrap_or(who.kind),
                        attr.level
                    ))
                    .or_default() += 1;
            }
        }
    });
    json!({"rows":rows,"counts":counts})
}

#[cfg(test)]
pub mod checks {
    use super::*;
    use creditia_core::model::{Registry, UNATTR};
    use regex::Regex;
    use std::{collections::HashSet, sync::LazyLock};

    const INPUT_FIELDS: &[&str] = &[
        "agentEmails",
        "aliases",
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
        "hideNames",
        "hidePaths",
        "hideTitles",
        "hook",
        "hunks",
        "id",
        "ignoreTags",
        "integrations",
        "joins",
        "key",
        "keys",
        "level",
        "lines_stamp",
        "markers",
        "me",
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
        "sign",
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
    const COMMON_LABELS: &[&str] = &[
        "",
        "main",
        "sample commit",
        "research sample",
        "strict",
        "standard",
        "loose",
    ];
    static COMMIT_ALIAS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\A0{32}[0-9a-f]{8}\z").unwrap());
    static FILE_ALIAS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\Afile-[0-9]+\.data\z").unwrap());
    static ALIAS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(concat!(
            r"\A(?:session-[0-9]+|branch-[0-9]+|Person [0-9]+|h:owner-[0-9]+",
            r"|person-[0-9]+@example\.invalid",
            r"|(?:[0-9]+\+)?person-[0-9]+@users\.noreply\.github\.com)\z"
        ))
        .unwrap()
    });
    static JSON_STRING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#""(?:[^"\\]|\\.)*""#).unwrap());

    pub fn public_json(raw: &str, keys: &[&str], labels: &[&str]) -> Result<Value, &'static str> {
        // Recursive allowlist: adding an unexpected string anywhere requires review,
        // including map keys (where private config/remote values can otherwise hide).
        fn visit(
            value: &Value,
            keys: &[&str],
            labels: &[&str],
            strings: &mut usize,
        ) -> Result<(), &'static str> {
            match value {
                Value::String(s) => {
                    *strings += 1;
                    if !COMMON_LABELS.contains(&s.as_str())
                        && !labels.contains(&s.as_str())
                        && !ALIAS.is_match(s)
                        && !FILE_ALIAS.is_match(s)
                        && !COMMIT_ALIAS.is_match(s)
                    {
                        return Err("unreviewed fixture string");
                    }
                }
                Value::Array(items) => {
                    for item in items {
                        visit(item, keys, labels, strings)?;
                    }
                }
                Value::Object(entries) => {
                    for (key, child) in entries {
                        *strings += 1;
                        if !keys.contains(&key.as_str()) && !COMMIT_ALIAS.is_match(key) {
                            return Err("unreviewed fixture key");
                        }
                        visit(child, keys, labels, strings)?;
                    }
                }
                _ => {}
            }
            Ok(())
        }

        let value = serde_json::from_str(raw).map_err(|_| "invalid fixture JSON")?;
        let mut strings = 0;
        visit(&value, keys, labels, &mut strings)?;
        // Value keeps only the last occurrence of a key. Reject entries hidden by it.
        if JSON_STRING.find_iter(raw).count() != strings {
            return Err("duplicate keys in fixture JSON");
        }
        Ok(value)
    }

    pub fn public_input(
        raw: &str,
        reviewed_keys: &[&str],
        reviewed_labels: &[&str],
    ) -> Result<Input, &'static str> {
        let keys = [INPUT_FIELDS, reviewed_keys].concat();
        let value = public_json(raw, &keys, reviewed_labels)?;
        let input: Input = serde_json::from_value(value).map_err(|_| "invalid replay input")?;
        let mut paths = input
            .commits
            .iter()
            .flat_map(|c| c.files.iter().map(|f| &f.rel))
            .chain(input.events.iter().map(|e| &e.rel));
        if paths.any(|path| !FILE_ALIAS.is_match(path)) {
            return Err("fixture paths must be file aliases");
        }
        let config = &input.config;
        if !config.repo.is_empty()
            || !config.paths.is_empty()
            || !config.remotes.is_empty()
            || !config.markers.is_empty()
            || !config.exclude_markers.is_empty()
            || !config.privacy.aliases.is_empty()
            || !config.display.sign.is_empty()
        {
            return Err("fixture contains private project or display metadata");
        }
        if input
            .sessions
            .iter()
            .any(|s| s.provider.is_some() || s.tokens != 0 || s.prompts != 0)
        {
            return Err("fixture contains session provider or usage metadata");
        }
        Ok(input)
    }

    /// Per-commit author facts and line counts, without exporting email-derived IDs.
    pub fn team_authors(input: &Input) -> Value {
        let mut identities = HashSet::new();
        let mut by_commit = BTreeMap::new();
        replay_commits(input, |commit, engine, result| {
            let (author, name, me) = engine.author_of(commit);
            let mut counts = BTreeMap::<&str, usize>::new();
            for attr in result.lines.iter().flatten() {
                let who = &engine.reg.list[attr.who];
                let label = if who.id == author {
                    "author"
                } else if who.kind == "human" {
                    "other-human"
                } else {
                    &who.id
                };
                *counts.entry(label).or_default() += 1;
            }
            identities.insert(author);
            let summary = json!({"name": name, "me": me, "by": counts});
            assert!(
                by_commit.insert(commit.hash.clone(), summary).is_none(),
                "duplicate commit alias"
            );
        });
        assert!(
            identities.len() > 1,
            "team fixture must cover distinct Git authors"
        );
        assert!(
            by_commit
                .values()
                .any(|a| a["me"] == true && a["by"].get(UNATTR).is_some()),
            "team fixture must cover unattributed lines in a local author's commit"
        );
        assert!(
            by_commit
                .values()
                .any(|a| a["me"] == false && a["by"].get("author").is_some()),
            "team fixture must cover lines credited to a teammate"
        );
        json!(by_commit)
    }

    /// Run the same Git positions against two cold evidence snapshots.
    pub fn assert_pruned_replay(before: &Input, after: &Input) -> Value {
        assert_eq!(
            json!(before.commits),
            json!(after.commits),
            "Git history changed"
        );
        assert_eq!(before.stamps, after.stamps, "commit stamps changed");
        assert_eq!(
            json!(before.config),
            json!(after.config),
            "attribution config changed"
        );
        assert!(
            after.events.len() < before.events.len(),
            "no edit evidence was pruned"
        );

        // Match complete records, including duplicate occurrences.
        let mut original = HashMap::<String, usize>::new();
        for event in &before.events {
            *original
                .entry(serde_json::to_string(event).unwrap())
                .or_default() += 1;
        }
        for event in &after.events {
            let key = serde_json::to_string(event).unwrap();
            let count = original
                .get_mut(&key)
                .filter(|n| **n > 0)
                .expect("pruned evidence contains a new or duplicated event");
            *count -= 1;
        }

        let mut retained = Registry::default();
        for event in after.events.iter().filter(|e| !e.tab) {
            retained.agent(event.src.harness(), Some(&event.model));
        }
        let tab_retained = after.events.iter().any(|e| e.tab);
        let full = replay(before);
        let pruned = replay(after);
        let full_rows = full["rows"].as_array().unwrap();
        let pruned_rows = pruned["rows"].as_array().unwrap();
        assert_eq!(full_rows.len(), pruned_rows.len());
        assert!(
            full_rows
                .iter()
                .zip(pruned_rows)
                .any(|(a, b)| a[1] == "verified" && b[1] != "verified"),
            "fixture must cover a verified position that loses evidence"
        );
        for row in pruned_rows.iter().filter(|row| row[1] == "verified") {
            let contributor = row[0].as_str().unwrap();
            assert!(
                retained.find(contributor).is_some()
                    || (tab_retained && contributor.starts_with("human:")),
                "verified contributor has no retained edit evidence"
            );
        }
        pruned
    }
}
