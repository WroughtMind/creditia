use creditia_adapters::{git, squash};
use creditia_core::{
    config::Config,
    engine::Engine,
    model::{Index, Level, Store},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

struct Repo(PathBuf);
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "core.hooksPath=",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn commit(repo: &Path, message: &str) -> String {
    run(repo, &["add", "-A"]);
    run(repo, &["commit", "-qm", message]);
    run(repo, &["rev-parse", "HEAD"])
}
fn setup() -> Repo {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let repo = Repo(std::env::temp_dir().join(format!(
        "creditia-squash-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed)
        )));
    fs::create_dir_all(&repo.0).unwrap();
    run(&repo.0, &["init", "-q", "-b", "main"]);
    fs::write(repo.0.join("README.md"), "fixture base\n").unwrap();
    commit(&repo.0, "base");
    repo
}

#[test]
fn surviving_positions_follow_original_commits_not_quota_order() {
    let repo = setup();
    let p = &repo.0;
    run(p, &["checkout", "-qb", "work"]);
    let mut lines: Vec<_> = (0..10).map(|i| format!("original value {i}\n")).collect();
    fs::write(p.join("before.rs"), lines.concat()).unwrap();
    commit(
        p,
        "initial\n\nCreditia-Harness: codex\nCreditia-Lines: codex/model-a 10 / 10",
    );
    run(p, &["mv", "before.rs", "after.rs"]);
    commit(p, "rename only");
    for (i, line) in lines.iter_mut().enumerate().skip(6) {
        *line = format!("replaced value {i}\n");
    }
    fs::write(p.join("after.rs"), lines.concat()).unwrap();
    commit(
        p,
        "overwrite\n\nCreditia-Harness: claude-code\nCreditia-Lines: claude-code/model-b 4 / 4",
    );
    fs::write(
        p.join("mixed.rs"),
        "same repeated value\nsame repeated value\n",
    )
    .unwrap();
    commit(
        p,
        "mixed\n\nCreditia-Harness: codex\nCreditia-Lines: codex/model-a 1 / 2",
    );
    lines.drain(..2);
    fs::write(p.join("after.rs"), lines.concat()).unwrap();
    let head = commit(p, "remove two old lines");
    run(p, &["checkout", "-q", "main"]);
    run(p, &["merge", "--squash", "work"]);
    let merge = commit(p, "feature (#42)");
    let (mut commits, _) = git::read_commits(p, &merge, Some(&format!("{merge}^..{merge}")));
    let c = &mut commits[0];
    c.web = true;
    // Deliberately misleading aggregate totals; recovery must not use them.
    c.lines_stamp = Some((
        vec![
            ("codex/model-a".into(), 11),
            ("claude-code/model-b".into(), 4),
        ],
        16,
    ));
    let history = squash::local_history(p, c, &head).unwrap();
    let cfg = Config::default();
    let claims = history.declarations(&cfg);
    assert_eq!(claims.len(), 8);
    let idx = Index::build(&Store::default());
    let mut engine = Engine::new(&cfg, &idx, &[], &[], Default::default());
    engine.squash_declarations.insert(merge.clone(), claims);
    let attr = engine.attribute(c);
    for (file, rows) in c.files.iter().zip(&attr.lines) {
        if file.rel == "mixed.rs" {
            assert!(rows.iter().all(|a| a.level == Level::Unattributed));
        } else {
            assert_eq!(file.rel, "after.rs");
            let tools: Vec<_> = rows
                .iter()
                .map(|a| {
                    assert_eq!(a.level, Level::Declared);
                    engine.reg.list[a.who].harness.as_deref().unwrap()
                })
                .collect();
            assert_eq!(
                tools,
                [
                    "codex",
                    "codex",
                    "codex",
                    "codex",
                    "claude-code",
                    "claude-code",
                    "claude-code",
                    "claude-code"
                ]
            );
        }
    }
    // A persisted, exactly pinned snapshot works offline and cannot be reused
    // for another repository. No PR commits are appended to main's history.
    run(
        p,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example/fixture.git",
        ],
    );
    let cache = p.join("cache");
    let mut value = serde_json::to_value(&history).unwrap();
    value["repository"] = "example/fixture".into();
    value["number"] = 42.into();
    let dir = cache.join("squash-v1/example/fixture");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("{merge}.json")),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    let recovered = squash::recover(p, &cache, &commits, &[], &cfg, false);
    assert_eq!(recovered.declarations[&merge].len(), 8);
    assert_eq!(recovered.unavailable, 0);
    assert_eq!(commits.len(), 1);
    let mut custom = commits.clone();
    custom[0].subject = "custom merge title without a PR number".into();
    let aged_cache = squash::recover(p, &cache, &custom, &[], &cfg, false);
    assert_eq!(
        aged_cache.declarations[&merge].len(),
        8,
        "validated cache survives removal from the recent PR list"
    );
    run(
        p,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/example/other.git",
        ],
    );
    let missing = squash::recover(p, &cache, &commits, &[], &cfg, false);
    assert!(missing.declarations.is_empty());
    assert_eq!(missing.unavailable, 1);

    // Conflict-resolution content differs from the original head. That file
    // cannot be projected even if the message makes a whole-commit claim.
    fs::write(p.join("after.rs"), "manual resolution\n").unwrap();
    run(p, &["add", "after.rs"]);
    run(p, &["commit", "--amend", "--no-edit", "-q"]);
    let changed = run(p, &["rev-parse", "HEAD"]);
    let (changed_commits, _) =
        git::read_commits(p, &changed, Some(&format!("{changed}^..{changed}")));
    assert!(squash::local_history(p, &changed_commits[0], &head)
        .unwrap()
        .declarations(&cfg)
        .is_empty());
    assert!(squash::local_history(p, &changed_commits[0], &"0".repeat(40)).is_err());
}

