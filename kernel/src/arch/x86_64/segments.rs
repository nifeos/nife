//! The GDT and the TSS, which have no counterpart on the other two architectures.
//!
//! In 64-bit mode a segment's base and limit are ignored except for `fs` and `gs`, but the code
//! segment's `L`, `DPL` and `D/B` bits still decide the CPU's mode, so the GDT stays. The TSS gives
//! the stack on a ring change (`RSP0`), an unconditional stack per IST vector for a fault whose
//! cause is an unusable stack, and the I/O permission bitmap (§121 (x86 port I/O)).
//!
//! `syscall`/`sysret` derive four selectors from `IA32_STAR` by arithmetic (SS is CS + 8 entering;
//! user SS and CS are the base + 8 and + 16), so the order kernel code, kernel data, user data,
//! user code is forced, and reordering it faults `sysret`. See notes/x86-port/traps.md.

use core::arch::asm;
use core::cell::SyncUnsafeCell;

/// Selector for the kernel code segment. Also `IA32_STAR[47:32]`, from which `syscall` derives the
/// kernel SS as this + 8.
pub(crate) const KERNEL_CODE: u16 = 0x08;
/// Selector for the kernel data segment (ss/ds/es while in ring 0).
pub(crate) const KERNEL_DATA: u16 = 0x10;
/// Selector for the user data segment. `sysret` computes this as `IA32_STAR[63:48] + 8`.
#[allow(dead_code)] // Referenced once user mode exists; see the module header for why it is here now.
pub(crate) const USER_DATA: u16 = 0x18 | 3;
/// Selector for the user code segment. `sysret` computes this as `IA32_STAR[63:48] + 16`.
#[allow(dead_code)]
pub(crate) const USER_CODE: u16 = 0x20 | 3;
/// Selector for the TSS descriptor, which occupies **two** GDT slots (0x28 and 0x30) because a
/// 64-bit system descriptor is 16 bytes rather than 8.
const TSS_SELECTOR: u16 = 0x28;

/// The x86 I/O port space is 16 bits (`in`/`out` address exactly 64 Ki ports), one bit of "may this
/// ring-3 thread touch this port" each, so a full permission bitmap is `65536 / 8 == 8192` bytes.
const IOMAP_BYTES: usize = 65536 / 8;

/// One trailing guard byte, set to all ones, because the CPU may read **two** bytes of the bitmap
/// when it checks the highest port and a bitmap that ended exactly at the limit would read one byte
/// past it. Linux appends the same byte for the same reason. The `Tss` descriptor's limit covers it.
const IOMAP_GUARD: usize = 1;

/// Offset, in bytes from the base of a [`Tss`], to its I/O permission bitmap. This is the value
/// [`Tss::iomap_base`] holds while a thread that owns ports is running: everything up to it is the
/// fixed 104-byte TSS, and the bitmap follows.
const IOMAP_OFFSET: u16 = 104;

/// What [`Tss::iomap_base`] holds while **no** thread with a port capability is running: a value
/// past the descriptor's limit, which the CPU reads as "there is no bitmap", so every `in`/`out`
/// from ring 3 faults. This is the lazy form DECISIONS §121 named (2026-08-25, binding here): a
/// thread that holds no port capability switches with `iomap_base` pointing past the segment limit,
/// and the bitmap is written into the TSS only when a holder is on a side of the switch. `0xFFFF`
/// is past the limit for any TSS this kernel builds (the descriptor limit is `size_of::<Tss>() - 1`,
/// far below 64 Ki).
const IOMAP_BASE_DENY_ALL: u16 = 0xFFFF;

