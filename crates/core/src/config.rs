//! 项目配置：`.creditia/config.json`（repo 内）+ 作品库级别的默认值。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Director {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub emails: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Privacy {
    /// 隐藏 PR / 提交标题，输出 `PR #N`
    #[serde(default)]
    pub hide_titles: bool,
    /// 隐藏文件名
    #[serde(default)]
    pub hide_paths: bool,
    /// 人名 → 对外显示的别名
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    /// 是否在展示里写出主导者（默认不写，用户觉得土）
    #[serde(default)]
    pub director: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub name: String,
    pub repo: String,
    pub branch: String,
    #[serde(default)]
    pub remotes: Vec<String>,
    /// 判断路径是否属于本项目（小写匹配）
    #[serde(default)]
    pub markers: Vec<String>,
    #[serde(default)]
    pub exclude_markers: Vec<String>,
    #[serde(default)]
    pub directors: Vec<Director>,
    /// agent 邮箱 → harness
    #[serde(default)]
    pub agent_emails: HashMap<String, String>,
    #[serde(default)]
    pub bot_emails: Vec<String>,
    /// 分支前缀 → harness（如 `codex/`）
    #[serde(default)]
    pub branch_prefixes: HashMap<String, String>,
    /// 证据覆盖分段 [(名称, 起, 止)]
    #[serde(default)]
    pub segments: Vec<(String, String, String)>,
    #[serde(default)]
    pub privacy: Privacy,
    #[serde(default)]
    pub display: Display,
    /// schema 版本，升级时迁移
    #[serde(default = "one")]
    pub version: u32,
}

fn one() -> u32 { 1 }

impl Default for Config {
    fn default() -> Self {
        Config {
            name: String::new(),
            repo: String::new(),
            branch: "main".into(),
            remotes: vec![],
            markers: vec![],
            exclude_markers: vec![],
            directors: vec![],
            agent_emails: default_agent_emails(),
            bot_emails: vec!["41898282+github-actions[bot]@users.noreply.github.com".into()],
            branch_prefixes: [("codex/", "codex"), ("cursor/", "cursor"), ("claude/", "claude-code"), ("grok/", "grok"), ("kimi/", "kimi")]
                .iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            segments: vec![],
            privacy: Privacy::default(),
            display: Display::default(),
            version: 1,
        }
    }
}

/// 已知的 agent 提交邮箱。不写死用户是谁，只做工具映射。
pub fn default_agent_emails() -> HashMap<String, String> {
    [
        ("cursoragent@cursor.com", "cursor"),
        ("noreply@anthropic.com", "claude-code"),
        ("codex@users.noreply.github.com", "codex"),
        ("codex@openai.com", "codex"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect()
}

impl Config {
    pub fn load(path: &Path) -> Result<Config, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
        serde_json::from_str(&s).map_err(|e| format!("解析 {} 失败：{e}", path.display()))
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let s = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, s + "\n").map_err(|e| e.to_string())
    }
    pub fn director_emails(&self) -> Vec<String> {
        self.directors.iter().flat_map(|d| d.emails.iter().map(|e| e.to_lowercase())).collect()
    }
    /// 展示用的人名：别名优先
    pub fn display_name(&self, name: &str) -> String {
        self.privacy.aliases.get(name).cloned().unwrap_or_else(|| name.to_string())
    }
}
