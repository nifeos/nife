//! **`printenv`**: print the inert-configuration page (milestone 47's environment-variable fork,
//! DECISIONS §111, notes/env-config.md).
//!
//! The whole program is: read a page, print what it carries. It holds two capabilities and neither
//! can change anything, `date`'s shape one manifest field over ([`grant_plan::Manifest::configuration`]
//! rather than [`grant_plan::Manifest::clock`]). The page is mapped `READ`-only, which is why there
//! is no `printenv -s TZ=...`: the missing authority is a page permission, not a check here.
//!
//! `environment_contract::ConfigPage` answers `Option<&str>` per key: `Some(value)` when the page is
//! valid and carries that key, and `None` both when the key was never set and when nobody assembled
//! a page at all. This program tells "no grant" apart the way `date` tells "no clock capability"
//! apart: by checking whether it holds the capability before printing anything.
//!
//! ```text
//! printenv                  TZ=UTC
//!                            LANG=C
//!                            TERM=dumb
//! printenv  (a key unset)   TZ=UTC
//!                            LANG (unset)
//!                            TERM=dumb
//! printenv  (no grant)      printenv: no configuration was granted
//! ```
//!
//! # BUGS
//!
//! - **No filtering.** Unix's `printenv NAME` prints one variable; this always prints all three,
//!   because `ArgSpec` has no position yet.
//! - **The three keys are hardcoded.** Nothing here enumerates the page; a fourth validated domain
//!   would need a fourth line written by hand.
//!
//! Name: ratified 2026-09-13 (calef, working the unratified worklist). Unix's own name, true here:
//! upstream `printenv` with no arguments prints the whole environment, which is exactly this
//! program. Refused `env` (`env(1)` also sets variables and runs a command, neither possible here).
//! Refused `config` and `environment` (generic, and `environment_contract` spends the stem).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and documenting an
// OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use environment_contract::ConfigPage;
use user_mode_runtime::{exit, invoke, send};

/// Slot 0: where the output goes. An endpoint with `WRITE`, and the same 16-bytes-per-message
/// framing the std PAL's stdout uses (`w0` = the byte count, `w1`|`w2` = the bytes, little-endian).
const REPORT: u64 = 0;

/// Slot 1: the config page's `PageFrame` capability, with `READ` and nothing else. Its *presence*
/// is what [`config_page`] probes for; its rights are what stop this program setting anything.
const CONFIG_SLOT: u64 = 1;

/// Where the wiring maps the config page, read-only. Must match
/// `crates/system_initializer`'s `CHILD_CONFIG_VA`.
const CONFIG_VA: u64 = address_space_map::pair_page(0x00e0_0000);

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    let Some(page) = config_page() else {
        line(b"printenv: no configuration was granted");
        send(REPORT, byte_sink_contract::eof(), 0, 0);
        exit();
    };

    key_line(&page, b"TZ", page.tz());
    key_line(&page, b"LANG", page.lang());
    key_line(&page, b"TERM", page.term());

    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// One `KEY=value` line when the domain holds a value, `KEY (unset)` when the page is valid and
/// simply does not carry it. `_page` is unused directly but keeps this a method-shaped call at
/// every site, so a reader sees the three lines are the same operation three times rather than
/// three hand-written formats that happen to agree today.
fn key_line(_page: &ConfigPage, key: &[u8], value: Option<&str>) {
    let mut buf = [0u8; 96];
    let mut n = 0;
    push(&mut buf, &mut n, key);
    match value {
        Some(v) => {
            push(&mut buf, &mut n, b"=");
            push(&mut buf, &mut n, v.as_bytes());
        }
        None => push(&mut buf, &mut n, b" (unset)"),
    }
    line(&buf[..n]);
}

/// **Append what fits and silently drop the rest**, advancing `*n` by what was taken.
///
/// The truncation is deliberate and is this program's whole answer to a value it did not write:
/// `TZ`, `LANG` and `TERM` come out of a page `system_initializer` filled, and a line that will not
/// fit is a short line rather than a fault. What must never happen is the other thing, which is
/// this writing past the array; the proof at the bottom of this file is that claim, stated for
/// every starting offset including ones no caller here produces.
///
/// The guard is per byte rather than per call on purpose: a length check up front would have to
/// know `bytes.len()` and `*n` are both trustworthy, and one of them comes from the page.
fn push(buf: &mut [u8; 96], n: &mut usize, bytes: &[u8]) {
    for &b in bytes {
        if *n < buf.len() {
            buf[*n] = b;
            *n += 1;
        }
    }
}

