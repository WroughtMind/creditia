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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub language: Language,
    pub appearance: Appearance,
    pub reduced_motion: bool,
    pub last_project: Option<String>,
    pub named: bool,
    pub style: String,
    pub view: String,
    pub export_format: String,
    pub export_size: String,
    pub auto_analyze: bool,
    pub projects: std::collections::BTreeMap<String, ProjectPreferences>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectPreferences {
    pub group: String,
    pub auto_analyze: Option<bool>,
    pub variants: std::collections::BTreeMap<String,String>,
    pub order: Vec<String>,
}
impl Default for ProjectPreferences {
    fn default()->Self {Self{group:"auto".into(),auto_analyze:None,variants:Default::default(),order:vec![]}}
}
impl Default for Preferences {
    fn default()->Self {Self{language:Language::Zh,appearance:Appearance::System,reduced_motion:false,last_project:None,named:true,style:"minimal-light".into(),view:"wall".into(),export_format:"png".into(),export_size:"native".into(),auto_analyze:false,projects:Default::default()}}
}
impl Preferences {
    pub fn read(settings: &Value) -> Self {
        if settings["global"].is_object() {
            let mut global=settings["global"].clone();
            global["language"]=global["lang"].clone();
            global["projects"]=settings["projects"].clone();
            serde_json::from_value(global).unwrap_or_default()
        } else {serde_json::from_value(settings["gpui"].clone()).unwrap_or_default()}
    }
    pub fn shared_patch(&self)->Value {
        serde_json::json!({"global":{"lang":self.language.code(),"appearance":self.appearance,"reducedMotion":self.reduced_motion,"named":self.named,"style":self.style,"view":self.view,"exportFormat":self.export_format,"exportSize":self.export_size,"lastProject":self.last_project,"autoAnalyze":self.auto_analyze},"projects":self.projects})
    }
    pub fn project(&self,id:Option<&str>)->ProjectPreferences {id.and_then(|id|self.projects.get(id)).cloned().unwrap_or_default()}
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

/// Capture owned renderer inputs before displaying a save dialog. Subsequent
/// project, style, layout, or language changes cannot alter the pending export.
pub fn export_snapshot(
    raw: &Value,
    catalog: &Value,
    selection: (usize, usize, usize),
    language: &Language,
    format: &str,
    size: &str,
) -> Result<Value, String> {
    use serde_json::json;
    if !["png", "svg", "pdf", "gif", "html", "md", "txt", "json", "embed"].contains(&format)
        || !catalog["formats"].as_array().is_some_and(|formats| formats.iter().any(|item| item["id"] == format)) {
        return Err("The selected export format is unavailable".into());
    }
    if !catalog["sizes"].as_array().is_some_and(|sizes| sizes.iter().any(|item| item["id"] == size)) {
        return Err("The selected export size is unavailable".into());
    }
    let (style_index, piece_index, variant_index) = selection;
    let style = catalog["styles"].as_array().and_then(|styles| styles.get(style_index))
        .ok_or("The selected style is unavailable")?;
    let piece = style["pieces"].as_array().and_then(|pieces| pieces.get(piece_index))
        .ok_or("The selected piece is unavailable")?;
    if format == "gif" && piece["animated"].as_bool() != Some(true) {
        return Err("This piece does not support GIF animation".into());
    }
    let style_id = style["id"].as_str().ok_or("The selected style has no id")?;
    let piece_id = piece["id"].as_str().ok_or("The selected piece has no id")?;
    let mut request = json!({
        "version":1,"id":"native-export","op":"export","raw":raw,
        "style":style_id,"piece":piece_id,"lang":language.code(),
        "group":"auto","named":true,"format":format,"size":size,
    });
    if let Some(variants) = piece["variants"].as_array().filter(|items| !items.is_empty()) {
        let variant = variants.get(variant_index).and_then(|item| item["id"].as_str())
            .ok_or("The selected layout is unavailable")?;
        request["variant"] = json!(variant);
    }
    Ok(request)
}

pub fn export_with_preferences(raw:&Value,catalog:&Value,selection:(usize,usize,usize),preferences:&Preferences,format:&str,size:&str,transparent:bool,font_mode:&str)->Result<Value,String>{
    let mut request=export_snapshot(raw,catalog,selection,&preferences.language,format,size)?;
    request["group"]=serde_json::json!(preferences.project(preferences.last_project.as_deref()).group);
    request["named"]=serde_json::json!(preferences.named);
    request["transparent"]=serde_json::json!(transparent);
    request["fontMode"]=serde_json::json!(font_mode);
    Ok(request)
}

pub fn export_filename(request: &Value) -> String {
    let component = |value: &str| {
        let cleaned: String = value.chars().map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }
        }).scan(0usize, |bytes, c| {
            *bytes += c.len_utf8();
            (*bytes <= 60).then_some(c)
        }).collect();
        let cleaned = cleaned.trim_matches(['.', ' ']);
        if cleaned.is_empty() { "creditia".to_string() } else { cleaned.to_string() }
    };
    format!("{}-{}-{}.{}",
        component(request["raw"]["project"]["name"].as_str().unwrap_or("creditia")),
        component(request["piece"].as_str().unwrap_or("artwork")),
        component(request["style"].as_str().unwrap_or("style")),
        request["format"].as_str().unwrap_or("png"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unicode_export_names_fit_filesystem_component_limits() {
        let name = export_filename(&json!({
            "raw":{"project":{"name":"😀".repeat(60)}},
            "piece":"作品".repeat(60),"style":"水墨".repeat(60),"format":"png"
        }));
        assert!(name.len() <= 255);
        assert!(name.ends_with(".png"));
        assert!(name.starts_with('😀'));
    }

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

    #[test]
    fn export_captures_archive_and_selection_before_user_changes_them() {
        let mut raw = json!({"project":{"name":"../项目:a"}, "totals":{"linesAdded":12}});
        let catalog = json!({
            "formats":[{"id":"png"},{"id":"gif"}], "sizes":[{"id":"square"}],
            "styles":[{"id":"ink","pieces":[{"id":"tier","animated":true,"variants":[{"id":"rows"},{"id":"center"}]}]}]
        });
        let request = export_snapshot(&raw, &catalog, (0,0,1), &Language::En, "png", "square").unwrap();
        raw["totals"]["linesAdded"] = json!(99);
        assert_eq!(request["raw"]["totals"]["linesAdded"], 12);
        assert_eq!(request["variant"], "center");
        assert_eq!(request["lang"], "en");
        assert_eq!(request["size"], "square");
        assert_eq!(request["group"], "auto");
        assert_eq!(request["named"], true);
        assert!(!export_filename(&request).contains('/'));
        assert!(!export_filename(&request).contains(':'));
    }

    #[test]
    fn export_rejects_unavailable_animation_and_stale_layout() {
        let catalog = json!({
            "formats":[{"id":"png"},{"id":"gif"}], "sizes":[{"id":"native"}],
            "styles":[{"id":"ink","pieces":[{"id":"receipt","animated":false,"variants":[{"id":"slip"}]}]}]
        });
        assert!(export_snapshot(&json!({}), &catalog, (0,0,0), &Language::Zh, "gif", "native").is_err());
        assert!(export_snapshot(&json!({}), &catalog, (0,0,4), &Language::Zh, "png", "native").is_err());
        assert!(export_snapshot(&json!({}), &catalog, (0,0,0), &Language::Zh, "png", "unknown").is_err());
    }
}

pub mod ui_model;
pub mod renderer;
pub mod native_preview;
pub mod app;
