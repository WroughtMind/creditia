//! 归因引擎：逐个非 merge 提交、逐行匹配证据，再按降级链补全。

use crate::config::Config;
use crate::hash::short_key;
use crate::model::{canonical_harness, Attr, BlameFile, Commit, CommitAttr, Index, Level, Registry, Session, Src, Unit, JOIN_WINDOWS};
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
    /// 主导者的 GitHub 账号名：同一个账号的几种 noreply 邮箱（`123+login@…`、`login@…`）都是他
    director_logins: HashSet<String>,
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
        let director_logins = cfg.director_emails().iter().filter_map(|e| gh_login(e)).map(String::from).collect();
        Engine { cfg, strictness: Strictness::parse(&cfg.level), idx, sessions, reg, director_emails, director_logins, unit_branch, stamps, used: HashMap::new() }
    }

    fn is_director(&self, email: &str) -> bool {
        self.director_emails.contains(email) || gh_login(email).is_some_and(|l| self.director_logins.contains(l))
    }

    fn director(&self) -> usize {
        0
    }

    /// 提交是谁提交的（按贡献者看用）：(贡献者 id, git 作者名, 是不是你)。只说提交者，不说代码是谁写的：
    /// 你的提交里 agent 写的行、别人提交里的盖章，仍按归因结果算在工具名下
    pub fn author_of(&self, c: &Commit) -> (String, String, bool) {
        if self.is_director(&c.email) {
            let d = &self.reg.list[self.director()];
            return (d.id.clone(), d.name.clone().unwrap_or_default(), true);
        }
        let name = if c.name.is_empty() { c.email.clone() } else { c.name.clone() };
        (person_id(&c.email), self.cfg.display_name(&name), false)
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
            let ranges = hunk_ranges(f);
            for (i, &k) in f.keys.iter().enumerate() {
                el.push(None);
                if k == 0 {
                    // 短行：按同一改动块里它上面、下面最近的长行定位，对上了就跟那次写入走
                    let (s, e) = ranges.iter().copied().find(|&(s, e)| s <= i && i < e).unwrap_or((0, f.keys.len()));
                    let sh = f.short.get(i).copied().unwrap_or(0);
                    let up = (s..i).rev().find(|&j| f.keys[j] != 0).map(|j| (j, 'v', (j + 1..=i).filter(|&x| f.keys[x] == 0).count()));
                    let down = (i + 1..e).find(|&j| f.keys[j] != 0).map(|j| (j, '^', (i..j).filter(|&x| f.keys[x] == 0).count()));
                    let hit = if sh == 0 { None } else { [up, down].into_iter().flatten().find_map(|(j, d, ord)| self.take(&f.rel, short_key(&f.rel, f.keys[j], d, ord, sh), lo, hi, c.author_ts).map(|(b, _)| b)) };
                    fl.push(hit.filter(|b| !b.tab).map(|b| Attr { who: self.reg.agent(b.src.harness(), Some(&b.model)), level: Level::Verified }));
                    continue;
                }
                let Some((best, multi)) = self.take(&f.rel, k, lo, hi, c.author_ts) else {
                    fl.push(None);
                    continue;
                };
                if multi {
                    conflicts += 1;
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
        // 第一轮补充：格式化拆行。没对上的连续 2~4 行拼回一行，正好是 agent 一次写入里的一行，就是那次写的
        for (fi, f) in c.files.iter().enumerate() {
            if f.joins.len() != f.keys.len() * JOIN_WINDOWS {
                continue;
            }
            for (s, e) in hunk_ranges(f) {
                let mut i = s;
                while i < e {
                    let mut took = 0;
                    for w in (2..=JOIN_WINDOWS + 1).rev() {
                        if i + w > e || (i..i + w).any(|j| lines[fi][j].is_some()) || (i..i + w).all(|j| f.keys[j] == 0) {
                            continue;
                        }
                        let key = f.joins[i * JOIN_WINDOWS + (w - 2)];
                        if key == 0 {
                            continue;
                        }
                        if let Some((b, multi)) = self.take(&f.rel, key, lo, hi, c.author_ts) {
                            if b.tab {
                                continue;
                            }
                            conflicts += multi as usize;
                            let who = self.reg.agent(b.src.harness(), Some(&b.model));
                            for j in i..i + w {
                                lines[fi][j] = Some(Attr { who, level: Level::Verified });
                                if f.keys[j] != 0 {
                                    ev_of[fi][j] = Some((b.src.harness(), b.model.clone()));
                                    *hits.entry(b.src.harness()).or_default() += 1;
                                }
                            }
                            took = w;
                            break;
                        }
                    }
                    i += took.max(1);
                }
            }
        }
        // 第二轮：提交本身的声明（盖章、agent 邮箱、Co-authored-by、bot、别人的提交），覆盖所有没对上的行。
        // 带 Creditia-Lines 的提交，工具的声明只按它记下的行数算（见最后的配额），不再整个提交算给工具
        let declared = self.declared(c, c.lines_stamp.is_some());
        let mut reason = declared.as_ref().map(|d| d.1.clone());
        // 第三轮：同一改动块里的推断。块里一半以上的行有逐行证据、其中八成以上是同一个工具，
        // 才把块里其余的行推给它；推断出来的行不再当证据。模型分不清时只记工具。
        // 提交有声明时，只在块的工具和声明一致时推断（补上声明里没有的模型），否则以声明为准
        let declared_h = declared.as_ref().map(|d| self.reg.list[d.0.who].harness.clone());
        if self.strictness != Strictness::Strict {
            for (fi, f) in c.files.iter().enumerate() {
                let inserted = self.inserted(f, &ev_of[fi]);
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
                    for ((l, &k), &ins) in lines[fi][s..e].iter_mut().zip(&f.keys[s..e]).zip(&inserted[s..e]) {
                        if k != 0 && l.is_none() && !ins {
                            *l = Some(Attr { who, level: Level::Inferred });
                        }
                    }
                    reason.get_or_insert_with(|| crate::tr!("同一改动块内有逐行证据（推断）", "line evidence in the same change (inferred)"));
                }
            }
        }
        // 第四轮：还没着落的行走提交级的降级（会话、分支前缀、未确认）。
        // 没有 AI 证据不说明是人写的：creditia 只认 AI 留下的记录，不推断谁手写
        let (fallback, r) = match &declared {
            Some(d) => d.clone(),
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
            // 自己对上了写入的短行不再跟随别人
            let placed: Vec<bool> = fl.iter().zip(&f.keys).map(|(a, &k)| k == 0 && a.is_some()).collect();
            // 过短的行（`}`、空行）跟随同一改动块里它上面最近的长行（`}` 收的是上面那段代码），
            // 上面没有就跟随下面最近的；块里没有长行时，跟随块里最多的那个，再不行跟随整个提交的主要贡献者
            for (s, e) in hunk_ranges(f) {
                let mut cnt: HashMap<usize, (usize, Level)> = HashMap::new();
                for i in (s..e).filter(|&i| f.keys[i] != 0) {
                    let x = cnt.entry(res[i].who).or_insert((0, res[i].level));
                    x.0 += 1;
                    x.1 = x.1.max(res[i].level);
                }
                // 跟随别人的短行是推断：逐行证据说的是那一行，不是它旁边的 `}`
                let follow = cnt
                    .iter()
                    .max_by_key(|(w, (n, _))| (*n, std::cmp::Reverse(**w)))
                    .map(|(&w, &(_, lv))| Attr { who: w, level: if lv == Level::Verified { Level::Inferred } else { lv } })
                    .unwrap_or(main_attr);
                let near = |i: usize| (s..i).rev().chain(i + 1..e).find(|&j| f.keys[j] != 0);
                for i in (s..e).filter(|&i| f.keys[i] == 0 && !placed[i]) {
                    res[i] = match near(i) {
                        Some(j) => Attr { who: res[j].who, level: if res[j].level == Level::Verified { Level::Inferred } else { res[j].level } },
                        None => follow,
                    };
                }
            }
            out.push(res);
        }
        // 配额：Creditia-Lines 记着提交时各工具写了几行。现在少算了的（日志后来被清理了）从不是 agent 的行里按顺序补足；
        // 多出来的不动——那是提交以后才读到的证据
        let mut reason = reason;
        let mut main = main;
        if let Some((quota, stamped)) = &c.lines_stamp {
            // 网页 squash 合并把原分支各提交的 Creditia-Lines 加在一起：后面的提交改写过前面的行时，
            // 总数比这个提交实际新增的多，按比例缩
            let actual: usize = out.iter().map(Vec::len).sum();
            let scale = |n: usize| if *stamped > actual { n * actual / stamped } else { n };
            let mut have: HashMap<String, usize> = HashMap::new();
            for a in out.iter().flatten() {
                if let Some(h) = self.reg.list[a.who].harness.clone().filter(|_| self.reg.list[a.who].kind == "agent") {
                    *have.entry(h).or_default() += 1;
                }
            }
            let mut filled = false;
            for (id, n) in quota {
                let (h, m) = match id.split_once('/') {
                    Some((h, m)) => (canonical_harness(h), Some(m)),
                    None => (canonical_harness(id), None),
                };
                let got = have.entry(h.clone()).or_default();
                let n = scale(*n);
                let mut deficit = n.saturating_sub(*got);
                *got = got.saturating_sub(n);
                if deficit == 0 {
                    continue;
                }
                let who = self.reg.agent(&h, m);
                for a in out.iter_mut().flatten() {
                    if deficit == 0 {
                        break;
                    }
                    if self.reg.list[a.who].kind != "agent" {
                        *a = Attr { who, level: Level::Declared };
                        deficit -= 1;
                        filled = true;
                    }
                }
            }
            if filled {
                reason = crate::tr!("Creditia-Lines 盖章", "Creditia-Lines stamp");
                let mut counts: HashMap<usize, usize> = HashMap::new();
                for (fi, f) in c.files.iter().enumerate() {
                    for (i, a) in out[fi].iter().enumerate() {
                        if f.keys[i] != 0 {
                            *counts.entry(a.who).or_default() += 1;
                        }
                    }
                }
                main = counts.iter().max_by_key(|(w, n)| (**n, std::cmp::Reverse(**w))).map(|(w, _)| *w);
            }
        }
        CommitAttr { lines: out, hits, conflicts, main, tab_lines, reason }
    }

    /// 取一条还没被认领、时间在窗口里的证据：时间最接近提交的那条。返回它和「候选里是否有不止一个工具」
    fn take(&mut self, rel: &str, key: u32, lo: i64, hi: i64, at: i64) -> Option<(crate::model::Ev, bool)> {
        let id = (rel.to_string(), key);
        let all = self.idx.map.get(&id)?;
        let used = self.used.entry(id).or_insert_with(|| vec![false; all.len()]);
        let cands: Vec<usize> = (0..all.len()).filter(|&i| all[i].ts >= lo && all[i].ts <= hi && !used[i]).collect();
        let bi = *cands.iter().min_by_key(|&&i| (at - all[i].ts).abs())?;
        let multi = cands.iter().map(|&i| all[i].src).collect::<HashSet<Src>>().len() > 1;
        // Cursor 的行哈希表一行只存一次，不按次数消耗
        if all[bi].src != Src::Cursor {
            used[bi] = true;
        }
        Some((all[bi].clone(), multi))
    }

    /// 夹在两行逐行证据之间、自己没有证据的行，而那两行在同一个工具的某次写入里本来是紧挨着的：
    /// 这几行是后来插进去的，不是那次写入写的，不能被改动块推断带走。返回每一行是否是这样插进去的
    fn inserted(&self, f: &crate::model::FileAdd, ev: &[Option<(&'static str, String)>]) -> Vec<bool> {
        let n = f.keys.len();
        let mut out = vec![false; n];
        for (s, e) in hunk_ranges(f) {
            let long: Vec<usize> = (s..e).filter(|&i| f.keys[i] != 0).collect();
            let mut i = 0;
            while i < long.len() {
                if ev[long[i]].is_some() {
                    i += 1;
                    continue;
                }
                let j = (i..long.len()).find(|&j| ev[long[j]].is_some()).unwrap_or(long.len());
                if i > 0 && j < long.len() {
                    let (a, b) = (long[i - 1], long[j]);
                    let h = ev[a].as_ref().map(|x| x.0);
                    let adjacent = self.idx.map.get(&(f.rel.clone(), f.keys[a])).is_some_and(|all| all.iter().any(|x| x.next == f.keys[b] && Some(x.src.harness()) == h));
                    if adjacent {
                        for &k in &long[i..j] {
                            out[k] = true;
                        }
                    }
                }
                i = j;
            }
        }
        out
    }

    /// 提交本身写明的来源。按优先级：盖章 trailer → agent 提交邮箱 → Co-authored-by → Assisted-by → bot → aider 的作者名 → 别人的提交（算那个人）。
    /// `counted`：提交带 Creditia-Lines（提交时逐行算过），盖章、Co-authored-by、Assisted-by 就不再代表整个提交
    fn declared(&mut self, c: &Commit, counted: bool) -> Option<(Attr, String)> {
        if let Some((h, m)) = self.stamps.get(&c.hash).cloned().filter(|_| !counted) {
            return Some((Attr { who: self.reg.agent(&h, m.as_deref()), level: Level::Declared }, crate::tr!("Creditia 盖章", "Creditia stamp")));
        }
        if let Some(h) = self.cfg.agent_emails.get(&c.email).cloned() {
            return Some((Attr { who: self.reg.agent(&h, None), level: Level::Declared }, crate::tr!("agent 提交邮箱 {}", "agent commit email {}", c.email)));
        }
        for co in c.coauthors.iter().filter(|_| !counted) {
            if let Some((_, h)) = self.cfg.agent_emails.iter().find(|(e, _)| co.contains(e.as_str())) {
                let h = h.clone();
                let m = coauthor_model(co);
                return Some((Attr { who: self.reg.agent(&h, m.as_deref()), level: Level::Declared }, format!("Co-authored-by {co}")));
            }
        }
        for a in c.assisted.iter().filter(|_| !counted) {
            if let Some((h, m)) = assisted_by(a) {
                return Some((Attr { who: self.reg.agent(&h, m.as_deref()), level: Level::Declared }, format!("Assisted-by {a}")));
            }
        }
        if self.cfg.bot_emails.contains(&c.email) || c.email.contains("[bot]") {
            let h = if c.email.contains("dependabot") { "dependabot" } else if c.email.contains("renovate") { "renovate" } else { "github-actions" };
            return Some((Attr { who: self.reg.agent(h, None), level: Level::Declared }, crate::tr!("bot 账号", "bot account")));
        }
        if c.name.trim_end().ends_with("(aider)") {
            return Some((Attr { who: self.reg.agent("aider", None), level: Level::Declared }, crate::tr!("aider 署名的作者", "author marked (aider)")));
        }
        // 在 GitHub 网页上合并的提交，作者只是开 PR、点合并的人（还是那个账号的邮箱，不是写代码时用的），不算他写的
        if !c.web && !self.is_director(&c.email) {
            // 以邮箱为身份，显示 git 作者名
            let name = if c.name.is_empty() { c.email.clone() } else { c.name.clone() };
            let name = self.cfg.display_name(&name);
            // id 用邮箱的短哈希：档案可能被公开，不能带出邮箱
            return Some((Attr { who: self.reg.human(&person_id(&c.email), &name), level: Level::Declared }, crate::tr!("其他协作者提交", "someone else's commit")));
        }
        None
    }

    /// 既没有逐行证据、也没有声明的行：同时间窗口的会话（loose，推断）→ 分支前缀（standard，推断）→ 未确认
    fn guess(&mut self, c: &Commit) -> (Attr, String) {
        if self.strictness == Strictness::Loose {
            if let Some(s) = self.session_at(c, None) {
                let (h, m) = (s.src.harness(), s.model.clone());
                return (Attr { who: self.reg.agent(h, Some(&m)), level: Level::Inferred }, crate::tr!("提交时 {h} 会话正在进行（推断）", "a {h} session was running at commit time (inferred)"));
            }
        }
        if self.strictness != Strictness::Strict {
            if let Some(b) = self.unit_branch.get(&c.hash) {
                if let Some((_, h)) = self.cfg.branch_prefixes.iter().find(|(p, _)| b.starts_with(p.as_str())) {
                    let h = h.clone();
                    return (Attr { who: self.reg.agent(&h, None), level: Level::Inferred }, crate::tr!("分支前缀 {b}（推断）", "branch prefix {b} (inferred)"));
                }
            }
        }
        (Attr { who: self.reg.unattributed(), level: Level::Unattributed }, crate::tr!("没有证据", "no evidence"))
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

/// GitHub noreply 邮箱里的账号名：`123+login@users.noreply.github.com` 和 `login@users.noreply.github.com` 是同一个账号
fn gh_login(email: &str) -> Option<&str> {
    let local = email.strip_suffix("@users.noreply.github.com")?;
    Some(local.rsplit_once('+').map_or(local, |(_, l)| l)).filter(|l| !l.is_empty())
}

/// Co-authored-by 的名字里写了模型（Claude Code 会写 `Claude Opus 5.5 <noreply@anthropic.com>`）就取出来；
/// 只有工具名（`Claude`、`Copilot`）时没有模型
fn coauthor_model(co: &str) -> Option<String> {
    let name = co.split('<').next().unwrap_or("").trim();
    let words: Vec<&str> = name.split_whitespace().collect();
    (words.len() >= 2 && name.chars().any(|c| c.is_ascii_digit())).then(|| words.join("-").to_lowercase())
}

/// `Assisted-by: 工具:模型 [别的工具…]`（Linux 内核的 AI 署名格式，Crush 默认写这个）→ (工具, 模型)。
/// 写人名和邮箱的老用法（`Assisted-by: 某人 <a@b>`）不算
fn assisted_by(v: &str) -> Option<(String, Option<String>)> {
    let (name, rest) = match v.split_once(':') {
        Some((n, r)) if !n.contains('<') => (n.trim(), r.trim()),
        _ => (v.trim(), ""),
    };
    let key = name.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-");
    let h = match key.as_str() {
        "claude" | "claude-code" | "anthropic-claude" => "claude-code",
        "codex" | "openai-codex" => "codex",
        "copilot" | "github-copilot" => "copilot",
        "gemini" | "gemini-cli" => "gemini",
        "qwen" | "qwen-code" => "qwen",
        "roo" | "roo-code" => "roo",
        "kilo" | "kilo-code" => "kilo",
        "agy" | "antigravity" | "antigravity-cli" | "google-antigravity" => "antigravity",
        k => k,
    };
    // 没写模型、又不是认识的工具名：多半是人
    let known = Src::all().iter().any(|s| s.harness() == h) || ["devin", "jules", "windsurf"].contains(&h);
    if h.is_empty() || v.contains('@') || (rest.is_empty() && !known) {
        return None;
    }
    let model = rest.split_whitespace().next().map(|m| m.to_string());
    Some((h.to_string(), model))
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

/// 存活代码归因：blame 行 → 引入它的提交里同一行（按原行号；旧缓存没有行号时按同路径同哈希），
/// 都对不上时用那个提交的主要贡献者。直接在提交里查，不另建逐行的大表（大仓库上省内存）
pub fn survival(blame: &[(String, BlameFile)], commits: &HashMap<&str, &Commit>, attrs: &HashMap<String, CommitAttr>, unattr: usize) -> Vec<Attr> {
    let mut out = vec![];
    let none = Attr { who: unattr, level: Level::Unattributed };
    for (_, lines) in blame {
        // 同一个文件的 blame 行大多来自少数几个提交：记住上一次找到的文件
        let mut last: Option<(&str, usize)> = None;
        for b in lines.iter() {
            let (Some(c), Some(a)) = (commits.get(b.commit), attrs.get(b.commit)) else {
                out.push(none);
                continue;
            };
            let fi = match last {
                Some((h, fi)) if h == b.commit && c.files.get(fi).is_some_and(|f| f.rel == b.rel) => Some(fi),
                _ => c.files.iter().position(|f| f.rel == b.rel),
            };
            if let Some(fi) = fi {
                last = Some((c.hash.as_str(), fi));
            }
            let line = fi.and_then(|fi| {
                let f = &c.files[fi];
                let i = if b.orig > 0 && !f.at.is_empty() { f.at.binary_search(&b.orig).ok() } else { None };
                let i = i.or_else(|| (b.key != 0).then(|| f.keys.iter().position(|&k| k == b.key)).flatten());
                i.and_then(|i| a.lines.get(fi)?.get(i).copied())
            });
            out.push(line.or(a.main.map(|w| Attr { who: w, level: Level::Declared })).unwrap_or(none));
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
        Commit { hash: "a".repeat(40), email: email.into(), author_ts: 1000, commit_ts: 1000, subject: "s".into(), files: vec![FileAdd { rel: "a.rs".into(), keys, ..Default::default() }], ..Default::default() }
    }

    #[test]
    fn verified_wins_and_short_lines_follow_main() {
        let mut store = Store::default();
        let g = store.groups.entry("x".into()).or_default();
        g.evs.push(Ev { rel: "a.rs".into(), key: 7, src: Src::Codex, model: "m".into(), session: "s".into(), ts: 900, tab: false, next: 0 });
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
        store.groups.entry("x".into()).or_default().evs.push(Ev { rel: "a.rs".into(), key: 7, src: Src::Codex, model: "m".into(), session: "s".into(), ts: 900, tab: false, next: 0 });
        let idx = Index::build(&store);
        let mut c = cfg();
        c.level = "strict".into();
        let mut e = Engine::new(&c, &idx, &[], &[], HashMap::new());
        let a = e.attribute(&commit("me@x.com", vec![7, 9]));
        assert_eq!(a.lines[0][0].level, Level::Verified);
        assert_eq!(a.lines[0][1].level, Level::Unattributed);
    }

    fn ev(key: u32, src: Src, model: &str) -> Ev {
        Ev { rel: "a.rs".into(), key, src, model: model.into(), session: "s".into(), ts: 900, tab: false, next: 0 }
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
    fn assisted_by_trailers() {
        assert_eq!(assisted_by("Crush:claude-sonnet-4-5"), Some(("crush".into(), Some("claude-sonnet-4-5".into()))));
        assert_eq!(assisted_by("Claude:claude-3-opus coccinelle sparse"), Some(("claude-code".into(), Some("claude-3-opus".into()))));
        assert_eq!(assisted_by("GitHub Copilot"), Some(("copilot".into(), None)));
        // 人名、邮箱不算
        assert_eq!(assisted_by("Jane Doe <jane@example.com>"), None);
        assert_eq!(assisted_by("Jane Doe"), None);
        let store = Store::default();
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let mut c = commit("me@x.com", vec![1]);
        c.assisted = vec!["Crush:gpt-5".into()];
        let a = e.attribute(&c);
        assert_eq!(e.reg.list[a.lines[0][0].who].id, "a:crush/gpt-5");
    }

    #[test]
    fn lines_inserted_between_adjacent_evidence_are_not_inferred() {
        // Codex 一次写了 1、2、3（1 后面紧跟 2）：提交里 1 和 2 之间多了 8、9，是别人插进去的；
        // 3 后面没有证据的 4 是 Codex 那次写入之外的，但不在两行紧挨的证据之间，照常推断
        let mut store = Store::default();
        let mk = |key, next| Ev { next, ..ev(key, Src::Codex, "m") };
        store.groups.entry("x".into()).or_default().evs.extend([mk(1, 2), mk(2, 3), mk(3, 0), mk(5, 0)]);
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let a = e.attribute(&commit("me@x.com", vec![1, 8, 9, 2, 3, 4, 5]));
        let l = &a.lines[0];
        assert_eq!(l[1].level, Level::Unattributed);
        assert_eq!(l[2].level, Level::Unattributed);
        assert_eq!(l[5].level, Level::Inferred);
    }

    #[test]
    fn short_lines_follow_their_own_write() {
        // `}` 跟在 Claude 的第 2 行后面写下：提交里它上面紧挨的是 Codex 的行，也照样算 Claude
        let mut store = Store::default();
        let g = store.groups.entry("x".into()).or_default();
        let sh = crate::hash::short_hash("}");
        g.evs.extend([ev(1, Src::Codex, "m"), ev(2, Src::Claude, "c"), ev(3, Src::Codex, "m")]);
        g.evs.push(ev(crate::hash::short_key("a.rs", 2, 'v', 1, sh), Src::Claude, "c"));
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let mut c = commit("me@x.com", vec![2, 0, 1, 0, 3]);
        c.files[0].short = vec![0, sh, 0, sh, 0];
        let a = e.attribute(&c);
        let l = &a.lines[0];
        assert_eq!(e.reg.list[l[1].who].id, "a:claude-code/c");
        // 没有自己证据的短行跟随它上面最近的长行
        assert_eq!(e.reg.list[l[3].who].id, "a:codex/m");
    }

    #[test]
    fn reflowed_lines_join_back() {
        // agent 写了一长行，格式化工具拆成两行（逗号、分号、空白不参与比较）
        use crate::hash::{line_key, normalize};
        let n = |l: &str| normalize(l, ".rs");
        let mut store = Store::default();
        store.groups.entry("x".into()).or_default().evs.push(ev(line_key("a.rs", &n("let total = compute(alpha, beta);")), Src::Codex, "m"));
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let (a, b) = (n("    let total = compute(alpha,"), n("        beta);"));
        let mut c = commit("me@x.com", vec![line_key("a.rs", &a), line_key("a.rs", &b)]);
        let mut joins = vec![0u32; 2 * JOIN_WINDOWS];
        joins[0] = line_key("a.rs", &(a.clone() + &b));
        c.files[0].joins = joins;
        let r = e.attribute(&c);
        assert!(r.lines[0].iter().all(|x| x.level == Level::Verified), "{:?}", r.lines[0]);
        assert_eq!(e.reg.list[r.lines[0][1].who].id, "a:codex/m");
    }

    #[test]
    fn lines_stamp_is_a_quota_not_the_whole_commit() {
        // 在 Kimi 里提交：hook 记下 Kimi 写了 2 行、共 3 行。日志后来没了：2 行按章补给 Kimi，另 1 行未确认（不推断是谁手写）
        let store = Store::default();
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut stamps = HashMap::new();
        stamps.insert("a".repeat(40), ("kimi".to_string(), Some("k3".to_string())));
        let mut e = Engine::new(&cfg, &idx, &[], &[], stamps);
        let mut c = commit("me@x.com", vec![1, 2, 3]);
        c.lines_stamp = Some((vec![("kimi/k3".into(), 2)], 3));
        let r = e.attribute(&c);
        let ids: Vec<&str> = r.lines[0].iter().map(|x| e.reg.list[x.who].id.as_str()).collect();
        assert_eq!(ids.iter().filter(|i| **i == "a:kimi/k3").count(), 2, "{ids:?}");
        assert_eq!(ids.iter().filter(|i| **i == "unattributed").count(), 1, "{ids:?}");
        // 章写着 0 行：整个提交都不算给 Kimi
        let mut c = commit("me@x.com", vec![4, 5]);
        c.lines_stamp = Some((vec![], 2));
        c.coauthors = vec!["claude opus 5.5 <noreply@anthropic.com>".into()];
        let r = e.attribute(&c);
        assert!(r.lines[0].iter().all(|x| x.level == Level::Unattributed), "{:?}", r.lines[0]);
    }

    #[test]
    fn web_merge_is_not_the_mergers_code() {
        // GitHub 网页上合并的提交：作者是别的邮箱也不算那个人写的；找不到记录就是未确认
        let store = Store::default();
        let idx = Index::build(&store);
        let cfg = cfg();
        let mut e = Engine::new(&cfg, &idx, &[], &[], HashMap::new());
        let mut c = commit("someone@gmail.com", vec![1, 2, 3, 4]);
        c.web = true;
        let r = e.attribute(&c);
        assert!(r.lines[0].iter().all(|x| x.level == Level::Unattributed), "{:?}", r.lines[0]);
        // 正文里各提交的 Creditia-Lines 加起来是 6 / 8，提交实际只新增 4 行：按比例 3 行给 Codex
        c.hash = "b".repeat(40);
        c.lines_stamp = Some((vec![("codex/gpt-a".into(), 6)], 8));
        let r = e.attribute(&c);
        let ids: Vec<&str> = r.lines[0].iter().map(|x| e.reg.list[x.who].id.as_str()).collect();
        assert_eq!(ids.iter().filter(|i| **i == "a:codex/gpt-a").count(), 3, "{ids:?}");
        assert_eq!(ids.iter().filter(|i| **i == "unattributed").count(), 1, "{ids:?}");
        // 不是网页合并的、别人的提交：照旧算那个人
        let r = e.attribute(&commit("someone@gmail.com", vec![5]));
        assert_eq!(e.reg.list[r.lines[0][0].who].kind, "human");
    }

    #[test]
    fn same_github_account_is_the_same_person() {
        let store = Store::default();
        let idx = Index::build(&store);
        let mut c = Config::default();
        c.directors.push(crate::config::Director { id: "h:me".into(), name: "me".into(), emails: vec!["123+me@users.noreply.github.com".into()] });
        let mut e = Engine::new(&c, &idx, &[], &[], HashMap::new());
        let r = e.attribute(&commit("me@users.noreply.github.com", vec![1]));
        assert_eq!(r.lines[0][0].level, Level::Unattributed);
        let r = e.attribute(&commit("456+you@users.noreply.github.com", vec![1]));
        assert_eq!(e.reg.list[r.lines[0][0].who].kind, "human");
        assert_eq!(gh_login("me@x.com"), None);
    }

    #[test]
    fn stamp_parse() {
        assert_eq!(parse_stamp("Creditia-Harness: Codex\nCreditia-Model: gpt-6\n"), Some(("codex".into(), Some("gpt-6".into()))));
        assert_eq!(parse_stamp("Signed-off-by: x"), None);
    }
}
