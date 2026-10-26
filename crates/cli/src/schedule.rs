//! 定时收记录：每小时跑一次 `collect --all`，把各项目的新会话记录收进作品库。
//! agent 工具会清理自己的旧日志（Claude Code 默认只留 30 天），收进作品库的证据不会跟着没。
//! macOS 用 launchd，Linux 用 systemd 用户定时器，Windows 用计划任务。
//! App 设置里的「备份 AI 工具记录 · 一直」是另一份（名字不同，互不覆盖）；两份都开只是多收一次，收过的不会重复记。

use creditia_core::tr;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

type R<T> = Result<T, String>;

/// 多久收一次（秒），和 App 的后台备份一样
pub const EVERY: u64 = 3600;
const LABEL: &str = "dev.creditia.collect.cli";
const UNIT: &str = "creditia-collect";
const TASK: &str = "creditia-collect";

/// $0 是核心，$1 是作品库目录。先写临时文件再改名，读到的总是完整的一份
const SCRIPT: &str = r#"[ -x "$0" ] || exit 0; mkdir -p "$1" || exit 0; "$0" collect --all --json > "$1/collect-schedule.json.tmp" 2>/dev/null; mv -f "$1/collect-schedule.json.tmp" "$1/collect-schedule.json""#;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|h| !h.is_empty()).map(PathBuf::from)
}

fn out_file() -> PathBuf {
    creditia_store::data_dir().join("collect-schedule.json")
}

fn creditia_home() -> Option<String> {
    std::env::var("CREDITIA_HOME").ok().filter(|h| !h.is_empty())
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// launchd 任务。launchd 的 PATH 很短，补上 Homebrew 的位置，核心才找得到 git
pub fn plist(core: &Path, lib: &Path, creditia_home: Option<&str>) -> String {
    let home = creditia_home.map(|h| format!("\n    <key>CREDITIA_HOME</key><string>{}</string>", xml(h))).unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- creditia schedule install 装的：每小时把各项目的新记录收进作品库。creditia schedule uninstall 删除 -->
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/sh</string>
    <string>-c</string>
    <string>{}</string>
    <string>{}</string>
    <string>{}</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key><string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string>{home}
  </dict>
  <key>StartInterval</key><integer>{EVERY}</integer>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Background</string>
  <key>LowPriorityIO</key><true/>
  <key>Nice</key><integer>10</integer>
</dict>
</plist>
"#,
        xml(SCRIPT),
        xml(&core.display().to_string()),
        xml(&lib.display().to_string()),
    )
}

/// systemd 的一个参数：放进双引号；`\` `"` 转义，`$` `%` 写两遍（systemd 自己会展开它们）
fn sd(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "$$").replace('%', "%%"))
}

/// systemd 用户服务 + 定时器（错过的整点开机后补跑一次）
pub fn systemd(core: &Path, lib: &Path, creditia_home: Option<&str>) -> (String, String) {
    let env = creditia_home.map(|h| format!("Environment={}\n", sd(&format!("CREDITIA_HOME={h}")))).unwrap_or_default();
    let service = format!(
        "# creditia schedule install 装的：把各项目的新记录收进作品库。creditia schedule uninstall 删除\n[Unit]\nDescription=creditia: collect AI tool records into the library\n\n[Service]\nType=oneshot\n{env}ExecStart=/bin/sh -c {} {} {}\nNice=10\nIOSchedulingClass=idle\n",
        sd(SCRIPT),
        sd(&core.display().to_string()),
        sd(&lib.display().to_string()),
    );
    let timer = "# creditia schedule install 装的\n[Unit]\nDescription=creditia: collect AI tool records every hour\n\n[Timer]\nOnCalendar=hourly\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n".to_string();
    (service, timer)
}

/// Windows 计划任务要运行的命令
pub fn task_command(core: &Path) -> String {
    format!("\"{}\" collect --all", core.display())
}