#[test]
fn ordinary_merge_is_not_projected() {
    let repo = setup();
    let p = &repo.0;
    run(p, &["checkout", "-qb", "work"]);
    fs::write(p.join("a.rs"), "branch value\n").unwrap();
    let head = commit(p, "branch");
    run(p, &["checkout", "-q", "main"]);
    run(p, &["merge", "--no-ff", "-m", "merge", "work"]);
    let merge = run(p, &["rev-parse", "HEAD"]);
    let c = creditia_core::model::Commit {
        hash: merge,
        ..Default::default()
    };
    assert!(squash::local_history(p, &c, &head).is_err());
}

#[test]
fn real_prs_recover_only_unambiguous_surviving_declarations() {
    for (source, expected, total) in [
        (include_str!("fixtures/squash/pr124.json"), 262, 428),
        (include_str!("fixtures/squash/pr125.json"), 533, 609),
    ] {
        let value: serde_json::Value = serde_json::from_str(source).unwrap();
        let history: squash::History = serde_json::from_value(value["history"].clone()).unwrap();
        let commit: creditia_core::model::Commit =
            serde_json::from_value(value["commit"].clone()).unwrap();
        let cfg = Config::default();
        let claims = history.declarations(&cfg);
        assert_eq!(claims.len(), expected);
        let positions: std::collections::HashSet<_> =
            claims.iter().map(|c| (&c.rel, c.at)).collect();
        assert_eq!(positions.len(), expected, "no duplicate positions");
        let idx = Index::build(&Store::default());
        let mut engine = Engine::new(&cfg, &idx, &[], &[], Default::default());
        engine
            .squash_declarations
            .insert(commit.hash.clone(), claims);
        let result = engine.attribute(&commit);
        assert_eq!(result.lines.iter().map(Vec::len).sum::<usize>(), total);
        assert_eq!(
            result
                .lines
                .iter()
                .flatten()
                .filter(|a| a.level == Level::Declared)
                .count(),
            expected
        );
        assert_eq!(
            result
                .lines
                .iter()
                .flatten()
                .filter(|a| a.level == Level::Unattributed)
                .count(),
            total - expected
        );
        assert!(result
            .lines
            .iter()
            .flatten()
            .all(|a| a.level != Level::Verified));
    }
}
