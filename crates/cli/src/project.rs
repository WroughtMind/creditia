use creditia_adapters as ad;
use creditia_core::config::Config;
use std::path::Path;

/// 自动探测项目配置：当前分支与远程；人员角色始终由用户填写
pub fn detect_config(repo: &Path) -> Config {
    let mut c = Config::default();
    c.repo = repo.display().to_string();
    c.name = repo.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let cur = ad::git::git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).trim().to_string();
    c.branch = if !cur.is_empty() && cur != "HEAD" {
        cur
    } else if ad::git::git_ok(repo, &["rev-parse", "--verify", "main"]) {
        "main".into()
    } else {
        "HEAD".into()
    };
    let (_, remotes) = ad::git::identity(repo);
    c.remotes = remotes.clone();
    c
}

/// 项目路径必须有明确边界；不按目录名或同名前缀猜测。
pub fn derive_markers(repo: &Path, remotes: &[String]) -> Vec<String> {
    let mut roots = ad::git::worktrees(repo);
    roots.push(repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf()).display().to_string());
    roots.extend(remotes.iter().cloned());
    roots.sort();
    roots.dedup();
    roots
}
