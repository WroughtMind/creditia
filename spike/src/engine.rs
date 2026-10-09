//! 归因引擎：逐个非 merge 提交、逐行匹配证据，再按降级链补全

use crate::ev::{Index, Session, Src};
use crate::git::{BlameLine, Commit, Unit};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Verified,
    Declared,
    Inferred,
    Unattributed,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Director {
    pub id: String,
    pub name: String,
    pub emails: Vec<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub name: String,
    pub repo: String,
    pub branch: String,
    pub remotes: Vec<String>,
    pub markers: Vec<String>,
    #[serde(default)]
    pub exclude_markers: Vec<String>,
    pub directors: Vec<Director>,
    pub agent_emails: HashMap<String, String>,
    #[serde(default)]
    pub bot_emails: Vec<String>,
    pub branch_prefixes: HashMap<String, String>,
    #[serde(default)]
    pub segments: Vec<(String, String, String)>,
    #[serde(default)]
    pub panel: HashMap<String, f64>,
}

pub const UNATTR: &str = "unattributed";

/// 一行的归属
#[derive(Clone, Copy)]
pub struct Attr {
    pub who: usize,
    pub level: Level,
}

#[derive(Serialize, Clone, Debug)]
pub struct Who {
    pub id: String,
    pub kind: &'static str,
    pub harness: Option<String>,
    pub model: Option<String>,
    pub name: Option<String>,
}

#[derive(Default)]
pub struct Registry {
    pub list: Vec<Who>,
    by_id: HashMap<String, usize>,
}

impl Registry {
    pub fn agent(&mut self, harness: &str, model: Option<&str>) -> usize {
        let model = model.filter(|m| !m.is_empty());
        let id = match model {
            Some(m) => format!("a:{harness}/{m}"),
            None => format!("a:{harness}"),
        };
        self.get(id, |id| Who { id, kind: "agent", harness: Some(harness.into()), model: model.map(String::from), name: None })
    }
    pub fn human(&mut self, id: &str, name: &str) -> usize {
        self.get(id.into(), |id| Who { id, kind: "human", harness: None, model: None, name: Some(name.into()) })
    }
    pub fn unattributed(&mut self) -> usize {
        self.get(UNATTR.into(), |id| Who { id, kind: "unattributed", harness: None, model: None, name: None })
    }
    fn get(&mut self, id: String, mk: impl FnOnce(String) -> Who) -> usize {
        if let Some(&i) = self.by_id.get(&id) {
            return i;
        }
        self.list.push(mk(id.clone()));
        self.by_id.insert(id, self.list.len() - 1);
        self.list.len() - 1
    }
    /// 展示用的分组：agent 按工具，人按人
    pub fn group(&self, i: usize) -> String {
        let w = &self.list[i];
        match w.kind {
            "agent" => w.harness.clone().unwrap(),
            "human" => w.id.clone(),
            _ => UNATTR.into(),
        }
    }
}

/// 单个提交的归因结果
pub struct CommitAttr {
    pub lines: Vec<Vec<Attr>>,
    /// 每个工具的 verified 命中行数（报告与抽样用）
    pub hits: HashMap<&'static str, usize>,
    pub conflicts: usize,
    pub main: Option<usize>,
    pub tab_lines: usize,
    pub reason: String,
}

pub struct Engine<'a> {
    pub cfg: &'a Config,
    pub idx: &'a Index,
    pub sessions: &'a [Session],
    pub reg: Registry,
    director_emails: HashSet<String>,
    pub unit_branch: HashMap<String, String>,
}

const WINDOW_BEFORE: i64 = 14 * 86400;
const WINDOW_AFTER: i64 = 3600;

impl<'a> Engine<'a> {
    pub fn new(cfg: &'a Config, idx: &'a Index, sessions: &'a [Session], units: &[Unit]) -> Self {
        let mut reg = Registry::default();
        for d in &cfg.directors {
            reg.human(&d.id, &d.name);
        }
        reg.unattributed();
        let director_emails = cfg.directors.iter().flat_map(|d| d.emails.iter().map(|e| e.to_lowercase())).collect();
        let mut unit_branch = HashMap::new();
        for u in units {
            if let Some(b) = &u.branch {
                for c in &u.commits {
                    unit_branch.insert(c.clone(), b.clone());
                }
            }
        }
        Engine { cfg, idx, sessions, reg, director_emails, unit_branch }
    }

