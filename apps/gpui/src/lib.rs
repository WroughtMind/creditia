//! UI data adapters. These preserve the existing archive vocabulary and never
//! infer contribution from commits, elapsed time, or missing evidence.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub exists: bool,
    #[serde(default)]
    pub has_credits: bool,
    #[serde(default)]
    pub evidence: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Zh,
    En,
}
impl Language {
    pub fn code(&self) -> &'static str {
        match self { Self::Zh => "zh", Self::En => "en" }
    }
    pub fn text(&self, zh: &'static str, en: &'static str) -> &'static str {
        match self { Self::Zh => zh, Self::En => en }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub language: Language,
    pub appearance: Appearance,
    pub reduced_motion: bool,
    pub last_project: Option<String>,
}

impl Preferences {
    pub fn read(settings: &Value) -> Self {
        serde_json::from_value(settings["gpui"].clone()).unwrap_or_default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contribution {
    pub name: String,
    pub model: Option<String>,
    pub lines_added: u64,
    pub verified_lines: u64,
}

/// Each archive contribution remains separate, including unattributed rows.
/// Only verified evidence is counted as verified; missing values remain zero.
pub fn contributions(raw: &Value) -> Vec<Contribution> {
    let mut rows: Vec<_> = raw["breakdown"].as_array().into_iter().flatten().map(|row| {
        let id = row["contributor"].as_str().unwrap_or("unattributed");
        let contributor = raw["contributors"].as_array().into_iter().flatten()
            .find(|c| c["id"].as_str() == Some(id));
        Contribution {
            name: contributor.and_then(|c| c["name"].as_str().or_else(|| c["harness"].as_str()))
                .unwrap_or(id).to_string(),
            model: contributor.and_then(|c| c["model"].as_str()).map(str::to_string),
            lines_added: row["linesAdded"].as_u64().unwrap_or(0),
            verified_lines: row["byEvidence"]["verified"]["linesAdded"].as_u64().unwrap_or(0),
        }
    }).collect();
    rows.sort_by(|a, b| b.lines_added.cmp(&a.lines_added).then_with(|| a.name.cmp(&b.name)));
    rows
}

pub fn progress_text(progress: &Value, language: &Language) -> String {
    let stage = progress["stage"].as_str().unwrap_or("");
    let label = match stage {
        "history" => language.text("读取提交历史", "Reading commit history"),
        "logs" => language.text("读取工具日志", "Reading tool logs"),
        "attribute" => language.text("逐个提交归因", "Attributing commits"),
        "blame" => language.text("追溯当前文件", "Tracing current files"),
        _ => stage,
    };
    let done = progress["done"].as_u64().unwrap_or(0);
    let total = progress["total"].as_u64().unwrap_or(0);
    if total > 0 { format!("{label} · {done}/{total}") }
    else if done > 0 { format!("{label} · {done}") }
    else { label.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_and_non_verified_contributions_are_preserved() {
        let rows = contributions(&json!({
            "contributors": [{"id":"codex/model", "harness":"codex", "model":"model"}],
            "breakdown": [
                {"contributor":"unattributed", "linesAdded":12},
                {"contributor":"codex/model", "linesAdded":5,
                 "byEvidence":{"verified":{"linesAdded":2},"inferred":{"linesAdded":3}}}
            ]
        }));
        assert_eq!(rows[0].name, "unattributed");
        assert_eq!(rows[0].lines_added, 12);
        assert_eq!(rows[0].verified_lines, 0);
        assert_eq!(rows[1].verified_lines, 2);
        assert_eq!(rows[1].model.as_deref(), Some("model"));
    }

    #[test]
    fn preference_defaults_do_not_reinterpret_existing_artwork_theme() {
        let preferences = Preferences::read(&json!({"theme":"ink", "logo":"star"}));
        assert_eq!(preferences.appearance, Appearance::System);
        assert_eq!(preferences.language, Language::Zh);
        let recovered = Preferences::read(&json!({"gpui":{"language":"en","appearance":"dark"}}));
        assert_eq!(recovered.language, Language::En);
        assert_eq!(recovered.appearance, Appearance::Dark);
    }

    #[test]
    fn unknown_progress_total_is_not_reported_as_completion() {
        assert_eq!(progress_text(&json!({"stage":"logs","done":8,"total":0}), &Language::En), "Reading tool logs · 8");
    }
}
