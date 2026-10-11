//! 时间工具：ISO 8601 解析、公历转换、周编号、UTC 日期。

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

/// epoch 秒 → UTC 日期 `YYYY-MM-DD`
pub fn date_cn(ts: i64) -> String {
    let (y, m, d) = civil_from_days(ts.div_euclid(86400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// epoch 秒 → ISO 周 `2026-W27`（UTC）
pub fn iso_week(ts: i64) -> String {
    let days = ts.div_euclid(86400);
    let wd = (days + 3).rem_euclid(7);
    let thu = days - wd + 3;
    let (y, _, _) = civil_from_days(thu);
    let week = (thu - days_from_civil(y, 1, 1)) / 7 + 1;
    format!("{y}-W{week:02}")
}

/// 当前时间的 ISO 8601（UTC）
pub fn now_iso() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let (y, m, d) = civil_from_days(t.div_euclid(86400));
    let s = t.rem_euclid(86400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / 3600, s / 60 % 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn iso() {
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso("2026-10-02T14:55:10.021Z"), Some(1790952910));
        assert_eq!(parse_iso("2026-10-02T22:55:10+08:00"), Some(1790952910));
        assert_eq!(date_cn(1790952910), "2026-10-02");
        assert_eq!(iso_week(parse_iso("2026-07-03T12:00:00Z").unwrap()), "2026-W27");
    }
}
