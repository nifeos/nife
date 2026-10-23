//! The per-CPU interrupt stack: where a trap taken on kernel code runs its handler.
//!
//! One stack per core, over an unmapped guard page, in the `.interrupt_stacks` region. A trap from
//! kernel mode saves its frame on the interrupted stack and runs the handler here, so a thread's
//! stack no longer pays for a whole interrupt. A trap from user mode does not switch. See
//! notes/stack/interrupt-stack.md.
//!
//! Nothing that runs on an interrupt stack may context-switch away from it: the thread would resume
//! on a core's stack that the next interrupt has overwritten. `sched::schedule` debug-asserts it
//! and `script/stack-depth-check` proves no switch is reachable from the interrupt-stack entry.

//! # BUGS
//!
//! - A trap from user mode does not switch, so a deep syscall is still charged to the thread.
//! - The trap frame itself is still built on the interrupted stack, which must keep it.
//! - An overflow whose first fault is the vector's own frame store still cascades.
//! - The stacks cost `MAX_CPUS` slots whether or not the cores exist.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::cpu::MAX_CPUS;

/// The usable stack in one core's slot: four pages.
///
/// `script/stack-depth-check` gates it against the deepest chain reachable from the dispatcher,
/// about 4 KiB on both ISAs. The rest is for the fatal path's register dump and stack scan.
pub(crate) const SIZE: usize = 4 * 4096;

/// The unmapped page below each interrupt stack. `map_everything` skips it per core, and `verify`
/// refuses to install a map in which it translates.
const GUARD: usize = 4096;

/// A core's slot: its guard page, then the stack that grows down into it.
const SLOT: usize = GUARD + SIZE;

/// The stacks themselves.
///
/// The `UnsafeCell` keeps them out of `.rodata`, which the fine kernel map makes read-only. The
/// `link_section` gives the mapper a region of its own to punch the guard pages in; in `.bss` it
/// would be mapped as one range.
#[repr(C, align(4096))]
struct Stacks(UnsafeCell<[[u8; SLOT]; MAX_CPUS]>);

// SAFETY: each core writes only its own slot, and only as its stack through `sp`. No two cores ever
// touch the same bytes; the cell exists to place the stacks in writable memory, not to share them.
unsafe impl Sync for Stacks {}

#[cfg_attr(target_os = "none", unsafe(link_section = ".interrupt_stacks"))]
static STACKS: Stacks = Stacks(UnsafeCell::new([[0; SLOT]; MAX_CPUS]));

/// Whether the switch is live.
///
/// False until [`init`] runs after the fine kernel map is installed; before that the guard pages
/// are not holes, and a trap stays on whatever stack it interrupted.
static ARMED: AtomicBool = AtomicBool::new(false);

/// The low end of core `id`'s slot, which is its guard page.
///
/// An array index rather than a multiply, so `id` is bounds-checked against `MAX_CPUS`; a
/// multiply's overflow check would pass an `id` of `MAX_CPUS` (notes/overflow-checks.md).
pub(crate) fn guard(id: usize) -> u64 {
    // SAFETY: `STACKS.0.get()` points at the live static. `&raw const` of an element place projects
    // the address (bounds-checked) and neither reads the slot nor makes a reference to memory a
    // core may be running on as its stack.
    (unsafe { &raw const (*STACKS.0.get())[id] }) as u64
}

/// Core `id`'s usable stack as `(bottom, top)`: the slot above its guard page.
pub(crate) fn span(id: usize) -> (u64, u64) {
    // Wrapping, because it cannot wrap: `guard` refused any `id` past the array, so both sums stay
    // inside slot `id` or one past its end.
    let bottom = guard(id).wrapping_add(GUARD as u64);
    (bottom, bottom.wrapping_add(SIZE as u64))
}

/// The whole region, `(start, end)`, for the check against what the linker reserved.
#[cfg(test)]
pub fn region() -> (u64, u64) {
    let start = STACKS.0.get() as u64;
    (start, start + (MAX_CPUS * SLOT) as u64)
}

/// Arm the switch, once, on the boot core, after the fine kernel map is installed.
///
/// A test build also paints every slot for the high-water report. Every slot is unused here: no
/// secondary is up and no trap has run with the switch armed.
pub fn init() {
    #[cfg(any(test, feature = "system_tests"))]
    {
        for id in 0..MAX_CPUS {
            let (bottom, top) = span(id);
            // SAFETY: the slot is mapped (`map_everything` mapped every slot above its guard
            // page), page-aligned, and unused: nothing switches to an interrupt stack until the
            // store below.
            unsafe { crate::stack::paint(bottom, top) };
        }
    }
    // Release: the paint above, and the mapping the caller installed, must be visible to any core
    // that sees this flag set.
    ARMED.store(true, Ordering::Release);
}