/// A 64-bit Task State Segment, with its I/O permission bitmap.
///
/// Long mode keeps three ring stacks, seven interrupt stacks, the bitmap offset and the bitmap.
/// `#[repr(C, packed)]` because the layout is the CPU's and the reserved words are padding it
/// requires. The bitmap is inside the TSS (milestone 299 (the x86 port-range capability)) because
/// `iomap_base` is an offset from the TSS base; one per core, so no two cores share port
/// permissions.
#[repr(C, packed)]
struct Tss {
    _reserved0: u32,
    /// The stack pointer the CPU loads on a trap that raises privilege to ring 0. This is the field
    /// that makes user mode survivable.
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    _reserved1: u64,
    /// Seven unconditional stacks, selectable per IDT entry. The first three are this core's
    /// stacks for the double fault, the NMI and the machine check, in [`super::ist::VECTORS`]'s
    /// order, written by [`super::ist::install`]; the other four stay 0 and no gate selects them.
    ist: [u64; 7],
    _reserved2: u64,
    _reserved3: u16,
    /// Offset from the base of this TSS to the I/O permission bitmap ([`IOMAP_OFFSET`]) while a
    /// thread that owns ports runs, or [`IOMAP_BASE_DENY_ALL`] (past the limit, "no bitmap") while
    /// none does. [`set_port_range_grant`] is the only writer after boot; the initial value denies all,
    /// the only correct answer while no program has been granted a port. See DECISIONS §121.
    iomap_base: u16,
    /// The permission bitmap plus its guard byte. A **set** bit denies the port to ring 3; a
    /// **clear** bit permits it. The invariant [`set_port_range_grant`] maintains is that this is entirely
    /// ones (all denied) whenever no port grant is installed on this core, so installing one is a
    /// matter of clearing the grant's own bits and uninstalling is setting them back.
    iomap: [u8; IOMAP_BYTES + IOMAP_GUARD],
}

impl Tss {
    const fn new() -> Self {
        Self {
            _reserved0: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            _reserved1: 0,
            ist: [0; 7],
            _reserved2: 0,
            _reserved3: 0,
            // No holder at boot: no bitmap is consulted. The bitmap below is still all-ones so the
            // "all denied when nothing is installed" invariant holds the instant a holder installs.
            iomap_base: IOMAP_BASE_DENY_ALL,
            iomap: [0xFF; IOMAP_BYTES + IOMAP_GUARD],
        }
    }
}

const _: () = assert!(size_of::<Tss>() == 104 + IOMAP_BYTES + IOMAP_GUARD);
// `IOMAP_OFFSET` is the byte offset of the bitmap; the fixed part of a 64-bit TSS is 104 bytes.
const _: () = assert!(core::mem::offset_of!(Tss, iomap) == IOMAP_OFFSET as usize);

/// One TSS per core, indexed by `cpu::id()`: `rsp0` names a per-core stack, and a shared TSS would
/// let two cores race on one ring-0 stack pointer.
///
/// A `SyncUnsafeCell`, by calef's ruling of 2026-10-09 on milestone 891 (a mutable static becomes
/// an atomic, a lock or a recorded cell): the CPU reads it by address, through the descriptor `ltr`
/// loads. Each core touches only its own slot, and the CPU reads that slot only on that core.
static TSS: SyncUnsafeCell<[Tss; crate::cpu::MAX_CPUS]> =
    SyncUnsafeCell::new([const { Tss::new() }; crate::cpu::MAX_CPUS]);

/// The GDT: seven 8-byte slots, the last two of which are one 16-byte TSS descriptor. **One per
/// core**, for the same reason as [`TSS`]: the TSS descriptor's base address is this core's own
/// `TSS[cpu::id()]`, so sharing one GDT would mean every core's `ltr` loaded the SAME task
/// register, aliasing every core's ring-0 stack onto whichever one initialized last.
///
/// A `SyncUnsafeCell` for the reason [`TSS`] is one (milestone 891): `lgdt` takes its address. Each
/// core writes only its own row, in [`init`], before its own `lgdt`, and no core reads another's.
static GDT: SyncUnsafeCell<[[u64; 7]; crate::cpu::MAX_CPUS]> =
    SyncUnsafeCell::new([BASE_GDT; crate::cpu::MAX_CPUS]);

/// Every core's GDT starts identical: only the TSS descriptor (slots 5 and 6) differs per core, and
/// [`init`] fills those in from this core's own [`TSS`] entry.
const BASE_GDT: [u64; 7] = [
    0,                     // 0x00: the mandatory null descriptor
    0x00AF_9A00_0000_FFFF, // 0x08: kernel code, DPL 0, L=1
    0x00CF_9200_0000_FFFF, // 0x10: kernel data, DPL 0
    0x00CF_F200_0000_FFFF, // 0x18: user data, DPL 3
    0x00AF_FA00_0000_FFFF, // 0x20: user code, DPL 3, L=1
    0,                     // 0x28: TSS descriptor, low half, filled in by `init`
    0,                     // 0x30: TSS descriptor, high half
];

