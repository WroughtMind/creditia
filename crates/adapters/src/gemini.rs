//! Gemini CLI 与 Qwen Code（Gemini CLI 的分支，格式相同）：`<home>/tmp/<项目哈希>/chats/session-*.json`。
//! 目录名是项目根路径的 SHA-256，没有明文 cwd：用本项目已知的全部路径（主仓库、worktree、作品库里登记过的旧路径）
//! 逐个算哈希去对；新版本若在目录里写了 `.project_root`，直接读。只认 status 为 success 的写文件 / 替换调用。

use crate::{any_ts, new_text, path_arg, source_dir, Activity, Health, Stats};
use creditia_core::model::{fingerprint, FileGroup, PathMap, Session, Src, Store};
use creditia_core::util::date_cn;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn sha(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 一个项目目录：哈希目录 + 能确定的根路径
struct Proj {
    dir: PathBuf,
    root: Option<String>,
}

fn projects(src: Src) -> Vec<Proj> {
    let base = source_dir(src).join("tmp");
    let mut out = vec![];
    for e in std::fs::read_dir(&base).into_iter().flatten().flatten() {
        let dir = e.path();
        if !dir.join("chats").is_dir() {
            continue;
        }
        let root = std::fs::read_to_string(dir.join(".project_root")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        out.push(Proj { dir, root });
    }
    out
}

/// 这个哈希目录是不是本项目：明文根路径命中关键词，或哈希等于某个已知路径的哈希
fn belongs(p: &Proj, pm: &PathMap, hashes: &HashMap<String, String>) -> bool {
    if let Some(r) = &p.root {
        return pm.in_project(r);
    }
    let name = p.dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    hashes.contains_key(&name)
}

pub fn scan_src(src: Src, store: &mut Store, pm: &PathMap, roots: &[String], st: &mut Stats) {
    // 已知路径的各种写法都算一遍：原样、去掉末尾斜杠、macOS 的 /private 前缀
    let mut hashes = HashMap::new();
    for r in roots {
        let r = r.trim_end_matches('/');
        for v in [r.to_string(), format!("/private{r}"), r.strip_prefix("/private").unwrap_or(r).to_string()] {
            hashes.insert(sha(&v), v);
        }
    }
    let mut n = 0;
    for p in projects(src).into_iter().filter(|p| belongs(p, pm, &hashes)) {
        let root = p.root.clone().or_else(|| hashes.get(&p.dir.file_name().unwrap_or_default().to_string_lossy().to_string()).cloned()).unwrap_or_default();
        for f in std::fs::read_dir(p.dir.join("chats")).into_iter().flatten().flatten() {
            let path = f.path();
            if path.extension().map(|x| x != "json").unwrap_or(true) {
                continue;
            }
            let gk = format!("{}:{}", src.harness(), path.display());
            let fp = fingerprint(&path);
            if store.groups.get(&gk).map(|g| g.fp == fp).unwrap_or(false) {
                n += 1;
                continue;
            }
            let mut g = FileGroup { fp, ..Default::default() };
            let v: Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            parse(src, &v, &root, pm, &mut g);
            n += g.sessions.len();
            store.groups.insert(gk, g);
        }
    }
    if n > 0 {
        st.notes.push(format!("{}: 项目相关会话 {n} 个（按项目路径哈希识别）", src.harness()));
    }
}

fn parse(src: Src, v: &Value, root: &str, pm: &PathMap, g: &mut FileGroup) {
    let sid = v["sessionId"].as_str().unwrap_or("").to_string();
    let start = any_ts(&v["startTime"]).unwrap_or(0);
    let mut end = any_ts(&v["lastUpdated"]).unwrap_or(start);
    let (mut tokens, mut prompts) = (0i64, 0i64);
    let mut models: HashMap<String, usize> = HashMap::new();
    for msg in v["messages"].as_array().into_iter().flatten() {
        let ts = any_ts(&msg["timestamp"]).unwrap_or(start);
        end = end.max(ts);
        match msg["type"].as_str() {
            Some("user") => prompts += 1,
            Some("gemini") | Some("qwen") | Some("assistant") | Some("model") => {
                let model = msg["model"].as_str().unwrap_or("").to_string();
                if !model.is_empty() {
                    *models.entry(model.clone()).or_default() += 1;
                }
                tokens += msg["tokens"]["total"].as_i64().unwrap_or_else(|| ["input", "output", "cached", "thoughts", "tool"].iter().map(|k| msg["tokens"][*k].as_i64().unwrap_or(0)).sum());
                for tc in msg["toolCalls"].as_array().into_iter().flatten() {
                    if tc["status"].as_str().map(|s| s != "success").unwrap_or(false) {
                        continue;
                    }
                    let name = tc["name"].as_str().unwrap_or("");
                    let args = &tc["args"];
                    let (Some(p), Some(text)) = (path_arg(args), new_text(name, args)) else { continue };
                    let abs = if Path::new(&p).is_absolute() || root.is_empty() { p } else { format!("{}/{}", root.trim_end_matches('/'), p) };
                    let m = if model.is_empty() { models.iter().max_by_key(|(_, n)| **n).map(|(m, _)| m.clone()).unwrap_or_default() } else { model.clone() };
                    crate::push_text(&mut g.evs, pm, &abs, &text, src, &m, &sid, any_ts(&tc["timestamp"]).unwrap_or(ts));
                }
            }
            _ => {}
        }
    }
    let model = models.into_iter().max_by_key(|(_, n)| *n).map(|(m, _)| m).unwrap_or_default();
    if start > 0 {
        g.sessions.push(Session { src, id: sid, model, start, end, tokens, branch: None, prompts, reviewer: false, has_content: true, provider: None });
    }
}

/// Gemini CLI（扫描用的根路径由调用方给：主仓库 + worktree + 登记过的旧路径）
pub fn scan(store: &mut Store, pm: &PathMap, roots: &[String], st: &mut Stats) {
    scan_src(Src::Gemini, store, pm, roots, st);
    scan_src(Src::Qwen, store, pm, roots, st);
}

/// discover：只有写了 .project_root 的目录能知道是哪个项目；其余只能在已知项目里按哈希对上
pub fn discover() -> Vec<Activity> {
    let mut out = vec![];
    for src in [Src::Gemini, Src::Qwen] {
        for p in projects(src) {
            let Some(root) = p.root else { continue };
            let files: Vec<PathBuf> = std::fs::read_dir(p.dir.join("chats")).into_iter().flatten().flatten().map(|e| e.path()).collect();
            let last = files.iter().filter_map(|f| std::fs::metadata(f).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).max().unwrap_or(0);
            out.push(Activity { src, cwd: root, remote: None, sessions: files.len() as u64, last });
        }
    }
    out
}

pub fn doctor(src: Src) -> Health {
    let base = source_dir(src).join("tmp");
    let ps = projects(src);
    let files: Vec<PathBuf> = ps.iter().flat_map(|p| std::fs::read_dir(p.dir.join("chats")).into_iter().flatten().flatten().map(|e| e.path())).collect();
    let mt: Vec<i64> = files.iter().filter_map(|f| std::fs::metadata(f).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).collect();
    Health {
        source: src.harness().into(),
        path: base.display().to_string(),
        readable: base.is_dir(),
        format: "tmp/<项目哈希>/chats/session-*.json".into(),
        records: files.len() as u64,
        earliest: mt.iter().min().copied().map(date_cn),
        latest: mt.iter().max().copied().map(date_cn),
        note: format!("{} 个项目目录（按项目路径的 SHA-256 对应）", ps.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha_of_path() {
        assert_eq!(sha("/tmp/demo").len(), 64);
        assert_eq!(sha(""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }
}
