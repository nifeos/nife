//! **The supervised sub-server that dies once** (milestone 22 phase B.2).
//!
//! The thing the tree exists to keep alive. It is handed exactly one capability, a WRITE view of a
//! report endpoint, and one scalar: which attempt it is. Attempt 0 crashes on purpose; any later
//! attempt does its work and exits cleanly.
//!
//! That shape is deliberate, because it exercises **both** halves of the fault endpoint (DECISIONS
//! §26) against a real supervisor: a crash that must be restarted, and a clean exit that must not be.
//! A program that only crashed would prove the restart but not the policy.
//!

//! Name: ratified 2026-09-13 (calef, working the unratified worklist), replacing `flaky`.
//! Introduced 2026-07-29 as a supervision fixture: attempt 0 crashes on purpose and any later
//! attempt exits cleanly, which exercises both halves of DECISIONS §26 (the fault
//! endpoint) against a real supervisor.
//!
//! The reason the old name failed is the reason a blind sweep would be dangerous, and they are
//! the same reason. "Flaky" is the field's established word for a test that fails
//! intermittently, and this program does not fail intermittently: it fails deterministically,
//! once, by attempt number. So the name borrowed recognition it then contradicted, and a reader
//! who knew the word arrived with the wrong model. That borrowing also means the word appears
//! across this tree in its ordinary sense, in prose that must not move: `script/lint`,
//! `.github/workflows/verify.yml`, `design/fatal-risks/README.md`, and a quotation in
//! `notes/proof-retrospective.md` that is protected twice over. Enumerate before sweeping.
//!
//! Refused `dies_once`, which this header used to propose and which is the clearest description
//! in the file, because it is a verb phrase where the noun rule wants a thing. Refused `crasher`
//! as generic and wrong by half, since the later attempts do not crash and that is the half
//! proving a clean exit must *not* be restarted. Refused `restart_fixture`: once 175 lands this
//! file is in `fixtures/`, so the directory already says fixture. `first_attempt_crasher` is an
//! agent noun in the qualifier-plus-agent-noun shape ratified the same day for
//! `interrupt_heeder` and `interrupt_ignorer`, and it states the determinism the old name denied.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

// Each binary in the tree compiles the shared module but uses a different slice of it (the sub-server
// builds nothing, the supervisor holds no memory), so the unused halves are expected, not dead.

use supervision_protocol::REPORT_SERVER_RAN;
use user_mode_runtime::send;

/// Our one capability.
const REPORT: u64 = 0;

/// The unmapped address the first attempt loads from. Distinctive, so the fault address in the
/// supervisor's message is visibly real rather than a zero placeholder.
const BAD_ADDR: u64 = 0x00A5_0000;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, attempt: u64, _a2: u64) -> ! {
    send(REPORT, REPORT_SERVER_RAN, attempt, 0);

    if attempt == 0 {
        // Crash. A real load from an address nothing maps, so the kernel's fault path (not a
        // cooperative exit) is what the supervisor hears about.
        // SAFETY: deliberately unsafe. This is the fault under test.
        unsafe { core::ptr::read_volatile(BAD_ADDR as *const u64) };
    }

    // The restart did its work. Exit cleanly, which the supervisor must read as "finished."
    user_mode_runtime::exit()
}

user_mode_runtime::panic_handler!();
