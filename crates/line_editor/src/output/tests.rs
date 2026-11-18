use super::*;
extern crate std;
use std::vec::Vec;

/// Output expansion: every bare `\n` becomes `\r\n`, existing `\r\n` is untouched (the
/// engine never emits `\r\r\n`), and ANSI passes through.
#[test]
fn output_expansion_translates_newlines() {
    struct Buf(Vec<u8>);
    impl Sink for Buf {
        fn put(&mut self, b: &[u8]) {
            self.0.extend_from_slice(b);
        }
    }
    let mut out = Buf(Vec::new());
    expand_output(b"a\nb\x1b[1mc\n", &mut out);
    assert_eq!(out.0, b"a\r\nb\x1b[1mc\r\n");
}

/// A sink is never handed an empty write: empty input, a leading newline and a trailing newline
/// each leave no empty put behind.
#[test]
fn output_expansion_never_puts_an_empty_slice() {
    struct Puts(Vec<Vec<u8>>);
    impl Sink for Puts {
        fn put(&mut self, b: &[u8]) {
            self.0.push(b.to_vec());
        }
    }
    for input in [&b""[..], b"\n", b"a\n", b"\nb", b"\n\n"] {
        let mut out = Puts(Vec::new());
        expand_output(input, &mut out);
        assert!(out.0.iter().all(|put| !put.is_empty()), "{input:?}");
    }
}

/// The puts are exactly the text between newlines and each `\r\n`, in order, so a dropped
/// trailing segment, a dropped leading segment and an extra empty put all show.
#[test]
fn output_expansion_puts_each_segment_once_in_order() {
    struct Puts(Vec<Vec<u8>>);
    impl Sink for Puts {
        fn put(&mut self, b: &[u8]) {
            self.0.push(b.to_vec());
        }
    }
    let expand = |input: &[u8]| {
        let mut out = Puts(Vec::new());
        expand_output(input, &mut out);
        out.0
    };
    assert_eq!(
        expand(b"a\nb"),
        [b"a".to_vec(), b"\r\n".to_vec(), b"b".to_vec()]
    );
    assert_eq!(expand(b"ab"), [b"ab".to_vec()]);
    assert_eq!(expand(b"\nb"), [b"\r\n".to_vec(), b"b".to_vec()]);
    assert_eq!(expand(b"a\n"), [b"a".to_vec(), b"\r\n".to_vec()]);
    assert!(expand(b"").is_empty());
}
