//! Human-readable rows for the shared core's diagnostic and collection schemas.
//! Reading a session is deliberately distinct from having verified edit data.
use crate::Language;
use serde_json::Value;

pub fn analysis_warnings(result: &Value) -> Vec<String> {
    fn collect(value: &Value, rows: &mut Vec<String>) {
        for warning in value["warnings"].as_array().into_iter().flatten() {
            if let Some(text) = warning.as_str().filter(|text| !text.trim().is_empty()) {
                if !rows.iter().any(|row| row == text) {
                    rows.push(text.to_owned());
                }
            }
        }
        // add_project wraps the core result, while collect --all returns one
        // result per project. Do not treat arbitrary archive strings as warnings.
        if let Some(nested) = value.get("result") {
            collect(nested, rows);
        }
        for nested in value["collected"].as_array().into_iter().flatten() {
            collect(nested, rows);
        }
    }
    let mut rows = Vec::new();
    collect(result, &mut rows);
    rows
}

fn error_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_owned());
    }
    let code = value["code"].as_str();
    let detail = value["detail"]
        .as_str()
        .or_else(|| value["message"].as_str());
    match (code, detail) {
        (Some(code), Some(detail)) if !detail.is_empty() => Some(format!("{code}: {detail}")),
        (Some(code), _) => Some(code.to_owned()),
        (_, Some(detail)) => Some(detail.to_owned()),
        _ => None,
    }
}

fn age(at: u64, language: &Language) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let seconds = now.saturating_sub(at);
    let (count, zh, en) = if seconds < 60 {
        (seconds, "秒前", "seconds ago")
    } else if seconds < 3600 {
        (seconds / 60, "分钟前", "minutes ago")
    } else if seconds < 86400 {
        (seconds / 3600, "小时前", "hours ago")
    } else {
        (seconds / 86400, "天前", "days ago")
    };
    format!("{count} {}", language.text(zh, en))
}

