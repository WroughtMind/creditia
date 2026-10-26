//! Fixed native history locations and real workspace paths for local hosts.
use std::path::{Path, PathBuf};

pub(crate) fn directories(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs
}

pub(crate) fn mcpjam_sessions() -> Vec<PathBuf> {
    directories(&crate::home().join(".mcpjam/harness-local-sessions"))
}

pub(crate) fn cwd(path: &str) -> String {
    let portable = path.replace('\\', "/");
    // A history exported from Windows must not be resolved relative to this
    // machine's working directory when it is read on another platform.
    #[cfg(not(windows))]
    if portable.as_bytes().get(1) == Some(&b':') || path.starts_with("\\\\") {
        return lexical(&portable);
    }
    let mut ancestor = Path::new(&portable);
    let mut missing = Vec::new();
    loop {
        match ancestor.canonicalize() {
            Ok(mut resolved) => {
                for part in missing.into_iter().rev() {
                    resolved.push(part);
                }
                let unresolved = resolved.to_string_lossy().replace('\\', "/");
                let normalized = lexical(&unresolved);
                // A missing prefix followed by `..` can reveal an existing
                // symlink again. Resolve that normalized path before binding it.
                return if normalized == unresolved {
                    normalized
                } else {
                    cwd(&normalized)
                };
            }
            Err(e) => {
                // Missing historical files are expected. A dangling link or
                // inaccessible ancestor cannot establish a local project path.
                if e.kind() != std::io::ErrorKind::NotFound
                    || std::fs::symlink_metadata(ancestor).is_ok_and(|m| m.file_type().is_symlink())
                {
                    return String::new();
                }
            }
        }
        let Some(part) = ancestor.components().next_back() else {
            return lexical(&portable);
        };
        let Some(parent) = ancestor.parent() else {
            return lexical(&portable);
        };
        missing.push(part.as_os_str().to_os_string());
        ancestor = parent;
    }
}

pub(crate) fn target(path: &str, workdir: &str) -> String {
    let portable = path.replace('\\', "/");
    // Cloud patch imports bind relative files to an explicit repository URI.
    // Its path is lexical and must never resolve through this machine's cwd.
    if let Some((scheme, rest)) = portable
        .split_once(":/")
        .filter(|(s, _)| matches!(*s, "https" | "http"))
    {
        let rest = rest.trim_start_matches('/');
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if host.is_empty() || matches!(host, "." | "..") {
            return String::new();
        }
        return format!(
            "{scheme}://{host}{}",
            lexical(&format!("/{}", path.trim_start_matches('/')))
        );
    }
    if Path::new(&portable).is_absolute() || portable.as_bytes().get(1) == Some(&b':') {
        return cwd(path);
    }
    if workdir.is_empty() || path.is_empty() {
        return String::new();
    }
    target(&Path::new(workdir).join(portable).to_string_lossy(), "")
}

fn lexical(path: &str) -> String {
    let unc = path.starts_with("//");
    let rooted = path.starts_with('/');
    let drive = path.as_bytes().get(1) == Some(&b':');
    let floor = if unc {
        2
    } else if drive {
        1
    } else {
        0
    };
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.len() > floor && parts.last() != Some(&"..") => {
                parts.pop();
            }
            ".." if !rooted && !drive => parts.push(part),
            ".." => {}
            _ => parts.push(part),
        }
    }
    format!(
        "{}{}",
        if unc {
            "//"
        } else if rooted {
            "/"
        } else {
            ""
        },
        parts.join("/")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use creditia_core::model::PathMap;

    fn project() -> PathMap {
        PathMap {
            known: ["a.rs".into(), "sub/a.rs".into()].into(),
            markers: vec!["/creditia-path-project/".into()],
            excludes: vec![],
        }
    }

    #[test]
    fn traversal_cannot_claim_a_file_from_another_project() {
        let workdir = "/creditia-missing-history/creditia-path-project";
        for path in [
            "../other/a.rs",
            "/creditia-missing-history/creditia-path-project/../other/a.rs",
            "C:\\creditia-path-project\\..\\other\\a.rs",
            "\\\\server\\share\\creditia-path-project\\..\\other\\a.rs",
        ] {
            let resolved = target(path, workdir);
            assert!(
                project().to_rel(&resolved).is_none(),
                "{path} resolved to {resolved}"
            );
        }
        assert_eq!(
            project().to_rel(&target("sub/../a.rs", workdir)).as_deref(),
            Some("a.rs"),
            "deleted native histories still use their normalized original path"
        );
    }

    #[cfg(unix)]
    #[test]
    fn workspace_links_resolve_but_external_targets_are_excluded() {
        let root = std::env::temp_dir().join(format!(
            "creditia-paths-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let repo = root.join("creditia-path-project");
        let outside = root.join("outside");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("a.rs"), "outside content").unwrap();
        std::os::unix::fs::symlink(&repo, root.join("workspace")).unwrap();
        std::os::unix::fs::symlink(&outside, repo.join("external")).unwrap();
        std::os::unix::fs::symlink(outside.join("missing"), repo.join("dangling")).unwrap();
        let workdir = root.join("workspace");
        for path in [
            "external/a.rs",
            "external/sub/a.rs",
            "dangling/a.rs",
            "missing/../external/a.rs",
        ] {
            let resolved = target(path, workdir.to_str().unwrap());
            assert!(
                project().to_rel(&resolved).is_none(),
                "{path} resolved to {resolved}"
            );
        }
        assert_eq!(
            project()
                .to_rel(&target("sub/a.rs", workdir.to_str().unwrap()))
                .as_deref(),
            Some("sub/a.rs")
        );
    }
}
