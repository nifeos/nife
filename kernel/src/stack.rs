//! Kernel stack overflow detection and measurement.
//!
//! A canary under the boot stack, the guard-page lookup a fatal fault prints, and the paint that
//! measures how deep each kernel stack has been. See notes/stack.md,
//! notes/stack/reading-a-guard-page-report.md and notes/stack-high-water.md.

use core::ffi::c_void;

/// The canary, written at the very bottom of the boot stack, where nothing legitimate writes.
///
/// Four words, because a big frame moves `sp` past the bottom and then writes throughout the
/// frame, so a wider target is likelier to be hit. Not zero (fresh `.bss` is zeroes) and not a
/// plausible pointer or small integer, so a stray write is unlikely to reproduce it.
const CANARY: [u64; 4] = [
    0x57ac_c0de_57ac_c0de,
    0xc0ff_ee00_1eaf_babe,
    0xdead_c0de_5111_c0de,
    0xfeed_face_cafe_f00d,
];

/// Paint the canary. Call this before anything can use much stack.
pub fn init() {
    // SAFETY: `__stack_bottom` is inside our own image, and by definition nothing has
    // pushed a frame that deep, or we would already be dead.
    unsafe {
        core::ptr::write_volatile(bottom() as *mut [u64; 4], CANARY);
    }
}

/// Has anything scribbled below the stack?
pub(crate) fn is_intact() -> bool {
    // SAFETY: reading our own image.
    unsafe { core::ptr::read_volatile(bottom() as *const [u64; 4]) == CANARY }
}

/// How many bytes are left between `sp` and the bottom of the stack.
///
/// Negative means we are *already* below it and are actively corrupting the kernel.
pub(crate) fn headroom() -> i64 {
    let sp = crate::arch::current_sp();
    sp.wrapping_sub(bottom()) as i64
}

/// Which kernel stack a faulting address belongs to, when it lands in that stack's guard page.
///
/// Each kind is allocated a different way; notes/stack/reading-a-guard-page-report.md has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuardPage {
    /// The boot stack's guard page (`__stack_guard`). Core 0 runs the whole test suite on it.
    Boot,
    /// Secondary core `id`'s guard page.
    Secondary(usize),
    /// A thread kernel stack's guard page: the slot index within `thread::STACK_AREA`, from zero.
    /// Not a `ThreadId`: naming the thread needs the scheduler lock, which a fault handler may not
    /// take.
    Thread(u64),
    /// Core `id`'s interrupt-stack guard page: a trap handler chain ran off the bottom, which
    /// `script/stack-depth-check`'s interrupt-stack bound covers.
    Interrupt(usize),
}

/// Find the kernel stack guard page `addr` lies in, if any.
///
/// Returns the stack's kind, or `None` when `addr` is in no guard page. Takes no lock and touches
/// no allocator, because every caller is a fatal-fault path. See
/// notes/stack/reading-a-guard-page-report.md.
///
/// # BUGS
///
/// A `None` does not mean the stack is fine: a frame larger than the one-page guard can step over
/// it into the neighbouring slot without faulting (notes/load-sensitive-assertions.md).
pub(crate) fn guard_page_at(addr: u64) -> Option<GuardPage> {
    const PAGE: u64 = 4096;

    let boot = crate::arch::mmu::stack_guard();
    if (boot..boot + PAGE).contains(&addr) {
        return Some(GuardPage::Boot);
    }

    for id in 0..crate::cpu::MAX_CPUS {
        let g = crate::smp::secondary_stack_guard(id);
        if (g..g + PAGE).contains(&addr) {
            return Some(GuardPage::Secondary(id));
        }
    }

    if let Some(id) = crate::interrupt_stack::guard_page_at(addr) {
        return Some(GuardPage::Interrupt(id));
    }

    let (area, watermark) = crate::thread::stack_area_span();
    if (area..watermark).contains(&addr) {
        let off = addr - area;
        // A slot is [guard page][STACK_PAGES of stack]: its first page is the guard.
        if off % crate::thread::STACK_SLOT_SPAN < PAGE {
            return Some(GuardPage::Thread(off / crate::thread::STACK_SLOT_SPAN));
        }
    }

    None
}

