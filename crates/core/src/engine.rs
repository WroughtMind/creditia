//! 归因引擎：逐个非 merge 提交、逐行匹配证据，再按降级链补全。

use crate::config::Config;
use crate::model::{Attr, BlameLine, Commit, CommitAttr, Index, Level, Registry, Session, Src, Unit};
use std::collections::{HashMap, HashSet};

const WINDOW_BEFORE: i64 = 14 * 86400;
const WINDOW_AFTER: i64 = 3600;

/// 归因口径
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strictness {
    /// 只认逐行证据和明确声明（盖章、agent 邮箱、Co-authored-by、bot）
    Strict,
    /// 默认：再加两种推断——同一改动块内的延伸（块里大半有逐行证据，其余的行跟着它）和分支前缀
    Standard,
    /// 再加同时间窗口的会话（标为推断）
    Loose,
}
impl Strictness {
    pub fn parse(s: &str) -> Strictness {
        match s { "strict" => Strictness::Strict, "loose" => Strictness::Loose, _ => Strictness::Standard }
    }
}

pub struct Engine<'a> {
    pub cfg: &'a Config,
    pub strictness: Strictness,
    pub idx: &'a Index,
    pub sessions: &'a [Session],
    pub reg: Registry,
    director_emails: HashSet<String>,
    pub unit_branch: HashMap<String, String>,
    /// git trailer 盖章：commit → (harness, model)
    pub stamps: HashMap<String, (String, Option<String>)>,
    /// 已被提交认领过的证据（与 idx.map 里的 Vec 一一对应）
    used: HashMap<(String, u32), Vec<bool>>,
}