/// Accepts `doctor()`, `collect_info()`, a completed collection, or a core result.
/// Every returned value is plain text; the view owns layout and scrolling.
pub fn report_rows(value: &Value, language: &Language) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    if let Some(error) = error_text(&value["error"]) {
        rows.push((language.text("检查失败", "Check failed").into(), error));
    }
    if let Some(git) = value["git"].as_bool() {
        rows.push((
            "Git".into(),
            if git {
                language.text("可用", "Available")
            } else {
                language.text("未找到", "Not found")
            }
            .into(),
        ));
    }
    if let Some(gh) = value["gh"].as_bool() {
        rows.push((
            "GitHub CLI".into(),
            if gh {
                language.text("已登录", "Signed in")
            } else {
                language.text(
                    "未登录或未安装；PR 元数据可能缺少分支名",
                    "Not signed in or installed; PR metadata may lack branch names",
                )
            }
            .into(),
        ));
    }
    if let Some(path) = value["library"]["path"].as_str() {
        let projects = value["library"]["projects"].as_u64().unwrap_or(0);
        rows.push((
            language.text("项目库", "Project library").into(),
            format!("{path}\n{projects} {}", language.text("个项目", "projects")),
        ));
    }
    for source in value["sources"].as_array().into_iter().flatten() {
        let name = source["source"]
            .as_str()
            .unwrap_or_else(|| language.text("未知来源", "Unknown source"));
        let readable = source["readable"].as_bool() == Some(true);
        let scope = source["scope"].as_str().unwrap_or("");
        let state = if !readable && scope == "capture" {
            language.text("需要开启采集", "Capture required")
        } else if readable && scope == "sessions" {
            language.text("仅会话信息", "Session metadata only")
        } else if readable {
            language.text("可读取", "Readable")
        } else if source["unsupported"].as_bool() == Some(true) {
            language.text("编辑格式尚未确认", "Edit format unconfirmed")
        } else {
            language.text("未找到或不可读", "Not found or unreadable")
        };
        let mut lines = vec![state.to_owned()];
        if let Some(records) = source["records"].as_u64().filter(|count| *count > 0) {
            lines.push(format!("{records} {}", language.text("条记录", "records")));
        }
        if let Some(path) = source["path"].as_str().filter(|path| !path.is_empty()) {
            lines.push(path.into());
        }
        if let (Some(first), Some(last)) = (source["earliest"].as_str(), source["latest"].as_str())
        {
            lines.push(format!("{first} → {last}"));
        }
        if let Some(note) = source["note"].as_str().filter(|note| !note.is_empty()) {
            lines.push(note.into());
        }
        rows.push((name.to_owned(), lines.join("\n")));
    }
    if let Some(on) = value["on"].as_bool() {
        rows.push((
            language.text("自动采集", "Automatic collection").into(),
            if on {
                language.text("应用打开时启用", "Enabled while the app is open")
            } else {
                language.text("关闭", "Off")
            }
            .into(),
        ));
    }
    if value["running"].as_bool() == Some(true) {
        rows.push((
            language.text("采集状态", "Collection status").into(),
            language.text("正在采集", "Collecting").into(),
        ));
    }
    if let Some(supported) = value["agent"]["supported"].as_bool() {
        let status = if !supported {
            language.text(
                "此平台只在应用打开时采集",
                "This platform collects while the app is open",
            )
        } else if value["agent"]["on"].as_bool() == Some(true) {
            language.text("退出应用后继续采集", "Collection continues after quitting")
        } else {
            language.text("退出应用后暂停采集", "Collection stops after quitting")
        };
        rows.push((
            language.text("后台任务", "Background task").into(),
            status.into(),
        ));
    }
    let last = if value.get("last").is_some() {
        &value["last"]
    } else {
        value
    };
    if let Some(at) = last["at"].as_u64() {
        rows.push((
            language.text("最近采集", "Last collection").into(),
            age(at, language),
        ));
        let projects = last["projects"].as_u64().unwrap_or(0);
        let failed = last["failed"].as_u64().unwrap_or(0);
        let evidence = last["newEvidence"].as_u64().unwrap_or(0);
        rows.push((language.text("采集结果", "Collection result").into(), match language {
            Language::Zh => format!("{projects} 个项目成功，{failed} 个失败；新增 {evidence} 条证据"),
            Language::En => format!("{projects} projects succeeded, {failed} failed; {evidence} new pieces of evidence"),
        }));
        if last["by"] == "agent" {
            rows.push((
                language.text("运行方式", "Run by").into(),
                language
                    .text("系统后台任务", "System background task")
                    .into(),
            ));
        }
    } else if value.get("last").is_some() {
        rows.push((
            language.text("最近采集", "Last collection").into(),
            language
                .text("尚无采集记录", "No collection recorded yet")
                .into(),
        ));
    }
    for warning in analysis_warnings(value) {
        rows.push((language.text("提醒", "Warning").into(), warning));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_core_and_add_project_warning_shapes_without_promoting_notes() {
        let result = json!({"warnings":["outer"],"result":{"warnings":["inner","outer"]},"collected":[{"warnings":["source"]}],"notes":["not a warning"]});
        assert_eq!(analysis_warnings(&result), ["outer", "inner", "source"]);
    }

    #[test]
    fn diagnostic_rows_keep_session_and_capture_limits_explicit() {
        let rows = report_rows(
            &json!({"git":true,"gh":false,"sources":[
                {"source":"session-reader","readable":true,"scope":"sessions","records":4,"path":"/logs"},
                {"source":"capture-reader","readable":false,"scope":"capture"}
            ]}),
            &Language::En,
        );
        assert!(rows.iter().any(|(name, text)| name == "session-reader"
            && text.contains("Session metadata only")
            && text.contains("4 records")));
        assert!(
            rows.iter()
                .any(|(name, text)| name == "capture-reader" && text == "Capture required")
        );
        assert!(!rows.iter().any(|(_, text)| text.contains("verified")));
    }

    #[test]
    fn collection_failure_counts_and_diagnostic_errors_remain_visible() {
        let rows = report_rows(
            &json!({"error":{"code":"core_missing","detail":"/app/core"},"last":{"at":1,"projects":3,"failed":2,"newEvidence":8}}),
            &Language::Zh,
        );
        assert!(
            rows.iter()
                .any(|(_, text)| text == "core_missing: /app/core")
        );
        assert!(
            rows.iter()
                .any(|(_, text)| text.contains("2 个失败") && text.contains("8 条证据"))
        );
    }
}
