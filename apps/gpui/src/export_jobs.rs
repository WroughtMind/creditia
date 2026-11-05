//! Recoverable export queue. Requests and destinations are immutable once queued;
//! archived inputs are shared across formats instead of copied into each job.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};

pub const MAX_JOBS: usize = 64;
pub const MAX_PENDING: usize = 16;
pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Queued,
    Rendering,
    Writing,
    Saved,
    Failed,
    Cancelled,
    Interrupted,
}
impl State {
    pub fn active(self) -> bool {
        matches!(self, Self::Rendering | Self::Writing)
    }
    pub fn pending(self) -> bool {
        self == Self::Queued || self.active()
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Rendering => "rendering",
            Self::Writing => "writing",
            Self::Saved => "saved",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "queued" => Self::Queued,
            "rendering" => Self::Rendering,
            "writing" => Self::Writing,
            "saved" => Self::Saved,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "interrupted" => Self::Interrupted,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    parameters: Arc<Value>,
    archive: Arc<Value>,
    archive_key: u64,
    parameter_bytes: usize,
}
impl Request {
    /// Call in the renderer's background task, not during a window redraw.
    pub fn materialize(&self) -> Value {
        let mut request = (*self.parameters).clone();
        request["raw"] = (*self.archive).clone();
        request
    }
    pub fn format(&self) -> &str {
        self.parameters["format"].as_str().unwrap_or("png")
    }
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: u64,
    pub attempt: u64,
    pub label: String,
    pub format: String,
    pub request: Option<Request>,
    pub path: PathBuf,
    pub state: State,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ActiveJob {
    pub id: u64,
    pub attempt: u64,
    pub label: String,
    pub format: String,
    pub request: Request,
    pub path: PathBuf,
}
#[derive(Clone, Copy, Debug)]
pub struct Enqueued {
    pub id: u64,
    pub deduplicated: bool,
}

#[derive(Clone, Default)]
pub struct Queue {
    jobs: Vec<Job>,
    archives: BTreeMap<u64, (Arc<Value>, usize)>,
    next_id: u64,
}

fn encoded_bytes(value: &Value) -> Result<usize, String> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| error.to_string())
}
fn validate_parameters(value: &Value) -> Result<(), String> {
    if !value.is_object()
        || value["version"] != 1
        || value["op"] != "export"
        || !value["format"].as_str().is_some_and(|format| {
            [
                "png", "svg", "pdf", "gif", "html", "md", "txt", "json", "embed",
            ]
            .contains(&format)
        })
    {
        return Err("Invalid queued export request".into());
    }
    Ok(())
}
fn validate_path(path: &std::path::Path) -> Result<(), String> {
    if !path.is_absolute() || path.file_name().is_none() || path.to_str().is_none() {
        return Err("The export destination must be an absolute UTF-8 file path".into());
    }
    Ok(())
}