/// Locate `addr` in the thread-stack area, for saying where a faulting `sp` was.
///
/// Returns `(slot, bytes above that slot's lowest usable byte)`, negative inside the slot's guard
/// page, or `None` outside the area. Unlike [`guard_page_at`] it answers for ordinary live stack
/// too. Reads one relaxed atomic and takes no lock, because every caller is a fatal-fault path.
pub(crate) fn thread_stack_site(addr: u64) -> Option<(u64, i64)> {
    let (area, watermark) = crate::thread::stack_area_span();
    if !(area..watermark).contains(&addr) {
        return None;
    }
    let span = crate::thread::STACK_SLOT_SPAN;
    let off = addr - area;
    let slot = off / span;
    // The guard is the slot's first page, so its lowest usable byte is one page in. Signed: a
    // negative value is in the guard page, and its size says how far one step reached.
    let above = (off % span) as i64 - 4096;
    Some((slot, above))
}

/// Report a fault at `addr` that landed in a kernel stack guard page; do nothing otherwise.
///
/// `addr` is the faulting address and `interrupted_sp` the `sp` saved in the trap frame (the live
/// one is the interrupt stack's). Prints which stack, how far below its bottom the access landed,
/// where `sp` was, and for a thread or interrupt stack a scan of its words. Called from both ISAs'
/// fatal-fault paths. notes/stack/reading-a-guard-page-report.md says how to read it.
pub(crate) fn warn_if_guard_page(addr: u64, interrupted_sp: u64) {
    let Some(kind) = guard_page_at(addr) else {
        return;
    };

    crate::println!();
    crate::println!("  *** KERNEL STACK OVERFLOW ***");
    // The span to scan. The scan itself runs last: it is the only part that can fault, and a
    // fault there must not take the lines below with it.
    let mut scan: Option<(u64, u64)> = None;
    match kind {
        GuardPage::Boot => {
            crate::println!("  {addr:#018x} is in the BOOT stack's guard page.");
            crate::println!(
                "  bottom {:#018x}, so the faulting address is {} bytes below it.",
                crate::arch::mmu::stack_bottom(),
                crate::arch::mmu::stack_bottom().saturating_sub(addr),
            );
        }
        GuardPage::Secondary(id) => {
            let (bottom, _) = crate::smp::secondary_stack_span(id);
            crate::println!("  {addr:#018x} is in core {id}'s boot-stack guard page.");
            crate::println!(
                "  bottom {bottom:#018x}, so the faulting address is {} bytes below it.",
                bottom.saturating_sub(addr),
            );
        }
        GuardPage::Thread(slot) => {
            let (area, _) = crate::thread::stack_area_span();
            let bottom = area + slot * crate::thread::STACK_SLOT_SPAN + 4096;
            crate::println!(
                "  {addr:#018x} is in THREAD stack slot {slot}'s guard page (thread.rs)."
            );
            crate::println!(
                "  bottom {bottom:#018x}, so the faulting address is {} bytes below it, on a \
                 {}-byte stack.",
                bottom.saturating_sub(addr),
                crate::thread::STACK_PAGES * 4096,
            );
            scan = Some((bottom, bottom + (crate::thread::STACK_PAGES * 4096) as u64));
        }
        GuardPage::Interrupt(id) => {
            let (bottom, top) = crate::interrupt_stack::span(id);
            crate::println!("  {addr:#018x} is in core {id}'s INTERRUPT stack guard page.");
            crate::println!(
                "  bottom {bottom:#018x}, so the faulting address is {} bytes below it, on a \
                 {}-byte stack.",
                bottom.saturating_sub(addr),
                crate::interrupt_stack::SIZE,
            );
            scan = Some((bottom, top));
        }
    }

    // Everything above is about the faulting address; this is where `sp` was.
    let sp = interrupted_sp;
    match thread_stack_site(sp) {
        Some((slot, above)) if above >= 0 => crate::println!(
            "  trapped sp {sp:#018x} is in THREAD stack slot {slot}, {above} bytes above its \
             bottom."
        ),
        Some((slot, above)) => crate::println!(
            "  trapped sp {sp:#018x} is in THREAD stack slot {slot}'s GUARD PAGE, {} bytes below \
             its bottom.",
            -above
        ),
        None => crate::println!(
            "  trapped sp {sp:#018x} is not in the thread-stack area (a boot, secondary or \
             interrupt stack)."
        ),
    }
    crate::println!(
        "  Compare the two lines. Same slot: that stack overflowed. Slot N-1: EITHER a"
    );
    crate::println!("  store just past THAT stack's top (slot N's guard page begins where slot");
    crate::println!("  N-1's stack ends), OR the vector faulted building its own frame and walked");
    crate::println!("  sp down until it fit, which lands there too. Check whether the reported");
    crate::println!("  fault PC is inside the vector table to tell those apart. Any other slot,");
    crate::println!("  or none: a stray pointer, not a stack at all.");
    crate::println!("  The guard page is ONE page. A frame larger than that can step over it");
    crate::println!("  into the slot below without faulting; see notes/stack.md.");

    // Last, because it is the only part of this report that can itself fault.
    if let Some((bottom, top)) = scan {
        print_text_words(bottom, top);
    }
}

