//! Persistent, bounded preview service and isolated export workers.
use base64::Engine as _;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

const REQUEST_LIMIT: usize = 8 * 1024 * 1024;
const RESPONSE_LIMIT: usize = 64 * 1024 * 1024;
const STDERR_LIMIT: usize = 64 * 1024;
const QUEUE_LIMIT: usize = 32;
const QUEUE_BYTES: usize = 32 * 1024 * 1024;
const RESULT_CACHE_BYTES: usize = 32 * 1024 * 1024;
const ARCHIVE_CACHE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
/// Immutable archive snapshot shared by visible tiles and animation samples.
/// Construct on a background executor: JSON size is measured once here, never
/// again for each frame. Each renderer process registers it on first use.
#[derive(Clone)]
pub struct RenderArchive {
    raw: Arc<Value>,
    bytes: usize,
}
impl RenderArchive {
    pub fn new(raw: Arc<Value>) -> Result<Self, String> {
        if !raw.is_object() {
            return Err("An artwork archive must be an object".into());
        }
        let bytes = serde_json::to_vec(raw.as_ref())
            .map_err(|error| error.to_string())?
            .len();
        // Leave room for the archive-registration envelope and its native ID.
        if bytes > REQUEST_LIMIT - 256 {
            return Err("Artwork archive exceeds the 8 MiB transport limit".into());
        }
        Ok(Self { raw, bytes })
    }
}
struct OwnedChild {
    child: Child,
    reaped: bool,
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
#[derive(Debug)]
enum Failure {
    Transport(String),
    Application(String, String),
}
impl Failure {
    fn message(&self) -> String {
        match self {
            Self::Transport(message) => message.clone(),
            Self::Application(code, message) => format!("{code}: {message}"),
        }
    }
}
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Transport(message)
    }
}

fn resource_roots(executable: &Path) -> Vec<PathBuf> {
    let directory = executable.parent().unwrap_or_else(|| Path::new("."));
    let mut roots = Vec::new();
    if directory.file_name().is_some_and(|name| name == "MacOS") {
        if let Some(contents) = directory.parent() {
            roots.push(contents.join("Resources"));
        }
    }
    roots.push(directory.join("resources"));
    roots
}
/// Release builds always use the bundled runtime. Overrides are deliberate;
/// source checkout and PATH fallback are available only in development builds.
fn renderer_command() -> Result<Command, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("Cannot locate application: {error}"))?;
    let bridge = std::env::var_os("CREDITIA_RENDER_BRIDGE").map(PathBuf::from);
    let node = std::env::var_os("CREDITIA_NODE").map(PathBuf::from);
    let node_name = if cfg!(windows) { "node.exe" } else { "node" };
    let bundle = resource_roots(&executable).into_iter().find(|root| {
        root.join("render/tools/bridge.mjs").is_file() && root.join(node_name).is_file()
    });
    let (node, bridge) = match (node, bridge, bundle) {
        (node, bridge, Some(root)) => (node.unwrap_or_else(|| root.join(node_name)), bridge.unwrap_or_else(|| root.join("render/tools/bridge.mjs"))),
        (Some(node), Some(bridge), None) => (node, bridge),
        (node, bridge, None) if cfg!(debug_assertions) => (node.unwrap_or_else(|| PathBuf::from(node_name)), bridge.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/render/tools/bridge.mjs"))),
        _ => return Err("Artwork runtime is missing from the application bundle. Reinstall Creditia, or set CREDITIA_NODE and CREDITIA_RENDER_BRIDGE for development.".into()),
    };
    if !bridge.is_file() {
        return Err(format!("Artwork bridge is missing: {}", bridge.display()));
    }
    let mut command = Command::new(node);
    command
        .arg(bridge)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    Ok(command)
}
fn request_timeout(op: &str) -> Result<Duration, String> {
    match std::env::var("CREDITIA_RENDER_TIMEOUT_MS") {
        Ok(value) => value
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .map(Duration::from_millis)
            .ok_or_else(|| "CREDITIA_RENDER_TIMEOUT_MS must be a positive integer".into()),
        Err(std::env::VarError::NotPresent) => Ok(Duration::from_secs(match op {
            "export" => 180,
            "frames" => 120,
            _ => 45,
        })),
        Err(error) => Err(format!("Invalid renderer timeout: {error}")),
    }
}
fn validate_request(input: &Value) -> Result<usize, String> {
    if !input.get("id").is_some_and(valid_id) {
        return Err("Renderer request requires a string or safe integer id".into());
    }
    if input.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("Renderer request requires protocol version 1".into());
    }
    let bytes = serde_json::to_vec(input).map_err(|error| error.to_string())?;
    if bytes.len() > REQUEST_LIMIT {
        return Err("Renderer request exceeds 8 MiB".into());
    }
    Ok(bytes.len())
}
#[derive(Default)]
struct Budget {
    jobs: usize,
    bytes: usize,
}
struct Permit {
    budget: Arc<Mutex<Budget>>,
    bytes: usize,
}
impl Permit {
    fn reserve(budget: &Arc<Mutex<Budget>>, bytes: usize) -> Result<Self, String> {
        let mut current = budget.lock().unwrap_or_else(|e| e.into_inner());
        if current.jobs >= QUEUE_LIMIT || current.bytes.saturating_add(bytes) > QUEUE_BYTES {
            return Err("Renderer queue is full; retry after visible artwork finishes".into());
        }
        current.jobs += 1;
        current.bytes += bytes;
        Ok(Self {
            budget: budget.clone(),
            bytes,
        })
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut current = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        current.jobs = current.jobs.saturating_sub(1);
        current.bytes = current.bytes.saturating_sub(self.bytes);
    }
}
struct Job {
    input: Value,
    archive: Option<RenderArchive>,
    cancellation: Cancellation,
    deadline: Instant,
    priority: u8,
    sequence: u64,
    response: mpsc::SyncSender<Result<Value, String>>,
    _permit: Permit,
}
struct Service {
    sender: mpsc::SyncSender<Job>,
    budget: Arc<Mutex<Budget>>,
}
static SERVICE: OnceLock<Result<Service, String>> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(1);
static EXPORT_SLOT: Mutex<()> = Mutex::new(());

