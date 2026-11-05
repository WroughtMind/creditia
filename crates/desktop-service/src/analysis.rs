use crate::{core_cmd, core_result, Fail, Res};
use serde_json::Value;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Runs on the worker's stderr reader thread. Callers must forward this value to
/// their UI executor; they must not access window state directly in the callback.
pub type ProgressSink = Arc<dyn Fn(Value) + Send + Sync + 'static>;

/// Cancellation can be requested before a background executor starts the job.
/// A token is single-use; create a fresh one for every analysis or add-project job.
#[derive(Clone, Default)]
pub struct AnalysisCancellation {
    cancelled: Arc<AtomicBool>,
}
impl AnalysisCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let mut slot = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(running) = slot.as_mut() {
            if Arc::ptr_eq(&self.cancelled, &running.cancelled) {
                stop(&mut running.child);
            }
        }
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

struct Running {
    child: Child,
    cancelled: Arc<AtomicBool>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

// Keep the operation reserved until its output has drained. Git descendants can
// outlive the worker while retaining a pipe, and cancellation must still be able
// to kill that process group after the worker itself has exited.
struct RunningGuard(Arc<AtomicBool>);
impl Drop for RunningGuard {
    fn drop(&mut self) {
        let mut slot = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        if slot.as_ref().is_some_and(|running| Arc::ptr_eq(&self.0, &running.cancelled)) {
            *slot = None;
        }
    }
}

pub fn analysis_running() -> bool {
    RUNNING.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

fn drain<R: std::io::Read + Send + 'static>(reader: Option<R>) -> Option<std::thread::JoinHandle<Vec<u8>>> {
    reader.map(|mut reader| {
        std::thread::spawn(move || {
            let mut bytes = vec![];
            let _ = reader.read_to_end(&mut bytes);
            bytes
        })
    })
}

fn drain_err<R: std::io::Read + Send + 'static>(reader: Option<R>, progress: Option<ProgressSink>) -> Option<std::thread::JoinHandle<Vec<u8>>> {
    use std::io::BufRead;
    reader.map(|reader| {
        std::thread::spawn(move || {
            let mut rest = vec![];
            for line in std::io::BufReader::new(reader).split(b'\n').map_while(Result::ok) {
                if let Some(payload) = line.strip_prefix(b"@progress ") {
                    if let (Some(sink), Ok(value)) = (&progress, serde_json::from_slice::<Value>(payload)) {
                        sink(value);
                    }
                } else {
                    rest.extend_from_slice(&line);
                    rest.push(b'\n');
                }
            }
            rest
        })
    })
}

/// Runs exactly the existing `creditia-core analyze --repo … --github --json`
/// contract. A second concurrent analysis returns `busy`.
pub fn run_analysis(repo: &str, lang: Option<&str>, progress: Option<ProgressSink>) -> Res<Value> {
    run_analysis_with_cancel(repo, lang, progress, AnalysisCancellation::default())
}

pub fn run_analysis_with_cancel(repo: &str, lang: Option<&str>, progress: Option<ProgressSink>, cancellation: AnalysisCancellation) -> Res<Value> {
    run_core_job(&["analyze", "--repo", repo, "--github"], lang, progress, cancellation, Some(repo))
}

/// Same cancellation and exclusivity contract for evidence collection/import.
pub fn run_core_job_with_cancel(args: &[&str], lang: Option<&str>, progress: Option<ProgressSink>, cancellation: AnalysisCancellation) -> Res<Value> {
    run_core_job(args, lang, progress, cancellation, None)
}

fn job_lease() -> Res<std::fs::File> {
    let directory = creditia_store::data_dir();
    std::fs::create_dir_all(&directory).map_err(|e| Fail::with("io", e))?;
    std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(directory.join("desktop-analysis.lock")).map_err(|e| Fail::with("io", e))
}

