//! 归因引擎：逐个非 merge 提交、逐行匹配证据，缺少证据时明确留白。

use crate::config::Config;
use crate::model::{Attr, BlameLine, Commit, CommitAttr, Index, Level, Registry, Session, Unit};
use std::collections::{HashMap, HashSet};

pub struct Engine<'a> {
    pub cfg: &'a Config,
    pub idx: &'a Index,
    pub reg: Registry,
    pub stamps: HashMap<String, (String, Option<String>)>,
}

impl<'a> Engine<'a> {
    pub fn new(cfg: &'a Config, idx: &'a Index, _sessions: &'a [Session], _units: &[Unit], stamps: HashMap<String, (String, Option<String>)>) -> Self {
        let mut reg = Registry::default();
        reg.unattributed();
        Engine { cfg, idx, reg, stamps }
    }

    pub fn attribute(&mut self, c: &Commit) -> CommitAttr {
        let mut hits = HashMap::new();
        let mut conflicts = 0;
        let mut tab_lines = 0;
        let unknown = Attr { who: self.reg.unattributed(), level: Level::Unattributed };
        // 声明只证明工具；没有模型声明时不按相近会话猜模型。
        let declaration = if let Some((h, m)) = self.stamps.get(&c.hash).cloned() {
            Some(Attr { who: self.reg.agent(&h, m.as_deref()), level: Level::Declared })
        } else if let Some(h) = self.cfg.agent_emails.get(&c.email).cloned() {
            Some(Attr { who: self.reg.agent(&h, None), level: Level::Declared })
        } else {
            let authors: HashSet<String> = c.coauthors.iter().flat_map(|co| {
                self.cfg.agent_emails.iter().filter(|(email, _)| co.to_lowercase().contains(&format!("<{}>", email))).map(|(_, h)| h.clone())
            }).collect();
            if authors.len() == 1 {
                Some(Attr { who: self.reg.agent(authors.iter().next().unwrap(), None), level: Level::Declared })
            } else { None }
        };
        let mut counts: HashMap<usize, usize> = HashMap::new();
        let lines = c.files.iter().map(|f| f.keys.iter().map(|key| {
            let candidates: Vec<_> = self.idx.map.get(&(f.rel.clone(), *key)).into_iter().flatten()
                .filter(|e| e.ts > 0 && e.ts <= c.author_ts.max(c.commit_ts)).collect();
            let identities: HashSet<_> = candidates.iter().map(|e| (e.src, crate::model::normalize_model(&e.model), e.provider.as_deref(), e.tab)).collect();
            let attr = if identities.len() > 1 {
                conflicts += 1;
                unknown
            } else if let Some(e) = candidates.iter().max_by_key(|e| e.ts) {
                if e.tab {
                    // Tab 记录不能确定键盘前是哪位人；单独计数，不把功劳塞给提交最多的人。
                    tab_lines += 1;
                    unknown
                } else {
                    *hits.entry(e.src.harness()).or_insert(0) += 1;
                    Attr { who: self.reg.agent_via(e.src.harness(), Some(&e.model), e.provider.as_deref()), level: Level::Verified }
                }
            } else { declaration.unwrap_or(unknown) };
            *counts.entry(attr.who).or_insert(0) += 1;
            attr
        }).collect()).collect();
        let main = counts.iter().max_by(|(wa, a), (wb, b)| a.cmp(b).then_with(|| wb.cmp(wa))).map(|(w, _)| *w);
        CommitAttr { lines, hits, conflicts, main, tab_lines, reason: if declaration.is_some() { "提交声明与逐行记录分别计量" } else { "仅按可核对的逐行记录；缺项保留未确认" }.into() }
    }
}

/// 存活代码归因：blame 行 → 引入它的提交里同路径同哈希的那一行
pub fn survival(blame: &[(String, Vec<BlameLine>)], per_commit: &HashMap<String, (HashMap<(String, u32), Attr>, Option<Attr>)>, unattr: usize) -> Vec<Attr> {
    let mut out = vec![];
    for (_, lines) in blame {
        for b in lines {
            let a = per_commit.get(&b.commit).and_then(|(m, _main)| m.get(&(b.rel.clone(), b.key)).copied());
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
    use crate::model::{Ev, FileAdd, Store, Src};

    fn cfg() -> Config {
        Config::default()
    }

    fn commit(email: &str, keys: Vec<u32>) -> Commit {
        Commit { hash: "a".repeat(40), email: email.into(), name: "".into(), author_ts: 1000, commit_ts: 1000, subject: "s".into(), coauthors: vec![], files: vec![FileAdd { rel: "a.rs".into(), keys, binary: false }] }
    }

    #[test]
    fn verified_lines_do_not_assign_the_rest() {
        let mut store = Store::default();
        let g = store.groups.entry("x".into()).or_default();
        g.evs.push(Ev { provider: None, rel: "a.rs".into(), key: 7, src: Src::Codex, model: "m".into(), session: "s".into(), ts: 900, tab: false });
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let a = e.attribute(&commit("me@x.com", vec![7, 0, 9]));
        assert_eq!(a.lines[0][0].level, Level::Verified);
        assert_eq!(a.lines[0][1].level, Level::Unattributed);
        assert_eq!(a.lines[0][2].level, Level::Unattributed);
        assert_eq!(e.reg.list[a.lines[0][0].who].id, "a:codex/m");
    }

    #[test]
    fn declarations_are_distinct_from_verified_evidence() {
        let store = Store::default();
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        // 仅有人类提交身份，无实际写作证据 → 未确认
        let a = e.attribute(&commit("me@x.com", vec![1]));
        assert_eq!(a.lines[0][0].level, Level::Unattributed);
        // agent 邮箱 → declared
        let a = e.attribute(&commit("cursoragent@cursor.com", vec![1]));
        assert_eq!(a.lines[0][0].level, Level::Declared);
        assert_eq!(e.reg.list[a.lines[0][0].who].harness.as_deref(), Some("cursor"));
        // 其他提交身份也不能证明实际写作者
        let a = e.attribute(&commit("other@y.com", vec![1]));
        assert_eq!(e.reg.list[a.lines[0][0].who].kind, "unattributed");
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