/// 0: focused art, 1: visible tiles, larger: preloads. Queued cancellation is
/// local to its job and cannot kill another request's process.
pub fn request_with_priority(
    input: Value,
    cancellation: Cancellation,
    priority: u8,
) -> Result<Value, String> {
    request_internal(input, None, cancellation, priority)
}
pub fn request_with_archive(
    input: Value,
    archive: RenderArchive,
    cancellation: Cancellation,
    priority: u8,
) -> Result<Value, String> {
    if !matches!(input["op"].as_str(), Some("render" | "frames"))
        || input.get("raw").is_some()
        || input.get("archiveId").is_some()
    {
        return Err(
            "An archive handle requires a render/frames request without raw or archiveId".into(),
        );
    }
    request_internal(input, Some(archive), cancellation, priority)
}
fn request_internal(
    input: Value,
    archive: Option<RenderArchive>,
    cancellation: Cancellation,
    priority: u8,
) -> Result<Value, String> {
    let bytes = validate_request(&input)?
        .saturating_add(archive.as_ref().map_or(0, |archive| archive.bytes));
    if cancellation.cancelled() {
        return Err("Renderer request cancelled".into());
    }
    let deadline = Instant::now()
        .checked_add(request_timeout(input["op"].as_str().unwrap_or(""))?)
        .ok_or("Renderer timeout is out of range")?;
    if input["op"] == "export" {
        let _slot = loop {
            if cancellation.cancelled() {
                return Err("Export cancelled".into());
            }
            if Instant::now() >= deadline {
                return Err("Export timed out while queued".into());
            }
            match EXPORT_SLOT.try_lock() {
                Ok(slot) => break slot,
                Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    thread::sleep(Duration::from_millis(20))
                }
            }
        };
        let mut session = Session::start(renderer_command()).map_err(|error| error.message())?;
        return session
            .exchange(input, &cancellation, deadline)
            .map_err(|error| error.message());
    }
    let service = SERVICE
        .get_or_init(|| {
            let (sender, receiver) = mpsc::sync_channel(QUEUE_LIMIT);
            thread::Builder::new()
                .name("creditia-artwork-service".into())
                .spawn(move || supervise(receiver))
                .map_err(|error| format!("Unable to start artwork service: {error}"))?;
            Ok(Service {
                sender,
                budget: Arc::new(Mutex::new(Budget::default())),
            })
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let permit = Permit::reserve(&service.budget, bytes)?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let job = Job {
        input,
        archive,
        cancellation: cancellation.clone(),
        deadline,
        priority,
        sequence: SEQUENCE.fetch_add(1, Ordering::Relaxed),
        response: sender,
        _permit: permit,
    };
    service.sender.try_send(job).map_err(|error| match error {
        mpsc::TrySendError::Full(_) => "Renderer queue is full".to_string(),
        mpsc::TrySendError::Disconnected(_) => {
            "Artwork service stopped unexpectedly; restart the application".to_string()
        }
    })?;
    loop {
        if cancellation.cancelled() {
            return Err("Renderer request cancelled".into());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            cancellation.cancel();
            return Err("Renderer request timed out".into());
        }
        match receiver.recv_timeout(remaining.min(Duration::from_millis(20))) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Artwork service stopped before returning a result".into());
            }
        }
    }
}
pub fn request(input: Value, cancellation: Cancellation) -> Result<Value, String> {
    request_with_priority(input, cancellation, 1)
}

