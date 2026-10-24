//! Fuzz the package fetch's HTTP response reader with arbitrary bytes, cut into arbitrary reads.
//!
//! `crates/http_response` runs in the progenitor on network bytes **before** the package digest
//! check, so a head that panics it takes the prompt away (milestone 198 (a package manager, and the
//! trivial install that makes a second customer possible)). The request writer, `get_request`, is
//! exercised as well: whatever host and path the fuzzer picks, a request the writer accepts has
//! exactly one blank line, at its end, so no header can be smuggled.

//! # What is asserted, beyond "it returned"
//!
//! 1. **How the reads fall does not change the answer.** The first input byte picks a read size;
//!    the rest is fed whole and in reads of that size, which must give the same status, body and
//!    verdict. An error must be the same error, except that the split reader may already have
//!    handed back body before a `BodyTooLong`, as the crate documents.
//! 2. **The body is never more than was declared, and is always the tail of the read it came in**,
//!    so the head never leaks into a body that is then hashed.

#![no_main]

use http_response::{Error, Response, get_request};
use libfuzzer_sys::fuzz_target;

/// Feed `bytes` in reads of `step`, checking property 2 on every read, and return the outcome.
fn read_in(bytes: &[u8], step: usize) -> (Result<(), Error>, Response, Vec<u8>) {
    let mut response = Response::new();
    let mut body = Vec::new();
    for chunk in bytes.chunks(step) {
        match response.feed(chunk) {
            Ok(part) => {
                assert!(
                    chunk.ends_with(part),
                    "the body is not the tail of its read"
                );
                body.extend_from_slice(part);
                if let Some(length) = response.content_length() {
                    assert!(body.len() as u64 <= length, "more body than was declared");
                }
            }
            Err(e) => return (Err(e), response, body),
        }
    }
    (Ok(()), response, body)
}

fuzz_target!(|data: &[u8]| {
    let Some((&first, bytes)) = data.split_first() else {
        return;
    };
    let step = usize::from(first).max(1);

    let (whole, whole_response, whole_body) = read_in(bytes, bytes.len().max(1));
    let (split, split_response, split_body) = read_in(bytes, step);

    assert_eq!(whole, split, "the verdict depends on how the reads fell");
    assert_eq!(whole_response.status(), split_response.status());
    assert_eq!(
        whole_response.content_length(),
        split_response.content_length()
    );
    match whole {
        Ok(()) => {
            assert_eq!(
                whole_body, split_body,
                "the body depends on how the reads fell"
            );
            assert_eq!(whole_response.is_complete(), split_response.is_complete());
            if whole_response.is_complete() {
                assert_eq!(
                    Some(whole_body.len() as u64),
                    whole_response.content_length()
                );
            }
        }
        // The split reader may have handed back body before the read that overran; the whole
        // reader refuses that read in one piece. Anything handed back is still a prefix of the body.
        Err(Error::BodyTooLong) => assert!(whole_body.is_empty()),
        Err(_) => {
            assert!(whole_body.is_empty() && split_body.is_empty());
        }
    }

    // The request writer: split the same bytes into a host and a path at the step.
    let cut = step.min(bytes.len());
    if let (Ok(host), Ok(path)) = (
        core::str::from_utf8(&bytes[..cut]),
        core::str::from_utf8(&bytes[cut..]),
    ) {
        let mut out = [0u8; 512];
        if let Some(n) = get_request(host, path, &mut out) {
            let request = &out[..n];
            let blank = request.windows(4).position(|w| w == b"\r\n\r\n");
            assert_eq!(blank, Some(n - 4), "a request carried a second blank line");
            assert_eq!(
                request.windows(2).filter(|w| *w == b"\r\n").count(),
                3,
                "a request carried a line it was not asked for"
            );
        }
    }
});
