//! A round-robin scheduler, and the preemption that makes it mean something.
//!
//! The timer fires, the handler calls [`schedule`], and the CPU is taken from a thread that never
//! asked to give it up (DECISIONS §5 (preemptive threads with real stacks)). Three rules hold:
//! release the run-queue lock before switching, keep interrupts masked across the switch, and
//! have a brand-new thread unmask its own interrupts. notes/threads.md says why breaking each is a
//! bug, and notes/scheduler.md covers the per-core queues, placement and stealing.

use core::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, Ordering};

use intrusive_fifo::Unqueued;
use thread_wake_handshake::{SwitchOutVerdict, WakeVerdict};

use crate::cpu;
use crate::sync::{IrqSafeMutex, rank};
use crate::thread::{
    CapabilityTableLock, Context, QuotaToken, State, Thread, ThreadId, Wait, WaitRole, switch_to,
};

// Each child is part of this module split across files (§271 (the scheduler file splits into
// submodules of `sched`)), not a client of it: it takes `use super::*` to reach `IPC_TABLES` and
// the thread table, and is re-exported whole, so every path outside `sched` is unchanged.
mod capabilities;
mod configure;
mod futex;
mod inspection;
mod interrupt_route;
mod ipc_tables;
mod notification;
mod placement;
mod process;
mod region_reaping;
mod rendezvous;
mod spawning;
mod suspension;
mod switch_decision;
mod thread_control_block;
mod thread_table;
mod timer;

pub use capabilities::*;
#[cfg(target_arch = "riscv64")]
pub use configure::current_thread_pointer;
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
#[cfg_attr(not(any(test, feature = "system_tests")), allow(unused_imports))] // as the function
pub use configure::grant_cycle_counter;
#[cfg(feature = "system_tests")]
pub use configure::grant_cycle_counter_to_current;
pub use configure::{
    configure_thread_control_block, configure_thread_control_block_joining,
    configure_thread_control_block_with_thread_pointer, set_thread_pointer,
};
#[cfg_attr(not(feature = "system_tests"), allow(unused_imports))]
pub use futex::{futex_queued, futex_waiters};
pub use futex::{futex_wait, futex_wake, is_current_space};
pub use inspection::*;
pub use interrupt_route::*;
pub use ipc_tables::*;
pub use notification::*;
pub use placement::*;
pub use process::{ProcessId, create_process_from, destroy as destroy_process, with_holders};
#[cfg(feature = "system_tests")]
pub use process::{process_state, process_threads_started, reap_corpses_now, strand_member};
pub use region_reaping::*;
pub use rendezvous::*;
pub use spawning::*;
pub use suspension::{resume_thread_control_block, suspend_thread_control_block};
pub use thread_control_block::*;
pub use thread_table::*;
pub use timer::*;

/// **Per-cpu ring of the last few scheduler events** (module name provisional): the last
/// [`trace::DEPTH`] events each core performed, which [`dump_threads`] prints, so the path into a
/// wedge is on the serial log. One writer per ring, its own core; compiled out of `--features
/// bench` builds. See notes/scheduler-instruments.md.
#[cfg(not(feature = "bench"))]
mod trace;

/// The bench build's no-op twin of [`trace`]: same names, same signatures, nothing recorded, so
/// the benchmark numbers the tripwire watches never measure the instrument and the call sites
/// need no `cfg` of their own.
#[cfg(feature = "bench")]
#[path = "sched/trace_disabled.rs"]
mod trace;

/// **A corruption tripwire over `IpcTables`'s registries** (module name provisional): armed, it
/// re-reads the thread table and the rendezvous registry on each tick and prints every byte that
/// changed. It shows rather than judges. Unarmed it costs one relaxed load per tick; compiled out
/// of `--features bench` builds. See notes/scheduler-instruments.md.
#[cfg(not(feature = "bench"))]
// An exception, marked as AGENTS.md asks: nothing arms this today, and it is kept because it is
// how a board hang gets diagnosed. notes/scheduler-instruments.md has the case for keeping it.
#[allow(dead_code)]
mod canary;

/// The bench build's no-op twin of [`canary`], so the call sites carry no `cfg` of their own.
#[cfg(feature = "bench")]
#[path = "sched/canary_disabled.rs"]
mod canary;

#[cfg(test)]
mod tests;

mod current;
mod departure;
mod ipc_outcome;
mod preemption;
mod switch;
mod waking;

use current::*;
use departure::*;
pub use departure::{exit, exit_thread, fault, yield_now};
use ipc_outcome::*;
pub use ipc_outcome::{take_ipc_aborted, take_ipc_refused};
use preemption::*;
pub use preemption::{on_tick, preempt_if_needed};
pub(crate) use switch::finish_switch;
use switch::*;
pub use switch::{init, is_running, mark_current_fp_live, schedule};
use waking::*;
