//! Recover declarations through a GitHub squash without adding the PR's commits
//! to main's totals. Cache Git provenance, not config-dependent attribution.

use crate::{command::output, git};
use creditia_core::config::Config;
use creditia_core::engine::Engine;
use creditia_core::model::{squash_number, Commit, Index, LineDeclaration, PrMeta, Store};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Origin {
    rel: String,
    at: u32,
    commit: String,
    source_rel: String,
    source_at: u32,
}

#[derive(Serialize, Deserialize)]
pub struct History {
    version: u32,
    repository: String,
    number: u64,
    merge: String,
    head: String,
    commits: Vec<Commit>,
    stamps: HashMap<String, (String, Option<String>)>,
    origins: Vec<Origin>,
}

#[derive(Default)]
pub struct Recovery {
    pub declarations: HashMap<String, Vec<LineDeclaration>>,
    pub unavailable: usize,
}

fn oid(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Accept only ordinary github.com remotes. Never pass a repository-provided
/// transport, shell helper, hostname or credential-bearing URL to a fetch.
pub fn github_repository(repo: &Path) -> Option<String> {
    let url = git::git(repo, &["remote", "get-url", "origin"]);
    let url = url.trim();
    let path = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("git@github.com:"))
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))?;
    let path = path
        .trim_end_matches('/')
        .strip_suffix(".git")
        .unwrap_or(path.trim_end_matches('/'));
    let (owner, name) = path.split_once('/')?;
    let component = |s: &str| {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    };
    (component(owner) && component(name)).then(|| path.to_lowercase())
}

fn run_git(repo: &Path, args: &[&str], deadline: Instant) -> Result<String, String> {
    output(
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args([
                "--literal-pathspecs",
                "-c",
                "core.quotepath=false",
                "-c",
                "core.hooksPath=",
            ])
            .args(args),
        deadline,
    )
}

