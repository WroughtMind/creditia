//! 时间工具：ISO 8601 解析、公历转换、周编号、时区（按项目的提交定，不写死）。

/// 公历日期 → 1970-01-01 起的天数（Howard Hinnant 算法）
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 解析 ISO 8601（`2026-10-02T14:55:10.021Z` / `+08:00`）为 epoch 秒
pub fn parse_iso(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let days = days_from_civil(n(0..4)?, n(5..7)?, n(8..10)?);
    let mut t = days * 86400 + n(11..13)? * 3600 + n(14..16)? * 60 + n(17..19)?;
    let rest = &s[19..];
    let rest = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    if let Some(off) = rest.strip_prefix('+').map(|o| (1, o)).or(rest.strip_prefix('-').map(|o| (-1, o))) {
        let (sign, o) = off;
        let h: i64 = o.get(0..2)?.parse().ok()?;
        let m: i64 = o.get(3..5).and_then(|x| x.parse().ok()).unwrap_or(0);
        t -= sign * (h * 3600 + m * 60);
    }
    Some(t)
}

/// 解析时区偏移 `+08:00` / `+0800` / `-05` / `Z` / `UTC` 为秒
pub fn parse_offset(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("z") || s.eq_ignore_ascii_case("utc") {
        return Some(0);
    }
    let (sign, o) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    let o = o.replace(':', "");
    let h: i64 = o.get(0..2)?.parse().ok()?;
    let m: i64 = if o.len() > 2 { o.get(2..4)?.parse().ok()? } else { 0 };
    (h <= 14 && m < 60).then_some(sign * (h * 3600 + m * 60))
}

/// 未设置时的哨兵值
const UNSET: i64 = i64::MIN;
static OFFSET: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(UNSET);

/// `CREDITIA_TZ`（如 `+08:00`、`UTC`）：指定了就全部按它
pub fn forced_offset() -> Option<i64> {
    std::env::var("CREDITIA_TZ").ok().and_then(|v| parse_offset(&v))
}

/// 这台电脑的时区偏移（只在没有别的依据时用）
fn machine_offset() -> i64 {
    #[cfg(unix)]
    if let Ok(o) = std::process::Command::new("date").arg("+%z").output() {
        if let Some(s) = parse_offset(&String::from_utf8_lossy(&o.stdout)) {
            return s;
        }
    }
    0
}

/// 日期、周、生成时间用的时区偏移（秒）：`CREDITIA_TZ` → 分析时按项目定下的（`set_offset`）→ 这台电脑的
pub fn offset() -> i64 {
    let v = OFFSET.load(std::sync::atomic::Ordering::Relaxed);
    if v != UNSET {
        return v;
    }
    let v = forced_offset().unwrap_or_else(machine_offset);
    OFFSET.store(v, std::sync::atomic::Ordering::Relaxed);
    v
}

/// 分析一个项目时定下时区（`CREDITIA_TZ` 优先）
pub fn set_offset(secs: i64) {
    OFFSET.store(forced_offset().unwrap_or(secs), std::sync::atomic::Ordering::Relaxed);
}

/// epoch 秒 → 指定时区的日期 `YYYY-MM-DD`
pub fn date_at(ts: i64, off: i64) -> String {
    let (y, m, d) = civil_from_days((ts + off).div_euclid(86400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// epoch 秒 → 当前时区的日期 `YYYY-MM-DD`
pub fn date_local(ts: i64) -> String {
    date_at(ts, offset())
}

/// epoch 秒 → 指定时区的 ISO 周 `2026-W27`
pub fn week_at(ts: i64, off: i64) -> String {
    let days = (ts + off).div_euclid(86400);
    let wd = (days + 3).rem_euclid(7);
    let thu = days - wd + 3;
    let (y, _, _) = civil_from_days(thu);
    let week = (thu - days_from_civil(y, 1, 1)) / 7 + 1;
    format!("{y}-W{week:02}")
}

/// 偏移秒 → `+08:00`
pub fn fmt_offset(off: i64) -> String {
    let a = off.abs();
    format!("{}{:02}:{:02}", if off < 0 { '-' } else { '+' }, a / 3600, a / 60 % 60)
}

/// 当前时间的 ISO 8601（当前时区）
pub fn now_iso() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let off = offset();
    let (y, m, d) = civil_from_days((t + off).div_euclid(86400));
    let s = (t + off).rem_euclid(86400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}{}", s / 3600, s / 60 % 60, s % 60, fmt_offset(off))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn iso() {
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso("2026-10-02T14:55:10.021Z"), Some(1790952910));
        assert_eq!(parse_iso("2026-10-02T22:55:10+08:00"), Some(1790952910));
        assert_eq!(date_at(1790952910, 8 * 3600), "2026-10-02");
        // 同一时刻（北京 10 月 3 日凌晨 1 点）：纽约还是 10 月 2 日，东京已经是 10 月 3 日
        assert_eq!(date_at(1790952910 + 3 * 3600, -4 * 3600), "2026-10-02");
        assert_eq!(date_at(1790952910 + 3 * 3600, 9 * 3600), "2026-10-03");
        assert_eq!(week_at(parse_iso("2026-07-03T12:00:00Z").unwrap(), 0), "2026-W27");
    }
    #[test]
    fn offsets() {
        assert_eq!(parse_offset("+08:00"), Some(28800));
        assert_eq!(parse_offset("+0800"), Some(28800));
        assert_eq!(parse_offset("-0530"), Some(-19800));
        assert_eq!(parse_offset("-05"), Some(-18000));
        assert_eq!(parse_offset("UTC"), Some(0));
        assert_eq!(parse_offset("beijing"), None);
        assert_eq!(fmt_offset(-19800), "-05:30");
        assert_eq!(fmt_offset(28800), "+08:00");
    }
}