fn run_core_job(args: &[&str], lang: Option<&str>, progress: Option<ProgressSink>, cancellation: AnalysisCancellation, tracked_repo: Option<&str>) -> Res<Value> {
    // Cancellation belongs to this operation, not to the next one that starts
    // after its process exits. A global boolean would race with a fast restart.
    if cancellation.is_cancelled() { return Err(Fail::new("cancelled")); }
    let cancelled = cancellation.cancelled.clone();
    let lease = job_lease()?;
    lease.try_lock().map_err(|_| Fail::new("busy"))?;
    let task_id = format!("{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
    let (out, err) = {
        let mut slot = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        if cancellation.is_cancelled() {
            return Err(Fail::new("cancelled"));
        }
        if slot.is_some() {
            return Err(Fail::new("busy"));
        }
        let mut cmd = core_cmd(args);
        if let Some(lang) = lang { cmd.env("CREDITIA_LANG", lang); }
        cmd.env("CREDITIA_PROGRESS", "1");
        let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| Fail::with("core_missing", e))?;
        if let Some(repo) = tracked_repo {
            let record = serde_json::json!({"id":task_id,"state":"running","repo":repo,"lang":lang.unwrap_or("zh"),"startedAt":now()});
            if let Err(error) = crate::edit_app_settings(|settings| settings["tasks"]["analysis"] = record) {
                stop(&mut child); let _ = child.wait(); return Err(error);
            }
        }
        let pipes = (drain(child.stdout.take()), drain_err(child.stderr.take(), progress));
        *slot = Some(Running { child, cancelled: cancelled.clone() });
        pipes
    };
    let _running = RunningGuard(cancelled.clone());
    let result = (|| {
    // Poll the process rather than joining pipes first: child Git processes may
    // hold a pipe open until the entire process group is cancelled.
    let status = loop {
        {
            let mut slot = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
            match slot.as_mut().map(|running| running.child.try_wait()) {
                Some(Ok(None)) => {}
                Some(Ok(Some(status))) => break status,
                Some(Err(error)) => {
                    if let Some(mut running) = slot.take() {
                        stop(&mut running.child);
                        let _ = running.child.wait();
                    }
                    return Err(Fail::with("core_failed", error));
                }
                None => return Err(Fail::new("cancelled")),
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    if cancelled.load(Ordering::SeqCst) {
        return Err(Fail::new("cancelled"));
    }
    let stdout = out.and_then(|t| t.join().ok()).unwrap_or_default();
    let stderr = err.and_then(|t| t.join().ok()).unwrap_or_default();
    if cancelled.load(Ordering::SeqCst) {
        return Err(Fail::new("cancelled"));
    }
    core_result(status, &stdout, &stderr)
    })();
    if tracked_repo.is_some() {
        let saved = crate::edit_app_settings(|settings| {
            let task = &mut settings["tasks"]["analysis"];
            if task["id"].as_str() != Some(task_id.as_str()) { return; }
            task["finishedAt"] = serde_json::json!(now());
            match &result {
                Ok(_) => { task["state"] = serde_json::json!("completed"); task["error"] = Value::Null; }
                Err(error) => { task["state"] = serde_json::json!(if error.code == "cancelled" { "cancelled" } else { "failed" }); task["error"] = serde_json::to_value(error).unwrap_or(Value::Null); }
            }
        });
        if result.is_ok() { saved?; }
    }
    result
}

fn stop(child: &mut Child) {
    #[cfg(unix)]
    if let Ok(pgid) = i32::try_from(child.id()) {
        if pgid > 1 {
            // SAFETY: core_cmd starts a fresh process group, whose id is this
            // child's pid. No memory is accessed by this signal operation.
            unsafe { libc::kill(-pgid, libc::SIGKILL) };
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()]).creation_flags(0x0800_0000).status();
    }
    let _ = child.kill();
}

/// Cancels the current process group, including Git children. The running call
/// returns `cancelled`; the next call can start once the worker has exited.
pub fn cancel_analyze() {
    if let Some(running) = RUNNING.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        running.cancelled.store(true, Ordering::SeqCst);
        stop(&mut running.child);
    }
}

fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }

pub fn analysis_status() -> Value { crate::app_settings()["tasks"]["analysis"].clone() }

/// A task persisted as running after its owning process vanished is recoverable.
/// Checking the OS lease avoids marking a second live desktop's task interrupted.
pub fn recover_interrupted_analysis() -> Res<Value> {
    let lease = job_lease()?;
    if lease.try_lock().is_err() { return Ok(analysis_status()); }
    crate::edit_app_settings(|settings| {
        let task = &mut settings["tasks"]["analysis"];
        if task["state"].as_str() == Some("running") {
            task["state"] = serde_json::json!("interrupted");
            task["interruptedAt"] = serde_json::json!(now());
        }
    })?;
    Ok(analysis_status())
}

/// Analysis is incremental in the existing core. Retrying the same saved request
/// resumes from its on-disk cache instead of inventing another analysis engine.
pub fn resume_analysis(progress: Option<ProgressSink>, cancellation: AnalysisCancellation) -> Res<Value> {
    let task = recover_interrupted_analysis()?;
    if !matches!(task["state"].as_str(), Some("interrupted" | "failed" | "cancelled")) { return Err(Fail::new("no_interrupted_task")); }
    let repo = task["repo"].as_str().ok_or(Fail::new("no_interrupted_task"))?;
    run_analysis_with_cancel(repo, task["lang"].as_str(), progress, cancellation)
}
