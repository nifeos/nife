//! **`vmstat`: the machine's counters since boot** (milestone 126 (the `procps` package), DECISIONS §225 (`free` sees the
//! machine and your share), `crates/vmstat`).
//!
//! One read of the machine statistics page and one of the ambient monotonic counter, for the
//! per-second rates. `crates/vmstat` decides the columns and says which upstream ones have no
//! subject here and why.

//! # Capability contract
//!
//! | slot | what | why |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the report goes |
//! | 8 | the diagnostics sink, `WRITE` | where a withheld page is said |
//! | 11 | the machine statistics page, `READ`, also mapped read-only | every figure |
//!
//! No clock and no budget: the seconds are `user_mode_runtime::monotonic_ns`, which every
//! process holds (`uptime`'s finding), and `vmstat` says nothing about "yours".

//! # EXAMPLES
//!
//! ```text
//! $ vmstat
//! procs ------memory (KiB)------ --system-- -cpu-
//!     r       free      total     in     cs busy  id
//!     1     110592     131072    104     35   12  88
//! ```

//! # BUGS
//!
//! `crates/vmstat`'s: no interval (milestone 106 (a wait that ends on either the interrupt or the deadline)), and `busy` is `microseconds` and `sy` together.
//!
//! Name: ratified 2026-09-27 (calef, #1360's table), as milestone 126's `free` lane minted it.

#![no_std]
// Program entry points, not the crates/ library surface the ratchet of milestone 68 (code-quality
// gates: one lint policy) tracks (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`)).
#![allow(missing_docs)]
#![no_main]

use machine_statistics_contract::{PAGE_VA, Snapshot};
use user_mode_runtime::{exit, is_granted, monotonic_ns, send};

const REPORT: u64 = 0;
const DIAG_SLOT: u64 = grant_plan::DIAGNOSTICS_SLOT;
const MACHINE_SLOT: u64 = grant_plan::MACHINE_SLOT;

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
        id: machine_statistics_contract::INTERFACE_ID,
        version: machine_statistics_contract::VERSION
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
    let has_diag = is_granted(DIAG_SLOT);
    let granted = is_granted(MACHINE_SLOT);
    let machine = if granted {
        // SAFETY: granted only alongside a read-only mapping of the same frame at `PAGE_VA`, which
        // lives as long as this process (`system_initializer`'s spawn service).
        unsafe { Snapshot::read(PAGE_VA) }
    } else {
        None
    };
    let now = monotonic_ns();
    let diag = if has_diag { DIAG_SLOT } else { REPORT };
    vmstat::write_diagnostics(granted, machine.as_ref(), &mut |b| write_on(diag, b));
    if has_diag {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }
    vmstat::write_report(machine.as_ref(), now, &mut |b| write_on(REPORT, b));
    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// Write bytes to an endpoint under the sink contract, sixteen at a time.
fn write_on(slot: u64, bytes: &[u8]) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(slot, w0, w1, w2);
        rest = &rest[n..];
    }
}

user_mode_runtime::panic_handler!();