/// The operand `lgdt` and `lidt` take: a 16-bit limit (size minus one) and a 64-bit base.
/// `#[repr(C, packed)]` because the CPU reads exactly ten bytes and a Rust-chosen alignment hole
/// between the two fields would make it read the base from the wrong offset.
#[repr(C, packed)]
pub(crate) struct DescriptorTablePointer {
    pub limit: u16,
    pub base: u64,
}

/// Install the GDT and the TSS on this CPU, and reload every segment register from it.
///
/// `lgdt` leaves loaded selectors alone and CS changes only by a control transfer, so CS is
/// reloaded by a far return (`retfq`); a 64-bit far jump would need a memory operand.
///
/// # Safety
/// Must be called once per CPU, with a valid stack and `cpu::init_this_cpu` already run on it
/// (this fills `TSS`/`GDT` at `cpu::id()`'s own slot, and restores this core's per-CPU pointer
/// afterward; see below), before anything depends on the GDT: it replaces the boot GDT that
/// `boot.s` installed, and the old one goes away.
pub unsafe fn init() {
    let id = crate::cpu::id();

    // The TSS descriptor holds the TSS's address, known only once loaded, across three fields.
    // SAFETY: indexing this core's own slot with `id` in bounds (`cpu::id()` cannot exceed
    // `MAX_CPUS`, which is exactly how big `TSS` is); reading only the address, not the (possibly
    // concurrently-written-by-another-core-at-a-different-index) contents.
    let base = unsafe { (&raw const (*TSS.get())[id]) as u64 };
    let limit = (size_of::<Tss>() - 1) as u64;

    // trap.s writes this core's own `TSS[id].rsp0` through this per-CPU pointer, set before the
    // IDT exists (`cpu::X86TrapPerCpu::tss_rsp0_ptr`).
    // SAFETY: writes this core's own `PerCpu` slot, reached the same way every per-CPU write on
    // this architecture is, before any trap can be taken on this core.
    unsafe {
        *crate::cpu::current().x86_trap.tss_rsp0_ptr.get() =
            (&raw const (*TSS.get())[id].rsp0) as u64;
    }
    let low = (limit & 0xffff)
        | ((base & 0x00ff_ffff) << 16)
        | (0x89 << 40)                    // present, type 9 = available 64-bit TSS
        | (((limit >> 16) & 0xf) << 48)
        | (((base >> 24) & 0xff) << 56);
    let high = base >> 32;

    // SAFETY: this core writes only its OWN slot (`id`), before anything else on this core reads
    // its GDT; a different core's `init` writes a different slot.
    unsafe {
        (*GDT.get())[id][5] = low;
        (*GDT.get())[id][6] = high;
    }

    // SAFETY: as `base` above, this core's own slot only.
    let gdt_base = unsafe { (&raw const (*GDT.get())[id]) as u64 };
    let gdtr = DescriptorTablePointer {
        limit: (size_of::<[u64; 7]>() - 1) as u16,
        base: gdt_base,
    };

    // Loading a segment register in long mode zeroes that segment's base MSR, and `gs`'s base is
    // the per-CPU pointer `cpu::init_this_cpu` already set (the console lock reads it). So it is
    // saved and put back around the reload, here rather than by the caller. `fs` would need the
    // same if anything used its base. See notes/x86-port/traps.md.
    let gs_base = super::percpu();

    // SAFETY: `gdtr` describes the table above, which is well-formed by construction. The far
    // return reloads CS from a descriptor whose L bit is set, so the CPU stays in 64-bit mode; the
    // data selectors are reloaded before anything can use a stale one.
    unsafe {
        asm!(
            "lgdt [{gdtr}]",
            // Push the new CS and the address to continue at, then far-return into it.
            "push {code:r}",
            "lea {tmp}, [rip + 2f]",
            "push {tmp}",
            "retfq",
            "2:",
            "mov ds, {data:e}",
            "mov es, {data:e}",
            "mov ss, {data:e}",
            "mov fs, {data:e}",
            "mov gs, {data:e}",
            "ltr {tss:x}",
            gdtr = in(reg) &gdtr,
            code = in(reg) KERNEL_CODE as u64,
            data = in(reg) KERNEL_DATA as u32,
            tss = in(reg) TSS_SELECTOR,
            // `out`, never `lateout`: `lea` writes `tmp` while `data` and `tss` are unread, and
            // sharing a register made `ltr` load a code address, `#GP` then a double fault. Only a
            // release boot shows it (`cargo xtask uefi-boot --release`).
            tmp = out(reg) _,
            options(preserves_flags),
        );
    }

    // Put the per-CPU pointer back; see the comment above the `asm!`.
    super::set_percpu(gs_base);

    // This core's NMI, double-fault and machine-check stacks, before `exceptions::init` loads the
    // IDT whose gates select them.
    // SAFETY: once per core, on this core, before its IDT exists, with this core's own block.
    unsafe { super::ist::install(id, gs_base as u64) };
}