/// Print every word in `[bottom, top)` that points into `.text`, deepest first, up to 40.
///
/// A conservative backtrace, because this kernel keeps no frame pointers: read the hits as
/// candidates for `llvm-addr2line`. Asks the page tables before each page and reports unmapped
/// bytes instead of reading them. See notes/stack/reading-a-guard-page-report.md.
///
/// # BUGS
///
/// - Thread and interrupt stacks only; the boot and secondary stacks get no scan.
fn print_text_words(bottom: u64, top: u64) {
    const CAP: usize = 40;
    let text = crate::arch::mmu::text_start()..crate::arch::mmu::text_end();
    crate::println!(
        "  Words on the dead stack that point into .text ({:#x}..{:#x}), deepest first,",
        text.start,
        text.end
    );
    crate::println!("  as `bottom+offset: word` (candidate return addresses; no frame pointers):");
    let mut printed = 0usize;
    let mut p = bottom;
    let mut unmapped = 0u64;
    while p < top && printed < CAP {
        // A dropped `KernelStack` leaves its whole slot unmapped.
        if p.is_multiple_of(4096) && !crate::arch::mmu::is_mapped(p) {
            unmapped += 4096;
            p += 4096;
            continue;
        }
        // SAFETY: the page holding `p` is mapped (the check above is exactly that, and this
        // address is in a kernel stack slot, whose mapping only the reaper changes), 8-byte
        // aligned; volatile because the stack's owner is dead mid-store and nothing about this
        // memory is ordinary.
        let w = unsafe { core::ptr::read_volatile(p as *const u64) };
        if text.contains(&w) {
            crate::println!("    +{:#07x}: {w:#018x}", p - bottom);
            printed += 1;
        }
        p += 8;
    }
    if unmapped > 0 {
        crate::println!(
            "    !! {unmapped} of {} bytes of this stack are NOT MAPPED. The stack was freed",
            top - bottom
        );
        crate::println!(
            "    while something was still standing on it: a store from the dead owner walks the"
        );
        crate::println!(
            "    vector down to this slot's base, which is exactly the address above. See"
        );
        crate::println!("    notes/stack/kernel-stack-freed-under-its-owner.md.");
    }
    if printed == CAP {
        crate::println!("    ... capped at {CAP} words; the shallower stack is not shown.");
    }
}

/// Report a dead canary. Called from the panic and fault handlers, because a corrupted stack
/// makes every other diagnostic suspect.
pub(crate) fn warn_if_smashed() {
    if !is_intact() {
        crate::println!();
        crate::println!("  *** STACK OVERFLOW ***");
        crate::println!("  The canary below __stack_bottom is dead, so we have written");
        crate::println!("  through our own .bss/.data/.text. Nothing printed above this");
        crate::println!("  line can be trusted. See notes/stack.md.");
        crate::println!("  headroom: {} bytes", headroom());
    }
}

fn bottom() -> u64 {
    unsafe extern "C" {
        static __stack_bottom: c_void;
    }
    (&raw const __stack_bottom) as u64
}

#[cfg(any(test, feature = "system_tests"))]
fn top() -> u64 {
    unsafe extern "C" {
        static __stack_top: c_void;
    }
    (&raw const __stack_top) as u64
}

// --- Stack high-water measurement ---
//
// Paint a stack before use, then scan for the deepest word overwritten. `paint` and `high_water`
// are built everywhere (`ipc_stack_depth` and `progenitor_stack` use them); the kernel-stack
// report is test builds only, so painting adds nothing to a spawn outside a test. A watermark
// sees only exercised paths. See notes/stack-high-water.md.

/// The paint word. Not zero (fresh `.bss` is zeroes, and a stack full of zeroes would read as
/// untouched), not a plausible pointer or length, and not one of the canary words.
const PAINT: u64 = 0x5AFE_57AC_5AFE_57AC;

