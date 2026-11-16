//! The original desktop shell icons, preserving its 16 px grid and stroke geometry.
use gpui_kit::component::Icon;

/// Use the same sourced logo assets as the artwork renderer. Unknown keys have
/// no invented brand mark; their visible contributor name remains sufficient.
pub fn contributor(key: &str) -> Option<Icon> {
    let tool = key.strip_prefix("a:").unwrap_or(key).split('/').next()?;
    let bytes: &'static [u8] = match tool {
        "codex" | "openai" => include_bytes!("../../../packages/render/assets/logos/openai.svg"),
        "claude" | "claude-code" | "anthropic" => {
            include_bytes!("../../../packages/render/assets/logos/claude.svg")
        }
        "cursor" => include_bytes!("../../../packages/render/assets/logos/cursor.svg"),
        "gemini" | "gemini-cli" => {
            include_bytes!("../../../packages/render/assets/logos/geminicli.svg")
        }
        "copilot" | "github-copilot" => {
            include_bytes!("../../../packages/render/assets/logos/githubcopilot.svg")
        }
        "kimi" | "kimi-code" => include_bytes!("../../../packages/render/assets/logos/kimi.svg"),
        "qwen" | "qwen-code" => include_bytes!("../../../packages/render/assets/logos/qwen.svg"),
        "continue" => include_bytes!("../../../packages/render/assets/logos/continue.svg"),
        "windsurf" => include_bytes!("../../../packages/render/assets/logos/windsurf.svg"),
        "deepseek" => include_bytes!("../../../packages/render/assets/logos/deepseek.svg"),
        _ => return None,
    };
    Some(Icon::default().data(bytes))
}