fn supervise(receiver: mpsc::Receiver<Job>) {
    let mut session: Option<Session> = None;
    let mut pending: Vec<Job> = Vec::new();
    loop {
        if pending.is_empty() {
            match receiver.recv_timeout(Duration::from_secs(60)) {
                Ok(job) => pending.push(job),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    session = None;
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        while let Ok(job) = receiver.try_recv() {
            pending.push(job);
        }
        pending.sort_by_key(|job| (job.priority, job.sequence));
        let job = pending.remove(0);
        if job.cancellation.cancelled() || Instant::now() >= job.deadline {
            let _ = job.response.send(Err(
                "Renderer request cancelled or expired while queued".into()
            ));
            continue;
        }
        if session.is_none() {
            match Session::start(renderer_command()) {
                Ok(new) => session = Some(new),
                Err(error) => {
                    let _ = job.response.send(Err(error.message()));
                    continue;
                }
            }
        }
        let result = session.as_mut().expect("started renderer").render_archive(
            job.input,
            job.archive,
            &job.cancellation,
            job.deadline,
        );
        if matches!(result, Err(Failure::Transport(_))) {
            session = None;
        }
        let _ = job.response.send(result.map_err(|error| error.message()));
    }
}

enum Event {
    Written(u64, Result<(), String>),
    Record(Result<Vec<u8>, String>),
}
struct WriteJob {
    ticket: u64,
    bytes: Vec<u8>,
}
struct Archive {
    id: String,
    source: RenderArchive,
}
struct ResultEntry {
    key: Vec<u8>,
    value: Value,
    cost: usize,
}
struct Session {
    child: OwnedChild,
    writes: mpsc::SyncSender<WriteJob>,
    events: mpsc::Receiver<Event>,
    diagnostics: Arc<Mutex<Vec<u8>>>,
    archives: VecDeque<Archive>,
    results: VecDeque<ResultEntry>,
    archive_bytes: usize,
    result_bytes: usize,
    ticket: u64,
}
impl Session {
    fn start(command: Result<Command, String>) -> Result<Self, Failure> {
        let mut command = command?;
        let mut child = OwnedChild {
            child: command
                .spawn()
                .map_err(|error| format!("Unable to start renderer: {error}"))?,
            reaped: false,
        };
        let mut stdin = child
            .child
            .stdin
            .take()
            .ok_or_else(|| "Renderer stdin is unavailable".to_string())?;
        let stdout = child
            .child
            .stdout
            .take()
            .ok_or_else(|| "Renderer stdout is unavailable".to_string())?;
        let mut stderr = child
            .child
            .stderr
            .take()
            .ok_or_else(|| "Renderer stderr is unavailable".to_string())?;
        let (events_sender, events) = mpsc::sync_channel(4);
        let (writes, writer) = mpsc::sync_channel::<WriteJob>(1);
        let sender = events_sender.clone();
        thread::Builder::new()
            .name("creditia-render-input".into())
            .spawn(move || {
                while let Ok(job) = writer.recv() {
                    let result = stdin
                        .write_all(&job.bytes)
                        .and_then(|_| stdin.flush())
                        .map_err(|error| format!("Unable to write renderer request: {error}"));
                    let failed = result.is_err();
                    if sender.send(Event::Written(job.ticket, result)).is_err() || failed {
                        break;
                    }
                }
            })
            .map_err(|error| format!("Unable to start renderer input: {error}"))?;
        thread::Builder::new()
            .name("creditia-render-output".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    let record = read_record(&mut reader, RESPONSE_LIMIT);
                    let failed = record.is_err();
                    if events_sender.send(Event::Record(record)).is_err() || failed {
                        break;
                    }
                }
            })
            .map_err(|error| format!("Unable to start renderer output: {error}"))?;
        let diagnostics = Arc::new(Mutex::new(Vec::new()));
        let captured = diagnostics.clone();
        thread::Builder::new()
            .name("creditia-render-error".into())
            .spawn(move || {
                let mut chunk = [0u8; 4096];
                loop {
                    match stderr.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(length) => {
                            let mut bytes = captured.lock().unwrap_or_else(|e| e.into_inner());
                            let available = STDERR_LIMIT.saturating_sub(bytes.len());
                            bytes.extend_from_slice(&chunk[..length.min(available)]);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
            })
            .map_err(|error| format!("Unable to start renderer diagnostics: {error}"))?;
        Ok(Self {
            child,
            writes,
            events,
            diagnostics,
            archives: VecDeque::new(),
            results: VecDeque::new(),
            archive_bytes: 0,
            result_bytes: 0,
            ticket: 0,
        })
    }
    fn exchange(
        &mut self,
        input: Value,
        cancellation: &Cancellation,
        deadline: Instant,
    ) -> Result<Value, Failure> {
        if cancellation.cancelled() {
            return Err(Failure::Transport("Renderer request cancelled".into()));
        }
        let id = input["id"].clone();
        let mut bytes = serde_json::to_vec(&input).map_err(|error| error.to_string())?;
        if bytes.len() > REQUEST_LIMIT {
            return Err(Failure::Application(
                "request_too_large".into(),
                "Request exceeds 8 MiB".into(),
            ));
        }
        bytes.push(b'\n');
        self.ticket = self.ticket.wrapping_add(1);
        self.writes
            .try_send(WriteJob {
                ticket: self.ticket,
                bytes,
            })
            .map_err(|error| format!("Renderer input is unavailable: {error}"))?;
        let mut written = false;
        let mut response = None;
        loop {
            if cancellation.cancelled() {
                return Err(Failure::Transport("Renderer request cancelled".into()));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Failure::Transport("Renderer request timed out".into()));
            }
            if let Some(status) = self
                .child
                .child
                .try_wait()
                .map_err(|error| format!("Unable to inspect renderer: {error}"))?
            {
                self.child.reaped = true;
                let detail = self.diagnostics.lock().unwrap_or_else(|e| e.into_inner());
                return Err(Failure::Transport(format!(
                    "Renderer exited with {status}: {}",
                    String::from_utf8_lossy(&detail).trim()
                )));
            }
            match self
                .events
                .recv_timeout(remaining.min(Duration::from_millis(20)))
            {
                Ok(Event::Written(ticket, result)) => {
                    if ticket != self.ticket {
                        return Err(Failure::Transport("Renderer input order changed".into()));
                    }
                    result?;
                    written = true;
                }
                Ok(Event::Record(result)) => {
                    if response.is_some() {
                        return Err(Failure::Transport(
                            "Renderer returned multiple records for one request".into(),
                        ));
                    }
                    response = Some(result?);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(Failure::Transport("Renderer closed its pipes".into()));
                }
            }
            if written {
                if let Some(bytes) = response {
                    return decode_response(&bytes, &id);
                }
            }
        }
    }
    #[cfg(test)]
    fn render(
        &mut self,
        input: Value,
        cancellation: &Cancellation,
        deadline: Instant,
    ) -> Result<Value, Failure> {
        self.render_archive(input, None, cancellation, deadline)
    }
    fn render_archive(
        &mut self,
        mut input: Value,
        archive: Option<RenderArchive>,
        cancellation: &Cancellation,
        deadline: Instant,
    ) -> Result<Value, Failure> {
        let raw = if matches!(input["op"].as_str(), Some("render" | "frames")) {
            input.as_object_mut().and_then(|input| input.remove("raw"))
        } else {
            None
        };
        let archive = match (archive, raw) {
            (Some(archive), None) => Some(archive),
            (None, Some(raw)) => Some(
                RenderArchive::new(Arc::new(raw))
                    .map_err(|message| Failure::Application("invalid_archive".into(), message))?,
            ),
            (None, None) => None,
            (Some(_), Some(_)) => {
                return Err(Failure::Application(
                    "invalid_archive".into(),
                    "Archive handle and raw cannot be combined".into(),
                ));
            }
        };
        if let Some(archive) = archive {
            let id = self.register(archive.clone(), cancellation, deadline)?;
            input["archiveId"] = json!(id);
            let result = self.cached_exchange(input.clone(), cancellation, deadline);
            if matches!(&result, Err(Failure::Application(code, _)) if code == "unknown_archive") {
                self.archives.retain(|archive| archive.id != id);
                self.archive_bytes = self
                    .archives
                    .iter()
                    .map(|archive| archive.source.bytes)
                    .sum();
                input["archiveId"] = json!(self.register(archive, cancellation, deadline)?);
                return self.cached_exchange(input, cancellation, deadline);
            }
            result
        } else {
            self.cached_exchange(input, cancellation, deadline)
        }
    }
    fn register(
        &mut self,
        source: RenderArchive,
        cancellation: &Cancellation,
        deadline: Instant,
    ) -> Result<String, Failure> {
        if let Some(index) = self.archives.iter().position(|archive| {
            Arc::ptr_eq(&archive.source.raw, &source.raw) || archive.source.raw == source.raw
        }) {
            let archive = self.archives.remove(index).expect("matched archive");
            let id = archive.id.clone();
            self.archives.push_back(archive);
            return Ok(id);
        }
        let cost = source.bytes;
        while self.archives.len() >= 4
            || self.archive_bytes.saturating_add(cost) > ARCHIVE_CACHE_BYTES
        {
            let Some(old) = self.archives.pop_front() else {
                break;
            };
            self.archive_bytes = self.archive_bytes.saturating_sub(old.source.bytes);
            self.exchange(
                json!({"version":1,"id":"native-release","op":"release","archiveId":old.id}),
                cancellation,
                deadline,
            )?;
            self.results.clear();
            self.result_bytes = 0;
        }
        let id = format!("native-{}", SEQUENCE.fetch_add(1, Ordering::Relaxed));
        self.exchange(json!({"version":1,"id":"native-archive","op":"archive","archiveId":id,"raw":source.raw.as_ref()}), cancellation, deadline)?;
        self.archive_bytes += cost;
        self.archives.push_back(Archive {
            id: id.clone(),
            source,
        });
        Ok(id)
    }
    fn cached_exchange(
        &mut self,
        input: Value,
        cancellation: &Cancellation,
        deadline: Instant,
    ) -> Result<Value, Failure> {
        let cacheable = matches!(input["op"].as_str(), Some("catalog" | "render" | "frames"));
        let mut key_value = input.clone();
        key_value
            .as_object_mut()
            .expect("validated request")
            .remove("id");
        let key = serde_json::to_vec(&key_value).map_err(|error| error.to_string())?;
        if cacheable {
            if let Some(index) = self.results.iter().position(|entry| entry.key == key) {
                if cancellation.cancelled() {
                    return Err(Failure::Transport("Renderer request cancelled".into()));
                }
                let entry = self.results.remove(index).expect("matched result");
                let result = entry.value.clone();
                self.results.push_back(entry);
                return Ok(result);
            }
        }
        let value = self.exchange(input, cancellation, deadline)?;
        if cacheable {
            let cost = key.len().saturating_add(
                serde_json::to_vec(&value)
                    .map_err(|error| error.to_string())?
                    .len(),
            );
            if cost <= RESULT_CACHE_BYTES / 2 {
                while self.result_bytes.saturating_add(cost) > RESULT_CACHE_BYTES {
                    let Some(old) = self.results.pop_front() else {
                        break;
                    };
                    self.result_bytes = self.result_bytes.saturating_sub(old.cost);
                }
                self.result_bytes += cost;
                self.results.push_back(ResultEntry {
                    key,
                    value: value.clone(),
                    cost,
                });
            }
        }
        Ok(value)
    }
}
fn read_record(reader: &mut impl BufRead, limit: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|error| format!("Renderer output read failed: {error}"))?;
        if available.is_empty() {
            return Err("Renderer closed stdout before a complete response".into());
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let length = newline.map_or(available.len(), |index| index + 1);
        if out.len().saturating_add(length) > limit + 1 {
            return Err("Renderer response exceeds 64 MiB".into());
        }
        out.extend_from_slice(&available[..length]);
        reader.consume(length);
        if newline.is_some() {
            return Ok(out);
        }
    }
}
fn decode_response(bytes: &[u8], id: &Value) -> Result<Value, Failure> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| format!("Renderer output is not UTF-8: {error}"))?
        .trim();
    if text.len() > RESPONSE_LIMIT {
        return Err(Failure::Transport(
            "Renderer response exceeds 64 MiB".into(),
        ));
    }
    let mut response: Value = serde_json::from_str(text)
        .map_err(|error| format!("Renderer returned invalid JSON: {error}"))?;
    if response.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(Failure::Transport(
            "Renderer returned an unsupported protocol version".into(),
        ));
    }
    if response.get("id") != Some(id) {
        return Err(Failure::Transport(
            "Renderer response id does not match the request".into(),
        ));
    }
    match response.get("ok").and_then(Value::as_bool) {
        Some(true) => response
            .as_object_mut()
            .and_then(|object| object.remove("result"))
            .ok_or_else(|| Failure::Transport("Renderer response has no result".into())),
        Some(false) => Err(Failure::Application(
            response["error"]["code"]
                .as_str()
                .unwrap_or("render_failed")
                .into(),
            response["error"]["message"]
                .as_str()
                .unwrap_or("Renderer request failed")
                .into(),
        )),
        None => Err(Failure::Transport(
            "Renderer response has no boolean ok field".into(),
        )),
    }
}
/// Decode only a matching export response. The format is the user's chosen
/// format, not an extension or MIME type supplied by the renderer.
pub fn decode_export(result: &Value, format: &str) -> Result<Vec<u8>, String> {
    let mime = match format {
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "gif" => "image/gif",
        "html" => "text/html",
        "md" => "text/markdown",
        "txt" | "embed" => "text/plain",
        "json" => "application/json",
        _ => return Err(format!("Unsupported export format: {format}")),
    };
    if result["encoding"].as_str() != Some("base64") {
        return Err("Renderer export is not base64 encoded".into());
    }
    let extension = if format == "embed" { "html" } else { format };
    if result["ext"].as_str() != Some(extension) || result["mime"].as_str() != Some(mime) {
        return Err(format!(
            "Renderer export does not match the selected {format} format"
        ));
    }
    let encoded = result["bytes"]
        .as_str()
        .ok_or("Renderer export has no encoded bytes")?;
    if encoded.len() > RESPONSE_LIMIT {
        return Err("Renderer export exceeds 64 MiB".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| format!("Renderer export contains invalid base64: {error}"))?;
    if bytes.is_empty() {
        return Err("Renderer export is empty".into());
    }
    Ok(bytes)
}

struct PendingExport {
    path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl Drop for PendingExport {
    fn drop(&mut self) {
        drop(self.file.take());
        if self.committed {
            return;
        }
        #[cfg(windows)]
        if let Ok(metadata) = fs::metadata(&self.path) {
            // A failed replacement may leave preserved read-only permissions on
            // our temporary file. Clear those only on that private temp path.
            let mut permissions = metadata.permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                let _ = fs::set_permissions(&self.path, permissions);
            }
        }
        let _ = fs::remove_file(&self.path);
    }
}

/// Commit a completed export to a path chosen by the native save dialog. The
/// caller owns overwrite confirmation. Cancellation or any pre-commit failure
/// leaves the previous destination untouched and removes the temporary file.
///
/// The temporary file is on the destination filesystem. Rust's `fs::rename`
/// replaces an existing file on Unix and Windows without deleting it first.
/// After rename succeeds the export is committed, so a concurrent late cancel
/// cannot turn a successfully written file into a reported cancellation.
pub fn write_export(path: &Path, bytes: &[u8], cancellation: Cancellation) -> Result<(), String> {
    if path.file_name().is_none() {
        return Err("Export destination must name a file".into());
    }
    if bytes.is_empty() {
        return Err("Cannot write an empty export".into());
    }
    if cancellation.cancelled() {
        return Err("Export cancelled".into());
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut pending = create_export_temp(parent)?;
    let file = pending.file.as_mut().expect("new temporary export file");
    for chunk in bytes.chunks(64 * 1024) {
        if cancellation.cancelled() {
            return Err("Export cancelled".into());
        }
        file.write_all(chunk)
            .map_err(|error| format!("Unable to write export: {error}"))?;
    }
    file.flush()
        .map_err(|error| format!("Unable to flush export: {error}"))?;
    // Preserve the destination's access permissions when replacing a regular
    // file. A new Unix export starts private; its directory controls Windows ACLs.
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.is_file() {
            file.set_permissions(metadata.permissions())
                .map_err(|error| format!("Unable to preserve export permissions: {error}"))?;
        }
    }
    file.sync_all()
        .map_err(|error| format!("Unable to sync export: {error}"))?;
    drop(pending.file.take()); // Windows must not retain an open temporary-file handle.
    if cancellation.cancelled() {
        return Err("Export cancelled".into());
    }
    fs::rename(&pending.path, path).map_err(|error| format!("Unable to save export: {error}"))?;
    pending.committed = true;
    Ok(())
}

fn create_export_temp(parent: &Path) -> Result<PendingExport, String> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for _ in 0..128 {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".creditia-export-{}-{stamp}-{sequence}.tmp",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => {
                return Ok(PendingExport {
                    path,
                    file: Some(file),
                    committed: false,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Unable to create export file: {error}")),
        }
    }
    Err("Unable to reserve a temporary export file".into())
}

fn valid_id(id: &Value) -> bool {
    if let Some(value) = id.as_str() {
        return value.encode_utf16().count() <= 128;
    }
    if let Some(value) = id.as_i64() {
        return value.unsigned_abs() <= 9_007_199_254_740_991;
    }
    id.as_u64()
        .is_some_and(|value| value <= 9_007_199_254_740_991)
}

#[cfg(test)]
fn read_bounded(mut reader: impl Read, limit: usize, truncate: bool) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let length = match reader.read(&mut chunk) {
            Ok(length) => length,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("Renderer pipe read failed: {error}")),
        };
        if length == 0 {
            return Ok(bytes);
        }
        let available = limit.saturating_sub(bytes.len());
        if length > available && !truncate {
            return Err("Renderer response exceeds 64 MiB".into());
        }
        bytes.extend_from_slice(&chunk[..length.min(available)]);
        // Stderr keeps only the first 64 KiB but must still be drained so a noisy
        // renderer cannot deadlock on a full diagnostic pipe.
    }
}

