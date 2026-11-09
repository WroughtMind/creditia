// Acceptance-only instrumentation: injected into an immutable baseline checkout.
// This file is never part of the release app. Paths come from the launching CI
// process, never from renderer IPC arguments.
fn creditia_acceptance_enabled() -> bool {
    std::env::var("CREDITIA_ACCEPTANCE_MODE").as_deref() == Ok("1")
}
#[tauri::command]
fn acceptance_fixture() -> Result<Value, String> {
    if !creditia_acceptance_enabled() { return Err("Acceptance mode is disabled".into()); }
    let file = std::env::var_os("CREDITIA_ACCEPTANCE_ARCHIVE").ok_or("Missing archive")?;
    let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
#[tauri::command]
fn acceptance_event(event: Value) -> Result<(), String> {
    use std::io::Write;
    if !creditia_acceptance_enabled() { return Err("Acceptance mode is disabled".into()); }
    let file = std::env::var_os("CREDITIA_APP_EVENTS").ok_or("Missing event path")?;
    let mut output = std::fs::OpenOptions::new().create(true).append(true).open(file).map_err(|e| e.to_string())?;
    writeln!(output, "{}", event).map_err(|e| e.to_string())
}