/// Point an IST slot (1-based, as the IDT encodes it) at `top`, on this core.
///
/// # Safety
/// `top` must be the top of a stack no other IST vector shares. Two vectors on one stack means a
/// fault taken while handling the other overwrites its frame, which is precisely the situation IST
/// exists to survive.
pub(crate) unsafe fn set_interrupt_stack(slot: u8, top: u64) {
    assert!((1..=7).contains(&slot), "IST slots are 1..=7, not {slot}");
    let id = crate::cpu::id();
    // SAFETY: writes this CPU's own TSS `ist[slot - 1]`, in bounds by the assertion above. Through
    // a raw pointer because the TSS is `packed` and a reference to a field would be misaligned.
    unsafe {
        (&raw mut (*TSS.get())[id].ist)
            .cast::<u64>()
            .add(slot as usize - 1)
            .write_unaligned(top);
    }
}

/// Core `id`'s IST entry `slot` (one-based, as the IDT encodes it): the `rsp` the CPU loads for a
/// gate that selects it. Read by the tests that check [`super::ist::install`].
#[cfg(test)]
pub(crate) fn interrupt_stack(id: usize, slot: u8) -> u64 {
    assert!((1..=7).contains(&slot), "IST slots are 1..=7, not {slot}");
    // SAFETY: reads core `id`'s `ist[slot - 1]`, in bounds by the assertion above and by indexing,
    // unaligned because the TSS is `packed`. Written once at that core's bring-up and not after.
    unsafe {
        (&raw const (*TSS.get())[id].ist)
            .cast::<u64>()
            .add(slot as usize - 1)
            .read_unaligned()
    }
}

/// Which port grant this core's TSS bitmap holds now, or `None` on nearly every core; indexed by
/// `cpu::id()`. [`set_port_range_grant`] compares the incoming thread's grant with it and does
/// nothing on a match, which is what makes the switch cheap.
///
/// A `SyncUnsafeCell` (milestone 891, calef's ruling of 2026-10-09): it changes in the same masked
/// window as the bitmap it records, so a lock would add an acquisition for no second party. Each
/// core reads and writes only its own slot, with interrupts masked ([`set_port_range_grant_on`]).
static INSTALLED_PORT_GRANT: SyncUnsafeCell<[Option<(u16, u16)>; crate::cpu::MAX_CPUS]> =
    SyncUnsafeCell::new([None; crate::cpu::MAX_CPUS]);

/// Set every bit of ports `[base, base + count)` in `iomap` to `deny` (`true` faults the port from
/// ring 3). Port `p` is bit `p % 8` of byte `p / 8`; the range is clamped to the bitmap, so the
/// guard byte is never touched. A raw pointer, because the bitmap is a field of a packed static.
///
/// # Safety
/// `iomap` must point at a live `[u8; IOMAP_BYTES + IOMAP_GUARD]` (a core's own TSS bitmap), and the
/// caller must hold it exclusively (interrupts masked, this core's own slot). The index never
/// reaches the guard byte, because it is clamped to `IOMAP_BYTES * 8` ports.
unsafe fn write_range_bits(iomap: *mut u8, base: u16, count: u16, deny: bool) {
    for port in (base as usize)..(base as usize + count as usize).min(IOMAP_BYTES * 8) {
        let bit = 1u8 << (port % 8);
        // SAFETY: `port / 8 < IOMAP_BYTES`, so this is inside the array `iomap` points at; the
        // caller guarantees exclusive access to it.
        unsafe {
            let byte = iomap.add(port / 8);
            if deny {
                *byte |= bit;
            } else {
                *byte &= !bit;
            }
        }
    }
}

