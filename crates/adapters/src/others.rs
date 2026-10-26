//! 装了、但读不了编辑记录的工具：doctor 里列出来，说明原因，不报警告。
//! - Windsurf：Cascade 对话是 `~/.codeium/windsurf/cascade/*.pb`，加密的 protobuf，里面编辑步骤的结构没有公开；
//! - TRAE IDE：对话存在各工作区的 state.vscdb 里，格式没有公开（TRAE CLI 能读，见 trae.rs）。
//!
//! 这两个工具写的行只能靠提交盖章（`creditia hook install`）或提交信息里的 Co-authored-by 认出来。

use crate::{home, vscode_user_dirs, Health};
use creditia_core::tr;
use creditia_core::util::date_local;
use std::path::PathBuf;

fn mtimes(files: &[PathBuf]) -> (Option<String>, Option<String>) {
    let mt: Vec<i64> = files.iter().filter_map(|f| std::fs::metadata(f).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).collect();
    (mt.iter().min().copied().map(date_local), mt.iter().max().copied().map(date_local))
}

pub fn doctor() -> Vec<Health> {
    let mut out = vec![];
    let ws = std::env::var("CREDITIA_WINDSURF_DIR").map(PathBuf::from).unwrap_or_else(|_| home().join(".codeium/windsurf"));
    let pbs: Vec<PathBuf> = std::fs::read_dir(ws.join("cascade")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "pb").unwrap_or(false)).collect();
    if ws.is_dir() {
        let (earliest, latest) = mtimes(&pbs);
        out.push(Health {
            source: "windsurf".into(),
            path: ws.join("cascade").display().to_string(),
            readable: false,
            unsupported: true,
            format: "cascade/*.pb".into(),
            records: pbs.len() as u64,
            earliest,
            latest,
            note: tr!("加密的 protobuf，编辑内容读不出来；Windsurf 写的行请用提交盖章", "encrypted protobuf, edits can't be read; stamp Windsurf commits instead"),
        });
    }
    let trae: Vec<PathBuf> = vscode_user_dirs().into_iter().filter(|u| u.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().starts_with("Trae")).unwrap_or(false)).filter(|u| u.join("workspaceStorage").is_dir()).collect();
    if let Some(u) = trae.first() {
        let dbs: Vec<PathBuf> = std::fs::read_dir(u.join("workspaceStorage")).into_iter().flatten().flatten().map(|e| e.path().join("state.vscdb")).filter(|p| p.is_file()).collect();
        let (earliest, latest) = mtimes(&dbs);
        out.push(Health {
            source: "trae-ide".into(),
            path: u.join("workspaceStorage").display().to_string(),
            readable: false,
            unsupported: true,
            format: "workspaceStorage/<ws>/state.vscdb".into(),
            records: dbs.len() as u64,
            earliest,
            latest,
            note: tr!("对话格式没有公开，不读；TRAE CLI 的会话能读", "chat format isn't public, not read; TRAE CLI sessions are read"),
        });
    }
    out
}
