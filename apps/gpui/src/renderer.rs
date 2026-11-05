//! One bounded request per renderer process. Call `request` from a background
//! executor: the supervisor blocks while the shared renderer produces its result.
use serde_json::Value;
use base64::Engine as _;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc};
use std::thread;
use std::time::{Duration, Instant};

const REQUEST_LIMIT: usize = 8 * 1024 * 1024;
const RESPONSE_LIMIT: usize = 64 * 1024 * 1024;
const STDERR_LIMIT: usize = 64 * 1024;

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) { self.0.store(true, Ordering::Release); }
    fn cancelled(&self) -> bool { self.0.load(Ordering::Acquire) }
}

struct OwnedChild {
    child: Child,
    reaped: bool,
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            // Also covers pipe/thread setup failures, timeout, and cancellation.
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

enum PipeResult {
    Input(Result<(), String>),
    Output(Result<Vec<u8>, String>),
    Error(Result<Vec<u8>, String>),
}

/// Run the shared renderer, validating its response envelope before returning
/// only `result`. Closing stdin after one JSONL record makes the bridge exit.
/// The timeout covers startup, writing, rendering, pipe drain, and process exit.
pub fn request(input: Value, cancellation: Cancellation) -> Result<Value, String> {
    let id = input.get("id").filter(|id| valid_id(id))
        .ok_or_else(|| "Renderer request requires a string or safe integer id".to_string())?;
    if input.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("Renderer request requires protocol version 1".into());
    }
    if cancellation.cancelled() { return Err("Renderer request cancelled".into()); }
    let mut bytes = serde_json::to_vec(&input).map_err(|error| error.to_string())?;
    if bytes.len() > REQUEST_LIMIT { return Err("Renderer request exceeds 8 MiB".into()); }
    bytes.push(b'\n');

    let timeout = match std::env::var("CREDITIA_RENDER_TIMEOUT_MS") {
        Ok(value) => value.parse::<u64>().ok().filter(|value| *value > 0)
            .map(Duration::from_millis)
            .ok_or_else(|| "CREDITIA_RENDER_TIMEOUT_MS must be a positive integer".to_string())?,
        Err(std::env::VarError::NotPresent) => Duration::from_secs(45),
        Err(error) => return Err(format!("Invalid renderer timeout: {error}")),
    };
    let deadline = Instant::now().checked_add(timeout)
        .ok_or_else(|| "Renderer timeout is out of range".to_string())?;
    let executable = std::env::var_os("CREDITIA_NODE").unwrap_or_else(|| "node".into());
    let bridge = std::env::var_os("CREDITIA_RENDER_BRIDGE").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/render/tools/bridge.mjs"));
    let mut command = Command::new(executable);
    command.arg(bridge).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = OwnedChild {
        child: command.spawn().map_err(|error| format!("Unable to start renderer: {error}"))?,
        reaped: false,
    };
    let mut stdin = child.child.stdin.take().ok_or("Renderer stdin is unavailable")?;
    let stdout = child.child.stdout.take().ok_or("Renderer stdout is unavailable")?;
    let stderr = child.child.stderr.take().ok_or("Renderer stderr is unavailable")?;
    let (sender, receiver) = mpsc::channel();

    // All three pipes must progress concurrently: large archives can fill stdin
    // while startup diagnostics or the encoded artwork fills another pipe.
    let output_sender = sender.clone();
    thread::Builder::new().name("creditia-render-output".into()).spawn(move || {
        let _ = output_sender.send(PipeResult::Output(read_bounded(stdout, RESPONSE_LIMIT + 1, false)));
    }).map_err(|error| format!("Unable to read renderer output: {error}"))?;
    let error_sender = sender.clone();
    thread::Builder::new().name("creditia-render-error".into()).spawn(move || {
        let _ = error_sender.send(PipeResult::Error(read_bounded(stderr, STDERR_LIMIT, true)));
    }).map_err(|error| format!("Unable to read renderer errors: {error}"))?;
    thread::Builder::new().name("creditia-render-input".into()).spawn(move || {
        let result = stdin.write_all(&bytes).map_err(|error| error.to_string());
        drop(stdin); // EOF terminates the one-shot bridge after its response.
        let _ = sender.send(PipeResult::Input(result));
    }).map_err(|error| format!("Unable to write renderer request: {error}"))?;

