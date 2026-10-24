//! 作品库：本机所有项目、证据库、快照。SQLite，位置按平台惯例，`CREDITIA_HOME` 可覆盖。
//! 证据库只存「项目、相对路径、行哈希、工具、模型、会话、时间、是否 tab」，不存代码和 prompt。

use creditia_core::model::{Ev, Session, Src, Store};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

pub fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("CREDITIA_HOME") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = PathBuf::from(std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default());
    #[cfg(target_os = "macos")]
    { home.join("Library/Application Support/creditia") }
    #[cfg(target_os = "windows")]
    { PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("creditia") }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        std::env::var("XDG_DATA_HOME").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".local/share")).join("creditia")
    }
}

/// 每个项目的分析缓存目录（提交、证据分组、blame）
pub fn cache_dir(project: &str) -> PathBuf {
    data_dir().join("cache").join(project)
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub root_commit: String,
    pub remotes: Vec<String>,
    pub last_head: Option<String>,
    pub last_analyzed: Option<i64>,
}

pub struct Library {
    conn: Connection,
}

impl Library {
    pub fn open() -> Result<Library, String> {
        Self::open_in(data_dir())
    }
    fn open_in(dir: PathBuf) -> Result<Library, String> {
        std::fs::create_dir_all(&dir).map_err(|e| creditia_core::tr!("创建项目库目录失败 {}: {e}", "couldn't create the library folder {}: {e}", dir.display()))?;
        let conn = Connection::open(dir.join("library.sqlite")).map_err(|e| e.to_string())?;
        // App 和命令行可能同时打开作品库：被锁住时等一会儿，而不是直接报 database is locked
        conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e| e.to_string())?;
        // 切到 WAL 要独占整个库，busy_timeout 管不到这一步：两个进程（或并行的测试）同时第一次打开新库，
        // 其中一个会直接报 database is locked。被锁就稍等重试；已经是 WAL 的库这一步不需要锁
        let mut tries = 0;
        loop {
            match conn.query_row("pragma journal_mode=wal", [], |r| r.get::<_, String>(0)) {
                Ok(_) => break,
                Err(e) if tries < 50 && e.to_string().contains("locked") => {
                    tries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        conn.execute_batch(
            "create table if not exists projects(
               id text primary key, name text not null, path text not null, root_commit text not null,
               remotes text not null default '[]', last_head text, last_analyzed integer);
             create table if not exists evidence(
               project text not null, rel text not null, key integer not null, src text not null, model text not null,
               session text not null, ts integer not null, tab integer not null default 0,
               primary key(project, rel, key, src, session));
             create index if not exists idx_ev_project on evidence(project);
             create table if not exists sessions(
               project text not null, src text not null, id text not null, model text not null, start integer not null, end integer not null,
               tokens integer not null, branch text, prompts integer not null, reviewer integer not null, has_content integer not null,
               primary key(project, src, id));
             create table if not exists paths(
               project text not null, path text not null, kind text not null, last_seen integer not null,
               primary key(project, path));
             create table if not exists snapshots(
               project text not null, head text not null, generated_at text not null, json text not null,
               primary key(project, head));",
        )
        .map_err(|e| e.to_string())?;
        // 旧库补列：会话的渠道
        let _ = conn.execute("alter table sessions add column provider text", []);
        // 证据的下一行
        let _ = conn.execute("alter table evidence add column next integer not null default 0", []);
        // evidence2：每一次写入的每一行都单独记（主键带时间和序号）。旧表按会话去重，同一会话里写了几次的同一行只剩一条，
        // 日志被清理、缓存重建以后，这些行就从「逐行证据」掉成了推断或未确认
        conn.execute_batch(
            "create table if not exists evidence2(
               project text not null, rel text not null, key integer not null, src text not null, model text not null,
               session text not null, ts integer not null, tab integer not null default 0, next integer not null default 0,
               seq integer not null default 0,
               primary key(project, rel, key, src, session, ts, seq));
             create index if not exists idx_ev2_project on evidence2(project);
             create table if not exists coverage(
               project text not null, src text not null, from_ts integer not null, to_ts integer not null,
               primary key(project, src, from_ts));",
        )
        .map_err(|e| e.to_string())?;
        let ver: i64 = conn.query_row("pragma user_version", [], |r| r.get(0)).unwrap_or(0);
        if ver < 1 {
            conn.execute_batch(
                "begin;
                 insert or ignore into evidence2(project,rel,key,src,model,session,ts,tab,next,seq)
                   select project,rel,key,src,model,session,ts,tab,next,0 from evidence;
                 delete from evidence;
                 pragma user_version = 1;
                 commit;",
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(Library { conn })
    }

    pub fn upsert_project(&self, p: &Project) -> Result<(), String> {
        self.conn
            .execute(
                "insert into projects(id,name,path,root_commit,remotes,last_head,last_analyzed) values(?1,?2,?3,?4,?5,?6,?7)
                 on conflict(id) do update set name=excluded.name, path=excluded.path, remotes=excluded.remotes,
                 last_head=coalesce(excluded.last_head, projects.last_head), last_analyzed=coalesce(excluded.last_analyzed, projects.last_analyzed)",
                params![p.id, p.name, p.path, p.root_commit, serde_json::to_string(&p.remotes).unwrap(), p.last_head, p.last_analyzed],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn projects(&self) -> Result<Vec<Project>, String> {
        let mut st = self.conn.prepare("select id,name,path,root_commit,remotes,last_head,last_analyzed from projects order by last_analyzed desc").map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| {
                Ok(Project {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    path: r.get(2)?,
                    root_commit: r.get(3)?,
                    remotes: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
                    last_head: r.get(5)?,
                    last_analyzed: r.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?
            .flatten()
            .collect();
        Ok(rows)
    }

    /// 把内存证据写进证据库（幂等）。同一分组里时间、内容都相同的几条（一次写入里重复的行）按出现的序号分开记
    pub fn save_evidence(&mut self, project: &str, store: &Store) -> Result<usize, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        let mut n = 0;
        {
            let mut ins = tx
                .prepare("insert or ignore into evidence2(project,rel,key,src,model,session,ts,tab,next,seq) values(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)")
                .map_err(|e| e.to_string())?;
            let mut ses = tx
                .prepare("insert or replace into sessions(project,src,id,model,start,end,tokens,branch,prompts,reviewer,has_content,provider) values(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)")
                .map_err(|e| e.to_string())?;
            for g in store.groups.values() {
                let mut seq: std::collections::HashMap<(&str, u32, &str, i64), i64> = std::collections::HashMap::new();
                for e in &g.evs {
                    let k = seq.entry((e.rel.as_str(), e.key, e.session.as_str(), e.ts)).or_insert(0);
                    n += ins.execute(params![project, e.rel, e.key as i64, e.src.harness(), e.model, e.session, e.ts, e.tab as i64, e.next as i64, *k]).map_err(|x| x.to_string())?;
                    *k += 1;
                }
                for s in &g.sessions {
                    ses.execute(params![project, s.src.harness(), s.id, s.model, s.start, s.end, s.tokens, s.branch, s.prompts, s.reviewer as i64, s.has_content as i64, s.provider]).map_err(|x| x.to_string())?;
                }
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(n)
    }

    /// 从证据库读回（日志已被清理时仍能归因）
    pub fn load_evidence(&self, project: &str) -> Result<(Vec<Ev>, Vec<Session>), String> {
        let src_of = Src::from_harness;
        let mut st = self.conn.prepare("select rel,key,src,model,session,ts,tab,next from evidence2 where project=?1").map_err(|e| e.to_string())?;
        let evs = st
            .query_map([project], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, i64>(5)?, (r.get::<_, i64>(6)?, r.get::<_, i64>(7)?)))
            })
            .map_err(|e| e.to_string())?
            .flatten()
            .filter_map(|(rel, key, src, model, session, ts, (tab, next))| src_of(&src).map(|src| Ev { rel, key: key as u32, src, model, session, ts, tab: tab != 0, next: next as u32 }))
            .collect();
        let mut st2 = self.conn.prepare("select src,id,model,start,end,tokens,branch,prompts,reviewer,has_content,provider from sessions where project=?1").map_err(|e| e.to_string())?;
        let sessions = st2
            .query_map([project], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, r.get::<_, Option<String>>(6)?, r.get::<_, i64>(7)?, r.get::<_, i64>(8)?, (r.get::<_, i64>(9)?, r.get::<_, Option<String>>(10)?)))
            })
            .map_err(|e| e.to_string())?
            .flatten()
            .filter_map(|(src, id, model, start, end, tokens, branch, prompts, reviewer, (has_content, provider))| {
                src_of(&src).map(|src| Session { src, id, model, start, end, tokens, branch, prompts, reviewer: reviewer != 0, has_content: has_content != 0, provider })
            })
            .collect();
        Ok((evs, sessions))
    }

    pub fn save_snapshot(&self, project: &str, head: &str, generated_at: &str, json: &str) -> Result<(), String> {
        self.conn
            .execute("insert or replace into snapshots(project,head,generated_at,json) values(?1,?2,?3,?4)", params![project, head, generated_at, json])
            .map_err(|e| e.to_string())?;
        self.conn
            .execute("update projects set last_head=?2, last_analyzed=strftime('%s','now') where id=?1", params![project, head])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn snapshots(&self, project: &str) -> Result<Vec<(String, String)>, String> {
        let mut st = self.conn.prepare("select head, generated_at from snapshots where project=?1 order by generated_at").map_err(|e| e.to_string())?;
        let v = st.query_map([project], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?.flatten().collect();
        Ok(v)
    }

    /// 最新一份快照的 JSON（App 浏览档案用）
    pub fn snapshot_json(&self, project: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row("select json from snapshots where project=?1 order by generated_at desc limit 1", [project], |r| r.get::<_, String>(0))
            .map(Some)
            .or_else(|e| if matches!(e, rusqlite::Error::QueryReturnedNoRows) { Ok(None) } else { Err(e.to_string()) })
    }

    /// 改仓库配置里的展示选项（App 设置页用）：是否写出人名、落款。只动 display 段，其他原样保留。
    pub fn set_display(repo: &Path, named: bool, sign: &str) -> Result<(), String> {
        let p = repo.join(".creditia/config.json");
        let mut v: serde_json::Value = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({}));
        v["display"] = serde_json::json!({ "named": named, "sign": sign });
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n").map_err(|e| e.to_string())
    }

    /// 登记项目出现过的路径：主仓库、worktree、临时克隆、搬过家的旧路径、手动关联的路径。
    /// 日志里的路径靠这份清单认出属于哪个项目（目录不在了也能认）
    pub fn add_path(&self, project: &str, path: &str, kind: &str) -> Result<(), String> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        self.conn
            .execute("insert into paths(project,path,kind,last_seen) values(?1,?2,?3,?4) on conflict(project,path) do update set last_seen=excluded.last_seen, kind=case when paths.kind='manual' then 'manual' else excluded.kind end", params![project, path, kind, now])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn remove_path(&self, project: &str, path: &str) -> Result<usize, String> {
        self.conn.execute("delete from paths where project=?1 and path=?2", params![project, path]).map_err(|e| e.to_string())
    }
    /// (路径, 类型)
    pub fn paths(&self, project: &str) -> Result<Vec<(String, String)>, String> {
        let mut st = self.conn.prepare("select path, kind from paths where project=?1 order by last_seen desc").map_err(|e| e.to_string())?;
        let v = st.query_map([project], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?.flatten().collect();
        Ok(v)
    }

    /// 记下一段「本机日志完整」的时段，和已有的合并成不重叠的几段
    pub fn add_coverage(&mut self, project: &str, from: i64, to: i64) -> Result<(), String> {
        let mut all = self.coverage(project);
        all.push((from.min(to), to.max(from)));
        all.sort();
        let mut merged: Vec<(i64, i64)> = vec![];
        for (a, b) in all {
            match merged.last_mut() {
                Some(last) if a <= last.1 => last.1 = last.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("delete from coverage where project=?1", [project]).map_err(|e| e.to_string())?;
        for (a, b) in merged {
            tx.execute("insert into coverage(project,src,from_ts,to_ts) values(?1,'all',?2,?3)", params![project, a, b]).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    /// 本机日志完整的时段（按时间排好、不重叠）
    pub fn coverage(&self, project: &str) -> Vec<(i64, i64)> {
        let Ok(mut st) = self.conn.prepare("select from_ts, to_ts from coverage where project=?1 order by from_ts") else { return vec![] };
        st.query_map([project], |r| Ok((r.get(0)?, r.get(1)?))).map(|rows| rows.flatten().collect()).unwrap_or_default()
    }

    pub fn evidence_count(&self, project: &str) -> u64 {
        self.conn.query_row("select count(*) from evidence2 where project=?1", [project], |r| r.get::<_, i64>(0)).unwrap_or(0) as u64
    }

    pub fn path(&self) -> PathBuf {
        data_dir().join("library.sqlite")
    }
}

/// JSON 缓存读写（提交、分组、blame），放在 cache_dir 下
pub fn load_json<T: serde::de::DeserializeOwned + Default>(p: &Path) -> T {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
pub fn save_json<T: serde::Serialize>(p: &Path, v: &T) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    // 先写临时文件再改名：中途被取消也不会留下写了一半的缓存；边序列化边写，不在内存里攒整份
    let tmp = p.with_extension(format!("tmp{}", std::process::id()));
    let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut w = std::io::BufWriter::new(f);
    serde_json::to_writer(&mut w, v).map_err(|e| e.to_string())?;
    std::io::Write::flush(&mut w).map_err(|e| e.to_string())?;
    drop(w);
    std::fs::rename(&tmp, p).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use creditia_core::model::FileGroup;
    use std::sync::OnceLock;

    /// 整个测试进程共用一个临时作品库（CREDITIA_HOME 只设一次，避免并行测试互相改环境变量）；各测试用不同的项目 id
    fn lib() -> Library {
        static HOME: OnceLock<PathBuf> = OnceLock::new();
        HOME.get_or_init(|| {
            let p = std::env::temp_dir().join(format!("creditia-store-test-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::env::set_var("CREDITIA_HOME", &p);
            p
        });
        Library::open().unwrap()
    }

    fn ev(rel: &str, key: u32, session: &str) -> Ev {
        Ev { rel: rel.into(), key, src: Src::Codex, model: "gpt-test".into(), session: session.into(), ts: 100, tab: false, next: 0 }
    }

    fn session(id: &str, provider: Option<&str>) -> Session {
        Session { src: Src::Claude, id: id.into(), model: "claude-test".into(), start: 10, end: 20, tokens: 30, branch: Some("main".into()), prompts: 2, reviewer: false, has_content: true, provider: provider.map(String::from) }
    }

    #[test]
    fn evidence_round_trip_is_idempotent() {
        let mut l = lib();
        let mut store = Store::default();
        store.groups.insert("g1".into(), FileGroup { fp: (1, 1), evs: vec![ev("src/a.rs", 1, "s1"), ev("src/a.rs", 2, "s1")], sessions: vec![session("c1", Some("bedrock"))] });
        assert_eq!(l.save_evidence("p-ev", &store).unwrap(), 2);
        // 同样的证据再存一次不重复
        assert_eq!(l.save_evidence("p-ev", &store).unwrap(), 0);
        assert_eq!(l.evidence_count("p-ev"), 2);
        let (evs, ses) = l.load_evidence("p-ev").unwrap();
        assert_eq!(evs.len(), 2);
        assert!(evs.iter().all(|e| e.src == Src::Codex && e.model == "gpt-test" && e.rel == "src/a.rs"));
        assert_eq!(ses.len(), 1);
        assert_eq!(ses[0].provider.as_deref(), Some("bedrock"));
        assert_eq!(ses[0].branch.as_deref(), Some("main"));
        // 别的项目看不到
        assert_eq!(l.evidence_count("p-other"), 0);
    }

    /// 一次写入里重复的行（同一会话、同一时间、同一内容）：每一行都要留下，删日志后重建也不少
    #[test]
    fn repeated_lines_in_one_write_are_kept() {
        let mut l = lib();
        let mut store = Store::default();
        let evs: Vec<Ev> = (0..3).map(|_| ev("tests/t.rs", 9, "s1")).chain([ev("tests/t.rs", 10, "s1")]).collect();
        store.groups.insert("g".into(), FileGroup { fp: (1, 1), evs, sessions: vec![] });
        assert_eq!(l.save_evidence("p-dup", &store).unwrap(), 4);
        assert_eq!(l.save_evidence("p-dup", &store).unwrap(), 0);
        let (evs, _) = l.load_evidence("p-dup").unwrap();
        assert_eq!(evs.iter().filter(|e| e.key == 9).count(), 3);
    }

    #[test]
    fn coverage_merges() {
        let mut l = lib();
        l.add_coverage("p-cov", 100, 200).unwrap();
        l.add_coverage("p-cov", 150, 300).unwrap();
        l.add_coverage("p-cov", 500, 600).unwrap();
        assert_eq!(l.coverage("p-cov"), vec![(100, 300), (500, 600)]);
        l.add_coverage("p-cov", 250, 550).unwrap();
        assert_eq!(l.coverage("p-cov"), vec![(100, 600)]);
    }

    /// 几个进程同时第一次打开一个新库（App 和命令行、并行的测试）：都要打开成功，不能报 database is locked
    #[test]
    fn concurrent_first_opens_succeed() {
        for round in 0..40 {
            let dir = std::env::temp_dir().join(format!("creditia-store-race-{}-{round}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let bar = std::sync::Arc::new(std::sync::Barrier::new(8));
            let hs: Vec<_> = (0..8)
                .map(|_| {
                    let (bar, dir) = (bar.clone(), dir.clone());
                    std::thread::spawn(move || {
                        bar.wait();
                        Library::open_in(dir).map(|_| ())
                    })
                })
                .collect();
            for h in hs {
                h.join().unwrap().unwrap();
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn projects_upsert_keeps_last_head() {
        let l = lib();
        let p = Project { id: "p-up".into(), name: "demo".into(), path: "/tmp/demo".into(), root_commit: "abc".into(), remotes: vec!["git@x:demo.git".into()], last_head: Some("h1".into()), last_analyzed: Some(5) };
        l.upsert_project(&p).unwrap();
        // 再登记一次但不带 head：保留原来的
        l.upsert_project(&Project { name: "demo2".into(), last_head: None, last_analyzed: None, ..p.clone() }).unwrap();
        let got = l.projects().unwrap().into_iter().find(|x| x.id == "p-up").unwrap();
        assert_eq!(got.name, "demo2");
        assert_eq!(got.last_head.as_deref(), Some("h1"));
        assert_eq!(got.remotes, vec!["git@x:demo.git".to_string()]);
    }

    #[test]
    fn snapshots_latest_wins() {
        let l = lib();
        assert_eq!(l.snapshot_json("p-snap").unwrap(), None);
        l.save_snapshot("p-snap", "h1", "2026-01-01T00:00:00Z", r#"{"n":1}"#).unwrap();
        l.save_snapshot("p-snap", "h2", "2026-02-01T00:00:00Z", r#"{"n":2}"#).unwrap();
        assert_eq!(l.snapshot_json("p-snap").unwrap().as_deref(), Some(r#"{"n":2}"#));
        assert_eq!(l.snapshots("p-snap").unwrap().iter().map(|(h, _)| h.as_str()).collect::<Vec<_>>(), ["h1", "h2"]);
    }

    #[test]
    fn manual_paths_stay_manual() {
        let l = lib();
        l.add_path("p-path", "/old/demo", "manual").unwrap();
        // 后来自动扫描又见到这个路径：类型仍是 manual
        l.add_path("p-path", "/old/demo", "worktree").unwrap();
        l.add_path("p-path", "/tmp/clone", "clone").unwrap();
        let ps = l.paths("p-path").unwrap();
        assert!(ps.contains(&("/old/demo".to_string(), "manual".to_string())), "{ps:?}");
        assert_eq!(ps.len(), 2);
        assert_eq!(l.remove_path("p-path", "/tmp/clone").unwrap(), 1);
        assert_eq!(l.remove_path("p-path", "/tmp/clone").unwrap(), 0);
        assert_eq!(l.paths("p-path").unwrap().len(), 1);
    }

    #[test]
    fn set_display_keeps_other_fields() {
        let repo = std::env::temp_dir().join(format!("creditia-store-display-{}", std::process::id()));
        std::fs::create_dir_all(repo.join(".creditia")).unwrap();
        std::fs::write(repo.join(".creditia/config.json"), r#"{"name":"demo","level":"strict"}"#).unwrap();
        Library::set_display(&repo, true, "— demo").unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(repo.join(".creditia/config.json")).unwrap()).unwrap();
        assert_eq!(v["name"], "demo");
        assert_eq!(v["level"], "strict");
        assert_eq!(v["display"]["named"], true);
        assert_eq!(v["display"]["sign"], "— demo");
        // 没有配置文件也能写
        let fresh = repo.join("fresh");
        Library::set_display(&fresh, false, "").unwrap();
        assert!(fresh.join(".creditia/config.json").exists());
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn json_cache_round_trip() {
        let p = std::env::temp_dir().join(format!("creditia-store-json-{}/a/b.json", std::process::id()));
        save_json(&p, &vec![1, 2, 3]).unwrap();
        let v: Vec<i32> = load_json(&p);
        assert_eq!(v, [1, 2, 3]);
        // 不存在或坏掉的文件：返回默认值
        let missing: Vec<i32> = load_json(&p.with_file_name("none.json"));
        assert!(missing.is_empty());
        std::fs::write(&p, "not json").unwrap();
        let bad: Vec<i32> = load_json(&p);
        assert!(bad.is_empty());
    }
}
