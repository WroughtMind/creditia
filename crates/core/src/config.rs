//! 项目配置：`.creditia/config.json`（repo 内）+ 作品库级别的默认值。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub name: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub note: String,
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
    /// 是否公开展示用户确认的人员署名
    #[serde(default)]
    pub people: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub name: String,
    pub repo: String,
    pub branch: String,
    #[serde(default)]
    pub remotes: Vec<String>,
    /// 用户明确关联的其他工作副本或已移动的旧路径。
    #[serde(default)]
    pub paths: Vec<String>,
    /// 由用户确认的人员署名；不由提交数推断角色。
    #[serde(default)]
    pub people: Vec<Person>,
    /// agent 邮箱 → harness
    #[serde(default)]
    pub agent_emails: HashMap<String, String>,
    #[serde(default)]
    pub bot_emails: Vec<String>,
    /// 证据覆盖分段 [(名称, 起, 止)]
    #[serde(default)]
    pub segments: Vec<(String, String, String)>,
    #[serde(default)]
    pub privacy: Privacy,
    #[serde(default)]
    pub display: Display,
    /// 配置结构版本
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
            paths: vec![],
            people: vec![],
            agent_emails: default_agent_emails(),
            bot_emails: vec!["41898282+github-actions[bot]@users.noreply.github.com".into()],
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
    /// 展示用的人名：别名优先
    pub fn display_name(&self, name: &str) -> String {
        self.privacy.aliases.get(name).cloned().unwrap_or_else(|| name.to_string())
    }
}
