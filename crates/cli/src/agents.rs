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
    let tier = match a.flags.get("tier").map(String::as_str) {
        Some("tier1") => Some(agents::AgentTier::Tier1),
        Some("tier2") => Some(agents::AgentTier::Tier2),
        None if !a.on("tier") => None,
        _ => {
            return Err(tr!(
                "--tier 只接受 tier1 或 tier2",
                "--tier must be tier1 or tier2"
            ))
        }
    };
    let rows: Vec<_> = agents::all()
        .iter()
        .filter(|x| selected.is_none_or(|id| id == &x.id))
        .filter(|x| tier.is_none_or(|tier| tier == x.tier))
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
                "{:<22} {:<22} {} [{}]",
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
                },
                v["tier"].as_str().unwrap(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(flags: &[(&str, &str)]) -> Args {
        Args {
            cmd: "agents".into(),
            pos: vec![],
            flags: flags.iter().map(|&(k, v)| (k.into(), v.into())).collect(),
            switches: vec![],
        }
    }

    #[test]
    fn tier_filters_keep_catalogue_order_and_row_fields() {
        let (catalogue, _) = list(&args(&[])).unwrap();
        let rows = catalogue["agents"].as_array().unwrap();
        let summary = catalogue["summary"].as_str().unwrap();
        assert_eq!(rows.len(), agents::all().len());
        assert_eq!(catalogue["count"], rows.len());
        assert_eq!(summary.lines().count(), rows.len());
        for (row, summary) in rows.iter().zip(summary.lines()) {
            let tier = row["tier"].as_str().unwrap();
            assert!(matches!(tier, "tier1" | "tier2"));
            assert!(summary.starts_with(row["id"].as_str().unwrap()));
            assert!(summary.ends_with(&format!("[{tier}]")));
        }

        for tier in ["tier1", "tier2"] {
            let (filtered, _) = list(&args(&[("tier", tier)])).unwrap();
            let expected: Vec<_> = rows.iter().filter(|row| row["tier"] == tier).collect();
            assert!(!expected.is_empty());
            assert_eq!(filtered["agents"], json!(expected));
            assert_eq!(filtered["count"], expected.len());
        }
    }

    #[test]
    fn tier_can_be_combined_with_a_public_agent_id() {
        let (selected, _) = list(&args(&[("agent", "codex")])).unwrap();
        assert_eq!(selected["count"], 1);
        assert_eq!(selected["agents"][0]["tier"], "tier1");
        assert_eq!(
            list(&args(&[("agent", "codex"), ("tier", "tier1")]))
                .unwrap()
                .0,
            selected
        );
        let (empty, _) = list(&args(&[("agent", "codex"), ("tier", "tier2")])).unwrap();
        assert_eq!(empty, json!({"agents": [], "count": 0, "summary": ""}));
        assert!(list(&args(&[("agent", "unknown"), ("tier", "tier1")])).is_err());
    }

    #[test]
    fn invalid_or_missing_tier_is_an_error() {
        assert!(list(&args(&[("tier", "tier3")])).is_err());
        assert!(list(&args(&[("tier", "")])).is_err());
        let mut missing = args(&[]);
        missing.switches.push("tier".into());
        assert!(list(&missing).is_err());
    }
}