impl Queue {
    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }
    pub fn job(&self, id: u64) -> Option<&Job> {
        self.jobs.iter().find(|job| job.id == id)
    }
    pub fn active(&self) -> Option<&Job> {
        self.jobs.iter().find(|job| job.state.active())
    }
    pub fn input_bytes(&self) -> usize {
        self.archives
            .values()
            .map(|(_, bytes)| bytes)
            .sum::<usize>()
            + self
                .jobs
                .iter()
                .filter_map(|job| job.request.as_ref())
                .map(|request| request.parameter_bytes)
                .sum::<usize>()
    }
    fn prune_archives(&mut self) {
        self.archives.retain(|key, _| {
            self.jobs.iter().any(|job| {
                job.request
                    .as_ref()
                    .is_some_and(|request| request.archive_key == *key)
            })
        });
    }

    pub fn enqueue(
        &mut self,
        mut request: Value,
        path: PathBuf,
        label: String,
    ) -> Result<Enqueued, String> {
        validate_path(&path)?;
        validate_parameters(&request)?;
        if encoded_bytes(&request)? > MAX_REQUEST_BYTES {
            return Err("Export request exceeds 8 MiB".into());
        }
        let object = request.as_object_mut().expect("validated request");
        let raw = object
            .remove("raw")
            .filter(Value::is_object)
            .ok_or("Queued export requires an archive")?;
        object.remove("id"); // Transport sequence IDs are not export options.
        for job in &self.jobs {
            if job.state.pending()
                && job.path == path
                && job
                    .request
                    .as_ref()
                    .is_some_and(|input| *input.parameters == request && *input.archive == raw)
            {
                return Ok(Enqueued {
                    id: job.id,
                    deduplicated: true,
                });
            }
        }
        if self.jobs.iter().filter(|job| job.state.pending()).count() >= MAX_PENDING {
            return Err("The export queue is full; finish or cancel a queued job first".into());
        }
        if self.jobs.len() == MAX_JOBS {
            if let Some(index) = self.jobs.iter().position(|job| !job.state.pending()) {
                self.jobs.remove(index);
                self.prune_archives();
            } else {
                return Err("The export history is full".into());
            }
        }
        let raw_bytes = serde_json::to_vec(&raw).map_err(|error| error.to_string())?;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        raw_bytes.hash(&mut hash);
        let mut key = hash.finish();
        while self
            .archives
            .get(&key)
            .is_some_and(|(existing, _)| **existing != raw)
        {
            key = key.wrapping_add(1);
        }
        let parameter_bytes = encoded_bytes(&request)?;
        let extra = parameter_bytes
            + if self.archives.contains_key(&key) {
                0
            } else {
                raw_bytes.len()
            };
        if self.input_bytes().saturating_add(extra) > MAX_INPUT_BYTES {
            return Err(
                "Queued export archives exceed 32 MiB; finish or remove an older job first".into(),
            );
        }
        let next = self
            .next_id
            .checked_add(1)
            .ok_or("Export job ID overflow")?;
        let (archive, _) = self
            .archives
            .entry(key)
            .or_insert_with(|| (Arc::new(raw), raw_bytes.len()));
        let format = request["format"]
            .as_str()
            .expect("validated format")
            .to_string();
        let input = Request {
            parameters: Arc::new(request),
            archive: archive.clone(),
            archive_key: key,
            parameter_bytes,
        };
        let id = self.next_id;
        self.next_id = next;
        self.jobs.push(Job {
            id,
            attempt: 0,
            label,
            format,
            request: Some(input),
            path,
            state: State::Queued,
            error: None,
        });
        Ok(Enqueued {
            id,
            deduplicated: false,
        })
    }

    pub fn start_next(&mut self) -> Option<ActiveJob> {
        if self.active().is_some() {
            return None;
        }
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.state == State::Queued)?;
        let request = job.request.clone()?;
        job.attempt = job.attempt.checked_add(1)?;
        job.state = State::Rendering;
        Some(ActiveJob {
            id: job.id,
            attempt: job.attempt,
            label: job.label.clone(),
            format: job.format.clone(),
            request,
            path: job.path.clone(),
        })
    }
    pub fn mark_writing(&mut self, id: u64, attempt: u64) -> bool {
        let Some(job) = self
            .jobs
            .iter_mut()
            .find(|job| job.id == id && job.attempt == attempt && job.state == State::Rendering)
        else {
            return false;
        };
        job.state = State::Writing;
        true
    }
    /// A successful atomic save remains success even if cancellation arrived
    /// just after file replacement. Old attempts cannot overwrite retry state.
    pub fn finish(&mut self, id: u64, attempt: u64, result: Result<(), String>) -> bool {
        let Some(job) = self.jobs.iter_mut().find(|job| {
            job.id == id
                && job.attempt == attempt
                && (job.state.active() || job.state == State::Cancelled)
        }) else {
            return false;
        };
        match result {
            Ok(()) => {
                job.state = State::Saved;
                job.error = None;
                job.request = None;
            }
            Err(error) => {
                if job.state != State::Cancelled {
                    job.state = State::Failed;
                    job.error = Some(error);
                }
            }
        }
        self.prune_archives();
        true
    }
    /// The view must cancel its matching renderer token before starting another
    /// worker. Keeping `active()` true until finish prevents overlap.
    pub fn cancel_queued(&mut self, id: u64) -> bool {
        let Some(job) = self
            .jobs
            .iter_mut()
            .find(|job| job.id == id && job.state == State::Queued)
        else {
            return false;
        };
        job.state = State::Cancelled;
        true
    }
    pub fn finish_cancelled(&mut self, id: u64, attempt: u64) -> bool {
        let Some(job) = self
            .jobs
            .iter_mut()
            .find(|job| job.id == id && job.attempt == attempt && job.state.active())
        else {
            return false;
        };
        job.state = State::Cancelled;
        job.error = None;
        true
    }
    pub fn retry(&mut self, id: u64) -> Result<(), String> {
        if self.jobs.iter().filter(|job| job.state.pending()).count() >= MAX_PENDING {
            return Err("The export queue is full".into());
        }
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.id == id)
            .ok_or("Unknown export job")?;
        if !matches!(
            job.state,
            State::Failed | State::Cancelled | State::Interrupted
        ) || job.request.is_none()
        {
            return Err("This export cannot be retried".into());
        }
        job.state = State::Queued;
        job.error = None;
        Ok(())
    }
    pub fn remove(&mut self, id: u64) -> bool {
        let Some(index) = self
            .jobs
            .iter()
            .position(|job| job.id == id && !job.state.pending())
        else {
            return false;
        };
        self.jobs.remove(index);
        self.prune_archives();
        true
    }
    pub fn snapshot(&self) -> Value {
        let archives: serde_json::Map<String, Value> = self
            .archives
            .iter()
            .map(|(key, (value, _))| (key.to_string(), (**value).clone()))
            .collect();
        json!({"schema":1,"nextId":self.next_id,"archives":archives,"jobs":self.jobs.iter().map(|job| json!({
            "id":job.id,"attempt":job.attempt,"label":job.label,"format":job.format,"path":job.path,
            "state":job.state.as_str(),"error":job.error,"request":job.request.as_ref().map(|request| &*request.parameters),
            "archive":job.request.as_ref().map(|request| request.archive_key.to_string())
        })).collect::<Vec<_>>()})
    }
    pub fn from_snapshot(value: Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        if value["schema"] != 1 {
            return Err("Unsupported export queue schema".into());
        }
        let saved = value["jobs"].as_array().ok_or("Invalid export history")?;
        if saved.len() > MAX_JOBS {
            return Err("Export history exceeds its limit".into());
        }
        let mut queue = Self {
            next_id: value["nextId"].as_u64().ok_or("Invalid export sequence")?,
            ..Self::default()
        };
        let archives = value["archives"]
            .as_object()
            .ok_or("Invalid export archives")?;
        for (key, raw) in archives {
            if !raw.is_object() {
                return Err("Invalid saved export archive".into());
            }
            let key = key.parse::<u64>().map_err(|_| "Invalid archive ID")?;
            let bytes = encoded_bytes(raw)?;
            if bytes > MAX_REQUEST_BYTES {
                return Err("Saved export archive exceeds 8 MiB".into());
            }
            queue.archives.insert(key, (Arc::new(raw.clone()), bytes));
        }
        for record in saved {
            let id = record["id"].as_u64().ok_or("Invalid export ID")?;
            if id >= queue.next_id || queue.job(id).is_some() {
                return Err("Duplicate or future export ID".into());
            }
            let state = State::parse(record["state"].as_str().ok_or("Missing export state")?)
                .ok_or("Invalid export state")?;
            let path = PathBuf::from(record["path"].as_str().ok_or("Invalid export path")?);
            validate_path(&path)?;
            let format = record["format"]
                .as_str()
                .ok_or("Missing export format")?
                .to_owned();
            let request = if record["request"].is_null() {
                None
            } else {
                validate_parameters(&record["request"])?;
                if record["request"].get("raw").is_some() || record["request"]["format"] != format {
                    return Err("Invalid saved export options".into());
                }
                let key = record["archive"]
                    .as_str()
                    .ok_or("Missing export archive")?
                    .parse::<u64>()
                    .map_err(|_| "Invalid export archive ID")?;
                let (raw, bytes) = queue
                    .archives
                    .get(&key)
                    .ok_or("Missing archived export input")?;
                let parameter_bytes = encoded_bytes(&record["request"])?;
                if bytes.saturating_add(parameter_bytes).saturating_add(7) > MAX_REQUEST_BYTES {
                    return Err("Saved export request exceeds 8 MiB".into());
                }
                Some(Request {
                    parameters: Arc::new(record["request"].clone()),
                    archive: raw.clone(),
                    archive_key: key,
                    parameter_bytes,
                })
            };
            if state != State::Saved && request.is_none() {
                return Err("Unfinished export has no saved input".into());
            }
            queue.jobs.push(Job {
                id,
                attempt: record["attempt"].as_u64().ok_or("Invalid export attempt")?,
                label: record["label"]
                    .as_str()
                    .ok_or("Missing export label")?
                    .to_owned(),
                format,
                request,
                path,
                state: if state.pending() {
                    State::Interrupted
                } else {
                    state
                },
                error: record["error"].as_str().map(str::to_owned),
            });
        }
        queue.prune_archives();
        if queue.input_bytes() > MAX_INPUT_BYTES {
            return Err("Saved export queue exceeds 32 MiB".into());
        }
        Ok(queue)
    }
}

