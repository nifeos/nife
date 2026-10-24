//! Machine-checked proofs (Kani), run by `script/verify`. A name server, or anyone who can forge
//! its replies, feeds the name decoder, and it must terminate and stay in bounds on **every** byte
//! string, which a fuzzer can only sample. The compression pointer loop is the classic DNS parser
//! hang; the decoder's strictly-backwards fence makes it an error, and the first harness quantifies
//! that over every message of the proved size.
//!
//! The bounds are small because what is proved is structure (termination, bounds, round trips),
//! which grows no new behavior with length; tests and the fuzz target cover realistic lengths.
//! Measured: the decoder against its full 255-byte output ran past twenty solver minutes, and
//! against a 32-byte one it takes seconds, with the loop identical.

use super::*;

/// Every compression behaviour the decoder has (a pointer, a chain of them, a pointer into a label,
/// each malformed variant) is expressible in five bytes.
const MSG: usize = 5;
const OUT: usize = 32;

/// **The name decoder never panics and never hangs, on any input.** For every 5-byte message and
/// every starting offset, the engine returns (a pointer loop becomes `Error::PointerForward`), and on
/// success the resume offset lies inside the message and past where it started, and the written
/// length lies inside the output.
///
/// The unwind bound is the termination argument made concrete: each iteration consumes a label
/// (advancing within at most `MSG` bytes) or follows a pointer (and the fence strictly decreases, so
/// at most `MSG` follows). 32 covers that with room, and Kani fails loudly if it does not.
/// Falsification: replayable `crates/domain_name_system/falsifications/proofs.the_name_decoder_is_total_and_in_bounds.patch`
#[kani::proof]
#[kani::unwind(32)]
fn the_name_decoder_is_total_and_in_bounds() {
    let msg: [u8; MSG] = kani::any();
    let off: usize = kani::any();
    kani::assume(off <= MSG);

    let mut out = [0u8; OUT];
    if let Ok((len, resume)) = decode_name_into(&msg, off, &mut out) {
        assert!(resume <= MSG, "resume offset escaped the message");
        assert!(resume > off, "the decoder consumed nothing");
        assert!(len > 0 && len <= OUT, "written length escaped the output");
    }
}

/// **The header codec round-trips on every field value.** Six `u16`s in, the same six out. Every
/// decision `Query::accept` makes hangs off these fields, the transaction id first.
/// Falsification: replayable `crates/domain_name_system/falsifications/proofs.the_header_round_trips_on_every_value.patch`
#[kani::proof]
#[kani::unwind(8)]
fn the_header_round_trips_on_every_value() {
    let h = Header {
        id: kani::any(),
        flags: kani::any(),
        qdcount: kani::any(),
        ancount: kani::any(),
        nscount: kani::any(),
        arcount: kani::any(),
    };
    let mut buf = [0u8; HEADER_LEN];
    h.write(&mut buf).unwrap();
    assert_eq!(Header::parse(&buf).unwrap(), h);
}

/// **TCP reassembly never writes past its buffer and never calls a message whole that is not.**
/// Any two pieces of up to four bytes, into a six-byte buffer: whatever lengths they announce, `feed`
/// returns rather than panicking, **including when it is fed again after refusing**, and when it says
/// the message is whole, the message is exactly the bytes its two-byte prefix announced.
///
/// The second piece is fed whatever the first returned. The first version stopped at the first
/// error, and deleting `feed`'s overflow guard left it green: the defect only panics on the call
/// after the one that should have refused, which is a call a careless client makes.
/// Falsification: replayable `crates/domain_name_system/falsifications/proofs.tcp_reassembly_is_total_and_honest.patch`
#[kani::proof]
#[kani::unwind(12)]
fn tcp_reassembly_is_total_and_honest() {
    let first: [u8; 4] = kani::any();
    let second: [u8; 4] = kani::any();
    let split_a: usize = kani::any();
    let split_b: usize = kani::any();
    kani::assume(split_a <= 4 && split_b <= 4);

    let mut buf = [0u8; 6];
    let mut reply = TcpReply::new(&mut buf);
    let _ = reply.feed(&first[..split_a]);
    let whole = reply.feed(&second[..split_b]);
    if let Ok(true) = whole {
        let mut stream = [0u8; 8];
        stream[..split_a].copy_from_slice(&first[..split_a]);
        stream[split_a..split_a + split_b].copy_from_slice(&second[..split_b]);
        let announced = u16::from_be_bytes([stream[0], stream[1]]) as usize;
        let message = reply.message().expect("whole, so a message");
        assert_eq!(message.len(), announced);
        assert_eq!(message, &stream[2..2 + announced]);
    }
}
