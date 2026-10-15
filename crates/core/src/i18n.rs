//! 给人看的文案用哪种语言：`--lang en|zh`（命令行先调 set）或环境变量 `CREDITIA_LANG`；都没有就是中文。
//! 写进 credits.json 的数据不翻译（工具名、模型 id、路径、哈希），只翻译提醒、摘要、帮助和报错。

use std::sync::atomic::{AtomicU8, Ordering};

/// 0 还没定，1 中文，2 英文
static LANG: AtomicU8 = AtomicU8::new(0);

/// 按 `zh` / `en` 设定；其他值不改
pub fn set(lang: &str) {
    match lang.trim().to_lowercase().as_str() {
        "en" | "english" => LANG.store(2, Ordering::Relaxed),
        "zh" | "cn" | "zh-cn" | "chinese" => LANG.store(1, Ordering::Relaxed),
        _ => {}
    }
}

pub fn en() -> bool {
    match LANG.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let v = std::env::var("CREDITIA_LANG").map(|l| l.trim().to_lowercase().starts_with("en")).unwrap_or(false);
            LANG.store(if v { 2 } else { 1 }, Ordering::Relaxed);
            v
        }
    }
}

/// 当前语言的代码：`zh` / `en`
pub fn code() -> &'static str {
    if en() { "en" } else { "zh" }
}

/// 两种语言的同一句话，按当前语言取：`tr!("新增 {n} 条", "{n} new")`，参数写法同 `format!`
#[macro_export]
macro_rules! tr {
    ($zh:literal, $en:literal $(,)?) => {
        if $crate::i18n::en() { format!($en) } else { format!($zh) }
    };
    ($zh:literal, $en:literal, $($a:tt)*) => {
        if $crate::i18n::en() { format!($en, $($a)*) } else { format!($zh, $($a)*) }
    };
}
