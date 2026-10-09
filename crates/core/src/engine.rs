//! 归因引擎：逐个非 merge 提交、逐行匹配证据，再按降级链补全。

use crate::config::Config;
use crate::model::{Attr, BlameLine, Commit, CommitAttr, Index, Level, Registry, Session, Src, Unit};
use std::collections::{HashMap, HashSet};

const WINDOW_BEFORE: i64 = 14 * 86400;
const WINDOW_AFTER: i64 = 3600;

pub struct Engine<'a> {
    pub cfg: &'a Config,
    pub idx: &'a Index,
    pub sessions: &'a [Session],
    pub reg: Registry,
    director_emails: HashSet<String>,
    pub unit_branch: HashMap<String, String>,
    /// git trailer 盖章：commit → (harness, model)
    pub stamps: HashMap<String, (String, Option<String>)>,
}

impl<'a> Engine<'a> {
    pub fn new(cfg: &'a Config, idx: &'a Index, sessions: &'a [Session], units: &[Unit], stamps: HashMap<String, (String, Option<String>)>) -> Self {
        let mut reg = Registry::default();
        for d in &cfg.directors {
            reg.human(&d.id, &d.name);
        }
        if cfg.directors.is_empty() {
            // 没有配置主导者时仍保留 0 号位，避免 director() 越界
            reg.human("h:unknown", "");
        }
        reg.unattributed();
        let director_emails = cfg.director_emails().into_iter().collect();
        let mut unit_branch = HashMap::new();
        for u in units {
            if let Some(b) = &u.branch {
                for c in &u.commits {
                    unit_branch.insert(c.clone(), b.clone());
                }
            }
        }
        Engine { cfg, idx, sessions, reg, director_emails, unit_branch, stamps }
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
        // 主要贡献者：verified 最多的；没有就是降级结果
        let mut counts: HashMap<usize, usize> = verified.clone();
        let unmatched = lines.iter().flatten().filter(|a| a.is_none()).count();
        *counts.entry(fallback.who).or_default() += unmatched;
        let main = counts.iter().max_by_key(|(_, n)| **n).map(|(w, _)| *w);
        let main_attr = match main {
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

    /// 没有逐行证据时的降级：同提交的 verified 主力 → 盖章 trailer → 提交者 / Co-authored-by → bot →
    /// 其他人 → 同时间窗口会话 → 分支前缀 → 未确认
    fn fallback(&mut self, c: &Commit, top_verified: Option<usize>) -> (Attr, String) {
        if let Some(w) = top_verified {
            return (Attr { who: w, level: Level::Declared }, "同提交内有逐行证据".into());
        }
        if let Some((h, m)) = self.stamps.get(&c.hash).cloned() {
            return (Attr { who: self.reg.agent(&h, m.as_deref()), level: Level::Declared }, "Creditia 盖章".into());
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
            let name = self.cfg.display_name(&name);
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

/// 从提交信息 trailer 里读盖章：`Creditia-Harness: codex`、`Creditia-Model: gpt-5.6-sol`
pub fn parse_stamp(trailers: &str) -> Option<(String, Option<String>)> {
    let mut h = None;
    let mut m = None;
    for l in trailers.lines() {
        if let Some(v) = l.strip_prefix("Creditia-Harness:") {
            h = Some(v.trim().to_lowercase());
        } else if let Some(v) = l.strip_prefix("Creditia-Model:") {
            m = Some(v.trim().to_string()).filter(|s| !s.is_empty());
        }
    }
    h.map(|h| (h, m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Ev, FileAdd, Store};

    fn cfg() -> Config {
        let mut c = Config::default();
        c.directors.push(crate::config::Director { id: "h:me".into(), name: "me".into(), emails: vec!["me@x.com".into()] });
        c
    }

    fn commit(email: &str, keys: Vec<u32>) -> Commit {
        Commit { hash: "a".repeat(40), email: email.into(), name: "".into(), author_ts: 1000, commit_ts: 1000, subject: "s".into(), coauthors: vec![], files: vec![FileAdd { rel: "a.rs".into(), keys, binary: false }] }
    }

    #[test]
    fn verified_wins_and_short_lines_follow_main() {
        let mut store = Store::default();
        let g = store.groups.entry("x".into()).or_default();
        g.evs.push(Ev { rel: "a.rs".into(), key: 7, src: Src::Codex, model: "m".into(), session: "s".into(), ts: 900, tab: false });
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let a = e.attribute(&commit("me@x.com", vec![7, 0, 9]));
        assert_eq!(a.lines[0][0].level, Level::Verified);
        assert_eq!(a.lines[0][1].who, a.lines[0][0].who); // 过短行跟随主力
        assert_eq!(a.lines[0][2].level, Level::Declared); // 没对上的行跟随同提交的 verified 主力
        assert_eq!(e.reg.list[a.lines[0][0].who].id, "a:codex/m");
    }

    #[test]
    fn fallback_chain() {
        let store = Store::default();
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        // 主导者自己的提交，无任何证据 → 未确认
        let a = e.attribute(&commit("me@x.com", vec![1]));
        assert_eq!(a.lines[0][0].level, Level::Unattributed);
        // agent 邮箱 → declared
        let a = e.attribute(&commit("cursoragent@cursor.com", vec![1]));
        assert_eq!(a.lines[0][0].level, Level::Declared);
        assert_eq!(e.reg.list[a.lines[0][0].who].harness.as_deref(), Some("cursor"));
        // 别人 → 人类贡献者
        let a = e.attribute(&commit("other@y.com", vec![1]));
        assert_eq!(e.reg.list[a.lines[0][0].who].kind, "human");
        // 盖章优先于邮箱
        let mut stamps = HashMap::new();
        stamps.insert("a".repeat(40), ("kimi".to_string(), Some("k3".to_string())));
        let mut e = Engine::new(&cfg, &idx, &[], &[], stamps);
        let a = e.attribute(&commit("cursoragent@cursor.com", vec![1]));
        assert_eq!(e.reg.list[a.lines[0][0].who].id, "a:kimi/k3");
    }

    #[test]
    fn stamp_parse() {
        assert_eq!(parse_stamp("Creditia-Harness: Codex\nCreditia-Model: gpt-6\n"), Some(("codex".into(), Some("gpt-6".into()))));
        assert_eq!(parse_stamp("Signed-off-by: x"), None);
    }
}