type Save = Arc<dyn Fn(u64, &Value) -> Result<u64, String> + Send + Sync>;
#[derive(Default)]
struct JournalState {
    pending: Option<Queue>,
    writing: bool,
    stopping: bool,
    alive: bool,
    error: Option<String>,
    unread_error: Option<String>,
}
struct JournalShared {
    state: std::sync::Mutex<JournalState>,
    changed: std::sync::Condvar,
}
pub struct Journal {
    shared: Arc<JournalShared>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Journal {
    pub fn new(initial_revision: u64, notify: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self::start(
            initial_revision,
            Arc::new(|revision, value| {
                creditia_desktop_service::save_export_jobs(revision, value)
                    .map_err(|error| error.to_string())
            }),
            notify,
        )
    }
    fn start(mut revision: u64, save: Save, notify: Arc<dyn Fn() + Send + Sync>) -> Self {
        let shared = Arc::new(JournalShared {
            state: std::sync::Mutex::new(JournalState {
                alive: true,
                ..JournalState::default()
            }),
            changed: std::sync::Condvar::new(),
        });
        let work = shared.clone();
        let worker = std::thread::Builder::new()
            .name("creditia-export-journal".into())
            .spawn(move || {
                loop {
                    let queue = {
                        let mut state =
                            work.state.lock().unwrap_or_else(|error| error.into_inner());
                        while state.pending.is_none() && !state.stopping {
                            state = work
                                .changed
                                .wait(state)
                                .unwrap_or_else(|error| error.into_inner());
                        }
                        let Some(queue) = state.pending.take() else {
                            state.alive = false;
                            work.changed.notify_all();
                            break;
                        };
                        state.writing = true;
                        queue
                    };
                    // Queue clones contain only Arc-backed input references. Both
                    // snapshot expansion and disk serialization stay off the UI.
                    let result = save(revision, &queue.snapshot());
                    let failed = result.is_err();
                    {
                        let mut state =
                            work.state.lock().unwrap_or_else(|error| error.into_inner());
                        state.writing = false;
                        match result {
                            Ok(next) => revision = next,
                            Err(error) => {
                                state.error = Some(error.clone());
                                state.unread_error = Some(error);
                                if state.pending.is_none() {
                                    state.pending = Some(queue);
                                }
                                // A compare-and-save conflict must be resolved by
                                // reloading, never by silently advancing our lease.
                                state.alive = false;
                            }
                        }
                        work.changed.notify_all();
                    }
                    notify();
                    if failed {
                        break;
                    }
                }
            });
        match worker {
            Ok(worker) => Self {
                shared,
                worker: Some(worker),
            },
            Err(error) => {
                let message = format!("Cannot start export recovery writer: {error}");
                let mut state = shared
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                state.alive = false;
                state.error = Some(message.clone());
                state.unread_error = Some(message);
                drop(state);
                Self {
                    shared,
                    worker: None,
                }
            }
        }
    }
    pub fn submit(&self, queue: Queue) -> bool {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.stopping || !state.alive {
            return false;
        }
        state.pending = Some(queue);
        self.shared.changed.notify_one();
        true
    }
    pub fn take_error(&self) -> Option<String> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .unread_error
            .take()
    }
    pub fn blocked(&self) -> bool {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .error
            .is_some()
    }
    /// For shutdown/tests, never a button handler.
    pub fn flush(&self) -> Result<(), String> {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while state.alive && (state.writing || state.pending.is_some()) {
            state = self
                .shared
                .changed
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        state.error.clone().map_or(Ok(()), Err)
    }
}
impl Drop for Journal {
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
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(format: &str) -> Value {
        json!({"version":1,"op":"export","id":"transport","raw":{"project":{"name":"Before"}},"format":format,"style":"ink","piece":"tier","size":"square","group":"model","named":false})
    }
    fn path(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }
    #[test]
    fn deduplication_and_retry_preserve_the_original_options_and_destination() {
        let mut queue = Queue::default();
        let raw = request("png");
        let first = queue
            .enqueue(raw.clone(), path("a.png"), "A".into())
            .unwrap();
        let mut same = raw.clone();
        same["id"] = json!("new-transport");
        assert!(
            queue
                .enqueue(same, path("a.png"), "Changed label".into())
                .unwrap()
                .deduplicated
        );
        let running = queue.start_next().unwrap();
        assert!(queue.start_next().is_none());
        assert!(queue.finish(first.id, running.attempt, Err("disk full".into())));
        queue.retry(first.id).unwrap();
        let retry = queue.start_next().unwrap();
        assert_eq!(retry.path, path("a.png"));
        assert_eq!(retry.request.materialize()["group"], "model");
        assert!(!queue.finish(first.id, running.attempt, Ok(())));
        assert_eq!(queue.active().unwrap().attempt, retry.attempt);
        assert!(queue.finish(first.id, retry.attempt, Ok(())));
        assert_eq!(queue.input_bytes(), 0);
    }
    #[test]
    fn shared_archives_are_stored_once_and_recovery_never_starts_a_save_automatically() {
        let mut queue = Queue::default();
        queue
            .enqueue(request("png"), path("a.png"), "PNG".into())
            .unwrap();
        queue
            .enqueue(request("svg"), path("a.svg"), "SVG".into())
            .unwrap();
        let active = queue.start_next().unwrap();
        queue.mark_writing(active.id, active.attempt);
        let snapshot = queue.snapshot();
        assert_eq!(snapshot["archives"].as_object().unwrap().len(), 1);
        let mut restored = Queue::from_snapshot(snapshot).unwrap();
        assert!(
            restored
                .jobs()
                .iter()
                .all(|job| job.state == State::Interrupted)
        );
        assert!(restored.start_next().is_none());
        restored.retry(active.id).unwrap();
        assert!(restored.start_next().is_some());
    }
    #[test]
    fn invalid_future_or_incomplete_state_is_rejected_without_an_overwrite() {
        assert!(Queue::from_snapshot(json!({"schema":2})).is_err());
        let mut queue = Queue::default();
        queue
            .enqueue(request("png"), path("a.png"), "A".into())
            .unwrap();
        let mut value = queue.snapshot();
        value["archives"] = json!({});
        assert!(Queue::from_snapshot(value).is_err());
        assert!(
            queue
                .enqueue(request("png"), PathBuf::from("relative.png"), "A".into())
                .is_err()
        );
    }
    #[test]
    fn journal_advances_only_successful_revisions_and_stops_after_conflict() {
        let revisions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = revisions.clone();
        let journal = Journal::start(
            7,
            Arc::new(move |revision, _| {
                observed.lock().unwrap().push(revision);
                if revision == 7 {
                    Ok(8)
                } else {
                    Err("another window changed history".into())
                }
            }),
            Arc::new(|| {}),
        );
        assert!(journal.submit(Queue::default()));
        journal.flush().unwrap();
        assert!(journal.submit(Queue::default()));
        assert!(journal.flush().is_err());
        assert!(journal.blocked());
        assert!(journal.take_error().is_some());
        assert!(journal.take_error().is_none());
        assert!(!journal.submit(Queue::default()));
        assert_eq!(*revisions.lock().unwrap(), vec![7, 8]);
    }
}
