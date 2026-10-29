//! 归因准确率基准：模拟一个项目的开发过程，每一行是谁写的都已知；用真实二进制分析，逐行对答案。
//! 场景覆盖常见的难点：编辑带上下文、整文件覆盖写入、混合提交、格式化拆行、人改几个字、盖章、云端分支、
//! 同一工具两个模型、中文文档、日志丢失、重命名、同文件重复行、多个 agent 交替改同一文件、
//! Aider 的 SEARCH/REPLACE 记录、Copilot Chat 的编辑工具（含报错没写进去的编辑）。
//!
//! 运行：cargo test -p creditia-cli --test bench -- --nocapture（看表格；CREDITIA_BENCH_VERBOSE=1 列出每一行错的）
//! 结果同时写到 $CREDITIA_BENCH_OUT（如果设置了）。末尾的断言是底线：改引擎让它们变差，测试就失败。

use creditia_core::model::normalize_model;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ME: &str = "me@example.com";
const DAY: i64 = 86400;

#[derive(Clone, Debug)]
struct L {
    text: String,
    /// "claude-code/opus-x"、"codex"（只知道工具）、"me"（人写的）、"gen"（脚本生成的：按约定不算给 AI）
    who: &'static str,
}

struct Sim {
    root: PathBuf,
    repo: PathBuf,
    vars: HashMap<String, String>,
    t: i64,
    rng: u64,
    scenario: &'static str,
    files: BTreeMap<String, Vec<L>>,
    claude: Vec<Value>,
    claude_n: usize,
    codex: Vec<Value>,
    cursor: Vec<(String, String, i64, String)>,
    /// .aider.chat.history.md 的内容
    aider: String,
    /// Copilot Chat 一个会话里的请求
    copilot: Vec<Value>,
    /// 提交 → (场景, 提交时的文件内容)
    snaps: HashMap<String, (&'static str, BTreeMap<String, Vec<L>>)>,
    /// 不写日志（模拟日志被清理且从没采集过）
    lost: bool,
    /// Claude Code 旧版本：工具结果里没有 toolUseResult
    old_claude: bool,
}

fn git_in(repo: &Path, args: &[&str]) -> String {
    let o = Command::new("git").arg("-C").arg(repo).args(["-c", "user.name=Me", "-c", &format!("user.email={ME}"), "-c", "commit.gpgsign=false", "-c", "core.quotepath=false"]).args(args).output().unwrap();
    assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn iso(t: i64) -> String {
    let (y, m, d) = creditia_core::util::civil_from_days(t.div_euclid(DAY));
    let s = t.rem_euclid(DAY);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", s / 3600, s / 60 % 60, s % 60)
}

fn harness(who: &str) -> &str {
    who.split('/').next().unwrap()
}

/// 最长公共子序列：old → new 的逐行差异，返回 (' ' | '-' | '+', 旧下标, 新下标)
fn diff(old: &[String], new: &[String]) -> Vec<(char, usize, usize)> {
    let (n, m) = (old.len(), new.len());
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if old[i] == new[j] { dp[i + 1][j + 1] + 1 } else { dp[i + 1][j].max(dp[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, vec![]);
    while i < n || j < m {
        if i < n && j < m && old[i] == new[j] {
            out.push((' ', i, j));
            i += 1;
            j += 1;
        } else if j < m && (i == n || dp[i][j + 1] >= dp[i + 1][j]) {
            out.push(('+', i, j));
            j += 1;
        } else {
            out.push(('-', i, j));
            i += 1;
        }
    }
    out
}

/// 统一 diff 的 hunk（3 行上下文），Claude 的 structuredPatch 和 Codex 的 unified_diff 都用它
fn hunks(old: &[String], new: &[String]) -> Vec<(usize, usize, usize, usize, Vec<String>)> {
    let d = diff(old, new);
    let changed: Vec<usize> = d.iter().enumerate().filter(|(_, x)| x.0 != ' ').map(|(i, _)| i).collect();
    let mut out = vec![];
    let mut k = 0;
    while k < changed.len() {
        let s = changed[k].saturating_sub(3);
        let mut e = changed[k];
        while k + 1 < changed.len() && changed[k + 1] <= e + 7 {
            k += 1;
            e = changed[k];
        }
        let e = (e + 3).min(d.len() - 1);
        k += 1;
        let seg = &d[s..=e];
        let lines: Vec<String> = seg.iter().map(|&(c, i, j)| format!("{c}{}", if c == '-' { &old[i] } else { &new[j] })).collect();
        let os = seg.iter().find(|x| x.0 != '+').map(|x| x.1 + 1).unwrap_or(seg[0].1);
        let ns = seg.iter().find(|x| x.0 != '-').map(|x| x.2 + 1).unwrap_or(seg[0].2);
        let ol = seg.iter().filter(|x| x.0 != '+').count();
        let nl = seg.iter().filter(|x| x.0 != '-').count();
        out.push((os, ol, ns, nl, lines));
    }
    out
}

impl Sim {
    fn new() -> Sim {
        let root = std::env::temp_dir().join(format!("creditia-bench-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let repo = root.join("work/bench-app");
        fs::create_dir_all(&repo).unwrap();
        git_in(&repo, &["init", "-q", "-b", "main"]);
        let mut vars = HashMap::new();
        for (k, v) in [
            ("HOME", ""),
            ("CREDITIA_HOME", "lib"),
            ("CODEX_HOME", "codex"),
            ("CLAUDE_CONFIG_DIR", "claude"),
            ("CREDITIA_CURSOR_DIR", "cursor"),
            ("CREDITIA_CURSOR_STORAGE", "cursor-storage"),
            ("CREDITIA_GROK_DIR", "grok"),
            ("CREDITIA_KIMI_DIR", "kimi"),
            ("CREDITIA_GEMINI_DIR", "gemini"),
            ("CREDITIA_QWEN_DIR", "qwen"),
            ("CREDITIA_OPENCODE_DIR", "opencode"),
            ("CREDITIA_VSCODE_STORAGE", "vscode"),
            ("APPDATA", "AppData/Roaming"),
        ] {
            vars.insert(k.to_string(), root.join(v).display().to_string());
        }
        Sim { root, repo, vars, t: 1_780_000_000, rng: 0x9e37_79b9_7f4a_7c15, scenario: "", files: BTreeMap::new(), claude: vec![], claude_n: 0, codex: vec![], cursor: vec![], aider: String::new(), copilot: vec![], snaps: HashMap::new(), lost: false, old_claude: false }
    }

    fn rand(&mut self, n: usize) -> usize {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % n as u64) as usize
    }

    /// 生成 n 行像样的代码。约 1/8 是 `}`、空行这种短行，少量是各处都会出现的常见行
    fn gen(&mut self, path: &str, n: usize) -> Vec<String> {
        const W: [&str; 24] = ["user", "token", "cache", "frame", "query", "index", "score", "batch", "layer", "route", "store", "event", "chunk", "label", "shard", "queue", "patch", "probe", "range", "trace", "model", "scene", "graph", "blob"];
        const V: [&str; 12] = ["load", "parse", "merge", "split", "render", "flush", "build", "fetch", "apply", "check", "sort", "pack"];
        const ZH: [&str; 16] = ["配置", "缓存", "索引", "用户", "渲染", "导出", "日志", "模型", "分支", "提交", "样式", "作品", "数据", "路径", "版本", "时间"];
        let ext = path.rsplit('.').next().unwrap_or("");
        let mut out = vec![];
        for _ in 0..n {
            let r = self.rand(100);
            let (a, b, c) = (W[self.rand(24)], W[self.rand(24)], V[self.rand(12)]);
            let k = self.rand(1000);
            let line = if ext == "md" {
                format!("- {}{}的{}：先{}再处理第 {k} 项", ZH[self.rand(16)], ZH[self.rand(16)], ZH[self.rand(16)], ZH[self.rand(16)])
            } else if r < 8 {
                "}".into()
            } else if r < 12 {
                String::new()
            } else if r < 15 {
                match (ext, self.rand(3)) {
                    ("rs", 0) => "        return Ok(());".into(),
                    ("rs", 1) => "    Ok(())".into(),
                    ("rs", _) => "use std::collections::HashMap;".into(),
                    ("py", 0) => "        return None".into(),
                    ("py", _) => "import os".into(),
                    (_, 0) => "    return null;".into(),
                    (_, 1) => "import { useState } from \"react\";".into(),
                    _ => "  return result;".into(),
                }
            } else if r < 30 {
                match ext {
                    "rs" => format!("    let {a}_{b} = {c}_{a}(&{b}_list, {k}, {a}_opts, &mut {b}_state);"),
                    "py" => format!("    {a}_{b} = {c}_{a}({b}_list, {k}, opts={a}_opts, state={b}_state)"),
                    _ => format!("  const {a}{k} = {c}{b}({a}List, {k}, {b}Options, {{ strict: true }});"),
                }
            } else {
                match ext {
                    "rs" => format!("    let {a}_{k} = {b}.{c}({k});"),
                    "py" => format!("    {a}_{k} = {b}.{c}({k})"),
                    _ => format!("  const {a}{k} = {b}.{c}({k});"),
                }
            };
            out.push(line);
        }
        out
    }

    fn texts(&self, path: &str) -> Vec<String> {
        self.files.get(path).map(|v| v.iter().map(|l| l.text.clone()).collect()).unwrap_or_default()
    }

    fn write_disk(&self, path: &str) {
        let p = self.repo.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        match self.files.get(path) {
            Some(v) => fs::write(&p, v.iter().map(|l| format!("{}\n", l.text)).collect::<String>()).unwrap(),
            None => {
                let _ = fs::remove_file(&p);
            }
        }
    }

    /// 某个 agent 把文件改成 new（整份内容）。按工具写各自真实格式的日志
    fn agent_set(&mut self, who: &'static str, path: &str, new: Vec<String>, as_write: bool) {
        self.t += 120;
        let old = self.texts(path);
        let existed = self.files.contains_key(path);
        // 归属：diff 里没变的行保留原作者，新增的行归 who
        let prev = self.files.get(path).cloned().unwrap_or_default();
        let mut next = vec![];
        for (c, i, j) in diff(&old, &new) {
            match c {
                ' ' => next.push(prev[i].clone()),
                '+' => next.push(L { text: new[j].clone(), who }),
                _ => {}
            }
        }
        self.files.insert(path.to_string(), next);
        self.write_disk(path);
        if self.lost {
            return;
        }
        let abs = self.repo.join(path).display().to_string();
        let (h, model) = who.split_once('/').unwrap();
        let hk = hunks(&old, &new);
        match h {
            "claude-code" => {
                self.claude_n += 1;
                let id = format!("toolu_{}", self.claude_n);
                let cwd = self.repo.display().to_string();
                let (name, input, result) = if as_write || !existed {
                    let content = new.join("\n") + "\n";
                    let patch: Vec<Value> = hk.iter().map(|(os, ol, ns, nl, ls)| json!({ "oldStart": os, "oldLines": ol, "newStart": ns, "newLines": nl, "lines": ls })).collect();
                    let r = json!({ "type": if existed { "update" } else { "create" }, "filePath": abs, "content": content, "structuredPatch": if existed { json!(patch) } else { json!([]) }, "originalFile": if existed { json!(old.join("\n") + "\n") } else { Value::Null }, "userModified": false });
                    ("Write", json!({ "file_path": abs, "content": content }), r)
                } else {
                    // Edit：old_string / new_string 是改动范围加上下文（模型为了让 old_string 唯一，通常带几行上下文）
                    let d = diff(&old, &new);
                    let first = d.iter().position(|x| x.0 != ' ').unwrap_or(0);
                    let last = d.iter().rposition(|x| x.0 != ' ').unwrap_or(0);
                    let (s, e) = (first.saturating_sub(2), (last + 2).min(d.len() - 1));
                    let seg = &d[s..=e];
                    let os: Vec<&str> = seg.iter().filter(|x| x.0 != '+').map(|x| old[x.1].as_str()).collect();
                    let ns: Vec<&str> = seg.iter().filter(|x| x.0 != '-').map(|x| new[x.2].as_str()).collect();
                    let patch: Vec<Value> = hk.iter().map(|(os, ol, ns, nl, ls)| json!({ "oldStart": os, "oldLines": ol, "newStart": ns, "newLines": nl, "lines": ls })).collect();
                    let r = json!({ "filePath": abs, "oldString": os.join("\n"), "newString": ns.join("\n"), "originalFile": old.join("\n") + "\n", "structuredPatch": patch, "userModified": false, "replaceAll": false });
                    ("Edit", json!({ "file_path": abs, "old_string": os.join("\n"), "new_string": ns.join("\n") }), r)
                };
                self.claude.push(json!({ "type": "assistant", "cwd": cwd, "sessionId": "s1", "timestamp": iso(self.t), "message": { "id": format!("msg_{}", self.claude_n), "model": model, "usage": { "input_tokens": 10, "output_tokens": 10 }, "content": [{ "type": "tool_use", "id": id, "name": name, "input": input }] } }));
                let mut u = json!({ "type": "user", "cwd": cwd, "sessionId": "s1", "timestamp": iso(self.t + 1), "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": id, "content": "ok" }] } });
                if !self.old_claude {
                    u["toolUseResult"] = result;
                }
                self.claude.push(u);
            }
            "codex" => {
                self.codex.push(json!({ "timestamp": iso(self.t - 1), "type": "turn_context", "payload": { "model": model, "cwd": self.repo.display().to_string() } }));
                let change = if existed {
                    let body: String = hk.iter().map(|(os, ol, ns, nl, ls)| format!("@@ -{os},{ol} +{ns},{nl} @@\n{}\n", ls.join("\n"))).collect();
                    json!({ "type": "update", "unified_diff": body, "move_path": null })
                } else {
                    json!({ "type": "add", "content": new.join("\n") + "\n" })
                };
                self.codex.push(json!({ "timestamp": iso(self.t), "type": "event_msg", "payload": { "type": "item_completed", "item": { "type": "FileChange", "changes": { abs: change } } } }));
            }
            "cursor" => {
                for (c, _, j) in diff(&old, &new) {
                    if c == '+' {
                        self.cursor.push((path.to_string(), new[j].clone(), self.t, model.to_string()));
                    }
                }
            }
            "aider" | "copilot" => {
                // 和 Claude 的 Edit 一样：改动范围加两行上下文；新文件整份写
                let (os, ns) = if !existed {
                    (String::new(), new.join("\n"))
                } else {
                    let d = diff(&old, &new);
                    let first = d.iter().position(|x| x.0 != ' ').unwrap_or(0);
                    let last = d.iter().rposition(|x| x.0 != ' ').unwrap_or(0);
                    let seg = &d[first.saturating_sub(2)..=(last + 2).min(d.len() - 1)];
                    (seg.iter().filter(|x| x.0 != '+').map(|x| old[x.1].as_str()).collect::<Vec<_>>().join("\n"), seg.iter().filter(|x| x.0 != '-').map(|x| new[x.2].as_str()).collect::<Vec<_>>().join("\n"))
                };
                if h == "aider" {
                    let (y, mo, dd) = creditia_core::util::civil_from_days(self.t.div_euclid(DAY));
                    let sec = self.t.rem_euclid(DAY);
                    let local = format!("{y:04}-{mo:02}-{dd:02} {:02}:{:02}:{:02}", sec / 3600, sec / 60 % 60, sec % 60);
                    self.aider.push_str(&format!("\n# aider chat started at {local}\n\n> Main model: {model} with diff edit format  \n\n#### change {path}  \n\n{path}\n```\n<<<<<<< SEARCH\n{os}\n=======\n{ns}\n>>>>>>> REPLACE\n```\n\n> Applied edit to {path}  \n"));
                } else {
                    let (name, args) = if existed { ("replace_string_in_file", json!({ "filePath": abs, "oldString": os, "newString": ns })) } else { ("create_file", json!({ "filePath": abs, "content": ns })) };
                    self.copilot_request(model, name, args, false);
                }
            }
            _ => unreachable!(),
        }
    }

    fn copilot_request(&mut self, model: &str, name: &str, args: Value, failed: bool) {
        let id = format!("call_{}", self.copilot.len());
        let results = if failed { json!({ id.clone(): { "content": [{ "value": "ERROR while calling tool: The user cancelled the edit" }] } }) } else { json!({}) };
        self.copilot.push(json!({ "timestamp": self.t * 1000, "modelId": format!("copilot/{model}"), "result": { "metadata": { "toolCallRounds": [{ "toolCalls": [{ "id": id, "name": name, "arguments": args.to_string() }] }], "toolCallResults": results } } }));
    }

    /// Copilot 提出的新文件报错没写进去，人自己把同样的内容敲了进去：这些行是人的
    fn copilot_failed_then_human(&mut self, model: &str, path: &str, n: usize) {
        self.t += 120;
        let v = self.gen(path, n);
        let abs = self.repo.join(path).display().to_string();
        self.copilot_request(model, "create_file", json!({ "filePath": abs, "content": v.join("\n") }), true);
        self.t += 300;
        self.files.insert(path.to_string(), v.into_iter().map(|text| L { text, who: "me" }).collect());
        self.write_disk(path);
    }

    /// agent 在 at 处删 del 行、插入 n 行新代码
    fn agent_edit(&mut self, who: &'static str, path: &str, at: usize, del: usize, n: usize) {
        let mut v = self.texts(path);
        let add = self.gen(path, n);
        v.splice(at..(at + del).min(v.len()), add);
        self.agent_set(who, path, v, false);
    }

    /// agent 用 shell 命令改文件：命令原文里带着新内容（heredoc 或 python 脚本）。`read_only` 时命令只读文件、不改
    fn agent_shell(&mut self, who: &'static str, path: &str, new: Vec<String>, how: &str) {
        self.t += 120;
        let old = self.texts(path);
        let cmd = match how {
            "heredoc" => format!("mkdir -p $(dirname {path}) && cat > {path} <<'EOF'\n{}\nEOF", new.join("\n")),
            "python" => {
                // 只把改动的部分写进脚本：旧的那几行换成新的那几行
                let added: Vec<&str> = new.iter().filter(|l| !old.contains(l)).map(|s| s.as_str()).collect();
                format!("python3 - <<'PY'\np = '{path}'\ns = open(p).read()\ns = s.replace(\"__ANCHOR__\", \"\"\"\n{}\n\"\"\")\nopen(p, 'w').write(s)\nPY", added.join("\n"))
            }
            "gen" => format!("node scripts/gen.mjs > {path}"),
            _ => format!("cat {path} | head -50 && grep -n fn {path}"),
        };
        if how != "read" && how != "gen" {
            let prev = self.files.get(path).cloned().unwrap_or_default();
            let mut next = vec![];
            for (c, i, j) in diff(&old, &new) {
                match c {
                    ' ' => next.push(prev[i].clone()),
                    '+' => next.push(L { text: new[j].clone(), who }),
                    _ => {}
                }
            }
            self.files.insert(path.to_string(), next);
            self.write_disk(path);
        }
        let (h, model) = who.split_once('/').unwrap();
        let cwd = self.repo.display().to_string();
        match h {
            "claude-code" => {
                self.claude_n += 1;
                let id = format!("toolu_{}", self.claude_n);
                self.claude.push(json!({ "type": "assistant", "cwd": cwd, "sessionId": "s1", "timestamp": iso(self.t), "message": { "id": format!("msg_{}", self.claude_n), "model": model, "usage": { "input_tokens": 10, "output_tokens": 10 }, "content": [{ "type": "tool_use", "id": id, "name": "Bash", "input": { "command": cmd, "description": "write file" } }] } }));
                self.claude.push(json!({ "type": "user", "cwd": cwd, "sessionId": "s1", "timestamp": iso(self.t + 1), "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": id, "content": "", "is_error": false }] }, "toolUseResult": { "stdout": "", "stderr": "", "interrupted": false } }));
            }
            "codex" => {
                self.claude_n += 1;
                let id = format!("call_{}", self.claude_n);
                self.codex.push(json!({ "timestamp": iso(self.t - 1), "type": "turn_context", "payload": { "model": model, "cwd": cwd } }));
                let args = json!({ "command": ["bash", "-lc", cmd], "workdir": cwd }).to_string();
                self.codex.push(json!({ "timestamp": iso(self.t), "type": "response_item", "payload": { "type": "function_call", "name": "shell", "arguments": args, "call_id": id } }));
                self.codex.push(json!({ "timestamp": iso(self.t + 1), "type": "response_item", "payload": { "type": "function_call_output", "call_id": id, "output": json!({ "output": "", "metadata": { "exit_code": 0, "duration_seconds": 0.1 } }).to_string() } }));
            }
            _ => unreachable!(),
        }
    }

    /// agent 跑了一个生成脚本：文件内容是程序写的，日志里只有那条命令
    fn generated(&mut self, agent: &'static str, path: &str, n: usize) {
        self.t += 120;
        let v = self.gen(path, n);
        self.files.insert(path.to_string(), v.into_iter().map(|text| L { text, who: "gen" }).collect());
        self.write_disk(path);
        self.agent_shell(agent, path, vec![], "gen");
    }

    fn agent_new(&mut self, who: &'static str, path: &str, n: usize) {
        let v = self.gen(path, n);
        self.agent_set(who, path, v, true);
    }

    /// 人在 at 处插入 n 行（没有任何日志）
    fn human_edit(&mut self, path: &str, at: usize, n: usize) {
        self.t += 300;
        let add: Vec<L> = self.gen(path, n).into_iter().map(|text| L { text, who: "me" }).collect();
        let v = self.files.entry(path.to_string()).or_default();
        let at = at.min(v.len());
        v.splice(at..at, add);
        self.write_disk(path);
    }

    /// 人改几个字：重命名一个标识符。答案仍记原作者（AI 写的行，人顺手改了名）
    fn human_tweak(&mut self, path: &str, every: usize) {
        self.t += 300;
        let v = self.files.get_mut(path).unwrap();
        for (i, l) in v.iter_mut().enumerate() {
            if i % every == 0 && l.text.len() > 12 {
                l.text = l.text.replacen("_", "_v2_", 1).replacen("(", "( ", 0);
                if !l.text.contains("_v2_") {
                    l.text.push_str(" // ok");
                }
            }
        }
        self.write_disk(path);
    }

    /// 格式化：长参数列表拆成每个参数一行（prettier / rustfmt 的典型行为）。拆出来的行仍记原作者
    fn format(&mut self, path: &str) {
        self.t += 30;
        let v = self.files.remove(path).unwrap();
        let mut out = vec![];
        for l in v {
            if l.text.len() > 60 && l.text.contains('(') && l.text.contains(", ") {
                let open = l.text.find('(').unwrap();
                let close = l.text.rfind(')').unwrap();
                let indent = " ".repeat(l.text.len() - l.text.trim_start().len());
                out.push(L { text: l.text[..=open].to_string(), who: l.who });
                for a in l.text[open + 1..close].split(", ") {
                    out.push(L { text: format!("{indent}    {a},"), who: l.who });
                }
                out.push(L { text: format!("{indent}{}", &l.text[close..]), who: l.who });
            } else {
                out.push(l);
            }
        }
        self.files.insert(path.to_string(), out);
        self.write_disk(path);
    }

    /// 格式化把相邻的两行并成一行（短的对象字面量、链式调用收回一行）。并出来的行仍记原作者，但逐行证据对不上了
    fn join(&mut self, path: &str) {
        self.t += 30;
        let v = self.files.remove(path).unwrap();
        let mut out: Vec<L> = vec![];
        let mut i = 0;
        while i < v.len() {
            let long = |l: &L| l.text.trim().len() > 3;
            if i + 1 < v.len() && long(&v[i]) && long(&v[i + 1]) {
                out.push(L { text: format!("{} {}", v[i].text.trim_end(), v[i + 1].text.trim()), who: v[i].who });
                i += 2;
            } else {
                out.push(v[i].clone());
                i += 1;
            }
        }
        self.files.insert(path.to_string(), out);
        self.write_disk(path);
    }

    fn commit_as(&mut self, msg: &str, email: &str) -> String {
        self.t += 600;
        git_in(&self.repo, &["add", "-A"]);
        let date = format!("@{} +0000", self.t);
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.repo)
            .args(["-c", "user.name=X", "-c", &format!("user.email={email}"), "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", msg, "--date", &date])
            .env("GIT_COMMITTER_DATE", &date)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let h = git_in(&self.repo, &["rev-parse", "HEAD"]).trim().to_string();
        self.snaps.insert(h.clone(), (self.scenario, self.files.clone()));
        h
    }

    /// 在 GitHub 网页上合并：作者是 GitHub 账号的邮箱，提交者是 GitHub
    fn commit_web(&mut self, msg: &str, author: &str) -> String {
        self.t += 600;
        git_in(&self.repo, &["add", "-A"]);
        let date = format!("@{} +0000", self.t);
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.repo)
            .args(["-c", "user.name=X", "-c", &format!("user.email={author}"), "-c", "commit.gpgsign=false", "commit", "-q", "-m", msg, "--date", &date])
            .env("GIT_COMMITTER_DATE", &date)
            .env("GIT_COMMITTER_NAME", "GitHub")
            .env("GIT_COMMITTER_EMAIL", "noreply@github.com")
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let h = git_in(&self.repo, &["rev-parse", "HEAD"]).trim().to_string();
        self.snaps.insert(h.clone(), (self.scenario, self.files.clone()));
        h
    }

    fn commit(&mut self, msg: &str) -> String {
        self.commit_as(msg, ME)
    }

    fn scene(&mut self, name: &'static str) {
        self.scenario = name;
        self.t += 20 * DAY; // 场景之间隔开，超出证据时间窗口
        self.lost = false;
        self.old_claude = false;
    }

    fn flush_logs(&self) {
        fs::write(self.repo.join(".aider.chat.history.md"), &self.aider).unwrap();
        fs::write(self.repo.join(".git/info/exclude"), ".aider*\n").unwrap();
        let config_dir = if cfg!(target_os = "macos") { "Library/Application Support" } else if cfg!(target_os = "windows") { "AppData/Roaming" } else { ".config" };
        let ws = self.root.join(config_dir).join("Code/User/workspaceStorage/bench");
        fs::create_dir_all(ws.join("chatSessions")).unwrap();
        fs::write(ws.join("workspace.json"), json!({ "folder": format!("file://{}", self.repo.display()) }).to_string()).unwrap();
        fs::write(ws.join("chatSessions/b1.json"), json!({ "version": 3, "sessionId": "b1", "creationDate": 1_780_000_000_000i64, "requests": self.copilot }).to_string()).unwrap();

        let dir = self.root.join("claude/projects/-work-bench-app");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("s1.jsonl"), self.claude.iter().map(|v| v.to_string() + "\n").collect::<String>()).unwrap();

        let base = self.root.join("codex");
        fs::create_dir_all(base.join("sessions/2026/01/01")).unwrap();
        let rollout = base.join("sessions/2026/01/01/rollout-2026-01-01T00-00-00-t1.jsonl");
        fs::write(&rollout, self.codex.iter().map(|v| v.to_string() + "\n").collect::<String>()).unwrap();
        let conn = rusqlite::Connection::open(base.join("state_5.sqlite")).unwrap();
        conn.execute_batch("create table if not exists threads(id text primary key, rollout_path text not null, created_at integer not null, updated_at integer not null, source text not null default '', model_provider text not null default '', cwd text not null, title text not null default '', sandbox_policy text not null default '', approval_mode text not null default '', tokens_used integer not null default 0, has_user_event integer not null default 0, archived integer not null default 0, archived_at integer, git_sha text, git_branch text, git_origin_url text, model text);").unwrap();
        conn.execute("insert or replace into threads(id,rollout_path,created_at,updated_at,cwd,tokens_used,git_branch,model) values('t1',?1,?2,?3,?4,1000,'main','gpt-a')", rusqlite::params![rollout.display().to_string(), 1_780_000_000i64, self.t, self.repo.display().to_string()]).unwrap();

        let dir = self.root.join("cursor/ai-tracking");
        fs::create_dir_all(&dir).unwrap();
        let conn = rusqlite::Connection::open(dir.join("ai-code-tracking.db")).unwrap();
        conn.execute_batch("create table if not exists ai_code_hashes(hash text primary key, source text not null, fileExtension text, fileName text, requestId text, conversationId text, timestamp integer, model text, createdAt integer not null);").unwrap();
        use creditia_core::hash::{ext_of, line_key, normalize};
        for (rel, line, ts, model) in &self.cursor {
            let n = normalize(line, &ext_of(rel));
            if n.chars().count() < 4 {
                continue;
            }
            let h = format!("{:x}", line_key(rel, &n));
            let abs = self.repo.join(rel).display().to_string();
            conn.execute("insert or replace into ai_code_hashes(hash,source,fileExtension,fileName,requestId,conversationId,timestamp,model,createdAt) values(?1,'composer','ts',?2,'r','conv1',?3,?4,?3)", rusqlite::params![h, abs, ts * 1000, model]).unwrap();
        }
    }

    fn run(&self, args: &[&str]) -> Value {
        let o = Command::new(env!("CARGO_BIN_EXE_creditia-core")).args(args).arg("--json").env_clear().envs(&self.vars).env("PATH", std::env::var("PATH").unwrap()).env("CREDITIA_DUMP_LINES", self.root.join("dump.json")).output().unwrap();
        let s = String::from_utf8_lossy(&o.stdout).to_string();
        serde_json::from_str(&s).unwrap_or_else(|_| panic!("非 JSON 输出：{s}\n{}", String::from_utf8_lossy(&o.stderr)))
    }
}

