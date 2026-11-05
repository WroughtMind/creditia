//! Desktop-independent collection, archive refresh and incremental catch-up.
//! The host forwards events to its UI executor and owns this handle's lifetime.
use crate::{AnalysisCancellation, ProgressSink};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

struct State {
    stop: bool,
    active: bool,
    project: Option<String>,
    generation: u64,
    task: Option<AnalysisCancellation>,
}
struct Shared { state: Mutex<State>, wake: Condvar }

pub struct BackgroundService { shared: Arc<Shared> }
impl BackgroundService {
    pub fn start(events: ProgressSink) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State { stop: false, active: true, project: None, generation: 0, task: None }),
            wake: Condvar::new(),
        });
        let worker = shared.clone();
        std::thread::spawn(move || run(worker, events));
        Self { shared }
    }

    /// Replacing the watched project invalidates and cancels only this service's
    /// current task; it can never cancel a manual analysis owned by another view.
    pub fn watch_project(&self, project: Option<String>) {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.project == project { return; }
        if let Some(task) = state.task.take() { task.cancel(); }
        state.project = project;
        state.generation = state.generation.wrapping_add(1);
        self.shared.wake.notify_one();
    }
    pub fn set_active(&self, active: bool) {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.active = active;
        if active { self.shared.wake.notify_one(); }
    }
    pub fn refresh(&self) { self.shared.wake.notify_one(); }
    pub fn stop(&self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.stop = true;
        if let Some(task) = state.task.take() { task.cancel(); }
        self.shared.wake.notify_one();
    }
}
impl Drop for BackgroundService { fn drop(&mut self) { self.stop(); } }

fn current(shared: &Shared, generation: u64) -> bool {
    let state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
    !state.stop && state.generation == generation
}
fn begin(shared: &Shared, generation: u64) -> Option<AnalysisCancellation> {
    let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
    if state.stop || state.generation != generation { return None; }
    let token = AnalysisCancellation::default();
    state.task = Some(token.clone());
    Some(token)
}
fn finish(shared: &Shared, generation: u64) {
    let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
    if state.generation == generation { state.task = None; }
}

fn run(shared: Arc<Shared>, events: ProgressSink) {
    if let Ok(task) = crate::recover_interrupted_analysis() {
        if task["state"] == "interrupted" { events(json!({"kind":"analysis-interrupted","task":task})); }
    }
    #[cfg(target_os = "macos")]
    crate::collection::agent_refresh();
    let started = Instant::now();
    let mut stamps = HashMap::<String, String>::new();
    let mut tried = HashMap::<String, String>::new();
    let mut members: Option<Vec<String>> = None;
    loop {
        let (active, project, generation) = {
            let state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.stop { break; }
            (state.active, state.project.clone(), state.generation)
        };
        if active {
            if let Some(id) = project.clone().filter(|id| id != "__all__") {
                if let Ok(stamp) = crate::stamp(id.clone()) {
                    let changed = stamps.insert(id.clone(), stamp.clone()).is_some_and(|old| old != stamp);
                    if changed && current(&shared, generation) {
                        events(json!({"kind":"archive-changed","projectId":id,"generation":generation,"stamp":stamp}));
                    }
                }
                let preferences = crate::ui_preferences();
                let auto = preferences["projects"][&id]["autoAnalyze"].as_bool()
                    .unwrap_or(preferences["global"]["autoAnalyze"].as_bool().unwrap_or(true));
                if auto && !crate::analysis_running() && current(&shared, generation) {
                    if let (Ok(raw), Ok(head)) = (crate::credits(id.clone()), crate::head(id.clone())) {
                        let archived = raw["head"].as_str().unwrap_or_default();
                        if !archived.is_empty() && !head.is_empty() && head != archived && tried.get(&id) != Some(&head) {
                            if let Some(token) = begin(&shared, generation) {
                                let progress_id = id.clone();
                                let emit = events.clone();
                                let progress: ProgressSink = Arc::new(move |value| emit(json!({"kind":"analysis-progress","projectId":progress_id,"generation":generation,"progress":value})));
                                let lang = preferences["global"]["lang"].as_str().unwrap_or("zh").to_string();
                                events(json!({"kind":"analysis-started","projectId":id,"generation":generation,"automatic":true}));
                                let result = crate::analyze_with_cancel(id.clone(), Some(lang), Some(progress), token);
                                finish(&shared, generation);
                                if result.as_ref().err().is_none_or(|e| e.code != "busy" && e.code != "cancelled") { tried.insert(id.clone(), head); }
                                if current(&shared, generation) {
                                    match result {
                                        Ok(result) => {
                                            if let Ok(stamp) = crate::stamp(id.clone()) { stamps.insert(id.clone(), stamp); }
                                            events(json!({"kind":"analysis-finished","projectId":id,"generation":generation,"result":result}));
                                            events(json!({"kind":"archive-changed","projectId":id,"generation":generation}));
                                        }
                                        Err(error) if error.code != "busy" && error.code != "cancelled" => events(json!({"kind":"error","operation":"auto-analysis","projectId":id,"error":error})),
                                        Err(_) => {}
                                    }
                                }
                            }
                        }
                    }
                }
            } else if project.as_deref() == Some("__all__") {
                if let Ok(projects) = crate::projects() {
                    let ids = projects.as_array().into_iter().flatten().filter_map(|p| p["id"].as_str().map(str::to_string)).collect::<Vec<_>>();
                    let mut changed = members.as_ref().is_some_and(|previous| previous != &ids);
                    members = Some(ids);
                    for project in projects.as_array().into_iter().flatten() {
                        if let Some(id) = project["id"].as_str() {
                            if let Ok(stamp) = crate::stamp(id.to_string()) {
                                changed |= stamps.insert(id.into(), stamp.clone()).is_some_and(|old| old != stamp);
                            }
                        }
                    }
                    if changed && current(&shared, generation) { events(json!({"kind":"archive-changed","projectId":"__all__","generation":generation})); }
                }
            }
        }
        let last = crate::collection::last_collect()["at"].as_i64().unwrap_or(0);
        if started.elapsed() >= Duration::from_secs(30) && crate::collection::collect_on()
            && crate::collection::now() - last >= crate::collection::COLLECT_EVERY && current(&shared, generation) {
            if let Some(token) = begin(&shared, generation) {
                let result = crate::collection::collect_round_with_cancel(true, token);
                finish(&shared, generation);
                match result {
                    Ok(result) => events(json!({"kind":"collect-done","result":result})),
                    Err(error) if !["off", "busy", "cancelled"].contains(&error.code) => events(json!({"kind":"error","operation":"collect","error":error})),
                    Err(_) => {}
                }
            }
        }
        let state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.stop { break; }
        // No frame-rate timer: the thread sleeps until a focus/project event or
        // the 30-second archive refresh deadline, including while UI is idle.
        drop(shared.wake.wait_timeout(state, Duration::from_secs(30)));
    }
}
