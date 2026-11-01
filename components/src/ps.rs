//! **`ps`: who else is running, and who is allowed to ask** (notes/process-view.md). Walk one
//! supervision domain with `abi::rendezvous::SURVEY`, say what it found on one stream and what went
//! wrong on the other. The listing is `crates/ps`, host-tested; this is the syscall and two sinks.
//!
//! **It cannot enumerate the machine.** A Unix `ps` reads an ambient `/proc`. This holds a
//! supervision endpoint and sees exactly the threads whose deaths arrive on it: no pid space to
//! scan, nothing in reach that takes a process id it was not handed. The wide grant is not
//! forbidden but nameable: a machine-wide monitor is a `ps` handed the machine's endpoint, and
//! `caps ps` prints which one this is.

//! # Capability contract
//!
//! | slot | what | why |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the table goes |
//! | 7 | the process domain, `ENUMERATE` | the supervision endpoint whose members it may **name** |
//! | 8 | the diagnostics sink, `WRITE` | where a refusal goes, so `>` cannot swallow it |
//!
//! None names a process, and `TIME` is a second walk of the same endpoint under the same right.
//! Why `ENUMERATE` and not `READ` is at [`DOMAIN_SLOT`]; it is also why there is no `pkill`.

//! # EXAMPLES
//!
//! ```text
//! $ ps
//! PROC  THR         TID  STATE     TIME(ms)
//!    1    2           5  running        310
//!    1    2           6  blocked          0
//!    2    1           9  blocked         20
//! $ ps        ps: this process holds no process-domain capability
//! ```

//! # BUGS
//!
//! - No `CMD` column: a process here has no name (`crates/ps`). `PROC` is an ordinal for one run,
//!   not an identity, and `THR` counts the threads of this domain, never the process's true total.
//! - `pgrep` is this program filtered, with the same three capabilities.
//! - `ps` lists itself, `running`, as Unix's does.
//!
//! Name: ratified 2026-09-14 (calef): the name every reader knows, shared with `crates/ps` as one
//! thing split at the IO boundary.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use user_mode_runtime::{exit, is_granted, send, survey, survey_record};

/// The output sink: where the table goes. Slot 0 is where every spawned program's output lands.
const REPORT: u64 = 0;

/// **The process domain**: a supervision endpoint, `ENUMERATE`. Everything this program can see
/// comes through here and nothing else in the capability table names a process.
///
/// `ENUMERATE`, not `READ`: `READ` would also let it `RECEIVE` a death message or `REAP` a corpse
/// out from under the real supervisor, and a domain names its members without acting on them. A
/// named slot, not the next free one, so it does not move with whatever else a line granted (as §67
/// (a program's second stream is a declaration) did for diagnostics).
const DOMAIN_SLOT: u64 = grant_plan::DOMAIN_SLOT;

/// The declared second stream (DECISIONS §67): complaints about the run, never about the domain.
const DIAG_SLOT: u64 = grant_plan::DIAGNOSTICS_SLOT;

/// Whether this process was granted a second stream, decided once in [`_start`].
static HAS_DIAG: AtomicBool = AtomicBool::new(false);

// **The interfaces it links, and the version of each** (milestone 956 (the linked interfaces carry
// checkable versions)): the progenitor refuses these bytes when their ABI entry is not the version
// it serves, and `helpers/interface_declarations_check.py` holds the list to the link graph.
interfaces_note::carry_interfaces!(&[
    interfaces_note::Declared {
        id: abi::INTERFACE_ID,
        version: abi::ABI_VERSION
    },
    interfaces_note::Declared {
        id: address_space_map::INTERFACE_ID,
        version: address_space_map::VERSION
    },
    interfaces_note::Declared {
        id: byte_sink_contract::INTERFACE_ID,
        version: byte_sink_contract::VERSION
    },
    interfaces_note::Declared {
        id: clock_contract::INTERFACE_ID,
        version: clock_contract::VERSION
    },
    interfaces_note::Declared {
        id: counter_frequency_contract::INTERFACE_ID,
        version: counter_frequency_contract::VERSION
    },
    interfaces_note::Declared {
        id: current_cpu_contract::INTERFACE_ID,
        version: current_cpu_contract::VERSION
    },
    interfaces_note::Declared {
        id: entropy_contract::INTERFACE_ID,
        version: entropy_contract::VERSION
    },
    interfaces_note::Declared {
        id: grant_plan::INTERFACE_ID,
        version: grant_plan::VERSION
    },
    interfaces_note::Declared {
        id: program_argument_contract::INTERFACE_ID,
        version: program_argument_contract::VERSION
    },
    interfaces_note::Declared {
        id: socket_contract::INTERFACE_ID,
        version: socket_contract::VERSION
    },
    interfaces_note::Declared {
        id: user_mode_runtime::INTERFACE_ID,
        version: user_mode_runtime::VERSION
    },
]);

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    HAS_DIAG.store(is_granted(DIAG_SLOT), Ordering::Relaxed);

    // **Walk first, complain second, print third.** A survey cannot know its complaints up front,
    // and §67's reader drains diagnostics to end-of-stream *before* it reads a byte of output.
    // `MAXIMUM_ROWS` is the kernel's whole thread table, so truncation is unreachable: two
    // kilobytes on a twelve-page stack.
    let mut rows = [ps::Row::default(); ps::MAXIMUM_ROWS];
    let mut found = ps::collect(&mut rows, &mut |cursor| survey(DOMAIN_SLOT, cursor));
    // **The `TIME` column is a second walk** (milestone 282 (a thread's CPU time, and the `top` it
    // makes possible)): `SURVEY` answers one record per call, so the two facts are two walks joined
    // on the tid, over the same endpoint and right as the listing above.
    found.join_cpu_time(&mut |cursor| {
        survey_record(DOMAIN_SLOT, cursor, abi::survey::record::CPU_TIME)
    });
    // **And `PROC` and `THR` are a third** (milestone 942 (`ps` and `top` show which process each thread belongs to), §313 (`ENUMERATE` on a domain covers which of its threads share a process)): the process label, over the same
    // endpoint and right. The count is computed here from the rows, so the kernel exposes none.
    found.join_process(&mut |cursor| {
        survey_record(DOMAIN_SLOT, cursor, abi::survey::record::PROCESS)
    });

    found.write_diagnostics(&mut |bytes| write_on(diag_slot(), bytes));
    diag_end();

    found.write_report(&mut |bytes| write_on(REPORT, bytes));
    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// Which endpoint a complaint goes to: the declared second stream when there is one, and the output
/// otherwise. The fallback is honest rather than a silent drop, and it is what the kernel's own
/// tests spawn this program with, since they wire no second sink.
fn diag_slot() -> u64 {
    if HAS_DIAG.load(Ordering::Relaxed) {
        DIAG_SLOT
    } else {
        REPORT
    }
}

/// **Close the second stream**, which is not tidiness: its reader waits for the end before it reads
/// the first, so a `ps` that exited without this would leave the prompt blocked.
fn diag_end() {
    if HAS_DIAG.load(Ordering::Relaxed) {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }
}

/// Write bytes to an endpoint under the sink contract, sixteen at a time. No newline is added: the
/// listing already carries its own, because where a line ends is the table's business and not the
/// transport's.
fn write_on(slot: u64, bytes: &[u8]) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(slot, w0, w1, w2);
        rest = &rest[n..];
    }
}

user_mode_runtime::panic_handler!();
