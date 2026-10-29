//! Optional GitHub enrichment owns its helper processes, including helpers whose
//! direct parent has exited. Process groups / Windows jobs preserve that ownership.
use command_group::{CommandGroup, GroupChild};
use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

struct OwnedGroup(GroupChild);
impl Drop for OwnedGroup {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.inner().wait();
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    fn path() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "creditia-helper-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }
    fn helper(path: &std::path::Path, end: &str) -> Command {
        let script = format!(
            r#"
$info = New-Object System.Diagnostics.ProcessStartInfo
$info.FileName = 'powershell.exe'
$info.Arguments = '-NoProfile -NonInteractive -Command "Start-Sleep -Seconds 30"'
$info.UseShellExecute = $false
$p = [System.Diagnostics.Process]::Start($info)
Set-Content -LiteralPath $env:CREDITIA_TEST_PID -Value $p.Id
{end}
"#
        );
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .env("CREDITIA_TEST_PID", path);
        command
    }
    fn assert_stopped(path: &std::path::Path) {
        let pid = std::fs::read_to_string(path)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        let _ = std::fs::remove_file(path);
        let script =
            format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 1 }}");
        assert!(
            Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .status()
                .unwrap()
                .success(),
            "helper {pid} survived"
        );
    }
    #[test]
    fn job_cleans_timeout_and_exited_leader() {
        for end in ["Start-Sleep -Seconds 30", "exit 0", "exit 1"] {
            let path = path();
            let result = output(
                &mut helper(&path, end),
                Instant::now() + Duration::from_secs(5),
            );
            assert_eq!(result.is_ok(), end == "exit 0");
            assert_stopped(&path);
        }
    }
    #[test]
    fn parent_exit_helper() {
        let Some(path) = std::env::var_os("CREDITIA_TEST_HELPER_PID") else {
            return;
        };
        let _ = output(
            &mut helper(std::path::Path::new(&path), "Start-Sleep -Seconds 30"),
            Instant::now() + Duration::from_secs(60),
        );
    }
    #[test]
    fn job_closes_when_core_is_killed() {
        let path = path();
        let mut core = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "command::windows_tests::parent_exit_helper"])
            .env("CREDITIA_TEST_HELPER_PID", &path)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .is_none()
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(25));
        }
        core.kill().unwrap();
        core.wait().unwrap();
        assert!(path.exists(), "helper did not start");
        assert_stopped(&path);
    }
}

#[cfg(unix)]
fn guarded(command: &Command) -> Command {
    // The parent owns the only write end of stdin. Killing the core (App cancel
    // or Ctrl-C) closes it; the watcher then kills this entire process group.
    // Arguments are passed as argv, never interpolated into shell source.
    let script = r#"
exec 3<&0
(IFS= read -r unused <&3; kill -KILL "-$$") >/dev/null 2>&1 &
"$@" </dev/null 3<&- &
job=$!
wait "$job"
status=$?
exit "$status"
"#;
    let mut wrapper = Command::new("/bin/sh");
    wrapper
        .args(["-c", script, "creditia-helper"])
        .arg(command.get_program())
        .args(command.get_args());
    if let Some(dir) = command.get_current_dir() {
        wrapper.current_dir(dir);
    }
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            wrapper.env(key, value);
        } else {
            wrapper.env_remove(key);
        }
    }
    wrapper
}

pub(crate) fn output(command: &mut Command, deadline: Instant) -> Result<String, String> {
    if Instant::now() >= deadline {
        return Err("deadline".into());
    }
    #[cfg(unix)]
    let mut wrapper = guarded(command);
    #[cfg(unix)]
    let command = &mut wrapper;
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    command.stdin(Stdio::piped());
    #[cfg(windows)]
    command.stdin(Stdio::null());
    let mut builder = command.group();
    #[cfg(windows)]
    builder.kill_on_drop(true).creation_flags(0x0800_0000);
    let mut child = OwnedGroup(builder.spawn().map_err(|e| e.to_string())?);
    let stdout = child.0.inner().stdout.take().ok_or("missing stdout")?;
    let (sender, reader) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())
            .and_then(|_| {
                if bytes.len() > 16 * 1024 * 1024 {
                    return Err("output too large".into());
                }
                String::from_utf8(bytes).map_err(|e| e.to_string())
            });
        let _ = sender.send(result);
    });
    loop {
        match child.0.inner().try_wait() {
            Ok(Some(status)) => {
                // Terminate any descendants which inherited the output pipe,
                // even if their direct parent already exited successfully.
                let _ = child.0.kill();
                return if status.success() {
                    reader
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .map_err(|_| "reader timed out".to_string())?
                } else {
                    Err("command failed".into())
                };
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => return Err("command timed out".into()),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn path() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "creditia-helper-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }
    fn assert_stopped(path: &std::path::Path) {
        let pid = std::fs::read_to_string(path).unwrap();
        let _ = std::fs::remove_file(path);
        for _ in 0..40 {
            let status = Command::new("ps")
                .args(["-p", &pid, "-o", "stat="])
                .output()
                .unwrap();
            let state = String::from_utf8_lossy(&status.stdout);
            if state.trim().is_empty() || state.trim().starts_with('Z') {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("helper {pid} still running");
    }
    #[test]
    fn timeout_and_exited_leader_both_stop_helpers() {
        for end in ["wait", "exit 0", "exit 1"] {
            let path = path();
            let script = format!("sleep 30 & printf '%s' \"$!\" > \"$1\"; {end}");
            let result = output(
                Command::new("/bin/sh")
                    .args(["-c", &script, "fixture"])
                    .arg(&path),
                Instant::now() + Duration::from_secs(1),
            );
            assert_eq!(result.is_ok(), end == "exit 0");
            assert_stopped(&path);
        }
    }
    #[test]
    fn parent_exit_helper() {
        let Some(path) = std::env::var_os("CREDITIA_TEST_HELPER_PID") else {
            return;
        };
        let _ = output(
            Command::new("/bin/sh")
                .args([
                    "-c",
                    "sleep 30 & printf '%s' \"$!\" > \"$1\"; wait",
                    "fixture",
                ])
                .arg(path),
            Instant::now() + Duration::from_secs(60),
        );
    }
    #[test]
    fn killing_core_also_stops_its_command_group() {
        let path = path();
        let mut core = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "command::tests::parent_exit_helper"])
            .env("CREDITIA_TEST_HELPER_PID", &path)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .is_none()
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(25));
        }
        core.kill().unwrap();
        core.wait().unwrap();
        assert!(path.exists(), "helper did not start");
        assert_stopped(&path);
    }
    #[test]
    fn arguments_are_not_shell_source() {
        let expected = "space ; $(printf injected) ' \"";
        let actual = output(
            Command::new("printf").arg("%s").arg(expected),
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn lifeline_still_works_after_command_leader_has_exited() {
        let path = path();
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "sleep 30 & printf '%s' \"$!\" > \"$1\"; exit 0",
                "fixture",
            ])
            .arg(&path);
        let mut wrapper = guarded(&command);
        wrapper
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut owner = OwnedGroup(wrapper.group_spawn().unwrap());
        // Child::wait closes its own stdin; keep the parent's write handle alive
        // separately until we explicitly simulate core death below.
        let lifeline = owner.0.inner().stdin.take().unwrap();
        assert!(owner.0.inner().wait().unwrap().success());
        // Simulate core death between the leader exiting and the next poll.
        drop(lifeline);
        assert_stopped(&path);
    }
}