/// Whether a capability is in `slot`, without touching whatever it names.
///
/// [`config_page`]'s probe, `date`'s `is_granted()` lifted verbatim: invoke a method number no
/// object type defines, so the call can only be refused, and read *which* refusal came back. An
/// empty slot answers `NoSuchSlot`; a real object answers `BadMethod`, which is a refusal from
/// something that exists.
fn is_granted(slot: u64) -> bool {
    /// A method number no object type defines, so the invocation can only ever be refused.
    const NO_SUCH_METHOD: u64 = 0xffff;
    // SAFETY: a syscall that cannot succeed; the kernel validates the slot before the method.
    let r = unsafe { invoke(slot, NO_SUCH_METHOD, 0, 0, 0) };
    r != abi::Error::NoSuchSlot as i64
}

/// The config page, or `None` when this process was granted no configuration capability at all.
///
/// The probe has to answer **without touching the page**, for `date`'s reason (a different binary,
/// the same shape): a process granted nothing has nothing mapped at [`CONFIG_VA`], and a read
/// there would fault instead of answering.
fn config_page() -> Option<ConfigPage> {
    if !is_granted(CONFIG_SLOT) {
        return None;
    }
    // SAFETY: the wiring maps the config page read-only at CONFIG_VA alongside the capability the
    // probe just found, and nothing unmaps it. Without the capability we never build the pointer.
    Some(unsafe { ConfigPage::new(CONFIG_VA) })
}

/// Send `bytes` and a newline to the output endpoint, 16 bytes per message. `date`'s `line`/
/// `line_on` collapsed into one, because this program has only the one destination.
fn line(bytes: &[u8]) {
    let mut out = [0u8; 96];
    let n = bytes.len().min(out.len() - 1);
    out[..n].copy_from_slice(&bytes[..n]);
    out[n] = b'\n';
    for chunk in out[..n + 1].chunks(16) {
        let mut w1 = 0u64;
        let mut w2 = 0u64;
        for (i, &b) in chunk.iter().enumerate() {
            if i < 8 {
                w1 |= (b as u64) << (8 * i);
            } else {
                w2 |= (b as u64) << (8 * (i - 8));
            }
        }
        send(REPORT, chunk.len() as u64, w1, w2);
    }
}

// Kani links `std`, which already defines `panic_impl`, and a second definition is a duplicate lang
// item that does not compile. Milestone 193 took exactly this `cfg` on `kernel/src/panic.rs` for
// exactly this reason. What it costs is item 3 of notes/user-proofs.md's stub list: nothing proved
// below says anything about what this program does after a panic.
#[cfg(not(kani))]
user_mode_runtime::panic_handler!();

/// **What the prover can see of `printenv`, and what it cannot** (milestone 197).
///
/// Read notes/user-proofs.md before adding a harness here; it enumerates the stub boundary, and the
/// hazard of proving a program like this one is that a stub reads as coverage. The short version:
/// the boundary is **hard rather than soft**, because every capability this program holds is
/// reached through `user_mode_runtime`, whose calls are `asm!`, and Kani refuses an unsupported construct
/// instead of proving past it. A harness that wandered into [`line`], [`is_granted`] or
/// [`config_page`] would fail loudly rather than report a proof about a fiction. What is left is
/// [`push`], which is the only thing here that decides anything.
#[cfg(kani)]
mod proofs {
    /// The buffer every caller of [`super::push`] gives it.
    const CAP: usize = 96;
    /// A byte [`super::push`] never writes on its own, so an untouched cell is distinguishable
    /// from a written one without keeping a second copy of the buffer to compare against.
    const UNTOUCHED: u8 = 0xAA;