/// Install a thread's port grant into this core's TSS, lazily (milestone 299, §121).
/// `sched::install_port_grant` calls this on switch-in with the incoming thread's `(base, count)`.
///
/// An unchanged grant returns after one comparison, as on nearly every switch. A change writes only
/// the bits that move, then points `iomap_base` at the bitmap or past the limit; the CPU reads the
/// TSS on each `in`/`out`, so no `ltr` is needed. `#[cold]` keeps the body out of the IPC
/// fastpath's footprint (`script/fastpath-footprint`). See notes/x86-port/traps.md.
///
/// Context: interrupts masked and `sched::IPC_TABLES` held (the switch path; the single-hart bench
/// boot calls it bare); touches only this core's TSS.
#[cold]
pub fn set_port_range_grant(grant: Option<(u16, u16)>) {
    set_port_range_grant_on(crate::cpu::id(), grant);
}

/// [`set_port_range_grant`] for a core named by number rather than through `gs`: the NMI half of a
/// revocation broadcast names its core from the local APIC id register (`mmu::serve_shootdown_nmi`
/// says why). The two numbers agree on this port, and the sender `debug_assert`s it.
///
/// Context: a core's bitmap is written only by a thread holding `sched::IPC_TABLES`, or by an NMI
/// such a thread sent, so a revocation cannot interleave with a switch-path install
/// (`sched::install_port_grant` runs inside the lock). The exception is `bench_*`, whose boot pins
/// one hart and sends nothing.
#[cold]
fn set_port_range_grant_on(id: usize, grant: Option<(u16, u16)>) {
    // SAFETY: this core's own slot, read and written only here and only with interrupts masked on
    // the switch path; a different core touches a different index.
    let installed = unsafe { (*INSTALLED_PORT_GRANT.get())[id] };
    if installed == grant {
        return;
    }
    // The CPU reads this bitmap only on a ring-3 `in`/`out`, which cannot run while this does.
    // SAFETY: forming a raw pointer into this core's own TSS slot; no reference is taken.
    let iomap = unsafe { (&raw mut (*TSS.get())[id].iomap).cast::<u8>() };
    if let Some((base, count)) = installed {
        // SAFETY: this core's own bitmap, held exclusively (interrupts masked, own slot).
        unsafe { write_range_bits(iomap, base, count, true) }; // restore the outgoing holder's deny bits
    }
    let base_value = match grant {
        Some((base, count)) => {
            // SAFETY: as above.
            unsafe { write_range_bits(iomap, base, count, false) }; // permit the incoming holder's ports
            IOMAP_OFFSET
        }
        None => IOMAP_BASE_DENY_ALL,
    };
    // SAFETY: as above; `iomap_base` is a `u16` in the same packed TSS, written through a raw
    // pointer because a reference to a packed field would be misaligned.
    unsafe { (&raw mut (*TSS.get())[id].iomap_base).write_unaligned(base_value) };
    // SAFETY: this core's own slot.
    unsafe { (*INSTALLED_PORT_GRANT.get())[id] = grant };
}

/// **Reset this core's TSS if it currently grants exactly `(base, count)`** (milestone 299): the
/// core-local half of revocation. If the running thread's installed grant is the range being
/// revoked, re-deny its bits and point `iomap_base` past the limit, so a port access faults even
/// before the next context switch. If a different grant (or none) is installed, this does nothing.
///
/// This is what [`sched::delete_current_cap`](crate::sched::delete_current_cap) needs and the whole
/// of what it needs: the thread dropping its own port capability is the thread whose grant is
/// installed *here*, and a core's bitmap only ever permits the range for the thread currently
/// running on it, so there is no second core to tell. A `PortRange::REVOKE` names a holder that may
/// be running anywhere and calls [`revoke_port_grant_everywhere`] instead.
pub(crate) fn revoke_installed_port_grant(base: u16, count: u16) {
    revoke_installed_port_grant_on(crate::cpu::id(), base, count);
}

