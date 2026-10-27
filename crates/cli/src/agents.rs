//! Agent catalogue and an explicit, inherited identity for otherwise unknown launchers.
use crate::{Args, R};
use creditia_core::{agents, model::Src, tr};
use serde_json::{json, Value};

pub fn source(a: &Args) -> R<Src> {
    let harness = match (a.flags.get("agent"), a.flags.get("harness")) {
        (Some(_), Some(_)) => return Err("use either --agent or --harness".into()),
        (Some(id), None) => agents::by_id(id).map(|a| a.harness.as_str()),
        (None, Some(id)) => agents::canonical_harness(id),
        _ => None,
    };
    harness.and_then(Src::from_harness).ok_or_else(|| {
        tr!(
            "需要已登记的 --agent 或 --harness；见 creditia agents",
            "a known --agent or --harness is required; see creditia agents"
        )
    })
}

pub fn list(a: &Args) -> R<(Value, Vec<String>)> {
    let selected = a.flags.get("agent");
    if selected.is_some_and(|id| agents::by_id(id).is_none()) {
        return Err("unknown --agent; see creditia agents".into());
    }
    let rows: Vec<_> = agents::all()
        .iter()
        .filter(|x| selected.is_none_or(|id| id == &x.id))
        .map(|x| {
            let mut v = serde_json::to_value(x).expect("serializable catalogue");
            v["stamp"] = json!(format!("creditia stamp --agent {}", x.id));
            v["skillInstall"] = json!(format!(
                "npx skills add WroughtMind/creditia --skill creditia -a {}",
                x.id
            ));
            v["capture"] = json!("requires-acp-command");
            v["runtimeVerified"] = json!(false);
            v
        })
        .collect();
    let summary = rows
        .iter()
        .map(|v| {
            format!(
                "{:<22} {:<22} {}",
                v["id"].as_str().unwrap(),
                v["name"].as_str().unwrap(),
                if let (Some(zh), Some(en)) =
                    (v["historyNote"].as_str(), v["historyNoteEn"].as_str())
                {
                    if creditia_core::i18n::en() {
                        en.to_string()
                    } else {
                        zh.to_string()
                    }
                } else if v["history"] == "reader" {
                    tr!(
                        "有历史读取入口；具体格式见 doctor / ADAPTERS.md",
                        "history reader available; see doctor / ADAPTERS.md for format limits"
                    )
                } else {
                    tr!(
                        "提交署名可用；原生历史格式待核实",
                        "commit declarations available; native history unconfirmed"
                    )
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok((
        json!({"agents":rows,"count":rows.len(),"summary":summary}),
        vec![],
    ))
}

/// Transparent stdio and argv. Identity affects declarations, never fabricates edits.
pub fn run(a: &Args) -> R<i32> {
    let src = source(a)?;
    let program = a.pos.first().ok_or("a command after -- is required")?;
    let mut cmd = std::process::Command::new(program);
    cmd.args(&a.pos[1..])
        .env("CREDITIA_HARNESS", src.harness())
        .env_remove("CREDITIA_MODEL");
    if let Some(model) = a.flags.get("model") {
        if model.contains(['\n', '\r']) {
            return Err("model must be a single-line value".into());
        }
        cmd.env("CREDITIA_MODEL", model);
    }
    let status = cmd.status().map_err(|e| format!("{program}: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        Ok(status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)))
    }
    #[cfg(not(unix))]
    Ok(status.code().unwrap_or(1))
}
