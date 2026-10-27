use creditia_adapters::git::{units, versions};
use std::{fs, path::PathBuf, process::Command};

struct Repo(PathBuf);
impl Drop for Repo { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
impl Repo {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!("creditia-version-tags-{}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        fs::create_dir_all(&p).unwrap();
        let r = Self(p);
        r.git(&["init", "-q", "-b", "main"], "2026-01-01T00:00:00Z");
        r
    }
    fn git(&self, args: &[&str], date: &str) -> String {
        let o = Command::new("git").arg("-C").arg(&self.0)
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false", "-c", "core.hooksPath=/dev/null"])
            .args(args).env("GIT_AUTHOR_DATE", date).env("GIT_COMMITTER_DATE", date).output().unwrap();
        assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().into()
    }
    fn commit(&self, name: &str, date: &str) -> String {
        fs::write(self.0.join("source.txt"), name).unwrap();
        self.git(&["add", "source.txt"], date);
        self.git(&["commit", "-qm", name], date);
        self.git(&["rev-parse", "HEAD"], date)
    }
}

#[test]
fn versions_follow_history_not_annotation_dates_and_stay_on_the_analyzed_branch() {
    let r = Repo::new();
    let a = r.commit("first", "2026-01-01T00:00:00Z");
    let b = r.commit("second", "2026-01-02T00:00:00Z");
    let c = r.commit("unreleased", "2026-01-03T00:00:00Z");
    r.git(&["tag", "-am", "new release", "v2", &b], "2026-01-04T00:00:00Z");
    r.git(&["tag", "-am", "backfilled old release", "v1", &a], "2026-01-05T00:00:00Z");
    r.git(&["tag", "-am", "tag alias", "nested-v2", "v2"], "2026-01-06T00:00:00Z");
    r.git(&["tag", "archive/backup", &c], "2026-01-06T00:00:00Z");
    let tree = r.git(&["rev-parse", "HEAD^{tree}"], "2026-01-06T00:00:00Z");
    r.git(&["tag", "tree-only", &tree], "2026-01-06T00:00:00Z");
    r.git(&["checkout", "-qb", "side", &a], "2026-01-06T00:00:00Z");
    r.commit("side branch", "2026-01-07T00:00:00Z");
    r.git(&["tag", "side-release"], "2026-01-07T00:00:00Z");
    // HEAD 刻意停在别的分支，分析范围仍然是 main。
    let (_, parents) = units(&r.0, "main", &[]);
    let (tags, by) = versions(&r.0, "main", &parents, &["archive/**".into()]).unwrap();
    assert_eq!(tags.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["v1", "v2", "nested-v2"]);
    assert_eq!(by.get(&a).map(String::as_str), Some("v1"));
    assert_eq!(by.get(&b).map(String::as_str), Some("v2"));
    assert!(!by.contains_key(&c));
}

#[test]
fn no_eligible_tags_is_empty_but_git_read_failure_is_an_error() {
    let r = Repo::new();
    r.commit("first", "2026-01-01T00:00:00Z");
    r.git(&["tag", "archive/only-tag"], "2026-01-01T00:00:00Z");
    let (_, parents) = units(&r.0, "main", &[]);
    let (tags, by) = versions(&r.0, "main", &parents, &["archive/**".into()]).unwrap();
    assert!(tags.is_empty() && by.is_empty());
    assert_eq!(versions(&r.0, "main", &parents, &[]).unwrap().0.len(), 1);
    assert!(versions(&r.0, "missing-branch", &parents, &[]).is_err());
}