#[cfg(test)]
fn parse_response(bytes: &[u8], id: &Value) -> Result<Value, String> {
    decode_response(bytes, id).map_err(|error| error.message())
}

#[cfg(test)]
mod tests {
    use super::{
        Budget, Cancellation, Failure, Permit, QUEUE_BYTES, Session, decode_export, parse_response,
        read_bounded, read_record, resource_roots, valid_id, write_export,
    };
    use base64::Engine as _;
    use serde_json::json;
    use std::{
        fs,
        io::BufReader,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    };

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "creditia-render-export-test-{}-{stamp}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn rejects_wrong_request_and_extra_records() {
        assert!(
            parse_response(br#"{"version":1,"id":2,"ok":true,"result":{}}"#, &json!(1)).is_err()
        );
        assert!(
            parse_response(
                b"{\"version\":1,\"id\":1,\"ok\":true,\"result\":{}}\n{}",
                &json!(1)
            )
            .is_err()
        );
        assert_eq!(
            parse_response(
                b"{\"version\":1,\"id\":1,\"ok\":true,\"result\":{\"art\":null}}\n",
                &json!(1)
            )
            .unwrap(),
            json!({"art":null})
        );
    }

    #[test]
    fn preserves_renderer_error_and_bounds_diagnostics() {
        assert_eq!(parse_response(br#"{"version":1,"id":"r","ok":false,"error":{"code":"missing_data","message":"No sessions"}}"#, &json!("r")).unwrap_err(), "missing_data: No sessions");
        assert_eq!(read_bounded(&b"abcdef"[..], 3, true).unwrap(), b"abc");
        assert!(read_bounded(&b"abcdef"[..], 3, false).is_err());
    }

    #[test]
    fn ids_match_javascript_safe_integer_and_utf16_limits() {
        assert!(valid_id(&json!(9_007_199_254_740_991u64)));
        assert!(!valid_id(&json!(9_007_199_254_740_992u64)));
        assert!(!valid_id(&json!(1.5)));
        assert!(valid_id(&json!("😀".repeat(64))));
        assert!(!valid_id(&json!("😀".repeat(65))));
    }

    #[test]
    fn export_response_must_match_requested_format() {
        let bytes = b"complete rendered output";
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        for (format, mime) in [
            ("png", "image/png"),
            ("svg", "image/svg+xml"),
            ("pdf", "application/pdf"),
            ("gif", "image/gif"),
            ("html", "text/html"),
        ] {
            assert_eq!(
                decode_export(
                    &json!({"encoding":"base64","ext":format,"mime":mime,"bytes":encoded}),
                    format
                )
                .unwrap(),
                bytes
            );
        }
        assert!(
            decode_export(
                &json!({"encoding":"base64","ext":"png","mime":"image/png","bytes":encoded}),
                "pdf"
            )
            .is_err()
        );
        assert!(
            decode_export(
                &json!({"encoding":"base64","ext":"png","mime":"image/png","bytes":"invalid%"}),
                "png"
            )
            .is_err()
        );
        assert!(
            decode_export(
                &json!({"encoding":"base64","ext":"png","mime":"image/png","bytes":""}),
                "png"
            )
            .is_err()
        );
        assert!(decode_export(&json!({}), "jpg").is_err());
    }

    #[test]
    fn replacement_commits_complete_bytes_and_cleans_temporary_file() {
        let directory = TestDirectory::new();
        let destination = directory.0.join("piece.svg");
        fs::write(&destination, b"original").unwrap();
        write_export(
            &destination,
            b"complete replacement",
            Cancellation::default(),
        )
        .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"complete replacement");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn cancel_and_failed_commit_preserve_destination_and_remove_temporary_file() {
        let directory = TestDirectory::new();
        let destination = directory.0.join("piece.svg");
        fs::write(&destination, b"original").unwrap();
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(write_export(&destination, b"replacement", cancellation).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"original");
        let existing_directory = directory.0.join("folder");
        fs::create_dir(&existing_directory).unwrap();
        fs::write(existing_directory.join("keep"), b"untouched").unwrap();
        assert!(
            write_export(&existing_directory, b"replacement", Cancellation::default()).is_err()
        );
        assert_eq!(
            fs::read(existing_directory.join("keep")).unwrap(),
            b"untouched"
        );
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }

    fn fixture_session() -> Session {
        // A tiny actual JSONL process verifies pipe/lifecycle behavior, independent
        // of Satori/font availability. The native CI installs Node for the app.
        const SOURCE: &str = r#"
import { createInterface } from 'node:readline';
let registrations=0, renders=0; const archives=new Map();
for await (const line of createInterface({input:process.stdin})) {
  const r=JSON.parse(line); let result;
  if(r.op==='archive'){archives.set(r.archiveId,r.raw);registrations++;result={archiveId:r.archiveId};}
  else if(r.op==='release'){archives.delete(r.archiveId);result={released:true};}
  else if(r.op==='render'){
    if(r.piece==='delay') await new Promise(resolve=>setTimeout(resolve,5000));
    if(r.piece==='crash') process.exit(9);
    if(r.piece==='error'){process.stdout.write(JSON.stringify({version:1,id:r.id,ok:false,error:{code:'missing_data',message:'fixture'}})+'\n');continue;}
    result={raw:archives.get(r.archiveId),registrations,renders:++renders,pid:process.pid};
  } else result={pid:process.pid};
  process.stdout.write(JSON.stringify({version:1,id:r.id,ok:true,result})+'\n');
}
"#;
        let mut command = Command::new("node");
        command
            .args(["--input-type=module", "-e", SOURCE])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Session::start(Ok(command)).unwrap()
    }

    #[test]
    fn persistent_worker_reuses_archives_and_results_without_cross_project_mixup() {
        let mut worker = fixture_session();
        let cancel = Cancellation::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        let first = worker
            .render(
                json!({"version":1,"id":1,"op":"render","raw":{"project":"one"},"piece":"tier"}),
                &cancel,
                deadline,
            )
            .unwrap();
        let cached = worker
            .render(
                json!({"version":1,"id":2,"op":"render","raw":{"project":"one"},"piece":"tier"}),
                &cancel,
                deadline,
            )
            .unwrap();
        assert_eq!(first, cached);
        let next = worker
            .render(
                json!({"version":1,"id":3,"op":"render","raw":{"project":"one"},"piece":"roll"}),
                &cancel,
                deadline,
            )
            .unwrap();
        assert_eq!(next["registrations"], 1);
        assert_eq!(next["renders"], 2);
        assert_eq!(next["pid"], first["pid"]);
        let other = worker
            .render(
                json!({"version":1,"id":4,"op":"render","raw":{"project":"two"},"piece":"tier"}),
                &cancel,
                deadline,
            )
            .unwrap();
        assert_eq!(other["registrations"], 2);
        assert_eq!(other["raw"]["project"], "two");
    }

    #[test]
    fn immutable_archive_handles_share_storage_and_register_again_after_restart() {
        let raw = Arc::new(json!({"project":"immutable", "commits":[{"id":"one"}]}));
        let archive = RenderArchive::new(raw.clone()).unwrap();
        let cancel = Cancellation::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        let request = json!({"version":1,"id":"first","op":"render","piece":"tier"});
        let mut worker = fixture_session();
        let first = worker
            .render_archive(request.clone(), Some(archive.clone()), &cancel, deadline)
            .unwrap();
        assert!(Arc::ptr_eq(&worker.archives[0].source.raw, &raw));
        let next = worker
            .render_archive(
                json!({"version":1,"id":"next","op":"render","piece":"roll"}),
                Some(archive.clone()),
                &cancel,
                deadline,
            )
            .unwrap();
        assert_eq!(next["registrations"], 1);
        assert_eq!(next["renders"], 2);
        drop(worker);
        let mut restarted = fixture_session();
        let restored = restarted
            .render_archive(request, Some(archive), &cancel, deadline)
            .unwrap();
        assert_eq!(restored["raw"], first["raw"]);
        assert_eq!(restored["registrations"], 1);
        assert_ne!(restored["pid"], first["pid"]);
    }

    #[test]
    fn archive_handles_reject_ambiguous_requests_and_invalid_snapshots() {
        assert!(RenderArchive::new(Arc::new(Value::Null)).is_err());
        assert!(RenderArchive::new(Arc::new(json!({"text":"x".repeat(REQUEST_LIMIT)}))).is_err());
        let archive = RenderArchive::new(Arc::new(json!({}))).unwrap();
        for request in [
            json!({"op":"export"}),
            json!({"op":"render","raw":{}}),
            json!({"op":"frames","archiveId":"other"}),
        ] {
            assert!(
                request_with_archive(request, archive.clone(), Cancellation::default(), 0).is_err()
            );
        }
    }

    #[test]
    fn business_errors_keep_worker_alive_but_active_cancellation_is_transport_failure() {
        let mut worker = fixture_session();
        let cancel = Cancellation::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        let error = worker
            .render(
                json!({"version":1,"id":1,"op":"render","raw":{},"piece":"error"}),
                &cancel,
                deadline,
            )
            .unwrap_err();
        assert!(matches!(error, Failure::Application(_, _)));
        assert!(
            worker
                .render(
                    json!({"version":1,"id":2,"op":"render","raw":{},"piece":"tier"}),
                    &cancel,
                    deadline
                )
                .is_ok()
        );
        let control = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            control.cancel();
        });
        let started = Instant::now();
        let error = worker
            .render(
                json!({"version":1,"id":3,"op":"render","raw":{},"piece":"delay"}),
                &cancel,
                deadline,
            )
            .unwrap_err();
        assert!(matches!(error, Failure::Transport(_)));
        assert!(started.elapsed() < Duration::from_secs(2));
        drop(worker); // kills and reaps the suspended child, permitting a fresh service.
        let mut restarted = fixture_session();
        assert!(
            restarted
                .render(
                    json!({"version":1,"id":4,"op":"render","raw":{},"piece":"tier"}),
                    &Cancellation::default(),
                    deadline
                )
                .is_ok()
        );
    }