/// [`revoke_installed_port_grant`] for a core that names itself by number. The receiving half of
/// [`revoke_port_grant_everywhere`], called from the NMI handler; see [`set_port_range_grant_on`]
/// for why the handler names its core from the local APIC.
pub(super) fn revoke_installed_port_grant_on(id: usize, base: u16, count: u16) {
    // SAFETY: this core's own slot; see `set_port_range_grant_on`.
    if unsafe { (*INSTALLED_PORT_GRANT.get())[id] } == Some((base, count)) {
        set_port_range_grant_on(id, None);
    }
}

/// Take the grant out of every online core's TSS, returning once they have (milestone 315 (a port
/// revoke that reaches every core)): `PortRange::REVOKE`'s arch half.
///
/// A revoked holder may be running on another core whose bitmap still permits the range. The local
/// reset comes first, as in [`super::mmu::flush_tlb`]; the remote half rides the TLB shootdown's
/// NMI, since an ordinary IPI deadlocks against a core spinning for `IPC_TABLES` with interrupts
/// masked (notes/x86-tlb-shootdown.md). `spare_this_cpu` keeps the invoker's own grant, which the
/// take-back leaves in place; a whole-machine sweep passes `false`. See notes/x86-port/traps.md.
pub(crate) fn revoke_port_grant_everywhere(base: u16, count: u16, spare_this_cpu: bool) {
    if !spare_this_cpu {
        revoke_installed_port_grant(base, count);
    }
    super::mmu::revoke_port_grant_others(base, count);
}

/// The grant this core's TSS currently permits, if any: `(base, count)` as
/// [`set_port_range_grant`] installed it. Read for tests and diagnostics; the switch path keeps its
/// own copy in the thread.
pub fn installed_port_grant() -> Option<(u16, u16)> {
    // SAFETY: this core's own slot, read only; a different core reads a different index.
    unsafe { (*INSTALLED_PORT_GRANT.get())[crate::cpu::id()] }
}

/// Bench-only (§121's amendment, 2026-08-24): the whole-port-space bitmap the naive design writes
/// into the TSS on every switch-in, never installed as live.
///
/// [`IOMAP_BYTES`] of it, with no guard byte, since no port access checks it. It measures the 8 KiB
/// write alone: nothing points `iomap_base` at it. Milestone 299's lazy install is the real one;
/// this stays the upper-bound baseline `tss_iomap_lazy_switch` and `tss_iomap_lazy_nop` read
/// against. See design/decisions/0121-port-io-capability.md.
#[cfg(feature = "bench")]
#[repr(C, align(8))]
struct BenchIoBitmap([u8; IOMAP_BYTES]);

/// A second 8 KiB region, apart from the live `TSS` so this benchmark is never mistaken for the
/// real bitmap. One instance, because a bench boot runs one hart.
///
/// A `SyncUnsafeCell` with the live tables (calef's ruling of 2026-10-09, milestone 891): it stands
/// in for the bitmap the CPU would read, so it keeps their fixed placement. Plain access is sound
/// because the bench boot runs one hart and nothing else names this static.
#[cfg(feature = "bench")]
static BENCH_IOMAP: SyncUnsafeCell<BenchIoBitmap> =
    SyncUnsafeCell::new(BenchIoBitmap([0; IOMAP_BYTES]));

/// Write a full I/O permission bitmap's worth of bytes into this CPU's bench-only scratch region,
/// the way `schedule()` would write the incoming thread's bitmap into the TSS under option 1. The
/// pattern argument (varied per call by the caller) and the touch after are both there so nothing
/// about this write is provably dead code to the optimizer.
///
/// Safe to call from anywhere: the internal `unsafe` is this CPU's own static, single-hart, no
/// concurrent access, the same fact `bench.rs`'s single-threaded caller already relies on.
#[cfg(feature = "bench")]
pub(crate) fn bench_write_io_bitmap(pattern: u8) -> u8 {
    // SAFETY: single-hart bench boot; writes and reads this CPU's own static, never aliased.
    unsafe {
        let base = (&raw mut (*BENCH_IOMAP.get()).0).cast::<u8>();
        base.write_bytes(pattern, IOMAP_BYTES);
        base.add(IOMAP_BYTES - 1).read()
    }
}
