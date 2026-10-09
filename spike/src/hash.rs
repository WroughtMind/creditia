//! Cursor 行哈希复现：规范化规则与 murmurhash3_32 均取自 Cursor 安装包 main.js。

use regex::Regex;
use std::sync::OnceLock;

/// JS 正则 `\s` 的字符集（与 Rust 的 Unicode White_Space 不同：含 U+FEFF，不含 U+0085）
const WS: &str = r"[\t\n\x0B\x0C\r \x{a0}\x{1680}\x{2000}-\x{200a}\x{2028}\x{2029}\x{202f}\x{205f}\x{3000}\x{feff}]";

fn is_js_ws(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{a0}' | '\u{1680}'
        | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

struct Res {
    arrow: Regex,
    rb_sym: Regex,
}

fn res() -> &'static Res {
    static R: OnceLock<Res> = OnceLock::new();
    R.get_or_init(|| Res {
        arrow: Regex::new(&format!(r"\({WS}*([a-zA-Z_$][a-zA-Z0-9_$]*){WS}*\){WS}*=>")).unwrap(),
        rb_sym: Regex::new(&format!(r":{WS}*([a-zA-Z_][a-zA-Z0-9_]*){WS}*=>")).unwrap(),
    })
}

/// 与 Node `path.extname` 一致：没有扩展名或仅有前导点时返回 "."
pub fn ext_of(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    match file.rfind('.') {
        Some(i) if i > 0 => file[i..].to_lowercase(),
        _ => ".".to_string(),
    }
}

/// Cursor 的行规范化（Q2）。
/// 说明：模板字符串 `${}`、Ruby `#{}`、Python 赋值空格三条规则只删除空白，
/// 最后一步会删掉全部空白，所以省略不影响结果（Python 那条还依赖 Rust 不支持的负向先行断言）。
pub fn normalize(line: &str, ext: &str) -> String {
    let r = res();
    let mut n = line.trim_matches(is_js_ws).to_string();
    let js = matches!(ext, ".js" | ".ts" | ".jsx" | ".tsx");
    if js {
        n = r.arrow.replace_all(&n, "${1}=>").into_owned();
    }
    if ext == ".py" {
        n = n.replace("'''", "\"").replace("\"\"\"", "\"").replace("f\"", "\"").replace("f'", "\"");
    }
    if ext == ".rb" {
        n = r.rb_sym.replace_all(&n, "${1}:").into_owned();
    }
    let mut out = String::with_capacity(n.len());
    for c in n.chars() {
        if is_js_ws(c) || c == ';' || c == ',' {
            continue;
        }
        out.push(if c == '\'' || (js && c == '`') { '"' } else { c });
    }
    out
}

/// Cursor 的 murmurhash3_32：按 UTF-16 码元取低 8 位参与计算
pub fn murmur_js(s: &str) -> u32 {
    let bytes: Vec<u8> = s.encode_utf16().map(|u| (u & 0xff) as u8).collect();
    murmur3_32(&bytes, 0)
}

fn murmur3_32(data: &[u8], seed: u32) -> u32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;
    let mut h = seed;
    let chunks = data.chunks_exact(4);
    let tail = chunks.remainder();
    for ch in chunks {
        let mut k = u32::from_le_bytes([ch[0], ch[1], ch[2], ch[3]]);
        k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h ^= k;
        h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    let mut k = 0u32;
    if tail.len() >= 3 { k ^= (tail[2] as u32) << 16; }
    if tail.len() >= 2 { k ^= (tail[1] as u32) << 8; }
    if !tail.is_empty() {
        k ^= tail[0] as u32;
        h ^= k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

/// 新增行的统一索引键：`仓库内相对路径:+规范化后的行`
pub fn line_key(rel: &str, norm: &str) -> u32 {
    murmur_js(&format!("{rel}:+{norm}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_vectors_match_cursor_js() {
        let v: Vec<(String, String)> =
            serde_json::from_str(include_str!("../tests/vectors.json")).unwrap();
        assert!(v.len() > 10);
        for (input, hex) in v {
            assert_eq!(format!("{:x}", murmur_js(&input)), hex, "input={input:?}");
        }
    }

    #[test]
    fn normalize_vectors_match_cursor_js() {
        let v: Vec<(String, String, String)> =
            serde_json::from_str(include_str!("../tests/norm.json")).unwrap();
        for (line, ext, want) in v {
            assert_eq!(normalize(&line, &ext), want, "line={line:?} ext={ext}");
        }
    }

    #[test]
    fn ext_like_node() {
        assert_eq!(ext_of("a/b/c.TS"), ".ts");
        assert_eq!(ext_of(".gitignore"), ".");
        assert_eq!(ext_of("Makefile"), ".");
    }
}
