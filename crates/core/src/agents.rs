//! Public agent entry points. A catalogue entry is not a claim of native history support.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub harness: String,
    pub skills_dir: String,
    pub history: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_note_en: Option<String>,
}

#[derive(Deserialize)]
struct Catalogue {
    agents: Vec<Agent>,
}

pub fn all() -> &'static [Agent] {
    static AGENTS: OnceLock<Vec<Agent>> = OnceLock::new();
    AGENTS.get_or_init(|| {
        serde_json::from_str::<Catalogue>(include_str!("../../../packages/schema/agents.json"))
            .expect("bundled agent catalogue")
            .agents
    })
}

pub fn by_id(id: &str) -> Option<&'static Agent> {
    all().iter().find(|a| a.id == id)
}

/// Stable harness IDs take precedence: e.g. antigravity is the existing CLI ID.
/// --agent selects public entry IDs and can distinguish Antigravity IDE/CLI.
pub fn canonical_harness(id: &str) -> Option<&'static str> {
    crate::model::Src::all()
        .iter()
        .find(|s| s.harness() == id)
        .map(|s| s.harness())
        .or_else(|| by_id(id).map(|a| a.harness.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_public_agent_has_a_distinct_id_and_a_resolvable_executor() {
        let mut ids = std::collections::HashSet::new();
        for a in all() {
            assert!(ids.insert(&a.id), "{}", a.id);
            assert!(
                crate::model::Src::from_harness(&a.harness).is_some(),
                "{}",
                a.id
            );
            assert!(!a.name.is_empty());
            assert!(!std::path::Path::new(&a.skills_dir).is_absolute());
            assert!(!a.skills_dir.split('/').any(|p| p == ".."));
        }
    }
    #[test]
    fn editor_and_cli_names_are_not_interchanged() {
        assert_eq!(by_id("antigravity").unwrap().harness, "antigravity-ide");
        assert_eq!(by_id("antigravity-cli").unwrap().harness, "antigravity");
        assert_eq!(canonical_harness("antigravity"), Some("antigravity"));
        assert_eq!(canonical_harness("kimi-code-cli"), Some("kimi"));
        assert_eq!(canonical_harness("not-an-agent"), None);
    }
}
