//! 把日志里的绝对路径（主仓库、worktree、/tmp 副本）映射回仓库内相对路径

use std::collections::HashSet;

pub struct PathMap {
    pub known: HashSet<String>,
    /// 判断绝对路径是否属于本项目的关键词（目录名、仓库名）
    pub markers: Vec<String>,
    pub excludes: Vec<String>,
}

impl PathMap {
    /// 取能在仓库历史路径集合里找到的最长后缀
    pub fn in_project(&self, s: &str) -> bool {
        let lower = s.to_lowercase();
        self.markers.iter().any(|m| lower.contains(m.as_str())) && !self.excludes.iter().any(|m| lower.contains(m.as_str()))
    }

    pub fn to_rel(&self, abs: &str) -> Option<String> {
        if !self.in_project(abs) {
            return None;
        }
        let parts: Vec<&str> = abs.split('/').filter(|s| !s.is_empty()).collect();
        for i in 0..parts.len() {
            let cand = parts[i..].join("/");
            if self.known.contains(&cand) {
                return Some(cand);
            }
        }
        None
    }
}
