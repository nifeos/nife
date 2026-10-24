//! **The small primitives the portable kernel calls**: halting, waiting for an interrupt, the
//! stack pointer, the DMA write barrier and instruction-cache sync.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Stop this CPU forever, cheaply. `hlt` parks it until an interrupt; with interrupts masked and
/// nothing left to wake it, that is the rest of time at zero host CPU. The same discipline as the
/// other two architectures' `wfi`. See notes/skills/qemu-hygiene/SKILL.md, "Never leave QEMU running".
/// Takes a [`crate::arch::HaltReason`], whose constructors are the list of who may stop a core for good;
/// a thread whose work is done leaves with `sched::exit` instead (milestone 720 (provisional)).
pub fn halt(_: crate::arch::HaltReason) -> ! {
    loop {
        instructions::hlt();
    }
}

/// Park until the next interrupt (the scheduler's idle primitive).
pub fn wait_for_interrupt() {
    instructions::hlt();
}

/// This CPU's current stack pointer, for the stack-overflow canary check (stack.rs).
pub fn current_sp() -> u64 {
    instructions::read_rsp()
}

/// A DMA write memory barrier: order all prior stores before any device sees a later one.
///
/// `sfence`. x86 is TSO, so ordinary stores are already ordered against each other; `sfence` covers
/// the exceptions TSO does not, non-temporal stores and write-combining memory, either of which a
/// DMA buffer can be. See notes/x86-port/what-does-not-fit.md.
pub fn direct_memory_access_write_barrier() {
    instructions::sfence();
}

/// Make the instruction fetcher aware of code just written as data.
///
/// Nothing to do: x86's instruction cache is architecturally coherent with the data caches, so a
/// fetch sees an earlier store, on this core and on every other. A serializing instruction would
/// still be needed if the store and the fetch were separated by a jump the CPU may already have
/// speculated past; no caller here does that. See notes/x86-port/what-does-not-fit.md.
pub fn sync_icache(va: u64, len: usize) {
    let _ = (va, len);
}