/// 一次提交里每个文件新增行对应的新文件行号（与 creditia 读提交的参数一致）
fn added_lines(repo: &Path, hash: &str) -> HashMap<String, Vec<usize>> {
    let out = git_in(repo, &["show", "--format=", "-M", "--unified=0", "--no-color", "--no-ext-diff", hash]);
    let mut m: HashMap<String, Vec<usize>> = HashMap::new();
    let (mut cur, mut ln) = (String::new(), 0usize);
    for l in out.lines() {
        if let Some(p) = l.strip_prefix("+++ ") {
            cur = p.strip_prefix("b/").unwrap_or(p).to_string();
        } else if let Some(h) = l.strip_prefix("@@ ") {
            let plus = h.split(' ').find(|s| s.starts_with('+')).unwrap();
            ln = plus[1..].split(',').next().unwrap().parse().unwrap();
        } else if l.starts_with('+') {
            m.entry(cur.clone()).or_default().push(ln);
            ln += 1;
        }
    }
    m
}

#[derive(Default, Clone, Copy)]
struct Tally {
    ai: usize,
    exact: usize,
    tool_only: usize,
    wrong_tool: usize,
    missed: usize,
    human: usize,
    claimed: usize,
    /// AI 写的行被算给了人
    as_human: usize,
}
impl Tally {
    fn add(&mut self, truth: &str, pred: &str) {
        if truth == "me" || truth == "gen" {
            self.human += 1;
            if pred.starts_with("a:") {
                self.claimed += 1;
            }
            return;
        }
        self.ai += 1;
        let want = match truth.split_once('/') {
            Some((h, m)) => format!("a:{h}/{}", normalize_model(m)),
            None => format!("a:{truth}"),
        };
        let ph = pred.strip_prefix("a:").map(|p| p.split('/').next().unwrap());
        if pred == want {
            self.exact += 1;
        } else if ph == Some(harness(truth)) {
            self.tool_only += 1;
        } else if ph.is_some() {
            self.wrong_tool += 1;
        } else {
            self.missed += 1;
            self.as_human += pred.starts_with("h:") as usize;
        }
    }
    fn merge(&mut self, o: &Tally) {
        self.ai += o.ai;
        self.exact += o.exact;
        self.tool_only += o.tool_only;
        self.wrong_tool += o.wrong_tool;
        self.missed += o.missed;
        self.human += o.human;
        self.claimed += o.claimed;
        self.as_human += o.as_human;
    }
    fn json(&self) -> Value {
        let p = |a: usize, b: usize| if b == 0 { Value::Null } else { json!((a as f64 / b as f64 * 1000.0).round() / 10.0) };
        json!({ "aiLines": self.ai, "toolCorrect": p(self.exact + self.tool_only, self.ai), "modelCorrect": p(self.exact, self.ai), "wrongTool": p(self.wrong_tool, self.ai), "missed": p(self.missed, self.ai), "humanLines": self.human, "falseClaims": p(self.claimed, self.human), "aiToPeople": p(self.as_human, self.ai) })
    }
}