    let mut status: Option<ExitStatus> = None;
    let mut written = None;
    let mut output = None;
    let mut errors = None;
    loop {
        if cancellation.cancelled() { return Err("Renderer request cancelled".into()); }
        if Instant::now() >= deadline { return Err(format!("Renderer timed out after {} ms", timeout.as_millis())); }
        if status.is_none() {
            status = child.child.try_wait().map_err(|error| format!("Unable to wait for renderer: {error}"))?;
            if status.is_some() { child.reaped = true; }
        }
        if status.is_some() && written.is_some() && output.is_some() && errors.is_some() { break; }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining.min(Duration::from_millis(20))) {
            Ok(PipeResult::Input(result)) => written = Some(result),
            Ok(PipeResult::Output(result)) => output = Some(result?),
            Ok(PipeResult::Error(result)) => errors = Some(result?),
            Err(mpsc::RecvTimeoutError::Timeout) => {},
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // All pipe threads have completed. The process may still be
                // shutting down, so keep supervising it until exit or timeout.
                if written.is_none() || output.is_none() || errors.is_none() {
                    return Err("Renderer pipe closed without a result".into());
                }
                thread::sleep(remaining.min(Duration::from_millis(20)));
            }
        }
    }
    let status = status.expect("completed process");
    let diagnostic = String::from_utf8_lossy(errors.as_deref().unwrap_or_default()).trim().to_owned();
    if !status.success() {
        return Err(if diagnostic.is_empty() { format!("Renderer exited with {status}") }
            else { format!("Renderer exited with {status}: {diagnostic}") });
    }
    written.expect("completed input").map_err(|error| format!("Unable to send renderer request: {error}"))?;
    parse_response(&output.expect("completed output"), id)
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
        _ => return Err(format!("Unsupported export format: {format}")),
    };
    if result["encoding"].as_str() != Some("base64") {
        return Err("Renderer export is not base64 encoded".into());
    }
    if result["ext"].as_str() != Some(format) || result["mime"].as_str() != Some(mime) {
        return Err(format!("Renderer export does not match the selected {format} format"));
    }
    let encoded = result["bytes"].as_str().ok_or("Renderer export has no encoded bytes")?;
    if encoded.len() > RESPONSE_LIMIT { return Err("Renderer export exceeds 64 MiB".into()); }
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)
        .map_err(|error| format!("Renderer export contains invalid base64: {error}"))?;
    if bytes.is_empty() { return Err("Renderer export is empty".into()); }
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
        if self.committed { return; }
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
    if path.file_name().is_none() { return Err("Export destination must name a file".into()); }
    if bytes.is_empty() { return Err("Cannot write an empty export".into()); }
    if cancellation.cancelled() { return Err("Export cancelled".into()); }
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let mut pending = create_export_temp(parent)?;
    let file = pending.file.as_mut().expect("new temporary export file");
    for chunk in bytes.chunks(64 * 1024) {
        if cancellation.cancelled() { return Err("Export cancelled".into()); }
        file.write_all(chunk).map_err(|error| format!("Unable to write export: {error}"))?;
    }
    file.flush().map_err(|error| format!("Unable to flush export: {error}"))?;
    // Preserve the destination's access permissions when replacing a regular
    // file. A new Unix export starts private; its directory controls Windows ACLs.
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.is_file() {
            file.set_permissions(metadata.permissions())
                .map_err(|error| format!("Unable to preserve export permissions: {error}"))?;
        }
    }
    file.sync_all().map_err(|error| format!("Unable to sync export: {error}"))?;
    drop(pending.file.take()); // Windows must not retain an open temporary-file handle.
    if cancellation.cancelled() { return Err("Export cancelled".into()); }
    fs::rename(&pending.path, path).map_err(|error| format!("Unable to save export: {error}"))?;
    pending.committed = true;
    Ok(())
}

fn create_export_temp(parent: &Path) -> Result<PendingExport, String> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_nanos();
    for _ in 0..128 {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".creditia-export-{}-{stamp}-{sequence}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok(PendingExport { path, file: Some(file), committed: false }),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Unable to create export file: {error}")),
        }
    }
    Err("Unable to reserve a temporary export file".into())
}

