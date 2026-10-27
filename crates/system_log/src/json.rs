//! **F3 to JSONL**: one record, one JSON object, one line (§242 (a system log), Question 3).
//!
//! It writes and never parses. The field order is fixed (`seq`, `time`, `source`, `program`,
//! `user`, `severity`, `msg`, then the flags that are set), so two records diff cleanly and a grep
//! still works. Each byte that does not decode as UTF-8 becomes U+FFFD, so every line is valid
//! JSON. See notes/system-log.md, "The JSONL writer".

use system_log_contract::record::{Header, TEXT_MAX, flags};
use system_log_contract::{control, severity};

/// The longest line [`render`] can produce, newline included, with room to spare: every text byte
/// and every name byte escaped to six characters, and every number at twenty digits, is 1,901.
pub const LINE_MAX: usize = 2048;

const _: () = assert!(
    7 + 20
        + 8
        + 20
        + 10
        + 20
        + 11
        + (2 + 6 * control::NAME_MAX)
        + 8
        + (2 + 6 * control::NAME_MAX)
        + 13
        + 7
        + 9
        + 6 * TEXT_MAX
        + 1
        + 11
        + 22
        + 2
        <= LINE_MAX
);

struct Out<'a> {
    buf: &'a mut [u8],
    n: usize,
}

impl Out<'_> {
    fn raw(&mut self, s: &[u8]) {
        let end = (self.n + s.len()).min(self.buf.len());
        self.buf[self.n..end].copy_from_slice(&s[..end - self.n]);
        self.n = end;
    }

    fn num(&mut self, mut v: u64) {
        let mut d = [0u8; 20];
        let mut i = d.len();
        loop {
            i -= 1;
            d[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.raw(&d[i..]);
    }

    fn string(&mut self, s: &[u8]) {
        self.raw(b"\"");
        let mut rest = s;
        while !rest.is_empty() {
            let (valid, bad) = match core::str::from_utf8(rest) {
                Ok(v) => (v, 0),
                Err(e) => (
                    // `valid_up_to` is the length of a prefix that decoded, so this cannot fail.
                    core::str::from_utf8(&rest[..e.valid_up_to()]).unwrap_or(""),
                    e.error_len().unwrap_or(rest.len() - e.valid_up_to()),
                ),
            };
            for &b in valid.as_bytes() {
                match b {
                    b'"' => self.raw(b"\\\""),
                    b'\\' => self.raw(b"\\\\"),
                    b'\n' => self.raw(b"\\n"),
                    b'\r' => self.raw(b"\\r"),
                    b'\t' => self.raw(b"\\t"),
                    0..0x20 | 0x7f => {
                        const HEX: &[u8; 16] = b"0123456789abcdef";
                        self.raw(&[
                            b'\\',
                            b'u',
                            b'0',
                            b'0',
                            HEX[(b >> 4) as usize],
                            HEX[(b & 0xf) as usize],
                        ]);
                    }
                    _ => self.raw(&[b]),
                }
            }
            rest = &rest[valid.len()..];
            for _ in 0..bad {
                self.raw(b"\\ufffd");
            }
            rest = &rest[bad.min(rest.len())..];
        }
        self.raw(b"\"");
    }

    fn opt_string(&mut self, s: Option<&[u8]>) {
        match s {
            Some(s) => self.string(s),
            None => self.raw(b"null"),
        }
    }
}

/// Render one record as a JSONL line into `out` (at least [`LINE_MAX`] bytes) and return its
/// length, newline included. `program` and `user` are what the badge was registered as; `None`
/// renders as `null`.
pub fn render(
    h: &Header,
    program: Option<&[u8]>,
    user: Option<&[u8]>,
    text: &[u8],
    out: &mut [u8],
) -> usize {
    let mut o = Out { buf: out, n: 0 };
    o.raw(b"{\"seq\":");
    o.num(h.seq);
    o.raw(b",\"time\":");
    o.num(h.time);
    o.raw(b",\"source\":");
    o.num(h.source);
    o.raw(b",\"program\":");
    o.opt_string(program);
    o.raw(b",\"user\":");
    o.opt_string(user);
    o.raw(b",\"severity\":\"");
    o.raw(severity::name(h.severity).as_bytes());
    o.raw(b"\",\"msg\":");
    o.string(&text[..text.len().min(TEXT_MAX)]);
    if h.flags & flags::CUT != 0 {
        o.raw(b",\"cut\":true");
    }
    if h.flags & flags::DROPPED_BEFORE != 0 {
        o.raw(b",\"dropped_before\":true");
    }
    o.raw(b"}\n");
    o.n
}

/// The dropped-count line a reader gets for a gap: `count` records, `from` to `to` inclusive, left
/// the ring before this reader read them.
pub fn render_dropped(count: u64, from: u64, to: u64, out: &mut [u8]) -> usize {
    let mut o = Out { buf: out, n: 0 };
    o.raw(b"{\"dropped\":");
    o.num(count);
    o.raw(b",\"from\":");
    o.num(from);
    o.raw(b",\"to\":");
    o.num(to);
    o.raw(b"}\n");
    o.n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &[u8]) -> std::string::String {
        extern crate std;
        let h = Header {
            seq: 1,
            time: 2,
            source: 3,
            severity: 4,
            flags: 0,
            len: text.len() as u16,
            kind: 0,
        };
        let mut out = [0u8; LINE_MAX];
        let n = render(&h, Some(b"p"), None, text, &mut out);
        std::string::String::from_utf8(out[..n].to_vec()).unwrap()
    }

    extern crate std;

    /// Quotes, backslashes and control characters are escaped, and bytes that are not UTF-8 become
    /// U+FFFD: every line is valid JSON whatever a writer sent.
    #[test]
    fn hostile_text_still_makes_valid_json() {
        assert_eq!(
            line(b"a\"b\\c\td\x01e\x7f"),
            "{\"seq\":1,\"time\":2,\"source\":3,\"program\":\"p\",\"user\":null,\
             \"severity\":\"warning\",\"msg\":\"a\\\"b\\\\c\\td\\u0001e\\u007f\"}\n"
        );
        assert!(line(b"ok \xff\xfe \xe2\x82").contains(r#""msg":"ok \ufffd\ufffd \ufffd\ufffd"}"#));
        assert!(line("naïve ✓".as_bytes()).contains("\"msg\":\"naïve ✓\"}"));
    }

    /// The worst case fits the bound the ring and the window are sized by.
    #[test]
    fn the_worst_case_line_fits_line_max() {
        let h = Header {
            seq: u64::MAX,
            time: u64::MAX,
            source: u64::MAX,
            severity: 0,
            flags: 0xff,
            len: TEXT_MAX as u16,
            kind: 0,
        };
        let name = [0x01u8; control::NAME_MAX];
        let mut out = [0u8; LINE_MAX + 64];
        let n = render(&h, Some(&name), Some(&name), &[0x01; TEXT_MAX], &mut out);
        assert!(n <= LINE_MAX, "{n}");
        assert_eq!(out[n - 1], b'\n');
    }

    /// A newline and a carriage return are the short escapes, not the six-character ones.
    #[test]
    fn a_newline_and_a_carriage_return_use_their_short_escapes() {
        assert!(line(b"a\nb\rc").contains(r#""msg":"a\nb\rc"}"#));
    }
}