    fn director(&self) -> usize {
        0
    }

    pub fn attribute(&mut self, c: &Commit) -> CommitAttr {
        let lo = c.author_ts.min(c.commit_ts) - WINDOW_BEFORE;
        let hi = c.commit_ts.max(c.author_ts) + WINDOW_AFTER;
        let mut hits: HashMap<&'static str, usize> = HashMap::new();
        let mut conflicts = 0;
        let mut tab_lines = 0;
        let mut lines: Vec<Vec<Option<Attr>>> = vec![];
        // 第一轮：逐行查证据
        for f in &c.files {
            let mut fl = vec![];
            for &k in &f.keys {
                if k == 0 {
                    fl.push(None);
                    continue;
                }
                let cands: Vec<_> = self
                    .idx
                    .map
                    .get(&(f.rel.clone(), k))
                    .map(|v| v.iter().filter(|e| e.ts >= lo && e.ts <= hi).collect())
                    .unwrap_or_default();
                if cands.is_empty() {
                    fl.push(None);
                    continue;
                }
                let srcs: HashSet<Src> = cands.iter().map(|e| e.src).collect();
                if srcs.len() > 1 {
                    conflicts += 1;
                }
                // 取时间最接近提交的那条
                let best = cands.iter().min_by_key(|e| (c.author_ts - e.ts).abs()).unwrap();
                if best.tab {
                    tab_lines += 1;
                    fl.push(Some(Attr { who: self.director(), level: Level::Verified }));
                    continue;
                }
                *hits.entry(best.src.harness()).or_default() += 1;
                let who = self.reg.agent(best.src.harness(), Some(&best.model));
                fl.push(Some(Attr { who, level: Level::Verified }));
            }
            lines.push(fl);
        }
        // 第二轮：降级链
        let mut verified: HashMap<usize, usize> = HashMap::new();
        for a in lines.iter().flatten().flatten() {
            *verified.entry(a.who).or_default() += 1;
        }
        let top_verified = verified.iter().max_by_key(|(_, n)| **n).map(|(w, _)| *w);
        let (fallback, reason) = self.fallback(c, top_verified);
        let total_long = lines.iter().flatten().filter(|_| true).count();
        let _ = total_long;
        // 主要贡献者：verified 最多的；没有就是降级结果
        let mut counts: HashMap<usize, usize> = verified.clone();
        let unmatched = lines.iter().flatten().filter(|a| a.is_none()).count();
        *counts.entry(fallback.who).or_default() += unmatched;
        let main = counts.iter().max_by_key(|(_, n)| **n).map(|(w, _)| *w);
        let main_attr = match main {
            Some(w) if Some(w) == top_verified && w != fallback.who => Attr { who: w, level: Level::Declared },
            Some(w) if w == fallback.who => fallback,
            Some(w) => Attr { who: w, level: Level::Declared },
            None => fallback,
        };
        let mut out = vec![];
        for (fi, fl) in lines.into_iter().enumerate() {
            let keys = &c.files[fi].keys;
            out.push(
                fl.into_iter()
                    .zip(keys)
                    .map(|(a, &k)| match a {
                        Some(a) => a,
                        // 过短的行跟随主要贡献者
                        None if k == 0 => main_attr,
                        None => fallback,
                    })
                    .collect(),
            );
        }
        CommitAttr { lines: out, hits, conflicts, main, tab_lines, reason }
    }

