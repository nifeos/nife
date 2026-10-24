//! **Everything a network time client holds, and the clock page is still not reachable** (milestone
//! 51, split out of the client's binary by milestone 290; notes/ntp.md).
//!
//! Spawned with `components/src/network_time_client.rs`'s five slots and told the address at which
//! a process holding the *set* authority maps the clock page, it reports the address, writes there,
//! and faults: its address space maps that frame nowhere. **The boundary is the mapping, not the
//! layout.** This is the claim Unix cannot make, since `ntpd` runs as root.
//! `an_ntp_client_holds_no_writable_clock_page` in `system_tests/src/user/ntp_tests.rs` is the
//! assertion; this program is the thing it asserts about.

//! # What keeps it honest
//!
//! The fault comes from the **capability set**, not the code, so this need not share the client's
//! binary. It and the client are endowed by **one function**, `spawn_with_client_endowment` in
//! `kernel/src/user/ntp_service.rs`, which takes the image as a parameter, so a sixth slot added to
//! the client is a sixth slot here. **Never write a second capability list**: a hand-kept copy of a
//! fact the wiring holds is how a security claim decays without breaking a build (milestone 117's
//! `swish` `caps` bug). See notes/ntp/the-client-tests.md.

//! # Reaching the second report is the failure
//!
//! It reports [`RPT_PROBING`] **before** the write, so the test asserts that the user-fault counter
//! rose, that the faulting address was exactly the one it aimed at, and that **no second report is
//! waiting**. The fault is a translation fault (it maps the page nowhere), and the test
//! deliberately does not assert the kind: pinning it would fail if the page were ever mapped
//! read-only, a *weaker* system passing a stricter-looking assertion.

//! # BUGS
//!
//! - **This proves an absence, so it witnesses one wiring.** The client's endowment does not reach
//!   the clock page at the setter's address; showing no other page lets a client move time would be
//!   a proof over the whole capability graph, which nothing here performs.
//! - **It writes `clock_contract::state::SET` because a real attempt would**, and nothing checks
//!   the value, since the write never lands. If confinement broke, the page would hold a plausible
//!   state word, not a sentinel, and the test would catch it through `clock.page()` changing.
//!
//! Name: ratified 2026-09-14 (calef, milestone 290), ruled on being offered it against
//! `clock_page_write_attempt`. The register is this tree's existing fixture names
//! (`first_attempt_crasher`, `swap_boundary_witness`, `kernel_test_subject`), which say what the
//! fixture **proves** rather than what it does: what it does is one volatile store, and a reader
//! meeting `unwritable_clock_witness` in a boot log learns the property under test instead of the
//! instruction. Refused `clock_page_write_attempt` for that reason, and because an attempt is not a
//! thing, which is the noun rule one step further in. Refused keeping it as `ROLE_PROBE_CLOCK`
//! inside the client, on the "same binary" argument corrected above: the argument that looked
//! load-bearing was not, and the shape it justified put a test-only branch inside a component the
//! client's own header claimed had none. `network_time` is deliberately absent from this name: it
//! is not a time program, it is a witness to a mapping, and the endowment it borrows is named at
//! its one call site.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, send};

/// Slot 0: the report endpoint (WRITE). **The only slot this program uses**, and it is handed the
/// client's other four so that what it fails to reach, it fails to reach as a fully endowed network
/// time client rather than as a stripped-down one.
const REPORT: u64 = 0;

/// About to write the clock page at `w1`. The next thing this process does is fault; anything after
/// this report means it did not. One numbering space with the other two programs milestone 290 split
/// the `ntp` binary into, and the number did not move in that split.
pub const RPT_PROBING: u64 = 6;

/// `a0` is the address to write: where a process holding the *set* authority maps the clock page.
#[unsafe(no_mangle)]
pub extern "C" fn _start(va: u64) -> ! {
    send(REPORT, RPT_PROBING, va, 0);
    // SAFETY: deliberately not safe. This is the assertion: the write must fault. If it does not,
    // the process survives to send the report below, and the test fails on that.
    unsafe {
        core::ptr::write_volatile(va as *mut u64, clock_contract::state::SET);
    }
    send(REPORT, RPT_PROBING, va, 1);
    exit()
}

user_mode_runtime::panic_handler!();