/// Extract only origins with identical final blobs. Changed conflict-resolution
/// files are left unknown; matching another file is still useful.
pub fn local_history(repo: &Path, squash: &Commit, head: &str) -> Result<History, String> {
    if !oid(&squash.hash) || !oid(head) {
        return Err("invalid commit".into());
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let parent_line = run_git(
        repo,
        &["rev-list", "--parents", "-n", "1", &squash.hash],
        deadline,
    )?;
    let parents: Vec<_> = parent_line.split_whitespace().collect();
    if parents.len() != 2 {
        return Err("not a single-parent squash".into());
    }
    let base = run_git(repo, &["merge-base", "--all", parents[1], head], deadline)?;
    let base = base.trim();
    if !oid(base) {
        return Err("ambiguous or unavailable branch base".into());
    }
    let (commits, stamps) =
        git::read_commits_checked(repo, head, Some(&format!("{base}..{head}")))?;
    if commits.is_empty() {
        return Err("original commits unavailable".into());
    }
    let mut origins = Vec::new();
    for file in &squash.files {
        if file.binary || file.at.len() != file.keys.len() {
            continue;
        }
        let blob = |rev: &str| -> Result<Option<String>, String> {
            let tree = run_git(repo, &["ls-tree", "-z", rev, "--", &file.rel], deadline)?;
            Ok(tree.split('\0').find_map(|entry| {
                let (meta, path) = entry.split_once('\t')?;
                let hash = meta.split_whitespace().nth(2)?;
                (path == file.rel && oid(hash)).then(|| hash.to_string())
            }))
        };
        let (left, right) = (blob(&squash.hash)?, blob(head)?);
        if left.is_none() || left != right {
            continue;
        }
        let blame = run_git(
            repo,
            &["blame", "--incremental", head, "--", &file.rel],
            deadline,
        )?;
        let rows = parse_origins(&blame, &file.rel, &file.at);
        if rows.len() != file.at.len()
            || rows
                .iter()
                .map(|o| o.at)
                .collect::<std::collections::HashSet<_>>()
                .len()
                != file.at.len()
        {
            return Err("incomplete Git blame positions".into());
        }
        origins.extend(rows);
    }
    Ok(History {
        version: VERSION,
        repository: String::new(),
        number: 0,
        merge: squash.hash.clone(),
        head: head.into(),
        commits,
        stamps,
        origins,
    })
}

fn parse_origins(blame: &str, rel: &str, added: &[u32]) -> Vec<Origin> {
    let wanted: std::collections::HashSet<u32> = added.iter().copied().collect();
    let mut result = Vec::new();
    let mut chunk: Option<(&str, u32, u32, u32)> = None;
    for line in blame.lines() {
        if let Some(path) = line.strip_prefix("filename ") {
            // Git quotes control characters and backslashes. JSON decoding
            // handles those; unfamiliar C/octal quoting is rejected, not guessed.
            let path = if path.starts_with('"') {
                serde_json::from_str::<String>(path).ok()
            } else {
                Some(path.to_string())
            };
            if let (Some((hash, original, final_at, count)), Some(path)) = (chunk.take(), path) {
                for i in 0..count {
                    let (Some(at), Some(source_at)) =
                        (final_at.checked_add(i), original.checked_add(i))
                    else {
                        break;
                    };
                    if wanted.contains(&at) {
                        result.push(Origin {
                            rel: rel.into(),
                            at,
                            commit: hash.into(),
                            source_rel: path.clone(),
                            source_at,
                        });
                    }
                }
            }
        } else {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() == 4 && oid(fields[0]) {
                chunk = fields[1]
                    .parse()
                    .ok()
                    .zip(fields[2].parse().ok())
                    .zip(fields[3].parse().ok())
                    .map(|((a, b), n)| (fields[0], a, b, n));
            }
        }
    }
    result
}

impl History {
    pub fn declarations(&self, cfg: &Config) -> Vec<LineDeclaration> {
        let idx = Index::build(&Store::default());
        let mut engine = Engine::new(cfg, &idx, &[], &[], self.stamps.clone());
        let mut claims = HashMap::new();
        for commit in &self.commits {
            let Some(declaration) = engine.whole_declaration(commit) else {
                continue;
            };
            for file in &commit.files {
                claims.insert(
                    (commit.hash.as_str(), file.rel.as_str()),
                    (
                        file.at
                            .iter()
                            .copied()
                            .collect::<std::collections::HashSet<_>>(),
                        declaration.clone(),
                    ),
                );
            }
        }
        self.origins
            .iter()
            .filter_map(|o| {
                let (positions, (harness, model)) =
                    claims.get(&(o.commit.as_str(), o.source_rel.as_str()))?;
                // Blame can point to unchanged base lines or a merge resolution.
                // Only an actual added position in a declared original commit counts.
                positions.contains(&o.source_at).then(|| LineDeclaration {
                    rel: o.rel.clone(),
                    at: o.at,
                    harness: harness.clone(),
                    model: model.clone(),
                })
            })
            .collect()
    }
}

fn merged_head(
    value: &serde_json::Value,
    repository: &str,
    number: u64,
    merge: &str,
) -> Option<String> {
    if value["number"].as_u64() != Some(number)
        || value["merged"] != true
        || value["merge_commit_sha"] != merge
        || !value["base"]["repo"]["full_name"]
            .as_str()?
            .eq_ignore_ascii_case(repository)
    {
        return None;
    }
    let head = value["head"]["sha"].as_str()?;
    (oid(head) && head != merge).then(|| head.to_string())
}

pub fn recover(
    repo: &Path,
    cache: &Path,
    commits: &[Commit],
    prs: &[PrMeta],
    cfg: &Config,
    online: bool,
) -> Recovery {
    let mut result = Recovery::default();
    let Some(repository) = github_repository(repo) else {
        return result;
    };
    let directory = cache.join("squash-v1").join(&repository);
    let objects = directory.join("objects.git");
    // One total network budget, regardless of repository age or PR count.
    let deadline = Instant::now() + Duration::from_secs(60);
    for commit in commits {
        if !oid(&commit.hash) {
            continue;
        }
        let path = directory.join(format!("{}.json", commit.hash));
        let cached = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<History>(&b).ok())
            .filter(|h| {
                h.version == VERSION
                    && h.repository == repository
                    && h.number > 0
                    && h.merge == commit.hash
                    && oid(&h.head)
            });
        // A validated snapshot outlives the recent-PR listing window, including
        // for customized subjects that contain no PR number.
        if let Some(history) = cached {
            result
                .declarations
                .insert(commit.hash.clone(), history.declarations(cfg));
            continue;
        }
        let Some(number) = prs
            .iter()
            .find(|p| p.source == "github" && p.merge_commit.as_deref() == Some(&commit.hash))
            .map(|p| p.number)
            .or_else(|| squash_number(commit))
        else {
            continue;
        };
        let history = (|| {
            if !online || Instant::now() >= deadline {
                return None;
            }
            let text = output(
                Command::new("gh").args([
                    "api",
                    "--hostname",
                    "github.com",
                    &format!("repos/{repository}/pulls/{number}"),
                ]),
                deadline,
            )
            .ok()?;
            let value = serde_json::from_str(&text).ok()?;
            let head = merged_head(&value, &repository, number, &commit.hash)?;
            std::fs::create_dir_all(&directory).ok()?;
            if !objects.is_dir() {
                output(
                    Command::new("git")
                        .args(["init", "--bare", "--quiet"])
                        .arg(&objects),
                    deadline,
                )
                .ok()?;
            }
            // Isolated objects only: never checkout or update refs in the user's
            // repository. Explicit refspecs cannot move an existing local branch.
            output(
                Command::new("git")
                    .arg("-C")
                    .arg(&objects)
                    .args([
                        "-c",
                        "core.hooksPath=",
                        "fetch",
                        "--quiet",
                        "--no-tags",
                        "--no-write-fetch-head",
                    ])
                    .arg(repo)
                    .arg(format!(
                        "{}:refs/creditia/merges/{}",
                        commit.hash, commit.hash
                    )),
                deadline,
            )
            .ok()?;
            output(
                Command::new("git")
                    .arg("-C")
                    .arg(&objects)
                    .args([
                        "-c",
                        "core.hooksPath=",
                        "-c",
                        "credential.helper=",
                        "-c",
                        "credential.helper=!gh auth git-credential",
                        "fetch",
                        "--quiet",
                        "--no-tags",
                        "--no-write-fetch-head",
                    ])
                    .arg(format!("https://github.com/{repository}.git"))
                    .arg(format!(
                        "refs/pull/{number}/head:refs/creditia/pulls/{head}"
                    )),
                deadline,
            )
            .ok()?;
            if git::resolve_commit(&objects, &format!("refs/creditia/pulls/{head}")).as_deref()
                != Some(&head)
            {
                return None;
            }
            let mut history = local_history(&objects, commit, &head).ok()?;
            history.repository = repository.clone();
            history.number = number;
            if let Ok(bytes) = serde_json::to_vec(&history) {
                // Interrupted/partial writes are rejected on the next read.
                let _ = std::fs::write(&path, bytes);
            }
            Some(history)
        })();
        match history {
            Some(h) => {
                result
                    .declarations
                    .insert(commit.hash.clone(), h.declarations(cfg));
            }
            None => result.unavailable += 1,
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn association_requires_exact_merged_pr_and_repository() {
        let sha = "a".repeat(40);
        let mut v = serde_json::json!({"number":42,"merged":true,"merge_commit_sha":sha,"base":{"repo":{"full_name":"org/repo"}},"head":{"sha":"b".repeat(40)}});
        assert!(merged_head(&v, "org/repo", 42, &sha).is_some());
        for (repo, n, merge) in [
            ("other/repo", 42, sha.as_str()),
            ("org/repo", 43, sha.as_str()),
            ("org/repo", 42, "wrong"),
        ] {
            assert!(merged_head(&v, repo, n, merge).is_none());
        }
        v["merged"] = false.into();
        assert!(merged_head(&v, "org/repo", 42, &sha).is_none());
    }

    #[test]
    fn incremental_blame_uses_position_and_original_path() {
        let sha = "a".repeat(40);
        let text = format!("{sha} 7 3 3\nauthor X\nfilename \"old\\tname.rs\"\n");
        let rows = parse_origins(&text, "new.rs", &[4, 5, 8]);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].at, rows[0].source_at), (4, 8));
        assert_eq!(rows[0].source_rel, "old\tname.rs");
    }
}