fn valid_id(id: &Value) -> bool {
    if let Some(value) = id.as_str() { return value.encode_utf16().count() <= 128; }
    if let Some(value) = id.as_i64() { return value.unsigned_abs() <= 9_007_199_254_740_991; }
    id.as_u64().is_some_and(|value| value <= 9_007_199_254_740_991)
}

fn read_bounded(mut reader: impl Read, limit: usize, truncate: bool) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let length = match reader.read(&mut chunk) {
            Ok(length) => length,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("Renderer pipe read failed: {error}")),
        };
        if length == 0 { return Ok(bytes); }
        let available = limit.saturating_sub(bytes.len());
        if length > available && !truncate { return Err("Renderer response exceeds 64 MiB".into()); }
        bytes.extend_from_slice(&chunk[..length.min(available)]);
        // Stderr keeps only the first 64 KiB but must still be drained so a noisy
        // renderer cannot deadlock on a full diagnostic pipe.
    }
}

fn parse_response(bytes: &[u8], id: &Value) -> Result<Value, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("Renderer output is not UTF-8: {error}"))?.trim();
    if text.len() > RESPONSE_LIMIT { return Err("Renderer response exceeds 64 MiB".into()); }
    let mut response: Value = serde_json::from_str(text)
        .map_err(|error| format!("Renderer returned invalid JSON: {error}"))?;
    if response.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("Renderer returned an unsupported protocol version".into());
    }
    if response.get("id") != Some(id) { return Err("Renderer response id does not match the request".into()); }
    match response.get("ok").and_then(Value::as_bool) {
        Some(true) => response.as_object_mut().and_then(|object| object.remove("result"))
            .ok_or_else(|| "Renderer response has no result".into()),
        Some(false) => {
            let code = response["error"]["code"].as_str().unwrap_or("render_failed");
            let message = response["error"]["message"].as_str().unwrap_or("Renderer request failed");
            Err(format!("{code}: {message}"))
        }
        None => Err("Renderer response has no boolean ok field".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cancellation, decode_export, parse_response, read_bounded, valid_id, write_export};
    use base64::Engine as _;
    use serde_json::json;
    use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("creditia-render-export-test-{}-{stamp}-{sequence}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn rejects_wrong_request_and_extra_records() {
        assert!(parse_response(br#"{"version":1,"id":2,"ok":true,"result":{}}"#, &json!(1)).is_err());
        assert!(parse_response(b"{\"version\":1,\"id\":1,\"ok\":true,\"result\":{}}\n{}", &json!(1)).is_err());
        assert_eq!(parse_response(b"{\"version\":1,\"id\":1,\"ok\":true,\"result\":{\"art\":null}}\n", &json!(1)).unwrap(), json!({"art":null}));
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
        for (format, mime) in [("png", "image/png"), ("svg", "image/svg+xml"),
            ("pdf", "application/pdf"), ("gif", "image/gif"), ("html", "text/html")] {
            assert_eq!(decode_export(&json!({"encoding":"base64","ext":format,"mime":mime,"bytes":encoded}), format).unwrap(), bytes);
        }
        assert!(decode_export(&json!({"encoding":"base64","ext":"png","mime":"image/png","bytes":encoded}), "pdf").is_err());
        assert!(decode_export(&json!({"encoding":"base64","ext":"png","mime":"image/png","bytes":"invalid%"}), "png").is_err());
        assert!(decode_export(&json!({"encoding":"base64","ext":"png","mime":"image/png","bytes":""}), "png").is_err());
        assert!(decode_export(&json!({}), "jpg").is_err());
    }

    #[test]
    fn replacement_commits_complete_bytes_and_cleans_temporary_file() {
        let directory = TestDirectory::new();
        let destination = directory.0.join("piece.svg");
        fs::write(&destination, b"original").unwrap();
        write_export(&destination, b"complete replacement", Cancellation::default()).unwrap();
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
        assert!(write_export(&existing_directory, b"replacement", Cancellation::default()).is_err());
        assert_eq!(fs::read(existing_directory.join("keep")).unwrap(), b"untouched");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }
}