    /// 没有逐行证据时的降级：同提交的 verified 主力 → trailer / 提交者 → 同时间窗口会话 → 分支前缀 → 未确认
    fn fallback(&mut self, c: &Commit, top_verified: Option<usize>) -> (Attr, String) {
        if let Some(w) = top_verified {
            return (Attr { who: w, level: Level::Declared }, "同提交内有逐行证据".into());
        }
        if let Some(h) = self.cfg.agent_emails.get(&c.email).cloned() {
            let model = self.session_at(c, Some(&h)).map(|s| s.model.clone());
            return (Attr { who: self.reg.agent(&h, model.as_deref()), level: Level::Declared }, format!("提交者邮箱 {}", c.email));
        }
        for co in &c.coauthors {
            if let Some((_, h)) = self.cfg.agent_emails.iter().find(|(e, _)| co.contains(e.as_str())) {
                let h = h.clone();
                return (Attr { who: self.reg.agent(&h, None), level: Level::Declared }, format!("Co-authored-by {co}"));
            }
        }
        if self.cfg.bot_emails.contains(&c.email) {
            return (Attr { who: self.reg.agent("github-actions", None), level: Level::Declared }, "bot 账号".into());
        }
        if !self.director_emails.contains(&c.email) {
            // 以邮箱为身份，显示 git 作者名
            let name = if c.name.is_empty() { c.email.clone() } else { c.name.clone() };
            return (Attr { who: self.reg.human(&format!("h:{}", c.email), &name), level: Level::Declared }, "其他人提交".into());
        }
        if let Some(s) = self.session_at(c, None) {
            let (h, m) = (s.src.harness(), s.model.clone());
            return (Attr { who: self.reg.agent(h, Some(&m)), level: Level::Declared }, format!("同时间窗口有 {h} 会话"));
        }
        if let Some(b) = self.unit_branch.get(&c.hash) {
            if let Some((_, h)) = self.cfg.branch_prefixes.iter().find(|(p, _)| b.starts_with(p.as_str())) {
                let h = h.clone();
                return (Attr { who: self.reg.agent(&h, None), level: Level::Inferred }, format!("分支前缀 {b}"));
            }
        }
        (Attr { who: self.reg.unattributed(), level: Level::Unattributed }, "无证据".into())
    }

    /// 提交时间落在某个写代码会话里（会话结束后 30 分钟内也算）；分支一致的优先
    fn session_at(&self, c: &Commit, harness: Option<&str>) -> Option<&'a Session> {
        let t = c.author_ts;
        let br = self.unit_branch.get(&c.hash);
        self.sessions
            .iter()
            .filter(|s| !s.reviewer && s.start - 600 <= t && t <= s.end + 1800)
            .filter(|s| harness.map(|h| s.src.harness() == h).unwrap_or(true))
            .max_by_key(|s| (br.is_some() && s.branch.as_ref() == br, s.start))
    }
}

/// 存活代码归因：blame 行 → 引入它的提交里同路径同哈希的那一行
pub fn survival(blame: &[(String, Vec<BlameLine>)], per_commit: &HashMap<String, (HashMap<(String, u32), Attr>, Option<Attr>)>, unattr: usize) -> Vec<Attr> {
    let mut out = vec![];
    for (_, lines) in blame {
        for b in lines {
            let a = per_commit.get(&b.commit).and_then(|(m, main)| m.get(&(b.rel.clone(), b.key)).copied().or(*main));
            out.push(a.unwrap_or(Attr { who: unattr, level: Level::Unattributed }));
        }
    }
    out
}

/// 板块分类：按路径规则
pub fn area(rel: &str, binary: bool) -> &'static str {
    let l = rel.to_lowercase();
    if binary {
        return "assets";
    }
    if l.starts_with("tests/") || l.contains("selfcheck") || l.contains("/tests/") || l.contains("check/") || l.ends_with("tests.swift") {
        "tests"
    } else if l.ends_with(".md") || l.starts_with("docs/") {
        "docs"
    } else if l.starts_with("scripts/") || l.ends_with(".sh") || l.ends_with("package.swift") || l.ends_with(".yml") || l.ends_with(".yaml") || l.starts_with(".github/") {
        "build"
    } else if l.contains("/views/") || l.starts_with("website") || l.contains("/resources/editor/") || l.ends_with(".css") || l.ends_with(".html") || l.contains("theme") || l.starts_with("app/") {
        "frontend"
    } else if l.ends_with(".svg") || l.contains("/assets/") {
        "assets"
    } else {
        "core"
    }
}