/// Paint `[bottom, top)` with [`PAINT`].
///
/// # Safety
/// `[bottom, top)` must be a mapped, writable, 8-byte-aligned range that **nothing has used yet**.
/// This writes every word in it, so a live frame anywhere inside is corrupted: a call site painting
/// a stack that is in use has to skip the live portion itself (see `paint_boot_stack`, which stops
/// a margin below the running `sp`).
pub(crate) unsafe fn paint(bottom: u64, top: u64) {
    let mut p = bottom as *mut u64;
    while (p as u64) < top {
        // SAFETY: this function's own `# Safety` contract puts `[bottom, top)` in a mapped, unused,
        // aligned region, and the loop stays inside it. Volatile so the writes are not elided or
        // coalesced into something that assumes the region is ordinary memory.
        unsafe { core::ptr::write_volatile(p, PAINT) };
        p = p.wrapping_add(1);
    }
}

/// Measure the most bytes a painted stack `[bottom, top)` has ever used.
///
/// Scans up from `bottom` for the first word that is not [`PAINT`] and returns its distance from
/// `top`: the deepest point ever reached, not the depth now. Iterative, so it needs little stack of
/// its own. A deepest word that stored the paint value reads one word shallow.
///
/// # Safety
/// `[bottom, top)` must be a mapped, readable, 8-byte-aligned range, and it must be a range
/// [`paint`] ran over, or the answer is a reading of somebody else's bytes rather than a
/// measurement.
pub(crate) unsafe fn high_water(bottom: u64, top: u64) -> u64 {
    let mut p = bottom as *const u64;
    while (p as u64) < top {
        // SAFETY: this function's own `# Safety` contract puts `[bottom, top)` in a mapped, aligned
        // region, and the loop stays inside it. Volatile because another core may own this stack and
        // be running on it right now; we only ever compare a snapshot word against the pattern, and
        // a racing write can only make the stack look deeper, never shallower than it truly was.
        if unsafe { core::ptr::read_volatile(p) } != PAINT {
            return top - p as u64;
        }
        p = p.wrapping_add(1);
    }
    0
}

/// Where the boot-stack paint stopped, below the paint-time `sp`. A high-water equal to this floor
/// means nothing after the paint went deeper than it, not that this is the true maximum.
#[cfg(any(test, feature = "system_tests"))]
static BOOT_PAINT_CEILING: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Paint the boot stack from above the canary to 512 bytes below the current `sp`.
///
/// Called from `kernel_main` right after [`init`], so the live frames are skipped. Depth used
/// before this runs and never reached again is invisible; the report prints the floor. The 512-byte
/// margin covers the paint loop's own callees, which are real calls in a debug build.
#[cfg(any(test, feature = "system_tests"))]
pub(crate) fn paint_boot_stack() {
    let ceiling = crate::arch::current_sp() - 512;
    BOOT_PAINT_CEILING.store(ceiling, core::sync::atomic::Ordering::Relaxed);
    // Start above the canary words: painting them would kill the canary check.
    //
    // SAFETY: the boot stack is mapped by the linker script and is live from `_start`, and `ceiling`
    // is 512 bytes below the running `sp`, so the range is the unused part. The 512 is the margin
    // for the paint loop's own callees, which push frames below our `sp` while it runs.
    unsafe { paint(bottom() + core::mem::size_of_val(&CANARY) as u64, ceiling) };
}

/// Deepest thread-stack use seen so far, in bytes, over every reaped [`crate::thread::KernelStack`]
/// (scanned in its `Drop`) and, at report time, every live one. One number, because every kernel
/// thread stack is the same size.
#[cfg(any(test, feature = "system_tests"))]
static THREAD_STACK_MAX: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// How many thread stacks fed [`THREAD_STACK_MAX`], so the report says how much evidence the
/// number rests on.
#[cfg(any(test, feature = "system_tests"))]
static THREAD_STACK_SCANS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Record one thread stack's measured use (called from `KernelStack`'s `Drop`, and from the live
/// scan at report time).
#[cfg(any(test, feature = "system_tests"))]
pub(crate) fn note_thread_stack_use(used: u64) {
    use core::sync::atomic::Ordering;
    THREAD_STACK_MAX.fetch_max(used, Ordering::Relaxed);
    THREAD_STACK_SCANS.fetch_add(1, Ordering::Relaxed);
}