impl<'a> Engine<'a> {
    pub fn new(cfg: &'a Config, idx: &'a Index, sessions: &'a [Session], units: &[Unit], stamps: HashMap<String, (String, Option<String>)>) -> Self {
        let mut reg = Registry::default();
        for d in &cfg.directors {
            reg.human(&d.id, &d.name);
        }
        if cfg.directors.is_empty() {
            // 没有配置主导者时仍保留 0 号位，避免 director() 越界
            reg.human("h:me", "me");
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
        Engine { cfg, strictness: Strictness::parse(&cfg.level), idx, sessions, reg, director_emails, unit_branch, stamps, used: HashMap::new() }
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
        // 每条逐行证据记下是哪个工具、哪个模型，改动块推断用
        let mut ev_of: Vec<Vec<Option<(&'static str, String)>>> = vec![];
        // 第一轮：逐行查证据。同一条证据（一次写入的一行）只认领一次：
        // 人后来又敲了一行一模一样的 `return Ok(());`，不能再算给当初写它的 agent
        for f in &c.files {
            let mut fl = vec![];
            let mut el = vec![];
            for &k in &f.keys {
                el.push(None);
                if k == 0 {
                    fl.push(None);
                    continue;
                }
                let id = (f.rel.clone(), k);
                let Some(all) = self.idx.map.get(&id) else {
                    fl.push(None);
                    continue;
                };
                let used = self.used.entry(id).or_insert_with(|| vec![false; all.len()]);
                let cands: Vec<usize> = (0..all.len()).filter(|&i| all[i].ts >= lo && all[i].ts <= hi && !used[i]).collect();
                if cands.is_empty() {
                    fl.push(None);
                    continue;
                }
                let srcs: HashSet<Src> = cands.iter().map(|&i| all[i].src).collect();
                if srcs.len() > 1 {
                    conflicts += 1;
                }
                // 取时间最接近提交的那条
                let bi = *cands.iter().min_by_key(|&&i| (c.author_ts - all[i].ts).abs()).unwrap();
                let best = &all[bi];
                // Cursor 的行哈希表一行只存一次，不按次数消耗
                if best.src != Src::Cursor {
                    used[bi] = true;
                }
                if best.tab {
                    tab_lines += 1;
                    fl.push(Some(Attr { who: self.director(), level: Level::Verified }));
                    continue;
                }
                *hits.entry(best.src.harness()).or_default() += 1;
                let who = self.reg.agent(best.src.harness(), Some(&best.model));
                *el.last_mut().unwrap() = Some((best.src.harness(), best.model.clone()));
                fl.push(Some(Attr { who, level: Level::Verified }));
            }
            lines.push(fl);
            ev_of.push(el);
        }
        // 第二轮：提交本身的声明（盖章、agent 邮箱、Co-authored-by、bot、别人的提交），覆盖所有没对上的行
        let declared = self.declared(c);
        let mut reason = declared.as_ref().map(|d| d.1.clone());
        // 第三轮：同一改动块里的推断。块里一半以上的行有逐行证据、其中八成以上是同一个工具，
        // 才把块里其余的行推给它；推断出来的行不再当证据。模型分不清时只记工具。
        // 提交有声明时，只在块的工具和声明一致时推断（补上声明里没有的模型），否则以声明为准
        let declared_h = declared.as_ref().map(|d| self.reg.list[d.0.who].harness.clone());
        if self.strictness != Strictness::Strict {
            for (fi, f) in c.files.iter().enumerate() {
                for (s, e) in hunk_ranges(f) {
                    let long = (s..e).filter(|&i| f.keys[i] != 0).count();
                    let evs: Vec<&(&'static str, String)> = (s..e).filter_map(|i| ev_of[fi][i].as_ref()).collect();
                    if evs.is_empty() || evs.len() * 2 < long || (s..e).all(|i| f.keys[i] == 0 || lines[fi][i].is_some()) {
                        continue;
                    }
                    let mut by_h: HashMap<&str, usize> = HashMap::new();
                    for (h, _) in &evs {
                        *by_h.entry(h).or_default() += 1;
                    }
                    let (&h, &n) = by_h.iter().max_by_key(|(h, n)| (**n, std::cmp::Reverse(**h))).unwrap();
                    if n * 5 < evs.len() * 4 {
                        continue;
                    }
                    if declared_h.as_ref().is_some_and(|d| d.as_deref() != Some(h)) {
                        continue;
                    }
                    let mut by_m: HashMap<&str, usize> = HashMap::new();
                    for (_, m) in evs.iter().filter(|(x, _)| *x == h) {
                        *by_m.entry(m.as_str()).or_default() += 1;
                    }
                    let (&m, &mn) = by_m.iter().max_by_key(|(m, n)| (**n, std::cmp::Reverse(**m))).unwrap();
                    let who = self.reg.agent(h, (mn * 5 >= n * 4).then_some(m));
                    for (l, &k) in lines[fi][s..e].iter_mut().zip(&f.keys[s..e]) {
                        if k != 0 && l.is_none() {
                            *l = Some(Attr { who, level: Level::Inferred });
                        }
                    }
                    reason.get_or_insert_with(|| "同一改动块内有逐行证据（推断）".into());
                }
            }
        }
        // 第四轮：还没着落的行走提交级的降级（会话、分支前缀、未确认）
        let (fallback, r) = match declared {
            Some(d) => d,
            None => self.guess(c),
        };
        let reason = reason.unwrap_or(r);
        // 主要贡献者：所有长行里最多的那个
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for (fi, f) in c.files.iter().enumerate() {
            for (i, a) in lines[fi].iter().enumerate() {
                if f.keys[i] != 0 {
                    *counts.entry(a.map(|a| a.who).unwrap_or(fallback.who)).or_default() += 1;
                }
            }
        }
        let main = counts.iter().max_by_key(|(w, n)| (**n, std::cmp::Reverse(**w))).map(|(w, _)| *w);
        let main_attr = match main {
            Some(w) if w == fallback.who => fallback,
            Some(w) => Attr { who: w, level: Level::Declared },
            None => fallback,
        };
        let mut out = vec![];
        for (fi, fl) in lines.into_iter().enumerate() {
            let f = &c.files[fi];
            let mut res: Vec<Attr> = fl.iter().map(|a| a.unwrap_or(fallback)).collect();
            // 过短的行（`}`、空行）跟随所在改动块里最多的那个；块里没有长行就跟随整个提交的主要贡献者
            for (s, e) in hunk_ranges(f) {
                let mut cnt: HashMap<usize, (usize, Level)> = HashMap::new();
                for i in (s..e).filter(|&i| f.keys[i] != 0) {
                    let x = cnt.entry(res[i].who).or_insert((0, res[i].level));
                    x.0 += 1;
                    x.1 = x.1.max(res[i].level);
                }
                let follow = cnt
                    .iter()
                    .max_by_key(|(w, (n, _))| (*n, std::cmp::Reverse(**w)))
                    .map(|(&w, &(_, lv))| Attr { who: w, level: if lv == Level::Verified { Level::Declared } else { lv } })
                    .unwrap_or(main_attr);
                for i in (s..e).filter(|&i| f.keys[i] == 0) {
                    res[i] = follow;
                }
            }
            out.push(res);
        }
        CommitAttr { lines: out, hits, conflicts, main, tab_lines, reason }
    }

    /// 提交本身写明的来源。按优先级：盖章 trailer → agent 提交邮箱 → Co-authored-by → bot → aider 的作者名 → 别人的提交（算那个人）
    fn declared(&mut self, c: &Commit) -> Option<(Attr, String)> {
        if let Some((h, m)) = self.stamps.get(&c.hash).cloned() {
            return Some((Attr { who: self.reg.agent(&h, m.as_deref()), level: Level::Declared }, "Creditia 盖章".into()));
        }
        if let Some(h) = self.cfg.agent_emails.get(&c.email).cloned() {
            return Some((Attr { who: self.reg.agent(&h, None), level: Level::Declared }, format!("agent 提交邮箱 {}", c.email)));
        }
        for co in &c.coauthors {
            if let Some((_, h)) = self.cfg.agent_emails.iter().find(|(e, _)| co.contains(e.as_str())) {
                let h = h.clone();
                let m = coauthor_model(co);
                return Some((Attr { who: self.reg.agent(&h, m.as_deref()), level: Level::Declared }, format!("Co-authored-by {co}")));
            }
        }
        if self.cfg.bot_emails.contains(&c.email) || c.email.contains("[bot]") {
            let h = if c.email.contains("dependabot") { "dependabot" } else if c.email.contains("renovate") { "renovate" } else { "github-actions" };
            return Some((Attr { who: self.reg.agent(h, None), level: Level::Declared }, "bot 账号".into()));
        }
        if c.name.trim_end().ends_with("(aider)") {
            return Some((Attr { who: self.reg.agent("aider", None), level: Level::Declared }, "aider 署名的作者".into()));
        }
        if !self.director_emails.contains(&c.email) {
            // 以邮箱为身份，显示 git 作者名
            let name = if c.name.is_empty() { c.email.clone() } else { c.name.clone() };
            let name = self.cfg.display_name(&name);
            // id 用邮箱的短哈希：档案可能被公开，不能带出邮箱
            return Some((Attr { who: self.reg.human(&person_id(&c.email), &name), level: Level::Declared }, "其他人的提交".into()));
        }
        None
    }

    /// 既没有逐行证据、也没有声明的行：同时间窗口的会话（loose，推断）→ 分支前缀（standard，推断）→ 未确认
    fn guess(&mut self, c: &Commit) -> (Attr, String) {
        if self.strictness == Strictness::Loose {
            if let Some(s) = self.session_at(c, None) {
                let (h, m) = (s.src.harness(), s.model.clone());
                return (Attr { who: self.reg.agent(h, Some(&m)), level: Level::Inferred }, format!("提交时 {h} 会话正在进行（推断）"));
            }
        }
        if self.strictness != Strictness::Strict {
            if let Some(b) = self.unit_branch.get(&c.hash) {
                if let Some((_, h)) = self.cfg.branch_prefixes.iter().find(|(p, _)| b.starts_with(p.as_str())) {
                    let h = h.clone();
                    return (Attr { who: self.reg.agent(&h, None), level: Level::Inferred }, format!("分支前缀 {b}（推断）"));
                }
            }
        }
        (Attr { who: self.reg.unattributed(), level: Level::Unattributed }, "没有证据".into())
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

/// Co-authored-by 的名字里写了模型（Claude Code 会写 `Claude Opus 5.5 <noreply@anthropic.com>`）就取出来；
/// 只有工具名（`Claude`、`Copilot`）时没有模型
fn coauthor_model(co: &str) -> Option<String> {
    let name = co.split('<').next().unwrap_or("").trim();
    let words: Vec<&str> = name.split_whitespace().collect();
    (words.len() >= 2 && name.chars().any(|c| c.is_ascii_digit())).then(|| words.join("-").to_lowercase())
}

/// 一个文件的新增行按改动块切开：[(起, 止)]。没记改动块的（旧缓存）整个文件算一块
fn hunk_ranges(f: &crate::model::FileAdd) -> Vec<(usize, usize)> {
    let n = f.keys.len();
    let mut starts: Vec<usize> = f.hunks.iter().map(|&s| s as usize).filter(|&s| s < n).collect();
    if starts.first() != Some(&0) {
        starts.insert(0, 0);
    }
    starts.dedup();
    starts.iter().enumerate().map(|(i, &s)| (s, starts.get(i + 1).copied().unwrap_or(n))).filter(|(s, e)| s < e).collect()
}

/// 人的贡献者 id：邮箱（小写）的 FNV 哈希，十六进制 10 位。同一邮箱永远得到同一个 id，但看不出邮箱
pub fn person_id(email: &str) -> String {
    let h = email.trim().to_lowercase().bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    format!("h:{:010x}", h & 0xff_ffff_ffff)
}

/// 每个提交的归因：(路径, 行哈希) → 归因，和这个提交整体的归因（没有逐行证据时用）
pub type CommitAttrs = HashMap<String, (HashMap<(String, u32), Attr>, Option<Attr>)>;

/// 存活代码归因：blame 行 → 引入它的提交里同路径同哈希的那一行
pub fn survival(blame: &[(String, Vec<BlameLine>)], per_commit: &CommitAttrs, unattr: usize) -> Vec<Attr> {
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
    use crate::model::{Ev, FileAdd, Session, Store};

    fn cfg() -> Config {
        let mut c = Config::default();
        c.directors.push(crate::config::Director { id: "h:me".into(), name: "me".into(), emails: vec!["me@x.com".into()] });
        c
    }

    fn commit(email: &str, keys: Vec<u32>) -> Commit {
        Commit { hash: "a".repeat(40), email: email.into(), name: "".into(), author_ts: 1000, commit_ts: 1000, subject: "s".into(), coauthors: vec![], files: vec![FileAdd { rel: "a.rs".into(), keys, ..Default::default() }] }
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
        assert_eq!(a.lines[0][2].level, Level::Inferred); // 没对上的行跟随同提交的 verified 主力，标为推断
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
        // 别人 → 人类贡献者；id 里不带邮箱
        let a = e.attribute(&commit("other@y.com", vec![1]));
        assert_eq!(e.reg.list[a.lines[0][0].who].kind, "human");
        assert!(!e.reg.list[a.lines[0][0].who].id.contains('@'));
        assert_eq!(e.reg.list[a.lines[0][0].who].id, person_id("Other@Y.com"));
        // 盖章优先于邮箱
        let mut stamps = HashMap::new();
        stamps.insert("a".repeat(40), ("kimi".to_string(), Some("k3".to_string())));
        let mut e = Engine::new(&cfg, &idx, &[], &[], stamps);
        let a = e.attribute(&commit("cursoragent@cursor.com", vec![1]));
        assert_eq!(e.reg.list[a.lines[0][0].who].id, "a:kimi/k3");
        // aider 改写的作者名
        let mut cm = commit("me@x.com", vec![1]);
        cm.name = "Me (aider)".into();
        cm.hash = "b".repeat(40);
        let a = e.attribute(&cm);
        assert_eq!(e.reg.list[a.lines[0][0].who].harness.as_deref(), Some("aider"));
    }

    #[test]
    fn strictness_gates_guesses() {
        // 自己的提交，没有证据，但提交时有 codex 会话在跑：只有 loose 才认
        let store = Store::default();
        let idx = Index::build(&store);
        let sess = vec![Session { src: Src::Codex, id: "s".into(), model: "gpt-x".into(), start: 900, end: 1100, tokens: 0, branch: None, prompts: 1, reviewer: false, has_content: true, provider: None }];
        for (lv, want) in [("strict", Level::Unattributed), ("standard", Level::Unattributed), ("loose", Level::Inferred)] {
            let mut c = cfg();
            c.level = lv.into();
            let mut e = Engine::new(&c, &idx, &sess, &[], HashMap::new());
            let a = e.attribute(&commit("me@x.com", vec![1]));
            assert_eq!(a.lines[0][0].level, want, "{lv}");
        }
        // strict 下，同提交里没对上的行不延伸给有逐行证据的 agent
        let mut store = Store::default();
        store.groups.entry("x".into()).or_default().evs.push(Ev { rel: "a.rs".into(), key: 7, src: Src::Codex, model: "m".into(), session: "s".into(), ts: 900, tab: false });
        let idx = Index::build(&store);
        let mut c = cfg();
        c.level = "strict".into();
        let mut e = Engine::new(&c, &idx, &[], &[], HashMap::new());
        let a = e.attribute(&commit("me@x.com", vec![7, 9]));
        assert_eq!(a.lines[0][0].level, Level::Verified);
        assert_eq!(a.lines[0][1].level, Level::Unattributed);
    }

    fn ev(key: u32, src: Src, model: &str) -> Ev {
        Ev { rel: "a.rs".into(), key, src, model: model.into(), session: "s".into(), ts: 900, tab: false }
    }

    #[test]
    fn inference_stays_inside_its_hunk() {
        // 第一块 3 行有 2 行是 Codex 的逐行证据：第三行跟着推断；第二块 4 行一行证据都没有：不能被第一块带走
        let mut store = Store::default();
        store.groups.entry("x".into()).or_default().evs.extend([ev(1, Src::Codex, "m"), ev(2, Src::Codex, "m")]);
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let mut c = commit("me@x.com", vec![1, 2, 3, 0, 10, 11, 12, 13]);
        c.files[0].hunks = vec![0, 4];
        let a = e.attribute(&c);
        let l = &a.lines[0];
        assert_eq!(l[2].level, Level::Inferred);
        assert_eq!(l[2].who, l[0].who);
        assert_eq!(l[3].who, l[0].who); // 短行跟随所在的块
        assert!(l[4..].iter().all(|x| x.level == Level::Unattributed), "{l:?}");
        // 块里只有一小部分有证据：不推断
        let mut c = commit("me@x.com", vec![1, 20, 21, 22, 23]);
        c.hash = "b".repeat(40);
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let a = e.attribute(&c);
        assert!(a.lines[0][1..].iter().all(|x| x.level == Level::Unattributed));
    }

    #[test]
    fn declaration_beats_another_tools_evidence() {
        // 盖章 kimi 的提交里有 1 行 Codex 的逐行证据：那一行归 Codex，其余按盖章归 kimi
        let mut store = Store::default();
        store.groups.entry("x".into()).or_default().evs.push(ev(1, Src::Codex, "m"));
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut stamps = HashMap::new();
        stamps.insert("a".repeat(40), ("kimi".to_string(), Some("k3".to_string())));
        let mut e = Engine::new(&cfg, &idx, &[], &[], stamps);
        let a = e.attribute(&commit("me@x.com", vec![1, 2, 3]));
        assert_eq!(e.reg.list[a.lines[0][0].who].id, "a:codex/m");
        assert_eq!(e.reg.list[a.lines[0][1].who].id, "a:kimi/k3");
        assert_eq!(a.lines[0][1].level, Level::Declared);
    }

    #[test]
    fn evidence_is_claimed_once() {
        // Codex 写过一次这一行；两个提交里都出现了它：只有先提交的那个算 Codex
        let mut store = Store::default();
        store.groups.entry("x".into()).or_default().evs.push(ev(5, Src::Codex, "m"));
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let a = e.attribute(&commit("me@x.com", vec![5]));
        assert_eq!(a.lines[0][0].level, Level::Verified);
        let mut c = commit("me@x.com", vec![5]);
        c.hash = "c".repeat(40);
        let a = e.attribute(&c);
        assert_eq!(a.lines[0][0].level, Level::Unattributed);
    }

    #[test]
    fn coauthor_names_carry_models() {
        assert_eq!(coauthor_model("claude opus 5.5 <noreply@anthropic.com>").as_deref(), Some("claude-opus-5.5"));
        assert_eq!(coauthor_model("claude <noreply@anthropic.com>"), None);
        assert_eq!(coauthor_model("copilot <175728472+copilot@users.noreply.github.com>"), None);
        let store = Store::default();
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let mut c = commit("me@x.com", vec![1]);
        c.coauthors = vec!["claude opus 5.5 <noreply@anthropic.com>".into()];
        let a = e.attribute(&c);
        assert_eq!(e.reg.list[a.lines[0][0].who].id, "a:claude-code/claude-opus-5-5");
    }

    #[test]
    fn stamp_parse() {
        assert_eq!(parse_stamp("Creditia-Harness: Codex\nCreditia-Model: gpt-6\n"), Some(("codex".into(), Some("gpt-6".into()))));
        assert_eq!(parse_stamp("Signed-off-by: x"), None);
    }
}
