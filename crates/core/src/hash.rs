//! Cursor 行哈希复现：规范化规则与 murmurhash3_32 均取自 Cursor 安装包 main.js。
//! 阶段 0 已验证：按文件内容逐行重算，与 Cursor 库里的哈希逐字节一致。

use regex::Regex;
use std::borrow::Cow;
use std::fmt::Write;
use std::sync::OnceLock;

fn is_js_ws(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{a0}' | '\u{1680}'
        | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

struct Res {
    arrow: Regex,
    rb_sym: Regex,
}

/// JS 正则 `\s` 的字符集（与 Rust 的 Unicode White_Space 不同：含 U+FEFF，不含 U+0085）
const WS: &str = r"[\t\n\x0B\x0C\r \x{a0}\x{1680}\x{2000}-\x{200a}\x{2028}\x{2029}\x{202f}\x{205f}\x{3000}\x{feff}]";

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

/// Cursor 的行规范化。`${}`、`#{}`、Python 赋值空格三条规则只删除空白，最后一步会删掉全部空白，故省略。
pub fn normalize(line: &str, ext: &str) -> String {
    let mut out = String::new();
    normalize_into(line, ext, &mut out);
    out
}

pub fn normalize_into(line: &str, ext: &str, out: &mut String) {
    let mut n = Cow::Borrowed(line.trim_matches(is_js_ws));
    let js = matches!(ext, ".js" | ".ts" | ".jsx" | ".tsx");
    if n.contains("=>") {
        let replacement = if js {
            res().arrow.replace_all(&n, "${1}=>")
        } else if ext == ".rb" {
            res().rb_sym.replace_all(&n, "${1}:")
        } else {
            Cow::Borrowed(n.as_ref())
        };
        if let Cow::Owned(s) = replacement {
            n = Cow::Owned(s);
        }
    }
    if ext == ".py" {
        for from in ["'''", "\"\"\"", "f\"", "f'"] {
            if n.contains(from) {
                n = Cow::Owned(n.replace(from, "\""));
            }
        }
    }
    out.clear();
    out.reserve(n.len());
    for c in n.chars() {
        if is_js_ws(c) || c == ';' || c == ',' {
            continue;
        }
        out.push(if c == '\'' || (js && c == '`') { '"' } else { c });
    }
}

/// Cursor 的 murmurhash3_32：按 UTF-16 码元取低 8 位参与计算
pub fn murmur_js(s: &str) -> u32 {
    let mut hash = Murmur::default();
    hash.add(s);
    hash.finish()
}

#[derive(Clone, Copy, Default)]
struct Murmur {
    state: u32,
    tail: u32,
    len: u32,
}

impl Murmur {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;

    fn mix(k: u32) -> u32 {
        k.wrapping_mul(Self::C1).rotate_left(15).wrapping_mul(Self::C2)
    }

    fn chunk(&mut self, k: u32) {
        self.state ^= Self::mix(k);
        self.state = self.state.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }

    fn byte(&mut self, b: u8) {
        self.tail |= u32::from(b) << ((self.len % 4) * 8);
        self.len = self.len.wrapping_add(1);
        if self.len.is_multiple_of(4) {
            self.chunk(self.tail);
            self.tail = 0;
        }
    }

    fn add(&mut self, s: &str) {
        if !s.is_ascii() {
            for u in s.encode_utf16() {
                self.byte(u as u8);
            }
            return;
        }
        let padding = ((4 - self.len % 4) % 4) as usize;
        let (prefix, rest) = s.as_bytes().split_at(padding.min(s.len()));
        for &b in prefix {
            self.byte(b);
        }
        let (chunks, tail) = rest.as_chunks::<4>();
        for ch in chunks {
            self.chunk(u32::from_le_bytes(*ch));
        }
        self.len = self.len.wrapping_add((chunks.len() * 4) as u32);
        for &b in tail {
            self.byte(b);
        }
    }

    fn finish(self) -> u32 {
        let mut h = self.state ^ Self::mix(self.tail) ^ self.len;
        h ^= h >> 16;
        h = h.wrapping_mul(0x85eb_ca6b);
        h ^= h >> 13;
        h = h.wrapping_mul(0xc2b2_ae35);
        h ^ (h >> 16)
    }
}

impl Write for Murmur {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.add(s);
        Ok(())
    }
}

/// 预先计算路径前缀，可直接哈希拼接的多行文本。
#[derive(Clone, Copy)]
pub struct LineHasher(Murmur);

impl LineHasher {
    pub fn new(rel: &str) -> Self {
        let mut hash = Murmur::default();
        hash.add(rel);
        hash.add(":+");
        Self(hash)
    }

    pub fn hash<'a>(&self, lines: impl IntoIterator<Item = &'a str>) -> u32 {
        let mut hash = self.0;
        for line in lines {
            hash.add(line);
        }
        hash.finish()
    }
}

/// 新增行的统一索引键：`仓库内相对路径:+规范化后的行`
pub fn line_key(rel: &str, norm: &str) -> u32 {
    LineHasher::new(rel).hash([norm])
}

/// 过短的行（`}`、空行）本身的哈希；0 留给长行
pub fn short_hash(norm: &str) -> u32 {
    let mut hash = Murmur::default();
    hash.add("short:");
    hash.add(norm);
    hash.finish().max(1)
}

/// 过短的行靠它旁边的长行定位：`anchor` 是同一段里它上面（dir = 'v'）或下面（dir = '^'）最近的长行，
/// `ord` 是从那一行数过来第几个短行。一次写入里 `}` 跟在哪一行后面，提交里也一样，才对得上
pub fn short_key(rel: &str, anchor: u32, dir: char, ord: usize, short: u32) -> u32 {
    let mut hash = Murmur::default();
    write!(&mut hash, "{rel}:{dir}{anchor}:{ord}:{short}").unwrap();
    hash.finish().max(1)
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
