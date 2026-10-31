//! **What the bench boot and the shell agree on about the EL0 primitive programs**: how many
//! iterations each one times, which prefix its result line carries, and the line's spelling.
//!
//! The programs `null_syscall`, `sink_producer` and `sink_consumer` run two ways. The bench boot
//! (`kernel/src/bench/el0.rs`) spawns them on the quiet, `-icount` kernel and gates their rows
//! against `bench/baseline-*.txt`. A person types them at the production shell, and the same
//! programs print the same row under a different prefix, which is never gated (milestone 961 (the kernel's special boots become commands typed at the production shell),
//! decision D6). One definition of the loop counts keeps the two runs measuring the same thing
//! (AGENTS.md rule 7). A row reports ticks of `user_mode_runtime::now`, never converted time.
//!
//! # EXAMPLES
//!
//! ```
//! use primitive_benchmark_contract::{format_row, Prefix};
//! let mut out = [0u8; 96];
//! let n = format_row(&mut out, Prefix::Production, "null_syscall", 1234, 20_000);
//! assert_eq!(&out[..n], b"bench-production: null_syscall 1234 20000\n");
//! ```
//!
//! # BUGS
//!
//! - A row is only comparable with rows from the same boot kind: the production prefix marks a
//!   run with the console, `net_stack` and the shell competing for the core.
//!
//! Name: provisional (2026-10-31 UTC, lane/961-step-1-bench-commands). `_contract` is the suffix
//! for a crate two programs agree on (`byte_sink_contract`, `clock_contract`). Refused `bench_constants` (a
//! contract here is also a line format and a startup word, not only constants).

#![no_std]

/// Iterations of the null-syscall loop: large enough that the two clock reads are noise, small
/// enough to bear under TCG, where each `svc` is a full emulated exception.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
pub const NULL_SYSCALL_ITERATIONS: u64 = 20_000;

/// Sink messages in the timed part of the throughput loop, each carrying
/// `byte_sink_contract::INLINE_MAXIMUM` bytes. 5 000 messages are 80 000 bytes, past the 64 KiB a
/// Unix pipe buffers, so a producer that could run ahead would have had to stop at least once.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
pub const SINK_MESSAGES: u64 = 5_000;

/// Sink messages the consumer discards before it starts timing, so the first rendezvous and the
/// consumer's own start land outside the window. The producer sends this many extra.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
pub const SINK_WARMUP_MESSAGES: u64 = 64;

/// Timed `MAP_INTO` calls in `map_el0`. Each maps the one frame at a fresh address, so the count is
/// bounded by the page-table pages and revocation records the target space can hold. 500 fits one
/// L3 table (512 entries from the timed base), so the loop stays one table's worth of walks.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
pub const MAP_ITERATIONS: u64 = 500;

/// Untimed `MAP_INTO` calls before the timed ones, at a disjoint range, so the first cold
/// page-table allocation lands outside the window.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
pub const MAP_WARMUP: u64 = 8;

/// Pages of budget `map_el0` needs: its address space, the page tables and records the
/// [`MAP_WARMUP`] plus [`MAP_ITERATIONS`] maps commit, and the one frame it maps. Measured with
/// `MemoryRegion::USAGE` on 2026-10-31 (UTC): the address space and its tables committed 8 of the
/// region's pages, the frame is a ninth, and one page of margin makes ten. The shell's `--mem`
/// minimum for `map_el0` is this number (a test in `grant_plan` holds the two together).
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
pub const MAP_BUDGET_PAGES: u64 = 10;

/// The startup word (`x0`) the bench boot passes. The shell passes 0, so a program typed at the
/// prompt prints under [`Prefix::Production`].
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
pub const QUIET_BOOT_STARTUP_WORD: u64 = 1;

/// Which kind of boot ran the program, and so which prefix its row carries.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prefix {
    /// Typed at the production shell. Recorded in `bench/`, never gated.
    Production,
    /// Spawned by the bench boot. The gated rows of `bench/baseline-*.txt`.
    QuietBoot,
}

impl Prefix {
    /// The prefix for a startup word: [`QUIET_BOOT_STARTUP_WORD`] is the quiet boot, anything else
    /// is a person at the shell.
    ///
    /// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
    pub const fn from_startup_word(word: u64) -> Prefix {
        if word == QUIET_BOOT_STARTUP_WORD {
            Prefix::QuietBoot
        } else {
            Prefix::Production
        }
    }

    /// The text that opens a row, without the colon.
    ///
    /// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
    pub const fn text(self) -> &'static str {
        match self {
            Prefix::Production => "bench-production",
            Prefix::QuietBoot => "bench",
        }
    }
}

/// Write `<prefix>: <row> <ticks> <count>\n` into `out` and return its length.
///
/// Panics if `out` is shorter than the longest row (96 bytes covers any row name up to 40 bytes).
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
pub fn format_row(out: &mut [u8], prefix: Prefix, row: &str, ticks: u64, count: u64) -> usize {
    let mut n = 0;
    n = push(out, n, prefix.text().as_bytes());
    n = push(out, n, b": ");
    n = push(out, n, row.as_bytes());
    n = push(out, n, b" ");
    n = push_decimal(out, n, ticks);
    n = push(out, n, b" ");
    n = push_decimal(out, n, count);
    push(out, n, b"\n")
}

