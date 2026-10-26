//! CLI、桌面分析共用同一组读取器，新增来源不会只出现在 doctor 或发现列表。

use crate::{Activity, Health, Stats};
use creditia_core::model::{PathMap, Src, Store};
use std::path::PathBuf;

/// A readable source can contain session information without any successful edit
/// bodies. Keep that distinction in both CLI JSON and the desktop settings.
#[derive(serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReadScope {
    Edits,
    Sessions,
    Capture,
    Diagnostic,
}

#[derive(serde::Serialize)]
pub struct Diagnostic {
    #[serde(flatten)]
    pub health: Health,
    pub scope: ReadScope,
}
impl std::ops::Deref for Diagnostic {
    type Target = Health;
    fn deref(&self) -> &Health {
        &self.health
    }
}

pub fn scan_all(store: &mut Store, pm: &PathMap, roots: &[String], st: &mut Stats) {
    // A previous ACP capture is derived evidence, never a native record that
    // may suppress a more complete native/host reader during this scan.
    store.groups.retain(|key, _| !key.starts_with("acp:"));
    crate::codex::scan(store, pm, st);
    crate::claude::scan(store, pm, st);
    crate::cursor::scan(store, pm, st);
    crate::cursor_cli::scan(store, pm, st);
    crate::junie::scan(store, pm, st);
    crate::kiro::scan(store, pm, st);
    crate::kiro_sessions::scan(store, pm, st);
    crate::grok::scan(store, pm, st);
    crate::kimi::scan(store, pm, st);
    crate::gemini::scan(store, pm, roots, st);
    crate::qwen::scan(store, pm, st);
    crate::opencode::scan(store, pm, st);
    crate::cline::scan(store, pm, st);
    crate::aider::scan(store, pm, roots, st);
    crate::aiderdesk::scan(store, pm, roots, st);
    crate::dexto::scan(store, pm, roots, st);
    crate::pochi::scan(store, pm, st);
    crate::forgecode::scan(store, pm, st);
    crate::kode::scan(store, pm, st);
    crate::mux::scan(store, pm, st);
    crate::reasonix::scan(store, pm, st);
    crate::jazz::scan(store, pm, st);
    crate::deepagents::scan(store, pm, st);
    crate::fx::scan(store, pm, st);
    crate::astrbot::scan(store, pm, roots, st);
    crate::adal::scan(store, pm, roots, st);
    crate::firebender::scan(store, pm, roots, st);
    crate::moxby::scan(store, pm, st);
    crate::rovodev::scan(store, pm, roots, st);
    crate::lingma::scan(store, pm, st);
    crate::eve::scan(store, pm, roots, st);
    crate::posit::scan(store, pm, roots, st);
    crate::tinycloud::scan(store, pm, st);
    crate::zencoder::scan(store, pm, st);
    crate::codestudio::scan(store, pm, st);
    crate::terramind::scan(store, pm, st);
    crate::zenflow::scan(store, pm, st);
    crate::sarvam::scan(store, pm, st);
    crate::codearts::scan(store, pm, st);
    crate::copilot::scan(store, pm, st);
    crate::amp::scan(store, pm, st);
    crate::trae::scan(store, pm, st);
    crate::trae_native::scan(store, pm, st);
    crate::crush::scan(store, pm, roots, st);
    crate::goose::scan(store, pm, st);
    crate::cont::scan(store, pm, st);
    crate::zed::scan(store, pm, st);
    crate::amazonq::scan(store, pm, st);
    crate::antigravity::scan(store, pm, st);
    crate::zcode::scan(store, pm, st);
    for src in [Src::Pi, Src::Omp, Src::CommandCode] {
        crate::pi::scan(src, store, pm, st);
    }
    crate::hermes::scan(store, pm, st);
    crate::openclaw::scan(store, pm, st);
    crate::muse::scan(store, pm, st);
    crate::vibe::scan(store, pm, st);
    crate::minimax::scan(store, pm, st);
    crate::cli_agents::scan(store, pm, st);
    crate::ide_agents::scan(store, pm, st);
    crate::codeflicker::scan(store, pm, st);
    crate::comate::scan(store, pm, st);
    crate::tabnine_ide::scan(store, pm, st);
    crate::cascade::scan(store, pm, st);
    crate::warp::scan(store, pm, st);
    crate::traces::scan(store, pm, st);
    crate::cloud::scan(store, pm, st);
    crate::cloud_native::scan(store, pm, st);
    // Bolt/Lovable rebuild their own verified native import sets; cloud.rs only
    // keeps the remaining session-metadata readers.
    crate::bolt::scan(store, pm, st);
    crate::lovable::scan(store, pm, st);
    // The native ACP import supersedes CLI-hook copies of the same sessions.
    crate::devin_cloud::scan(store, pm, st);
    crate::editor_history::scan(store, pm, st);
    crate::dsh::scan(store, pm, st);
    crate::longtail::scan(store, pm, st);
    crate::remaining_native::scan(store, pm, roots, st);
    // Host and protocol captures run after native readers so the same session
    // and the same written line aren't counted again through another entry point.
    crate::t3::scan(store, pm, st);
    crate::acp::scan(store, pm, st);
}

