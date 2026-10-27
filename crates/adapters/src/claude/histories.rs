use super::*;
use crate::harness_paths::mcpjam_sessions;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};

pub(super) fn roots() -> Vec<PathBuf> {
    let mut out = vec![source_dir(Src::Claude).join("projects")];
    out.extend(
        mcpjam_sessions()
            .into_iter()
            .map(|p| p.join("home/.claude/projects")),
    );
    out
}

// Native session IDs survive export/copy. Subagents share a parent session ID
// but have their own agent-<id>.jsonl files and must stay separate.
fn identity(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(row) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(id) = row["sessionId"].as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        let stem = path.file_stem()?.to_str()?;
        let agent = if stem.starts_with("agent-") {
            stem
        } else {
            "main"
        };
        let session = if agent == "main" { id.to_string() } else { format!("{id}:{agent}") };
        return Some(format!("claude:native-v3:{session}"));
    }
    None
}

pub(super) fn files() -> Vec<(String, PathBuf)> {
    let mut selected: BTreeMap<String, PathBuf> = BTreeMap::new();
    for base in roots() {
        let mut files: Vec<_> = walkdir::WalkDir::new(base)
            .into_iter()
            .flatten()
            .filter(|e| {
                e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "jsonl")
            })
            .map(|e| e.path().to_path_buf())
            .collect();
        files.sort();
        for path in files {
            let Some(id) = identity(&path) else { continue };
            // A copied older prefix can have a newer filesystem timestamp.
            let quality = |p: &Path| {
                let (mtime, bytes) = fingerprint(p);
                (bytes, mtime)
            };
            if selected
                .get(&id)
                .is_none_or(|old| quality(&path) > quality(old))
            {
                selected.insert(id, path);
            }
        }
    }
    selected.into_iter().collect()
}