fn plist_path() -> Option<PathBuf> {
    Some(home()?.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
}

fn unit_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").filter(|x| !x.is_empty()).map(PathBuf::from).or_else(|| Some(home()?.join(".config")))?;
    Some(base.join("systemd/user"))
}

fn run(cmd: &str, args: &[&str]) -> R<String> {
    let o = Command::new(cmd).args(args).output().map_err(|e| format!("{cmd}: {e}"))?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).to_string())
    } else {
        Err(format!("{cmd} {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim()))
    }
}

/// 定时任务要跑的核心：就是现在这个程序（绝对路径）
fn core() -> R<PathBuf> {
    let p = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(p.canonicalize().unwrap_or(p))
}

/// 上一轮的结果：时间取文件的修改时间，数字从输出里数
fn last() -> Value {
    let p = out_file();
    let Some(at) = std::fs::metadata(&p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()) else { return Value::Null };
    let v: Value = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let c = v["collected"].as_array().cloned().unwrap_or_default();
    let failed = c.iter().filter(|x| x["error"].is_string()).count();
    let new: u64 = c.iter().filter_map(|x| x["newEvidence"].as_u64()).sum();
    json!({ "at": at.as_secs() as i64, "projects": c.len() - failed, "failed": failed, "newEvidence": new })
}

fn installed() -> (bool, Option<PathBuf>) {
    if cfg!(target_os = "macos") {
        let p = plist_path();
        (p.as_ref().is_some_and(|p| p.exists()), p)
    } else if cfg!(windows) {
        (run("schtasks", &["/Query", "/TN", TASK]).is_ok(), None)
    } else {
        let p = unit_dir().map(|d| d.join(format!("{UNIT}.timer")));
        (p.as_ref().is_some_and(|p| p.exists()), p)
    }
}

pub fn cmd(act: Option<&str>) -> R<Value> {
    match act {
        Some("status") | None => {
            let (on, path) = installed();
            let l = last();
            let when = l["at"].as_i64().map(|t| creditia_core::util::date_local(t)).unwrap_or_default();
            let summary = match (on, when.is_empty()) {
                (true, false) => tr!("已开启，每小时收一次；上次 {when}", "on, every hour; last run {when}"),
                (true, true) => tr!("已开启，每小时收一次；还没跑过", "on, every hour; hasn't run yet"),
                (false, _) => tr!("没有开启。creditia schedule install 开启", "off. Turn it on with creditia schedule install"),
            };
            Ok(json!({ "installed": on, "path": path, "every": EVERY, "last": l, "summary": summary }))
        }
        Some("install") => install(),
        Some("uninstall") => uninstall(),
        _ => Err(tr!("用法：creditia schedule install|uninstall|status", "usage: creditia schedule install|uninstall|status")),
    }
}

fn write(p: &Path, body: &str) -> R<()> {
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(p, body).map_err(|e| tr!("写入 {} 失败：{e}", "couldn't write {}: {e}", p.display()))
}

fn install() -> R<Value> {
    let core = core()?;
    let lib = creditia_store::data_dir();
    let ch = creditia_home();
    let mut notes = vec![];
    let path: PathBuf;
    if cfg!(target_os = "macos") {
        path = plist_path().ok_or(tr!("找不到用户目录", "no home directory"))?;
        let domain = format!("gui/{}", run("id", &["-u"])?.trim());
        let _ = run("launchctl", &["bootout", &domain, &path.display().to_string()]); // 装过旧的先卸；没装过会报错，不管
        write(&path, &plist(&core, &lib, ch.as_deref()))?;
        run("launchctl", &["bootstrap", &domain, &path.display().to_string()])?;
    } else if cfg!(windows) {
        path = PathBuf::from(TASK);
        run("schtasks", &["/Create", "/F", "/SC", "HOURLY", "/TN", TASK, "/TR", &task_command(&core)])?;
    } else {
        let dir = unit_dir().ok_or(tr!("找不到用户目录", "no home directory"))?;
        let (service, timer) = systemd(&core, &lib, ch.as_deref());
        write(&dir.join(format!("{UNIT}.service")), &service)?;
        path = dir.join(format!("{UNIT}.timer"));
        write(&path, &timer)?;
        let on = run("systemctl", &["--user", "daemon-reload"]).and_then(|_| run("systemctl", &["--user", "enable", "--now", &format!("{UNIT}.timer")]));
        if let Err(e) = on {
            // 没有 systemd 用户会话（容器、WSL 1、精简系统）：文件留着，给一行 cron
            let _ = std::fs::remove_file(dir.join(format!("{UNIT}.service")));
            let _ = std::fs::remove_file(&path);
            return Err(tr!(
                "没能开启 systemd 用户定时器（{e}）。可以改用 cron，在 crontab -e 里加一行：\n0 * * * * {} collect --all",
                "couldn't enable the systemd user timer ({e}). Use cron instead; add this line with crontab -e:\n0 * * * * {} collect --all",
                sh_quote(&core.display().to_string())
            ));
        }
        notes.push(tr!(
            "systemd 用户定时器只在你登录时跑；服务器上想一直跑：loginctl enable-linger",
            "systemd user timers run only while you're logged in; on a server, keep them running with: loginctl enable-linger"
        ));
    }
    Ok(json!({ "installed": true, "path": path, "every": EVERY, "notes": notes, "summary": tr!("已开启：每小时把各项目的新记录收进作品库（{}）", "on: every hour, new records from every project go into the library ({})", path.display()) }))
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn uninstall() -> R<Value> {
    let (on, path) = installed();
    if !on {
        return Ok(json!({ "installed": false, "summary": tr!("没有开启，不用关", "not on; nothing to remove") }));
    }
    if cfg!(target_os = "macos") {
        let p = path.unwrap();
        let domain = format!("gui/{}", run("id", &["-u"])?.trim());
        let _ = run("launchctl", &["bootout", &domain, &p.display().to_string()]);
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    } else if cfg!(windows) {
        run("schtasks", &["/Delete", "/F", "/TN", TASK])?;
    } else {
        let _ = run("systemctl", &["--user", "disable", "--now", &format!("{UNIT}.timer")]);
        if let Some(d) = unit_dir() {
            let _ = std::fs::remove_file(d.join(format!("{UNIT}.service")));
            let _ = std::fs::remove_file(d.join(format!("{UNIT}.timer")));
        }
        let _ = run("systemctl", &["--user", "daemon-reload"]);
    }
    Ok(json!({ "installed": false, "summary": tr!("已关闭定时收记录", "scheduled collection is off") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemd_quotes_paths_and_dollars() {
        let (service, timer) = systemd(Path::new("/opt/my apps/creditia-core"), Path::new("/home/me/100%/lib"), Some("/home/me/lib"));
        // systemd 会展开 $ 和 %：写两遍才是字面量
        assert!(service.contains(r#"ExecStart=/bin/sh -c "[ -x \"$$0\" ] || exit 0;"#), "{service}");
        assert!(service.contains(r#" "/opt/my apps/creditia-core" "/home/me/100%%/lib""#), "{service}");
        assert!(service.contains(r#"Environment="CREDITIA_HOME=/home/me/lib""#), "{service}");
        assert!(timer.contains("OnCalendar=hourly") && timer.contains("Persistent=true") && timer.contains("WantedBy=timers.target"));
    }

    #[test]
    fn plist_escapes_and_runs_hourly() {
        let p = plist(Path::new("/Applications/A & B/creditia-core"), Path::new("/Users/me/lib"), None);
        assert!(p.contains("<string>/Applications/A &amp; B/creditia-core</string>"));
        assert!(p.contains(&format!("<key>StartInterval</key><integer>{EVERY}</integer>")));
        assert!(p.contains(LABEL) && !p.contains("CREDITIA_HOME"));
    }

    #[test]
    fn windows_task_quotes_the_core() {
        assert_eq!(task_command(Path::new(r"C:\Program Files\creditia\creditia-core.exe")), r#""C:\Program Files\creditia\creditia-core.exe" collect --all"#);
    }
}