pub fn discover_all() -> Vec<Activity> {
    let mut all = vec![];
    for discover in [
        crate::codex::discover,
        crate::claude::discover,
        crate::cursor::discover,
        crate::cursor_cli::discover,
        crate::junie::discover,
        crate::kiro::discover,
        crate::kiro_sessions::discover,
        crate::grok::discover,
        crate::kimi::discover,
        crate::gemini::discover,
        crate::qwen::discover,
        crate::opencode::discover,
        crate::cline::discover,
        crate::copilot::discover,
        crate::amp::discover,
        crate::trae::discover,
        crate::trae_native::discover,
        crate::crush::discover,
        crate::goose::discover,
        crate::cont::discover,
        crate::zed::discover,
        crate::amazonq::discover,
        crate::antigravity::discover,
        crate::zcode::discover,
        crate::hermes::discover,
        crate::aiderdesk::discover,
        crate::dexto::discover,
        crate::pochi::discover,
        crate::forgecode::discover,
        crate::kode::discover,
        crate::mux::discover,
        crate::reasonix::discover,
        crate::jazz::discover,
        crate::deepagents::discover,
        crate::fx::discover,
        crate::astrbot::discover,
        crate::adal::discover,
        crate::firebender::discover,
        crate::moxby::discover,
        crate::rovodev::discover,
        crate::lingma::discover,
        crate::eve::discover,
        crate::posit::discover,
        crate::tinycloud::discover,
        crate::zencoder::discover,
        crate::codestudio::discover,
        crate::terramind::discover,
        crate::zenflow::discover,
        crate::sarvam::discover,
        crate::codearts::discover,
        crate::openclaw::discover,
        crate::muse::discover,
        crate::vibe::discover,
        crate::minimax::discover,
        crate::cli_agents::discover,
        crate::ide_agents::discover,
        crate::codeflicker::discover,
        crate::comate::discover,
        crate::tabnine_ide::discover,
        crate::cascade::discover,
        crate::warp::discover,
        crate::traces::discover,
        crate::cloud::discover,
        crate::cloud_native::discover,
        crate::bolt::discover,
        crate::lovable::discover,
        crate::devin_cloud::discover,
        crate::editor_history::discover,
        crate::dsh::discover,
        crate::t3::discover,
        crate::longtail::discover,
        crate::remaining_native::discover,
        crate::acp::discover,
    ] {
        all.extend(discover());
    }
    for src in [Src::Pi, Src::Omp, Src::CommandCode] {
        all.extend(crate::pi::discover(src));
    }
    all
}