/// Choose the stack a trap runs its handler on.
///
/// `from_user` says whether the trap came from user mode. Returns this core's interrupt-stack top,
/// or 0 to stay: before [`init`], for a trap from user mode, and when already on an interrupt
/// stack, where resetting `sp` would drop a nested fault through the frames of the handler it hit.
/// `#[inline(always)]` because a debug build, which the icount tripwire measures, would otherwise
/// add a call to every syscall. See notes/stack/interrupt-stack.md.
///
/// Context: each architecture's trap dispatcher, on every trap, interrupts included; takes no
/// lock.
#[inline(always)]
pub(crate) fn top_for_trap(from_user: bool) -> u64 {
    if from_user || !ARMED.load(Ordering::Acquire) {
        return 0;
    }
    if contains(crate::arch::current_sp()) {
        return 0; // nesting: this core is already running on it
    }
    span(crate::cpu::id()).1
}

/// Is `addr` anywhere in the interrupt-stack region?
///
/// `sched::schedule` asks it of its own `sp` on every switch. One subtraction and one compare over
/// the whole contiguous region, because a per-slot loop cost a debug build's `ctx_switch` 49.7%
/// (notes/stack/interrupt-stack.md). An `sp` in a guard page would already have faulted.
#[inline(always)]
pub(crate) fn contains(addr: u64) -> bool {
    addr.wrapping_sub(STACKS.0.get() as u64) < (MAX_CPUS * SLOT) as u64
}

/// Which core's interrupt-stack guard page `addr` falls in, if any, for `stack::guard_page_at`.
pub(crate) fn guard_page_at(addr: u64) -> Option<usize> {
    (0..MAX_CPUS).find(|&id| {
        let g = guard(id);
        (g..g + GUARD as u64).contains(&addr)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The linker reserved at least what this module lays out, starting where it starts.
    ///
    /// The layout is here and `.interrupt_stacks` is in two linker scripts. A region too small
    /// would put the last core's stack over whatever the linker put next, with no fault.
    #[test_case]
    fn the_interrupt_stack_region_is_the_size_the_linker_reserved() {
        unsafe extern "C" {
            static __interrupt_stacks_start: core::ffi::c_void;
            static __interrupt_stacks_end: core::ffi::c_void;
        }
        let start = (&raw const __interrupt_stacks_start) as u64;
        let end = (&raw const __interrupt_stacks_end) as u64;
        let (ours_start, ours_end) = region();
        assert_eq!(
            start, ours_start,
            "the region does not start where the linker put it"
        );
        assert!(
            ours_end <= end,
            "the interrupt stacks ({} bytes) do not fit the {} bytes the linker reserved",
            ours_end - ours_start,
            end - start,
        );
    }

    /// A slot's guard page begins exactly one byte past the slot below it.
    ///
    /// Reading a fault at a guard base depends on it (notes/stack/reading-a-guard-page-report.md).
    #[test_case]
    fn a_slots_guard_page_begins_where_the_slot_below_it_ends() {
        for id in 1..MAX_CPUS {
            let (_, below_top) = span(id - 1);
            assert_eq!(
                guard(id),
                below_top,
                "slot {id}'s guard does not abut slot {} 's top",
                id - 1
            );
        }
    }

    /// Some core's interrupt stack has been used, so traps do run on it.
    ///
    /// A nonzero high-water on a painted slot proves the switch fires. Any core rather than this
    /// one, because tests are not pinned and a core that ran only user threads never switches. Not
    /// a depth threshold: `stack::report_high_water` and `script/stack-depth-check` own that.
    #[test_case]
    fn interrupts_are_handled_on_the_interrupt_stack() {
        let used = (0..MAX_CPUS).any(|id| {
            let (bottom, top) = span(id);
            // SAFETY: a slot of the region `init` mapped and painted, read one word at a time.
            unsafe { crate::stack::high_water(bottom, top) > 0 }
        });
        assert!(
            used,
            "no core's interrupt stack has been touched: no trap has run on one, so either the \
             switch never armed or the dispatcher is not asking for it",
        );
    }
}
