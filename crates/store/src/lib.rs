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
        let dir = data_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建作品库目录失败 {}: {e}", dir.display()))?;
        let conn = Connection::open(dir.join("library.sqlite")).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "pragma journal_mode=wal;
             create table if not exists projects(
               id text primary key, name text not null, path text not null, root_commit text not null,
               remotes text not null default '[]', last_head text, last_analyzed integer);
             create table if not exists project_paths(project text not null, path text not null, primary key(project,path));
             create table if not exists evidence_records(
               project text not null, rel text not null, key integer not null, src text not null, model text not null, provider text not null default '',
               session text not null, ts integer not null, tab integer not null default 0,
               primary key(project, rel, key, src, model, provider, session, ts, tab));
             create index if not exists idx_records_project on evidence_records(project);
             create table if not exists activity_sessions(
               project text not null, src text not null, id text not null, model text not null, provider text not null default '', start integer not null, end integer not null,
               tokens integer not null, branch text, prompts integer not null, reviewer integer not null, has_content integer not null,
               primary key(project, src, id));
             create table if not exists snapshots(
               project text not null, head text not null, generated_at text not null, json text not null,
               primary key(project, head));",
        )
        .map_err(|e| e.to_string())?;
        Ok(Library { conn })
    }

    pub fn upsert_project(&self, p: &Project) -> Result<(), String> {
        self.register_path(&p.id, &p.path)?;
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

    pub fn register_path(&self, project: &str, path: &str) -> Result<(), String> {
        self.conn.execute("insert or ignore into project_paths(project,path) values(?1,?2)", params![project,path]).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn paths(&self, project: &str) -> Result<Vec<String>, String> {
        let mut statement = self.conn.prepare("select path from project_paths where project=?1 order by path").map_err(|e| e.to_string())?;
        let paths = statement.query_map([project], |row| row.get(0)).map_err(|e| e.to_string())?.collect::<rusqlite::Result<Vec<String>>>().map_err(|e| e.to_string())?;
        Ok(paths)
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

    /// 把内存证据写进证据库（幂等）
    pub fn save_evidence(&mut self, project: &str, store: &Store) -> Result<usize, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        let mut n = 0;
        {
            let mut ins = tx
                .prepare("insert or ignore into evidence_records(project,rel,key,src,model,session,ts,tab,provider) values(?1,?2,?3,?4,?5,?6,?7,?8,?9)")
                .map_err(|e| e.to_string())?;
            let mut ses = tx
                .prepare("insert or replace into activity_sessions(project,src,id,model,start,end,tokens,branch,prompts,reviewer,has_content,provider) values(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)")
                .map_err(|e| e.to_string())?;
            for g in store.groups.values() {
                for e in &g.evs {
                    n += ins.execute(params![project, e.rel, e.key as i64, e.src.harness(), e.model, e.session, e.ts, e.tab as i64, e.provider.as_deref().unwrap_or("")]).map_err(|x| x.to_string())?;
                }
                for s in &g.sessions {
                    ses.execute(params![project, s.src.harness(), s.id, s.model, s.start, s.end, s.tokens, s.branch, s.prompts, s.reviewer as i64, s.has_content as i64, s.provider.as_deref().unwrap_or("")]).map_err(|x| x.to_string())?;
                }
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(n)
    }

    /// 从证据库读回（日志已被清理时仍能归因）
    pub fn load_evidence(&self, project: &str) -> Result<(Vec<Ev>, Vec<Session>), String> {
        let src_of = |s: &str| Src::all().into_iter().find(|x| x.harness() == s);
        let mut st = self.conn.prepare("select rel,key,src,model,session,ts,tab,provider from evidence_records where project=?1").map_err(|e| e.to_string())?;
        let evs = st
            .query_map([project], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, i64>(5)?, r.get::<_, i64>(6)?, r.get::<_, String>(7)?))
            })
            .map_err(|e| e.to_string())?
            .flatten()
            .filter_map(|(rel, key, src, model, session, ts, tab, provider)| src_of(&src).map(|src| Ev { provider: if provider.is_empty() { None } else { Some(provider) }, rel, key: key as u32, src, model, session, ts, tab: tab != 0 }))
            .collect();
        let mut st2 = self.conn.prepare("select src,id,model,start,end,tokens,branch,prompts,reviewer,has_content,provider from activity_sessions where project=?1").map_err(|e| e.to_string())?;
        let sessions = st2
            .query_map([project], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, r.get::<_, Option<String>>(6)?, r.get::<_, i64>(7)?, r.get::<_, i64>(8)?, r.get::<_, i64>(9)?, r.get::<_, String>(10)?))
            })
            .map_err(|e| e.to_string())?
            .flatten()
            .filter_map(|(src, id, model, start, end, tokens, branch, prompts, reviewer, has_content, provider)| {
                src_of(&src).map(|src| Session { provider: if provider.is_empty() { None } else { Some(provider) }, src, id, model, start, end, tokens, branch, prompts, reviewer: reviewer != 0, has_content: has_content != 0 })
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

    pub fn evidence_count(&self, project: &str) -> u64 {
        self.conn.query_row("select count(*) from evidence_records where project=?1", [project], |r| r.get::<_, i64>(0)).unwrap_or(0) as u64
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
    std::fs::write(p, serde_json::to_vec(v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