/// Print every kernel stack's high-water mark and fail on any over its limit.
///
/// Run by the test runner after the last test. Depth depends on what ran, not on host load, apart
/// from where a trap frame lands. Limits and margins: notes/stack-high-water.md, "The gate".
#[cfg(any(test, feature = "system_tests"))]
pub(crate) fn report_high_water() {
    use core::sync::atomic::Ordering;

    let boot_bottom = bottom() + core::mem::size_of_val(&CANARY) as u64;
    let size = top() - boot_bottom;
    // SAFETY: the same range `paint_boot_stack` painted, on a stack the linker script mapped.
    let used = unsafe { high_water(boot_bottom, top()) };
    let floor = top() - BOOT_PAINT_CEILING.load(Ordering::Relaxed);
    crate::println!(
        "stack high-water: boot   {used}/{size} bytes ({}%), paint floor {floor}",
        used * 100 / size,
    );

    let boot_cpu = crate::arch::boot_cpu_id();
    let mut max_secondary = 0u64;
    for id in 0..crate::cpu::MAX_CPUS {
        if id == boot_cpu || crate::smp::online_harts_mask() & (1 << id) == 0 {
            continue; // the boot core runs on the linker-script stack; its slot was never painted
        }
        let (b, t) = crate::smp::secondary_stack_span(id);
        // SAFETY: the span of an online core's `.bss` stack slot, which `bring_up_secondaries`
        // painted whole before any `CPU_ON`. The `continue` above skipped every slot that was not.
        let cpu_used = unsafe { high_water(b, t) };
        max_secondary = max_secondary.max(cpu_used);
        crate::println!(
            "stack high-water: core{id}  {cpu_used}/{} bytes ({}%)",
            t - b,
            cpu_used * 100 / (t - b),
        );
    }

    // Every seat: `interrupt_stack::init` painted each slot, so an unused one reads 0.
    let mut max_interrupt = 0u64;
    for id in 0..crate::cpu::MAX_CPUS {
        let (b, t) = crate::interrupt_stack::span(id);
        // SAFETY: a slot of the region `interrupt_stack::init` mapped and painted before any core
        // could switch to one.
        let used = unsafe { high_water(b, t) };
        max_interrupt = max_interrupt.max(used);
        if used > 0 {
            crate::println!(
                "stack high-water: irq{id}   {used}/{} bytes ({}%)",
                t - b,
                used * 100 / (t - b),
            );
        }
    }

    // Live thread stacks too, so threads nothing ever reaps are counted.
    crate::sched::scan_live_thread_stacks();
    let tmax = THREAD_STACK_MAX.load(Ordering::Relaxed);
    let tsize = (crate::thread::STACK_PAGES * 4096) as u64;
    crate::println!(
        "stack high-water: thread {tmax}/{tsize} bytes ({}%, deepest of {} stacks)",
        tmax * 100 / tsize,
        THREAD_STACK_SCANS.load(Ordering::Relaxed),
    );

    // Checked after the printing, so a trip always comes with its numbers. The boot, secondary and
    // thread limits and their margins are in notes/stack-high-water.md, "The gate".
    assert!(
        used <= 61440,
        "boot stack high-water {used} exceeded 61440: the suite's deepest path has grown ~7 KiB \
         past its measured depth and is within a page of the guard (notes/stack-high-water.md)",
    );
    assert!(
        max_secondary <= 16384,
        "a secondary stack's high-water {max_secondary} exceeded 16384, ~2x anything measured; \
         the guard page below it (smp.rs) would still catch a real overflow, but something is \
         running much deeper on an idle-and-traps stack than the suite has ever measured",
    );
    // Half the 16 KiB slot, above the ~4 KiB chain `script/stack-depth-check` finds and far below
    // the guard. A trip means a handler chain grew, not that the stack is too small.
    assert!(
        max_interrupt <= 8192,
        "an interrupt stack's high-water {max_interrupt} exceeded 8192, half its slot: a trap \
         handler's chain has grown past anything measured (notes/stack/interrupt-stack.md, \
         script/stack-depth-check)",
    );
    assert!(
        tmax <= 18432,
        "thread stack high-water {tmax} exceeded 18432: some kernel thread is within 6 KiB of its \
         guard page, ~3 KiB past the measured worst-case stacking (notes/stack-high-water.md)",
    );
}

#[cfg(test)]
mod tests {
    //! Tests for stack overflow detection.

    /// The canary is intact and there is more than 4 KiB of stack left.
    ///
    /// The runner also checks the canary after every test (testing.rs), so a test that overflows
    /// is caught by name.
    #[test_case]
    fn stack_canary_is_intact_and_we_have_headroom() {
        assert!(crate::stack::is_intact(), "stack canary is already dead");
        assert!(
            crate::stack::headroom() > 4096,
            "less than 4 KiB of stack left: {}",
            crate::stack::headroom()
        );
    }
}