pub fn icon(name: &str) -> Icon {
    let svg: &'static [u8] = match name {
        "parameters" => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 4h3M9 4h5M2 8h7M13 8h1M2 12h1M7 12h7"/><circle cx="7" cy="4" r="2"/><circle cx="11" cy="8" r="2"/><circle cx="5" cy="12" r="2"/></svg>"#,
        "transparent" => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="12" height="12" rx="2"/><path d="M2 6h12M2 10h12M6 2v12M10 2v12"/></svg>"#,
        "file" => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9.5 2H4a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V5.5L9.5 2Z"/><path d="M9 2v4h4M5.5 9h5M5.5 11.5H9"/></svg>"#,
        "pen" => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="m10 2 4 4-7 7-4 1 1-4 6-8ZM9 4l3 3M3 14l3-3"/></svg>"#,
        "film" => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="12" height="12" rx="1.5"/><path d="M5 2v12M11 2v12M2 6h3M2 10h3M11 6h3M11 10h3"/></svg>"#,
        "globe" => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="8" r="6"/><ellipse cx="8" cy="8" rx="2.5" ry="6"/><path d="M2 8h12"/></svg>"#,
        "code" => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="m5 4-4 4 4 4M11 4l4 4-4 4M9 2 7 14"/></svg>"#,
        "text" => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h12M2 6.5h8M2 10h12M2 13.5h8"/></svg>"#,
        "json" => br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5.5 2H4a1 1 0 0 0-1 1v3.5L1.5 8 3 9.5V13a1 1 0 0 0 1 1h1.5M10.5 2H12a1 1 0 0 1 1 1v3.5L14.5 8 13 9.5V13a1 1 0 0 1-1 1h-1.5"/></svg>"#,
        "layout" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2.25" y="2.25" width="11.5" height="11.5" rx="1.5"/><path d="M2.5 6h11M7.25 6v7.5"/></svg>"#,
        "down" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="m4 6 4 4 4-4"/></svg>"#,
        "play" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5 2.75 13 8l-8 5.25z"/></svg>"#,
        "pause" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5.25 3v10M10.75 3v10"/></svg>"#,
        "replay" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.75 7A5.25 5.25 0 1 1 3 10.5M2.75 3.25V7H6.5"/></svg>"#,
        "image" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2.25" y="2.25" width="11.5" height="11.5" rx="1.5"/><circle cx="5.5" cy="5.5" r=".75"/><path d="m3 12 3.5-3.5 2 2 2-2 2.5 2.5"/></svg>"#,
        "wall" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2.75" y="2.75" width="4.5" height="6" rx="1"/><rect x="8.75" y="2.75" width="4.5" height="3.5" rx="1"/><rect x="2.75" y="10.25" width="4.5" height="3" rx="1"/><rect x="8.75" y="7.75" width="4.5" height="5.5" rx="1"/></svg>"#,
        "roam" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="6" y="6" width="4" height="4" rx="1"/><path d="M1.75 6H4v4H1.75M14.25 6H12v4h2.25M6 1.75V4h4V1.75M6 14.25V12h4v2.25"/></svg>"#,
        "close" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/></svg>"#,
        "prev" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9.75 3.75 5.5 8l4.25 4.25"/></svg>"#,
        "next" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6.25 3.75 10.5 8l-4.25 4.25"/></svg>"#,
        "updown" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5.5 6.25 8 3.75l2.5 2.5M5.5 9.75 8 12.25l2.5-2.5"/></svg>"#,
        "more" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="3.75" cy="8" r=".6"/><circle cx="8" cy="8" r=".6"/><circle cx="12.25" cy="8" r=".6"/></svg>"#,
        "search" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="7.25" cy="7.25" r="4.25"/><path d="M10.5 10.5l3 3"/></svg>"#,
        "addFolder" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.25 4.75a1 1 0 0 1 1-1h2.7l1.4 1.5h5.4a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3.25a1 1 0 0 1-1-1z"/><path d="M8 7.5v3.5M6.25 9.25h3.5"/></svg>"#,
        "folder" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.25 4.75a1 1 0 0 1 1-1h2.7l1.4 1.5h5.4a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3.25a1 1 0 0 1-1-1z"/></svg>"#,
        "refresh" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M13.25 8a5.25 5.25 0 1 1-1.54-3.71"/><path d="M13.25 2.5v3.25H10"/></svg>"#,
        "export" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M8 2.5V10M5 5.25l3-3 3 3"/><path d="M3 9.75v2.75a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V9.75"/></svg>"#,
        "copy" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="5.5" y="5.5" width="8" height="8" rx="1.5"/><path d="M10.5 3.5v-.25a1 1 0 0 0-1-1H3.5a1 1 0 0 0-1 1V9.5a1 1 0 0 0 1 1h.25"/></svg>"#,
        "link" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6.75 9.25 9.25 6.75"/><path d="M8.5 4.25l.9-.9a2.6 2.6 0 0 1 3.7 3.7l-.9.9M7.5 11.75l-.9.9a2.6 2.6 0 0 1-3.7-3.7l.9-.9"/></svg>"#,
        "remove" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.75 4.5h10.5M6.25 4.5V3a.75.75 0 0 1 .75-.75h2a.75.75 0 0 1 .75.75v1.5M4.25 4.5l.6 8.3a1 1 0 0 0 1 .95h4.3a1 1 0 0 0 1-.95l.6-8.3"/></svg>"#,
        "info" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="8" r="5.75"/><path d="M8 7.25v3.75"/><circle cx="8" cy="5" r=".4"/></svg>"#,
        "home" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="8" r="2.25"/><path d="M8 1.75v2.5M8 11.75v2.5M1.75 8h2.5M11.75 8h2.5"/></svg>"#,
        "check" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.5l3 3 6-6.5"/></svg>"#,
        "gear" => br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6.50 3.55L7.05 1.98L8.95 1.98L9.50 3.55L11.11 4.47L12.74 4.16L13.69 5.81L12.61 7.07L12.61 8.93L13.69 10.19L12.74 11.84L11.11 11.53L9.50 12.45L8.95 14.02L7.05 14.02L6.50 12.45L4.89 11.53L3.26 11.84L2.31 10.19L3.39 8.93L3.39 7.07L2.31 5.81L3.26 4.16L4.89 4.47Z"/><circle cx="8" cy="8" r="1.9"/></svg>"#,
        _ => return Icon::default(),
    };
    Icon::default().data(svg)
}
