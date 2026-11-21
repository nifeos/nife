#![no_std]
//! **The byte-sink test fixture**: the bytes an indifferent writer writes, the file a file sink
//! writes them into, and the sentinels its report carries. One definition, so the component and the
//! kernel-side test cannot disagree about what "the same bytes" means. They are a test harness's
//! facts, not part of the wire contract. Its one dependency is `byte_sink_contract`, whose `Sent`
//! it reports.
//!
//! Not published: nothing outside this tree has a reason to read it.
//!
//! Name: provisional. It holds what `byte_sink_contract` held as its `fixture` module, and follows
//! `socket_fixture` and `filesystem_fixture`. Considered `byte_sink_test_fixture`, which repeats
//! what `fixture` already says.

use byte_sink_contract::Sent;

/// What `fixtures/src/sink_transcript_writer.rs` writes. Long enough to span several messages and to
/// end mid-message, so a decoder that only ever sees full 16-byte chunks is not being flattered
/// by the fixture.
pub const TRANSCRIPT: &[u8] =
    b"a program does not know what its output slot holds, and that is the point.\n";

/// The name `fixtures/src/file_sink.rs` writes into. Created on first use rather than shipped in the
/// image, because the file's existence is part of what the test proves.
pub const SINK_NAME: &str = "sinkout";

/// A sink reports this once its destination is open and before it takes a byte, so a hang in
/// opening the file is distinguishable from a hang in the serve path. Same discipline as
/// `filesystem_contract::READY`.
pub const READY: u64 = 0x5142_0000_0000_0001;

/// A sink reports this after end-of-stream, with the byte total in the second word.
pub const DONE: u64 = 0x5142_0000_0000_0002;

/// A sink reports this on a message the contract does not define, with the offending first word
/// in the second position. It refuses rather than guessing (see
/// [`byte_sink_contract::Msg::Malformed`]).
pub const BAD: u64 = 0x5142_0000_0000_0003;

/// A writer's report of how its last `SEND` classified. A number, because the assertion it
/// serves is about telling `Gone` from `NoSink` and a boolean cannot carry that.
pub const fn code(sent: Sent) -> u64 {
    match sent {
        Sent::Ok => 0,
        Sent::Gone => 1,
        Sent::NoSink => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three report codes are distinct, which is the only property of them that matters: a test
    /// that could not tell `Gone` from `NoSink` in a report would not be testing anything.
    #[test]
    fn the_report_codes_tell_the_three_outcomes_apart() {
        assert_ne!(code(Sent::Ok), code(Sent::Gone));
        assert_ne!(code(Sent::Ok), code(Sent::NoSink));
        assert_ne!(code(Sent::Gone), code(Sent::NoSink));
    }
}