    /// **`push` writes only inside its buffer, appends only at the offset it was given, and never
    /// leaves that offset past the end.**
    ///
    /// The one thing standing between a value this program did not write and a byte past the end of
    /// a fixed array is the `*n < buf.len()` inside [`super::push`]. `key_line` calls it up to
    /// three times against one 96-byte buffer, with two of the three arguments coming out of the
    /// configuration page (`ConfigPage::tz`, `lang`, `term`), so the running offset is a function
    /// of somebody else's bytes. This says the guard holds for **every** offset it could be handed
    /// and every content, rather than for the three the program happens to produce.
    ///
    /// Four claims, none implied by the others:
    ///
    /// - Nothing outside `buf` is written. Kani checks that for free on any run, which is why the
    ///   starting offset below is left **unconstrained**: `push` must be memory-safe even when
    ///   handed an offset already past the end, and it is, because the guard is a comparison rather
    ///   than a subtraction.
    /// - The offset never *becomes* out of range. Conditional on starting in range, which is the
    ///   precondition every call site here satisfies by starting at zero.
    /// - It appends: nothing below the starting offset moves.
    /// - It reports what it wrote: nothing at or above the final offset moves either, so a caller
    ///   that keeps pushing lands on ground nobody has touched.
    ///
    /// **The unwind bound is `bytes.len() == 4`, and here is what it does and does not cost.** The
    /// loop body is one comparison and one store per byte, and the starting offset is symbolic over
    /// the whole of `usize`, so four bytes already exercise every way a push can meet the
    /// boundary: entirely below it, crossing it, sitting exactly on it, and entirely past it. What
    /// four bytes cannot see is a defect that only appears at some larger length, and no such
    /// defect is expressible in a loop whose state is a single `usize` that only ever increments.
    /// Stating the bound rather than raising it is the honest trade; raising it costs solver time
    /// and buys nothing this argument does not already give.
    ///
    /// Falsification: replayable `components/falsifications/proofs.push_never_writes_past_the_buffer_it_was_given.patch`
    #[kani::proof]
    fn push_never_writes_past_the_buffer_it_was_given() {
        let mut buf = [UNTOUCHED; CAP];
        let start: usize = kani::any();
        let mut n = start;
        let bytes: [u8; 4] = kani::any();

        super::push(&mut buf, &mut n, &bytes);

        if start <= CAP {
            assert!(n <= CAP);
            assert!(n >= start);
            // Both walks are written over the whole buffer with the interesting range selected
            // inside, rather than as `0..start` and `n..CAP`. A range whose endpoint is symbolic
            // has no unwinding bound CBMC can find, and the harness simply never terminates; this
            // shape costs `CAP` iterations of a byte comparison and finishes in a tenth of a
            // second. Found the slow way, and recorded in notes/user-proofs.md.
            for k in 0..CAP {
                if k < start || k >= n {
                    assert!(buf[k] == UNTOUCHED);
                }
            }
        } else {
            // Handed a nonsense offset it must still not write anything, and must not move it.
            assert!(n == start);
            for k in 0..CAP {
                assert!(buf[k] == UNTOUCHED);
            }
        }
    }

    /// **The buffer can actually be filled**, so the assertion above is not proved by an assumption
    /// set that never reaches the boundary.
    ///
    /// DECISIONS §134's note is that this tree carried 23 `cover` sites against 141 harnesses, and
    /// a bound nothing approaches is how a proof becomes decoration. This says an offset and four
    /// bytes exist that leave the buffer exactly full with bytes still to place, which is the
    /// truncation [`super::push`]'s own doc describes.
    ///
    /// **Falsification gap, found 2026-10-05 (milestone 323 (the falsification record is incomplete in five ways, and each was found by a different lane), batch 2):** the `cover` alone could not
    /// go red. Kani 0.67.0 reports an unsatisfiable `cover` as "0 of 1 cover properties satisfied"
    /// and still prints `VERIFICATION:- SUCCESSFUL` with exit code zero, so a `push` that reserved
    /// its last byte (`*n + 1 < buf.len()`) kept this harness green. The assertion below states the
    /// same boundary as a check: `push` takes exactly as many bytes as fit.
    /// Falsification: replayable `components/falsifications/proofs.the_buffer_can_be_filled_exactly.patch`
    #[kani::proof]
    fn the_buffer_can_be_filled_exactly() {
        let mut buf = [UNTOUCHED; CAP];
        let start: usize = kani::any();
        kani::assume(start <= CAP);
        let mut n = start;
        let bytes: [u8; 4] = kani::any();

        super::push(&mut buf, &mut n, &bytes);

        assert_eq!(n, core::cmp::min(CAP, start + bytes.len()));
        kani::cover!(n == CAP && start < CAP);
    }
}
