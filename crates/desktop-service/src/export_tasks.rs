//! Export recovery is kept apart from frequently written UI preferences.
use crate::{Fail, Res};
use serde_json::{json, Value};
use std::{path::Path, sync::Mutex};

const MAX_BYTES: u64 = 64 * 1024 * 1024;
static EXPORT_LOCK: Mutex<()> = Mutex::new(());

fn read(path: &Path) -> Res<Value> {
    match std::fs::metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({"schema":1,"revision":0,"queue":null}))
        }
        Err(error) => return Err(Fail::with("io", error)),
        Ok(metadata) if metadata.len() > MAX_BYTES => {
            return Err(Fail::new("export_history_too_large"))
        }
        Ok(_) => {}
    }
    let bytes = std::fs::read(path).map_err(|error| Fail::with("io", error))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Fail::new("export_history_too_large"));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| Fail::with("export_history_invalid", error))?;
    if !value.is_object()
        || value["schema"] != 1
        || value["revision"].as_u64().is_none()
        || !(value["queue"].is_null() || value["queue"].is_object())
    {
        return Err(Fail::new("export_history_invalid"));
    }
    Ok(value)
}

pub fn load_export_jobs() -> Res<Value> {
    read(&creditia_store::data_dir().join("export-jobs.json"))
}

fn save_in(directory: &Path, expected_revision: u64, queue: &Value) -> Res<u64> {
    if queue["schema"] != 1
        || !queue["archives"].is_object()
        || !queue["jobs"]
            .as_array()
            .is_some_and(|jobs| jobs.len() <= 64)
    {
        return Err(Fail::new("export_history_invalid"));
    }
    let _thread = EXPORT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::fs::create_dir_all(directory).map_err(|error| Fail::with("io", error))?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("export-jobs.lock"))
        .map_err(|error| Fail::with("io", error))?;
    lock.lock().map_err(|error| Fail::with("io", error))?;
    let path = directory.join("export-jobs.json");
    let mut saved = read(&path)?;
    let current = saved["revision"]
        .as_u64()
        .ok_or(Fail::new("export_history_invalid"))?;
    if current != expected_revision {
        return Err(Fail::with(
            "export_history_conflict",
            "Another desktop updated the export history; reload it before saving",
        ));
    }
    let revision = current
        .checked_add(1)
        .ok_or(Fail::new("export_history_invalid"))?;
    saved["revision"] = json!(revision);
    saved["queue"] = queue.clone();
    let bytes = serde_json::to_vec_pretty(&saved)
        .map_err(|error| Fail::with("export_history_invalid", error))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Fail::new("export_history_too_large"));
    }
    crate::settings::atomic_bytes(&path, &bytes)?;
    Ok(revision)
}

/// Compare-and-save prevents a second app instance from erasing recovery jobs.
/// Call from a serial background writer, and retain the returned revision.
pub fn save_export_jobs(expected_revision: u64, queue: &Value) -> Res<u64> {
    save_in(&creditia_store::data_dir(), expected_revision, queue)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_writer_and_malformed_history_never_replace_the_existing_bytes() {
        let directory = std::env::temp_dir().join(format!(
            "creditia-export-history-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let queue = json!({"schema":1,"nextId":0,"archives":{},"jobs":[]});
        assert_eq!(save_in(&directory, 0, &queue).unwrap(), 1);
        let path = directory.join("export-jobs.json");
        let first = std::fs::read(&path).unwrap();
        assert_eq!(
            save_in(&directory, 0, &queue).unwrap_err().code,
            "export_history_conflict"
        );
        assert_eq!(std::fs::read(&path).unwrap(), first);
        std::fs::write(&path, b"{broken").unwrap();
        assert_eq!(
            save_in(&directory, 1, &queue).unwrap_err().code,
            "export_history_invalid"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"{broken");
        let _ = std::fs::remove_dir_all(directory);
    }
}
