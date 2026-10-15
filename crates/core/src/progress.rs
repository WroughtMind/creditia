//! 分析进度。大仓库第一次分析要跑很久，得让人看到在走：
//! - `CREDITIA_PROGRESS=1`（App 用）：往 stderr 写 `@progress {"stage":…,"done":…,"total":…}`，一行一条；
//! - stderr 是终端时：在同一行刷新一句人话，跑完擦掉（JSON 在 stdout，不受影响）；
//! - 别的情况什么都不写。最多每 200 毫秒一条。

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// 0：没定；1：机器读的行；2：终端里的一行；3：不写
static MODE: AtomicU8 = AtomicU8::new(0);
static DIRTY: AtomicBool = AtomicBool::new(false);
static LAST: Mutex<Option<(Instant, &'static str)>> = Mutex::new(None);

/// 程序开始时调一次
pub fn init() {
    let m = if std::env::var_os("CREDITIA_PROGRESS").is_some_and(|v| v != "0") {
        1
    } else if std::io::stderr().is_terminal() {
        2
    } else {
        3
    };
    MODE.store(m, Ordering::Relaxed);
}

fn label(stage: &str) -> String {
    match stage {
        "history" => crate::tr!("读提交历史", "reading history"),
        "logs" => crate::tr!("读各工具的日志", "reading tool logs"),
        "attribute" => crate::tr!("逐个提交归因", "attributing commits"),
        "blame" => crate::tr!("追溯现在的每个文件", "tracing current files"),
        _ => stage.to_string(),
    }
}

/// 报一次进度；total 为 0 表示还不知道总数
pub fn tick(stage: &'static str, done: usize, total: usize) {
    let mode = MODE.load(Ordering::Relaxed);
    if mode == 0 || mode == 3 {
        return;
    }
    {
        let mut last = LAST.lock().unwrap();
        let fresh_stage = last.map(|(_, s)| s != stage).unwrap_or(true);
        if !fresh_stage && done != total && last.is_some_and(|(t, _)| t.elapsed().as_millis() < 200) {
            return;
        }
        *last = Some((Instant::now(), stage));
    }
    let mut e = std::io::stderr().lock();
    if mode == 1 {
        let _ = writeln!(e, "@progress {{\"stage\":\"{stage}\",\"done\":{done},\"total\":{total}}}");
    } else {
        let n = if total > 0 { format!(" {done}/{total}") } else if done > 0 { format!(" {done}") } else { String::new() };
        let _ = write!(e, "\r\x1b[2K{}{n}…", label(stage));
        DIRTY.store(true, Ordering::Relaxed);
    }
    let _ = e.flush();
}

/// 结束时擦掉终端里那一行
pub fn done() {
    if DIRTY.swap(false, Ordering::Relaxed) {
        let mut e = std::io::stderr().lock();
        let _ = write!(e, "\r\x1b[2K");
        let _ = e.flush();
    }
}