pub fn doctor_all() -> Vec<Diagnostic> {
    let mut all = vec![
        crate::codex::doctor(),
        crate::claude::doctor(),
        crate::cursor::doctor(),
        crate::cursor_cli::doctor(),
        crate::junie::doctor(),
        crate::kiro_sessions::doctor(),
        crate::grok::doctor(),
        crate::kimi::doctor(),
        crate::gemini::doctor(Src::Gemini),
        crate::qwen::doctor(),
        crate::cline::doctor(Src::Cline),
        crate::cline::doctor(Src::Roo),
        crate::cline::doctor(Src::Kilo),
        crate::aider::doctor(),
        crate::aiderdesk::doctor(),
        crate::dexto::doctor(),
        crate::pochi::doctor(),
        crate::forgecode::doctor(),
        crate::kode::doctor(),
        crate::mux::doctor(),
        crate::reasonix::doctor(),
        crate::jazz::doctor(),
        crate::deepagents::doctor(),
        crate::fx::doctor(),
        crate::astrbot::doctor(),
        crate::adal::doctor(),
        crate::firebender::doctor(),
        crate::moxby::doctor(),
        crate::rovodev::doctor(),
        crate::lingma::doctor(),
        crate::eve::doctor(),
        crate::posit::doctor(),
        crate::tinycloud::doctor(),
        crate::zencoder::doctor(),
        crate::codestudio::doctor(),
        crate::terramind::doctor(),
        crate::zenflow::doctor(),
        crate::sarvam::doctor(),
        crate::codearts::doctor(),
        crate::amp::doctor(),
        crate::trae::doctor(),
        crate::trae_native::doctor(),
        crate::crush::doctor(),
        crate::goose::doctor(),
        crate::cont::doctor(),
        crate::zed::doctor(),
        crate::amazonq::doctor(),
        crate::antigravity::doctor(),
        crate::zcode::doctor(),
        crate::hermes::doctor(),
        crate::openclaw::doctor(),
        crate::muse::doctor(),
        crate::vibe::doctor(),
        crate::minimax::doctor(),
        crate::codeflicker::doctor(),
        crate::comate::doctor(),
        crate::t3::doctor(),
    ];
    all.extend(crate::opencode::doctor());
    all.extend(crate::copilot::doctor());
    for src in [Src::Pi, Src::Omp, Src::CommandCode] {
        all.push(crate::pi::doctor(src));
    }
    all.extend(crate::cli_agents::doctor());
    all.extend(crate::kiro::doctor());
    all.extend(crate::ide_agents::doctor());
    all.extend(crate::cascade::doctor());
    all.extend(crate::warp::doctor());
    all.extend(crate::traces::doctor());
    all.extend(crate::cloud::doctor());
    all.extend(crate::cloud_native::doctor());
    all.extend(crate::bolt::doctor());
    all.extend(crate::lovable::doctor());
    all.extend(crate::devin_cloud::doctor());
    all.extend(crate::editor_history::doctor());
    all.extend(crate::dsh::doctor());
    all.extend(crate::longtail::doctor());
    all.extend(crate::remaining_native::doctor());
    let mut diagnostics: Vec<_> = all
        .into_iter()
        .map(|health| {
            let scope = if health.unsupported {
                ReadScope::Diagnostic
            } else if (health.source == "t3" && health.format.starts_with("legacy"))
                || (health.source == "antigravity-ide"
                    && !health.format.starts_with("native trajectory"))
                || matches!(health.source.as_str(), "devin-cloud" | "warp-oz" | "v0")
            {
                ReadScope::Sessions
            } else {
                ReadScope::Edits
            };
            Diagnostic { health, scope }
        })
        .collect();
    diagnostics.extend(crate::acp::doctor().into_iter().map(|health| Diagnostic {
        health,
        scope: ReadScope::Capture,
    }));
    diagnostics
}

/// 公共的导出/采集目录，环境变量只影响 Creditia 自己的数据。
pub fn capture_dir() -> PathBuf {
    std::env::var_os("CREDITIA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::xdg_data().join("creditia/captures"))
}
