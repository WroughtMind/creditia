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
    /// 是否在展示里写出人名（默认不写：人的贡献用做了什么来呈现）
    #[serde(default)]
    pub named: bool,
    /// 落款：用户主动填写才有
    #[serde(default)]
    pub sign: String,
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
    /// 你自己（本机用户）的提交身份。用来区分「我的提交里可能有 agent 写的代码」和「别人的提交」，
    /// 不会在任何展示里写成「主导者」。旧字段名 directors 照样能读
    #[serde(default, alias = "me")]
    pub directors: Vec<Director>,
    /// agent 邮箱 → harness
    #[serde(default)]
    pub agent_emails: HashMap<String, String>,
    #[serde(default)]
    pub bot_emails: Vec<String>,
    /// 分支前缀 → harness（如 `codex/`）
    #[serde(default)]
    pub branch_prefixes: HashMap<String, String>,
    /// 归因口径：strict 只认逐行证据和明确声明；standard（默认）再加两种推断：同一改动块内的延伸、分支前缀；loose 再加同时间窗口的会话。推断全部单独记账
    #[serde(default = "standard")]
    pub level: String,
    /// 本项目的其他路径（别的电脑、已删除的临时目录、搬家前的位置）：日志里的这些路径也算本项目
    #[serde(default)]
    pub paths: Vec<String>,
    /// 证据覆盖分段 [(名称, 起, 止)]
    #[serde(default)]
    pub segments: Vec<(String, String, String)>,
    /// 不算发布版本的 tag（通配，`**` 跨目录）。默认排除 `archive/**`：存档用的 tag 不是版本
    #[serde(default = "default_ignore_tags")]
    pub ignore_tags: Vec<String>,
    #[serde(default)]
    pub privacy: Privacy,
    #[serde(default)]
    pub display: Display,
    /// schema 版本，升级时迁移
    #[serde(default = "one")]
    pub version: u32,
}

fn one() -> u32 { 1 }
fn standard() -> String { "standard".into() }
pub fn default_ignore_tags() -> Vec<String> { vec!["archive/**".into()] }

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
            bot_emails: vec![
                "41898282+github-actions[bot]@users.noreply.github.com".into(),
                "49699333+dependabot[bot]@users.noreply.github.com".into(),
                "29139614+renovate[bot]@users.noreply.github.com".into(),
            ],
            level: "standard".into(),
            paths: vec![],
            branch_prefixes: [("codex/", "codex"), ("cursor/", "cursor"), ("claude/", "claude-code"), ("grok/", "grok"), ("kimi/", "kimi"), ("copilot/", "copilot"), ("devin/", "devin"), ("jules/", "jules"), ("opencode/", "opencode")]
                .iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            segments: vec![],
            ignore_tags: default_ignore_tags(),
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
        ("198982749+copilot@users.noreply.github.com", "copilot"),
        ("167198135+copilot@users.noreply.github.com", "copilot"),
        ("158243242+devin-ai-integration[bot]@users.noreply.github.com", "devin"),
        ("161369871+google-labs-jules[bot]@users.noreply.github.com", "jules"),
        ("noreply@aider.chat", "aider"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect()
}

impl Config {
    pub fn load(path: &Path) -> Result<Config, String> {
        let s = std::fs::read_to_string(path).map_err(|e| crate::tr!("读取 {} 失败：{e}", "couldn't read {}: {e}", path.display()))?;
        serde_json::from_str(&s).map_err(|e| crate::tr!("解析 {} 失败：{e}", "couldn't parse {}: {e}", path.display()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_file_gets_defaults() {
        let c: Config = serde_json::from_str(r#"{"name":"demo","repo":"/tmp/demo","branch":"main"}"#).unwrap();
        assert_eq!(c.level, "standard");
        assert_eq!(c.version, 1);
        assert_eq!(c.ignore_tags, ["archive/**"]);
        assert!(!c.display.named);
        assert!(c.directors.is_empty());
    }

    #[test]
    fn own_ignore_tags_replace_default() {
        let c: Config = serde_json::from_str(r#"{"name":"d","repo":"/d","branch":"main","ignoreTags":["nightly-*"]}"#).unwrap();
        assert_eq!(c.ignore_tags, ["nightly-*"]);
        let none: Config = serde_json::from_str(r#"{"name":"d","repo":"/d","branch":"main","ignoreTags":[]}"#).unwrap();
        assert!(none.ignore_tags.is_empty());
    }

    #[test]
    fn me_is_an_alias_of_directors() {
        let c: Config = serde_json::from_str(r#"{"name":"d","repo":"/d","branch":"main","me":[{"id":"me","name":"Me","emails":["Me@Example.com","second@example.com"]}]}"#).unwrap();
        assert_eq!(c.director_emails(), ["me@example.com", "second@example.com"]);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("creditia-config-test-{}", std::process::id()));
        let p = dir.join("nested/config.json");
        let mut c = Config { name: "demo".into(), repo: "/tmp/demo".into(), ..Config::default() };
        c.privacy.aliases.insert("Real Name".into(), "R.".into());
        c.save(&p).unwrap();
        let back = Config::load(&p).unwrap();
        assert_eq!(back.name, "demo");
        assert_eq!(back.agent_emails.get("noreply@anthropic.com").map(String::as_str), Some("claude-code"));
        assert_eq!(back.branch_prefixes.get("codex/").map(String::as_str), Some("codex"));
        assert_eq!(back.ignore_tags, ["archive/**"]);
        assert_eq!(back.display_name("Real Name"), "R.");
        assert_eq!(back.display_name("Other"), "Other");
        // 文件写成 camelCase
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(raw.contains("\"ignoreTags\"") && raw.contains("\"agentEmails\""), "{raw}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_reports_bad_files() {
        let dir = std::env::temp_dir().join(format!("creditia-config-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(Config::load(&dir.join("missing.json")).unwrap_err().contains("读取"));
        std::fs::write(dir.join("bad.json"), "{ not json").unwrap();
        assert!(Config::load(&dir.join("bad.json")).unwrap_err().contains("解析"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
