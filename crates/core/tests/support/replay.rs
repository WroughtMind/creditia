use creditia_core::{
    config::Config,
    engine::Engine,
    model::{Commit, Ev, FileGroup, Index, Session, Store},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub commits: Vec<Commit>,
    pub stamps: HashMap<String, (String, Option<String>)>,
    pub events: Vec<Ev>,
    pub sessions: Vec<Session>,
    pub config: Config,
}

/// Engine-only replay. This intentionally does not claim coverage of native log
/// ingestion, Git parsing, survival or the complete product report.
pub fn replay(input: &Input) -> Value {
    let mut store = Store::default();
    store.groups.insert(
        "fixture".into(),
        FileGroup {
            evs: input.events.clone(),
            sessions: input.sessions.clone(),
            ..Default::default()
        },
    );
    let idx = Index::build(&store);
    let mut engine = Engine::new(
        &input.config,
        &idx,
        &input.sessions,
        &[],
        input.stamps.clone(),
    );
    engine.set_history(&input.commits);
    let mut commits: Vec<_> = input.commits.iter().collect();
    commits.sort_by_key(|c| (c.commit_ts, c.author_ts));
    let mut rows = Vec::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for commit in commits {
        let result = engine.attribute(commit);
        for file in result.lines {
            for attr in file {
                let who = &engine.reg.list[attr.who];
                // Stable comparison of identities after anonymization. Agent IDs
                // keep tool/model semantics; private human IDs are not output.
                let label = if who.kind == "agent" {
                    who.id.clone()
                } else {
                    format!("{}:{}", who.kind, attr.who)
                };
                rows.push(json!([label, attr.level]));
                *counts
                    .entry(format!(
                        "{}:{:?}",
                        who.harness.as_deref().unwrap_or(who.kind),
                        attr.level
                    ))
                    .or_default() += 1;
            }
        }
    }
    json!({"rows":rows,"counts":counts})
}