/// Write one row to a sink in the sink contract's framing and end the stream.
///
/// `send` takes the three message words and returns what the sink answered; anything but `Ok` ends the
/// write, because a reader that stopped caring has nothing left to be told. The end-of-stream
/// message goes last, so a reader downstream (the shell, or another stage) stops waiting.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/961-step-1-bench-commands.
pub fn write_row(
    mut send: impl FnMut(u64, u64, u64) -> byte_sink_contract::Sent,
    prefix: Prefix,
    row: &str,
    ticks: u64,
    count: u64,
) {
    let mut line = [0u8; 96];
    let length = format_row(&mut line, prefix, row, ticks, count);
    let mut offset = 0;
    while offset < length {
        let (w0, w1, w2, took) = byte_sink_contract::pack(&line[offset..length]);
        if !matches!(send(w0, w1, w2), byte_sink_contract::Sent::Ok) {
            return;
        }
        offset += took;
    }
    send(byte_sink_contract::eof(), 0, 0);
}

fn push(out: &mut [u8], at: usize, bytes: &[u8]) -> usize {
    out[at..at + bytes.len()].copy_from_slice(bytes);
    at + bytes.len()
}

fn push_decimal(out: &mut [u8], at: usize, mut value: u64) -> usize {
    let mut digits = [0u8; 20];
    let mut count = 0;
    loop {
        digits[count] = b'0' + (value % 10) as u8;
        count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let mut n = at;
    while count > 0 {
        count -= 1;
        out[n] = digits[count];
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(prefix: Prefix, ticks: u64, count: u64) -> ([u8; 96], usize) {
        let mut out = [0u8; 96];
        let n = format_row(&mut out, prefix, "sink_throughput", ticks, count);
        (out, n)
    }

    #[test]
    fn a_row_crosses_a_sink_in_messages_and_ends_with_eof() {
        let mut bytes = [0u8; 96];
        let mut length = 0;
        let mut ended = false;
        let mut sent = 0;
        write_row(
            |w0, w1, w2| {
                // Bounded, so a loop that never ends fails here instead of hanging the run.
                sent += 1;
                assert!(sent <= 8, "write_row did not stop");
                let mut buf = [0u8; byte_sink_contract::INLINE_MAXIMUM];
                match byte_sink_contract::unpack(w0, w1, w2, &mut buf) {
                    byte_sink_contract::Msg::Bytes(n) => {
                        bytes[length..length + n].copy_from_slice(&buf[..n]);
                        length += n;
                    }
                    byte_sink_contract::Msg::Eof => ended = true,
                    byte_sink_contract::Msg::Malformed => panic!("malformed"),
                }
                byte_sink_contract::Sent::Ok
            },
            Prefix::QuietBoot,
            "null_syscall",
            9,
            20_000,
        );
        assert_eq!(&bytes[..length], b"bench: null_syscall 9 20000\n");
        assert!(ended);
    }

    #[test]
    fn a_row_sends_exactly_its_bytes_then_one_eof_and_never_an_empty_message() {
        // A loop that ran once more at the end would pack an empty slice, take zero bytes and never
        // finish, which a mutation run reports as a timeout rather than a failure. The panic past
        // the bound turns that hang into a failure in microseconds.
        let mut sent = 0;
        let mut empty = 0;
        write_row(
            |w0, w1, w2| {
                sent += 1;
                assert!(sent <= 8, "write_row did not stop");
                let mut buf = [0u8; byte_sink_contract::INLINE_MAXIMUM];
                if let byte_sink_contract::Msg::Bytes(0) =
                    byte_sink_contract::unpack(w0, w1, w2, &mut buf)
                {
                    empty += 1;
                }
                byte_sink_contract::Sent::Ok
            },
            Prefix::QuietBoot,
            "null_syscall",
            9,
            20_000,
        );
        // "bench: null_syscall 9 20000\n" is 28 bytes: two bytes messages and the end of stream.
        assert_eq!((sent, empty), (3, 0));
    }

    #[test]
    fn the_quiet_boot_row_is_the_spelling_script_bench_reads() {
        let (out, n) = row(Prefix::QuietBoot, 7, 80_000);
        assert_eq!(&out[..n], b"bench: sink_throughput 7 80000\n");
    }

    #[test]
    fn zero_and_the_largest_word_render() {
        let (out, n) = row(Prefix::Production, 0, u64::MAX);
        assert_eq!(
            &out[..n],
            b"bench-production: sink_throughput 0 18446744073709551615\n"
        );
    }

    #[test]
    fn the_shell_startup_word_is_production_and_only_the_bench_boots_word_is_quiet() {
        assert_eq!(Prefix::from_startup_word(0), Prefix::Production);
        assert_eq!(Prefix::from_startup_word(2), Prefix::Production);
        assert_eq!(
            Prefix::from_startup_word(QUIET_BOOT_STARTUP_WORD),
            Prefix::QuietBoot
        );
    }

    #[test]
    fn a_pipe_carries_more_than_a_unix_pipe_buffers() {
        assert!(SINK_MESSAGES * 16 > 64 * 1024);
    }
}
