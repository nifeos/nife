//! A thread reading the CPU's cycle counter from user mode (milestone 229 (build the cycle-counter
//! grant)).
//!
//! Reads the counter twice through `user_mode_runtime::cycle_reading` (calef's ruling B4,
//! 2026-10-07 UTC) and reports [`capability_witness_contract::CYCLE_COUNTER_WORD`], the difference,
//! and what they count (an `abi::cycle_counter::CycleMeaning` as its word). It holds a report
//! endpoint (slot 0) and nothing else: the grant is a property of this thread, which the context
//! switch writes into a system register before it runs. Without it the EL0 read is one
//! `PMUSERENR_EL0` (aarch64) or `scounteren` (riscv64) forbids, which traps and kills the thread;
//! the kernel-side test runs this both ways and asserts each outcome.

//! # Why two reads
//!
//! The first read proves the grant reached a thread not yet switched. The `yield` gives the
//! scheduler a chance to switch this thread out and back in, so the second read follows the context
//! switch re-applying the grant from the thread's own field. A yield does not guarantee a switch
//! (this may be the core's only runnable thread), so the second read is evidence; the
//! register-level assertions in `arch::*::timer`'s tests prove the write itself.

//! # Bugs
//!
//! **The difference is checked on the kernel side, and only partly.** The kernel starts
//! `PMCCNTR_EL0` on every core, so its test asserts the difference is positive wherever `arch::pmu`
//! reports the counter running (and always on `x86_64`). Not on riscv64, where the kernel's counter
//! and the `cycle` CSR this program reads may differ. Nothing asserts the size: under QEMU it is
//! emulator time. The meaning is checked on all three.
//!
//! Name: provisional (milestone 291). This was `hello`'s `CYCLE_COUNTER_CHILD` role, number 42.
//! Refused keeping `_child`: nothing builds this program as a child. The kernel's own test starts
//! it directly, because milestone 229 shipped the grant mechanism without a syscall method to set
//! it and there is therefore no userspace route to a granted thread; `_child` named a relationship
//! that does not exist.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{cycle_reading, exit, send, yield_now};

const REPORT: u64 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let first = cycle_reading();
    yield_now();
    let second = cycle_reading();
    send(
        REPORT,
        capability_witness_contract::CYCLE_COUNTER_WORD,
        second.count.wrapping_sub(first.count),
        second.meaning.to_word(),
    );
    exit()
}

user_mode_runtime::panic_handler!();
