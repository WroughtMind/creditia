//! Durable settings writes outside the UI executor. Pending snapshots coalesce;
//! an in-flight write always finishes before a newer snapshot can replace it.
use serde_json::Value;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

type Save = Arc<dyn Fn(Value) -> Result<(), String> + Send + Sync>;
type Notify = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct State {
    pending: Option<Value>,
    writing: bool,
    stopping: bool,
    alive: bool,
    last_error: Option<String>,
    unread_error: Option<String>,
}
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

pub struct PreferenceWriter {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl Default for PreferenceWriter {
    fn default() -> Self {
        Self::new()
    }
}
impl PreferenceWriter {
    pub fn new() -> Self {
        Self::with_notify(Arc::new(|| {}))
    }

    /// A host can notify its UI after a write without a periodic polling timer.
    /// The callback runs on the writer thread and must schedule, not mutate, UI.
    pub fn with_notify(notify: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self::start(
            Arc::new(|value| {
                creditia_desktop_service::patch_ui_preferences(value)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }),
            notify,
        )
    }

    fn start(save: Save, notify: Notify) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                alive: true,
                ..State::default()
            }),
            changed: Condvar::new(),
        });
        let background = shared.clone();
        let worker = std::thread::Builder::new()
            .name("creditia-preferences".into())
            .spawn(move || {
                loop {
                    let value = {
                        let mut state = background
                            .state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        while state.pending.is_none() && !state.stopping {
                            state = background
                                .changed
                                .wait(state)
                                .unwrap_or_else(|error| error.into_inner());
                        }
                        let Some(value) = state.pending.take() else {
                            state.alive = false;
                            background.changed.notify_all();
                            break;
                        };
                        state.writing = true;
                        value
                    };
                    let result = save(value);
                    {
                        let mut state = background
                            .state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        state.writing = false;
                        state.last_error = result.err();
                        state.unread_error = state.last_error.clone();
                        background.changed.notify_all();
                    }
                    notify();
                }
            });
        match worker {
            Ok(worker) => Self {
                shared,
                worker: Some(worker),
            },
            Err(error) => {
                let message = format!("Cannot start preference writer: {error}");
                let mut state = shared
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                state.alive = false;
                state.last_error = Some(message.clone());
                state.unread_error = Some(message);
                drop(state);
                Self {
                    shared,
                    worker: None,
                }
            }
        }
    }

    /// Submit a complete UI preference patch, never a partial delta: newer
    /// pending snapshots replace older ones while preserving the final choice.
    pub fn submit(&self, value: Value) {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !state.alive || state.stopping {
            return;
        }
        state.pending = Some(value);
        self.shared.changed.notify_one();
    }

    pub fn take_error(&self) -> Option<String> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .unread_error
            .take()
    }

    /// For orderly shutdown/tests, not for button handlers. Wait for the latest
    /// submitted snapshot to commit and report a failed final write accurately.
    pub fn flush(&self) -> Result<(), String> {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while state.alive && (state.pending.is_some() || state.writing) {
            state = self
                .shared
                .changed
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        state.last_error.clone().map_or(Ok(()), Err)
    }
}

impl Drop for PreferenceWriter {
    fn drop(&mut self) {
        {
            let mut state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.stopping = true;
            self.shared.changed.notify_all();
        }
        // The worker drains its final pending snapshot before stopping. The UI
        // never waits during interaction; shutdown waits to preserve settings.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::mpsc;

    #[test]
    fn serializes_inflight_write_and_coalesces_pending_snapshots() {
        let written = Arc::new(Mutex::new(Vec::new()));
        let output = written.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        let writer = PreferenceWriter::start(
            Arc::new(move |value| {
                if value == json!(1) {
                    started_tx.send(()).unwrap();
                    release_rx.lock().unwrap().recv().unwrap();
                }
                output.lock().unwrap().push(value);
                Ok(())
            }),
            Arc::new(|| {}),
        );
        writer.submit(json!(1));
        started_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        for i in 2..=20 {
            writer.submit(json!(i));
        }
        release_tx.send(()).unwrap();
        writer.flush().unwrap();
        assert_eq!(*written.lock().unwrap(), vec![json!(1), json!(20)]);
    }

    #[test]
    fn shutdown_commits_last_pending_value_and_errors_are_reported_once() {
        let written = Arc::new(Mutex::new(None));
        {
            let output = written.clone();
            let writer = PreferenceWriter::start(
                Arc::new(move |value| {
                    *output.lock().unwrap() = Some(value);
                    Ok(())
                }),
                Arc::new(|| {}),
            );
            writer.submit(json!({"named":false}));
        }
        assert_eq!(*written.lock().unwrap(), Some(json!({"named":false})));
        let writer =
            PreferenceWriter::start(Arc::new(|_| Err("disk full".into())), Arc::new(|| {}));
        writer.submit(json!({}));
        assert_eq!(writer.flush().unwrap_err(), "disk full");
        assert_eq!(writer.take_error().as_deref(), Some("disk full"));
        assert!(writer.take_error().is_none());
        assert_eq!(writer.flush().unwrap_err(), "disk full");
    }
}
