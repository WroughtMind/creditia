//! Ownership of a desktop worker survives both cancellation and desktop crashes.
//! Unix workers inherit the locked lease; Windows jobs kill their full process
//! tree when the desktop's final job handle closes, including abnormal exit.
use std::fs::File;
use std::process::{Child, Command};

#[cfg(windows)]
pub(crate) type OwnedChild = command_group::GroupChild;
#[cfg(not(windows))]
pub(crate) type OwnedChild = Child;

pub(crate) fn spawn(command: &mut Command, lease: &File) -> std::io::Result<OwnedChild> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        use std::os::unix::process::CommandExt;
        let descriptor = lease.as_raw_fd();
        command.process_group(0);
        // SAFETY: pre_exec runs in the child while its copied lease descriptor
        // is still valid. Only async-signal-safe fcntl calls are made; the
        // parent's CLOEXEC flag is unchanged, so unrelated children cannot
        // accidentally inherit the lease during concurrent spawns.
        unsafe {
            command.pre_exec(move || {
                let flags = libc::fcntl(descriptor, libc::F_GETFD);
                if flags < 0
                    || libc::fcntl(descriptor, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use command_group::CommandGroup;
        let _ = lease;
        // The command-group implementation creates the worker suspended, adds
        // it to the job, and resumes it; descendants cannot escape in between.
        command
            .group()
            .kill_on_drop(true)
            .creation_flags(0x0800_0000)
            .spawn()
    }
    #[cfg(not(windows))]
    {
        #[cfg(not(unix))]
        let _ = lease;
        command.spawn()
    }
}

pub(crate) fn inner(child: &mut OwnedChild) -> &mut Child {
    #[cfg(windows)]
    {
        child.inner()
    }
    #[cfg(not(windows))]
    {
        child
    }
}

pub(crate) fn stop(child: &mut OwnedChild) {
    #[cfg(unix)]
    if let Ok(pgid) = i32::try_from(child.id()) {
        if pgid > 1 {
            // SAFETY: spawn created a fresh process group with this child's id.
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
        }
    }
    #[cfg(windows)]
    {
        let _ = child.kill();
    }
    let _ = inner(child).kill();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    // A separate test process represents the desktop. The parent test kills it
    // without running Rust destructors, exactly as a crash/forced quit would.
    #[test]
    fn crash_owner_helper() {
        let Some(root) = std::env::var_os("CREDITIA_TEST_PROCESS_OWNER_ROOT") else {
            return;
        };
        let root = Path::new(&root);
        let lease = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("lease"))
            .unwrap();
        lease.lock().unwrap();
        #[cfg(unix)]
        let mut command = {
            let mut command = Command::new("/bin/sh");
            command.args([
                "-c",
                "printf '%s' \"$$\" > \"$CREDITIA_TEST_WORKER_PID\"; sleep 30",
            ]);
            command
        };
        #[cfg(windows)]
        let mut command = {
            let mut command = Command::new("powershell.exe");
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                r#"
$info = New-Object System.Diagnostics.ProcessStartInfo
$info.FileName = 'powershell.exe'
$info.Arguments = '-NoProfile -NonInteractive -Command "Start-Sleep -Seconds 30"'
$info.UseShellExecute = $false
$p = [System.Diagnostics.Process]::Start($info)
Set-Content -LiteralPath $env:CREDITIA_TEST_WORKER_PID -Value $p.Id
Start-Sleep -Seconds 30
"#,
            ]);
            command
        };
        command
            .env("CREDITIA_TEST_WORKER_PID", root.join("pid"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut worker = spawn(&mut command, &lease).unwrap();
        let _ = inner(&mut worker).wait();
    }

    #[test]
    fn forced_desktop_exit_cannot_release_an_active_worker_for_duplicate_recovery() {
        let root = std::env::temp_dir().join(format!(
            "creditia-process-owner-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut desktop = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "process::tests::crash_owner_helper"])
            .env("CREDITIA_TEST_PROCESS_OWNER_ROOT", &root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let pid = loop {
            if let Some(pid) = std::fs::read_to_string(root.join("pid"))
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
            {
                break Some(pid);
            }
            if Instant::now() >= deadline {
                break None;
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let _ = desktop.kill();
        let _ = desktop.wait();
        let pid = pid.expect("owned worker must start before crashing the desktop");
        #[cfg(unix)]
        {
            let lease = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(root.join("lease"))
                .unwrap();
            let still_reserved = lease.try_lock().is_err();
            // Clean up the deliberately orphaned worker before asserting. Its
            // inherited lock must block recovery until all its work has ended.
            let pgid = i32::try_from(pid).unwrap();
            assert!(pgid > 1);
            // SAFETY: pid was emitted by this test's freshly isolated shell.
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            let released = loop {
                if lease.try_lock().is_ok() {
                    break true;
                }
                if Instant::now() >= deadline {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(25));
            };
            assert!(
                still_reserved,
                "a live orphan must retain the analysis lease"
            );
            assert!(released, "finished work must release recovery");
        }
        #[cfg(windows)]
        {
            // Verify the grandchild too; killing only the direct core process
            // would still allow this descendant to write after recovery starts.
            let script =
                format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 1 }}");
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if Command::new("powershell.exe")
                    .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                    .status()
                    .unwrap()
                    .success()
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "worker descendant {pid} survived desktop termination"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