    #[test]
    fn worker_crash_does_not_return_a_stale_result() {
        let mut worker = fixture_session();
        let result = worker.render(
            json!({"version":1,"id":1,"op":"render","raw":{},"piece":"crash"}),
            &Cancellation::default(),
            Instant::now() + Duration::from_secs(5),
        );
        assert!(matches!(result, Err(Failure::Transport(_))));
    }

    #[test]
    fn queue_budget_releases_capacity_and_line_reader_keeps_records_separate() {
        let budget = Arc::new(Mutex::new(Budget::default()));
        let permit = Permit::reserve(&budget, QUEUE_BYTES).unwrap();
        assert!(Permit::reserve(&budget, 1).is_err());
        drop(permit);
        assert!(Permit::reserve(&budget, 1).is_ok());
        let mut reader = BufReader::new(&b"one\ntwo\n"[..]);
        assert_eq!(read_record(&mut reader, 3).unwrap(), b"one\n");
        assert_eq!(read_record(&mut reader, 3).unwrap(), b"two\n");
        assert!(read_record(&mut reader, 3).is_err());
        assert!(read_record(&mut BufReader::new(&b"long\n"[..]), 3).is_err());
    }

    #[test]
    fn bundled_resources_are_found_relative_to_the_executable() {
        assert_eq!(
            resource_roots(Path::new(
                "/Applications/Creditia.app/Contents/MacOS/creditia-gpui"
            ))[0],
            Path::new("/Applications/Creditia.app/Contents/Resources")
        );
        assert_eq!(
            resource_roots(Path::new("/opt/creditia/creditia-gpui"))[0],
            Path::new("/opt/creditia/resources")
        );
    }
}