fn pct(a: usize, b: usize) -> String {
    if b == 0 { "    -".into() } else { format!("{:5.1}", a as f64 / b as f64 * 100.0) }
}

#[test]
fn accuracy_bench() {
    let mut s = Sim::new();
    const OPUS: &str = "claude-code/opus-x";
    const SONNET: &str = "claude-code/sonnet-y";
    const GPT: &str = "codex/gpt-a";
    const GPT2: &str = "codex/gpt-b";
    const CUR: &str = "cursor/composer-c";

    // 起点：一个人写的骨架
    s.scene("00 人写的骨架");
    s.human_edit("README.md", 0, 3);
    s.human_edit("src/lib.rs", 0, 30);
    s.commit("init");

    s.scene("01 各写各的文件");
    s.agent_new(OPUS, "src/a.rs", 60);
    s.commit("claude part");
    s.agent_new(GPT, "web/b.ts", 50);
    s.commit("codex part");
    s.agent_new(CUR, "web/c.ts", 40);
    s.commit("cursor part");

    s.scene("02 编辑带上下文");
    s.agent_new(GPT, "src/ctx.rs", 40);
    s.agent_edit(OPUS, "src/ctx.rs", 20, 0, 6);
    s.commit("ctx edit");
    s.old_claude = true;
    s.agent_new(GPT, "src/ctx2.rs", 40);
    s.agent_edit(OPUS, "src/ctx2.rs", 20, 0, 6);
    s.commit("ctx edit, old log format");

    s.scene("03 整文件覆盖写入");
    s.agent_new(GPT, "web/ow.ts", 50);
    {
        let mut v = s.texts("web/ow.ts");
        for i in [5, 17, 29, 41] {
            v[i] = s.gen("web/ow.ts", 1).remove(0);
        }
        let more = s.gen("web/ow.ts", 3);
        v.extend(more);
        s.agent_set(OPUS, "web/ow.ts", v, true);
    }
    s.commit("overwrite");

    s.scene("04 混合提交");
    s.agent_edit(OPUS, "src/a.rs", 10, 0, 3);
    s.human_edit("tools/human.py", 0, 150);
    s.commit("mixed");

    s.scene("05 格式化拆行");
    s.agent_new(GPT, "web/fmt.ts", 80);
    s.format("web/fmt.ts");
    s.commit("formatted");

    s.scene("06 人改几个字");
    s.agent_new(OPUS, "tools/tw.py", 50);
    s.human_tweak("tools/tw.py", 5);
    s.commit("tweaked");

    s.scene("07 盖章提交");
    s.agent_new(OPUS, "src/st.rs", 30);
    s.commit("claude st");
    s.files.entry("src/st.rs".into()).or_default();
    {
        let add: Vec<L> = s.gen("src/st.rs", 40).into_iter().map(|text| L { text, who: "kimi/k3" }).collect();
        s.files.get_mut("src/st.rs").unwrap().extend(add);
        s.write_disk("src/st.rs");
    }
    s.commit("kimi work\n\nCreditia-Harness: kimi\nCreditia-Model: k3");

    s.scene("08 云端分支");
    git_in(&s.repo, &["checkout", "-q", "-b", "codex/cloud-task"]);
    {
        let add: Vec<L> = s.gen("src/cloud.rs", 60).into_iter().map(|text| L { text, who: "codex" }).collect();
        s.files.insert("src/cloud.rs".into(), add);
        s.write_disk("src/cloud.rs");
    }
    s.commit("cloud task");
    git_in(&s.repo, &["checkout", "-q", "main"]);
    s.t += 60;
    git_in(&s.repo, &["merge", "-q", "--no-ff", "codex/cloud-task", "-m", "Merge pull request #12 from me/codex/cloud-task\n\ncloud task"]);

    s.scene("09 同一工具两个模型");
    s.agent_new(OPUS, "web/two.ts", 40);
    s.agent_edit(SONNET, "web/two.ts", 10, 10, 10);
    s.agent_edit(GPT2, "web/two.ts", 30, 0, 5);
    s.commit("two models");

    s.scene("10 中文文档");
    s.agent_new(OPUS, "docs/guide.md", 150);
    s.commit("docs by claude");
    s.human_edit("docs/guide.md", 150, 150);
    s.commit("docs by me");

    s.scene("11 日志丢失");
    s.lost = true;
    s.agent_new(OPUS, "src/lost.rs", 50);
    s.commit("lost logs");

    s.scene("12 人自己写");
    s.agent_new(GPT, "web/side.ts", 20);
    s.commit("codex side");
    s.human_edit("src/hand.rs", 0, 120);
    s.commit("hand written");

    s.scene("13 重命名");
    git_in(&s.repo, &["mv", "src/a.rs", "src/core_a.rs"]);
    let v = s.files.remove("src/a.rs").unwrap();
    s.files.insert("src/core_a.rs".into(), v);
    s.human_edit("src/core_a.rs", 5, 2);
    s.commit("rename");

    s.scene("14 同文件重复的常见行");
    s.agent_new(GPT, "src/dup.rs", 60);
    s.commit("codex dup");
    s.t += 2 * DAY;
    s.human_edit("src/dup.rs", 60, 40);
    s.commit("me dup");

    s.scene("15 多个 agent 交替改");
    s.agent_new(GPT, "web/mix.ts", 30);
    s.agent_edit(OPUS, "web/mix.ts", 10, 0, 8);
    s.agent_edit(CUR, "web/mix.ts", 25, 2, 6);
    s.agent_edit(GPT, "web/mix.ts", 0, 0, 5);
    s.human_edit("web/mix.ts", 40, 6);
    s.commit("mixed agents");

    s.scene("16 盖章提交里夹着别的工具");
    s.agent_new(GPT, "src/st2.rs", 30);
    s.commit("codex st2");
    s.agent_edit(OPUS, "src/st2.rs", 5, 0, 3);
    {
        let add: Vec<L> = s.gen("src/st2.rs", 40).into_iter().map(|text| L { text, who: "kimi/k3" }).collect();
        s.files.get_mut("src/st2.rs").unwrap().extend(add);
        s.write_disk("src/st2.rs");
    }
    s.commit("kimi again\n\nCreditia-Harness: kimi\nCreditia-Model: k3");

    s.scene("17 Co-authored-by 提交");
    s.agent_new(OPUS, "web/co.ts", 60);
    s.format("web/co.ts");
    s.commit("claude with trailer\n\nCo-Authored-By: Claude <noreply@anthropic.com>");

    s.scene("18 agent 用 shell 写文件");
    s.agent_edit(GPT, "web/b.ts", 10, 0, 8);
    {
        let v = s.gen("web/gen.ts", 40);
        s.agent_shell(GPT, "web/gen.ts", v, "heredoc");
        let v = s.gen("src/sh.rs", 30);
        s.agent_shell(OPUS, "src/sh.rs", v, "heredoc");
        // python 脚本原地插入 6 行
        let mut v = s.texts("src/sh.rs");
        let add = s.gen("src/sh.rs", 6);
        v.splice(12..12, add);
        s.agent_shell(OPUS, "src/sh.rs", v, "python");
    }
    s.commit("shell-written files");

    s.scene("18b 生成脚本的输出");
    s.agent_edit(GPT, "web/b.ts", 30, 0, 8);
    s.generated(GPT, "web/generated.ts", 40);
    s.commit("codex edit + generator output");

    s.scene("18c agent 只读了人写的文件");
    s.human_edit("src/readme_me.rs", 0, 40);
    s.agent_shell(OPUS, "src/readme_me.rs", vec![], "read");
    s.agent_shell(GPT, "src/readme_me.rs", vec![], "read");
    s.commit("hand file the agents only read");

    s.scene("19 人在 agent 代码中间插几行");
    s.agent_new(OPUS, "src/mid.rs", 40);
    s.human_edit("src/mid.rs", 20, 4);
    s.commit("agent block with human lines inside");

    const AIDER: &str = "aider/sonnet-z";
    const COP: &str = "copilot/gpt-c";
    s.scene("20 Aider 和 Copilot");
    s.agent_new(AIDER, "py/tool.py", 40);
    s.agent_edit(COP, "py/tool.py", 12, 0, 6);
    s.agent_new(COP, "web/cop.ts", 30);
    s.agent_edit(AIDER, "web/cop.ts", 20, 2, 5);
    s.commit("aider and copilot");

    s.scene("21 Copilot 报错的编辑");
    s.copilot_failed_then_human("gpt-c", "web/failed.ts", 30);
    s.commit("written by hand after copilot failed");

    s.scene("22 在 agent 里提交的手写");
    s.agent_new(GPT, "web/stamped_ai.ts", 20);
    s.human_edit("src/stamped_me.rs", 0, 40);
    s.commit("committed from inside an agent\n\nCreditia-Harness: claude-code\nCreditia-Model: opus-x\nCreditia-Lines: codex/gpt-a 20 / 60");

    // 格式化把 agent 的行两两并成一行，逐行证据全对不上：记未确认，不能算给人
    s.scene("23 格式化合并的行");
    s.agent_new(SONNET, "web/joined.ts", 40);
    s.join("web/joined.ts");
    s.human_edit("src/joined_me.rs", 0, 30);
    s.commit("agent file reformatted onto fewer lines + hand file");

    // 在云端写、在 GitHub 网页上 squash 合并：本机没有日志，提交作者是 GitHub 账号的邮箱、提交者是 GitHub。
    // 保留真实原分支供缓存回放。正文配额本身无法证明哪些行在 squash 后存活。
    s.scene("24 GitHub 网页 squash 合并");
    git_in(&s.repo, &["remote", "add", "origin", "https://github.com/example/bench.git"]);
    git_in(&s.repo, &["checkout", "-qb", "cloud-work"]);
    s.t += 300;
    let v: Vec<L> = s.gen("src/web.rs", 30).into_iter().map(|text| L { text, who: GPT }).collect();
    s.files.insert("src/web.rs".into(), v);
    s.write_disk("src/web.rs");
    let original = s.commit("cloud original\n\nCreditia-Harness: codex\nCreditia-Lines: codex/gpt-a 30 / 30");
    // Original commits are not part of main's expected contribution totals.
    s.snaps.remove(&original);
    git_in(&s.repo, &["checkout", "-q", "main"]);
    git_in(&s.repo, &["merge", "--squash", "cloud-work"]);
    let squash = s.commit_web(
        "cloud work (#7)\n\n* first part\n\nCreditia-Harness: codex\nCreditia-Lines: codex/gpt-a 20 / 20\n\n* second part\n\nCreditia-Harness: codex\nCreditia-Lines: codex/gpt-a 10 / 10\n\nCo-authored-by: me <me@example.com>",
        "account@gmail.com",
    );

    s.flush_logs();
    let rs = s.repo.display().to_string();
    s.run(&["init", "--repo", &rs]);
    // The cache fixture represents already-validated PR metadata; API association
    // checks are tested separately. Derive every line from real Git provenance.
    let (commits,_) = creditia_adapters::git::read_commits(&s.repo,&squash,Some(&format!("{squash}^..{squash}")));
    let history = creditia_adapters::squash::local_history(&s.repo,&commits[0],&original).unwrap();
    let mut snapshot = serde_json::to_value(history).unwrap();
    snapshot["repository"] = "example/bench".into(); snapshot["number"] = 7.into();
    let (root,_) = creditia_adapters::git::identity(&s.repo);
    let dir=s.root.join("lib/cache").join(&root[..12]).join("squash-v1/example/bench");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{squash}.json")),serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let v = s.run(&["analyze", "--repo", &rs]);
    assert!(v["commits"].as_u64().unwrap() > 10, "{v}");
    let dump: Value = serde_json::from_str(&fs::read_to_string(s.root.join("dump.json")).unwrap()).unwrap();

    // ---- 历史：每个提交新增的每一行
    let mut by_scene: BTreeMap<&str, Tally> = BTreeMap::new();
    let mut share_t: BTreeMap<String, usize> = BTreeMap::new();
    let mut share_p: BTreeMap<String, usize> = BTreeMap::new();
    let bucket = |id: &str| id.strip_prefix("a:").map(|a| a.split('/').next().unwrap().to_string()).unwrap_or_else(|| "未确认".to_string());
    for c in dump["history"].as_array().unwrap() {
        let h = c["commit"].as_str().unwrap();
        let Some((scene, snap)) = s.snaps.get(h) else { continue };
        let added = added_lines(&s.repo, h);
        for f in c["files"].as_array().unwrap() {
            let rel = f["rel"].as_str().unwrap();
            let who = f["who"].as_array().unwrap();
            let lns = &added[rel];
            assert_eq!(lns.len(), who.len(), "{scene} {rel}");
            for (ln, p) in lns.iter().zip(who) {
                let l = &snap[rel][ln - 1];
                if l.text.trim().is_empty() {
                    continue;
                }
                let p = p.as_str().unwrap();
                if std::env::var_os("CREDITIA_BENCH_VERBOSE").is_some() && (l.who == "me" || l.who == "gen") == p.starts_with("a:") || (l.who != "me" && l.who != "gen" && !p.starts_with(&format!("a:{}", harness(l.who)))) {
                    println!("  ✗ {scene} {rel}:{ln} 真实 {} 算出 {p}  {}", l.who, l.text.trim());
                }
                by_scene.entry(scene).or_default().add(l.who, p);
                *share_t.entry(if l.who == "me" || l.who == "gen" { "未确认".into() } else { harness(l.who).to_string() }).or_default() += 1;
                *share_p.entry(bucket(p)).or_default() += 1;
            }
        }
    }
    // ---- 存活：HEAD 上的每一行
    let mut surv = Tally::default();
    for f in dump["survival"].as_array().unwrap() {
        let path = f["path"].as_str().unwrap();
        let Some(lines) = s.files.get(path) else { continue };
        for (l, p) in lines.iter().zip(f["who"].as_array().unwrap()) {
            if !l.text.trim().is_empty() {
                surv.add(l.who, p.as_str().unwrap());
            }
        }
    }

    let mut all = Tally::default();
    println!("\n场景                         AI行  工具对%  模型对%  错给别的AI%  漏掉%   非AI行  误认%  AI算给人%");
    for (k, t) in &by_scene {
        all.merge(t);
        println!("{:<26} {:>6} {}  {}  {}       {}  {:>6}  {}  {}", k, t.ai, pct(t.exact + t.tool_only, t.ai), pct(t.exact, t.ai), pct(t.wrong_tool, t.ai), pct(t.missed, t.ai), t.human, pct(t.claimed, t.human), pct(t.as_human, t.ai));
    }
    println!("{:<26} {:>6} {}  {}  {}       {}  {:>6}  {}  {}", "历史合计", all.ai, pct(all.exact + all.tool_only, all.ai), pct(all.exact, all.ai), pct(all.wrong_tool, all.ai), pct(all.missed, all.ai), all.human, pct(all.claimed, all.human), pct(all.as_human, all.ai));
    println!("{:<26} {:>6} {}  {}  {}       {}  {:>6}  {}  {}", "当前代码（存活）", surv.ai, pct(surv.exact + surv.tool_only, surv.ai), pct(surv.exact, surv.ai), pct(surv.wrong_tool, surv.ai), pct(surv.missed, surv.ai), surv.human, pct(surv.claimed, surv.human), pct(surv.as_human, surv.ai));
    let tot: usize = share_t.values().sum();
    println!("\n份额（历史，非空行）  真实    算出");
    let mut keys: Vec<&String> = share_t.keys().chain(share_p.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut l1 = 0.0;
    for k in keys {
        let (a, b) = (*share_t.get(k).unwrap_or(&0), *share_p.get(k).unwrap_or(&0));
        l1 += (a as f64 - b as f64).abs() / tot as f64;
        println!("{:<20} {}  {}", k, pct(a, tot), pct(b, tot));
    }
    println!("份额总偏差（L1/2）：{:.1} 个百分点\n", l1 / 2.0 * 100.0);

    let out = json!({
        "scenes": by_scene.iter().map(|(k, t)| (k.to_string(), t.json())).collect::<BTreeMap<_, _>>(),
        "history": all.json(), "survival": surv.json(), "shareError": (l1 / 2.0 * 1000.0).round() / 10.0,
        "shares": { "truth": share_t, "computed": share_p },
    });
    if let Ok(p) = std::env::var("CREDITIA_BENCH_OUT") {
        fs::write(p, serde_json::to_string_pretty(&out).unwrap()).unwrap();
    }
    let _ = fs::remove_dir_all(&s.root);
    // 底线（日志丢失这个场景本来就找不回来，已经算在里面）
    assert!(all.claimed * 100 <= all.human, "人写的行被算给 AI：{}/{}", all.claimed, all.human);
    assert!(all.wrong_tool * 100 <= all.ai, "错给别的 AI：{}/{}", all.wrong_tool, all.ai);
    assert!((all.exact + all.tool_only) * 100 >= all.ai * 95, "工具对的比例：{}/{}", all.exact + all.tool_only, all.ai);
    assert!(l1 / 2.0 <= 0.04, "份额总偏差 {:.1} 个百分点", l1 / 2.0 * 100.0);
    // 在 agent 里提交的手写不能被章整个算给 agent
    let t = &by_scene["22 在 agent 里提交的手写"];
    assert_eq!(t.claimed, 0, "22：人写的行被算给 AI");
    assert!((t.exact + t.tool_only) * 100 >= t.ai * 95, "22：AI 行 {}/{}", t.exact + t.tool_only, t.ai);
    // AI 写的行一行都不能算给人（网页合并的作者、找不到记录的行）
    assert_eq!(all.as_human, 0, "AI 写的行被算给了人");
    let t = &by_scene["24 GitHub 网页 squash 合并"];
    assert!((t.exact + t.tool_only) * 100 >= t.ai * 95, "24：AI 行 {}/{}", t.exact + t.tool_only, t.ai);
}
