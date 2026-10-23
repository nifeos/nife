//! **The 32-bit MMIO hole, and the window this kernel places device BARs in**: how far
//! the cacheable direct map may follow the firmware's map, and where in the hole above it a
//! window fits.
//!
//! Cut out of `mmu.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines). `super::discovery` and `super::machine` name the `x86_64`
//! items the index imports, as in `mmu.rs`.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// **How high the cacheable direct map may follow the loader's map**, given every span it
/// describes (RAM and reserved alike, in any order).
///
/// Start at [`LOW_MEGABYTE`] and extend upward while some entry begins at or below the current top
/// and ends above it. DRAM and the carve-outs firmware takes from it are contiguous, so the chain
/// covers them; the 32-bit MMIO hole above is a gap the map does not describe, so the chain stops
/// at its floor and the device windows stay out, for [`map_everything`]'s step 5 to map
/// device-typed. No 4 GiB constant: the hole's floor moves with how much memory firmware stole.
/// See notes/x86-port/kernel-page-tables.md.
pub(super) fn firmware_fill_ceiling(mut spans: impl FnMut(&mut dyn FnMut(u64, u64))) -> u64 {
    let mut ceiling = LOW_MEGABYTE;
    loop {
        let mut grown = ceiling;
        // One pass extends through every entry that already chains in map order; the outer loop is
        // what makes the answer independent of that order, since the firmware's map is not sorted
        // (xenon's puts `0xa0000..0x100000` after a 16 GiB region). It runs at most once per entry.
        spans(&mut |start, end| {
            if start <= grown && end > grown {
                grown = end;
            }
        });
        if grown == ceiling {
            return ceiling;
        }
        ceiling = grown;
    }
}

/// **The floor and the ceiling of the 32-bit MMIO hole, from the firmware's memory map alone**
/// (milestone 256).
///
/// The floor is [`firmware_fill_ceiling`]; the ceiling is the next span the map describes, or
/// 4 GiB. Pure in its argument, so the 16 GiB machine's map is a test value (`map_tests`).
///
/// # BUGS
///
/// A gap in the map means only that firmware did not call it RAM; unnamed windows are a risk.
///
/// Name: provisional (minted beside [`memory_mapped_io_window`]).
pub(super) fn firmware_mmio_hole(mut spans: impl FnMut(&mut dyn FnMut(u64, u64))) -> (u64, u64) {
    let floor = firmware_fill_ceiling(&mut spans);
    let mut ceiling = FOUR_GIB;
    spans(&mut |start, end| {
        if end > floor && start > floor && start < ceiling {
            ceiling = start;
        }
    });
    (floor, ceiling)
}

/// One past the last address a 32-bit BAR can name.
const FOUR_GIB: u64 = 0x1_0000_0000;

/// **The lowest `size`-aligned span of `size` bytes inside `lo..hi` that overlaps nothing in
/// `avoid`**, or `None` when the hole has no such room.
///
/// Aligned to `size` because a BAR's writable bits encode its size, so its address must be a
/// multiple of it. `avoid` is scanned once per candidate, quadratic in a list of at most five
/// entries; each step moves the candidate past something it collided with, so the loop runs at
/// most once per entry plus one.
///
/// Name: provisional (minted beside [`memory_mapped_io_window`]).
pub(super) fn window_in_hole(
    lo: u64,
    hi: u64,
    size: u64,
    mut avoid: impl FnMut(&mut dyn FnMut(u64, u64)),
) -> Option<u64> {
    let mut candidate = lo.next_multiple_of(size);
    loop {
        if candidate + size > hi {
            return None;
        }
        // The furthest end of anything this candidate runs into. Taking the furthest rather than
        // the first means one pass per collision group instead of one per entry.
        let mut past = candidate;
        avoid(&mut |start, end| {
            if end > start && start < candidate + size && end > candidate && end > past {
                past = end;
            }
        });
        if past == candidate {
            return Some(candidate);
        }
        candidate = past.next_multiple_of(size);
    }
}

/// **Why [`memory_mapped_io_window()`] could not answer.** Each arm is a machine this kernel does not know how to
/// place a BAR on, and every one of them is a panic rather than a fallback: the constant this
/// replaced was right on emulated machines and was RAM on the first real one, so falling back to
/// it would be the original bug wearing the clothes of a graceful degradation. Milestone 215 took
/// the same posture for the analogous case (no MCFG means no PCI, and deliberately no legacy
/// fallback).
///
/// Name: provisional. calef names the types, and this one was minted by a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryMappedIoWindowError {
    /// The boot structure could not be re-read, so there is no memory map to find a hole in.
    NoMemoryMap,
    /// **The two sources disagree.** The firmware's map stops describing memory at one address and
    /// the host bridge's `TOLUD` names another. One of them is wrong about this machine and
    /// nothing here can tell which, so neither is used.
    Disagreement { map: u64, tolud: u64 },
    /// The hole is real and has no room left in it for a window of the size asked for, once the
    /// windows this kernel already knows about are taken out of it. On xenon the framebuffer
    /// aperture sits at the very floor of the hole, so this arm is closer than it looks.
    NoRoom { lo: u64, hi: u64, size: u64 },
}

impl core::fmt::Display for MemoryMappedIoWindowError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MemoryMappedIoWindowError::NoMemoryMap => {
                write!(f, "no firmware memory map to read a hole from")
            }
            MemoryMappedIoWindowError::Disagreement { map, tolud } => write!(
                f,
                "the firmware map stops describing memory at {map:#x} but the host bridge's \
                 TOLUD says {tolud:#x}"
            ),
            MemoryMappedIoWindowError::NoRoom { lo, hi, size } => write!(
                f,
                "no {size:#x}-byte window free inside the MMIO hole {lo:#x}..{hi:#x}"
            ),
        }
    }
}

/// **Where this machine's PCI BARs may go, asked of the machine rather than assumed** (milestone
/// 256).
///
/// [`firmware_mmio_hole`] and the host bridge's `TOLUD` must agree, or this returns
/// [`MemoryMappedIoWindowError::Disagreement`]. See notes/x86-port/kernel-page-tables.md.
///
/// Name: ratified 2026-09-05 (calef). Refused `bar_window`, which the lane shipped on the grounds
/// that it was the vocabulary the retired `PCI_BAR_PHYS` used for the thing in its own first line;
/// calef asked what BAR stood for, which settled it: an argument from this project's own history
/// is not an argument that a reader knows the word. BAR is base address register, a PCI term, and
/// the window is where the blocks those registers point at appear in the physical address space.
/// Refused `device_register_window`, for accuracy: a base address register can map a framebuffer
/// or a memory aperture as well as a register block, so the plainer name would have been slightly
/// narrower than the truth. Refused `mmio_window`, under the naming rule calef set the same day: an
/// acronym is spelled out unless its expansion teaches nothing, and a reader who knows the term
/// recognizes the expansion instantly, so spelling it out costs the expert nothing. `io` stays,
/// because nobody bounces off it.
///
/// [`top_of_low_dram`] is still provisional; [`firmware_mmio_hole`] and [`window_in_hole`] carry
/// their own `Name:` lines.
pub fn memory_mapped_io_window(ecam: (u64, u64)) -> Result<(u64, u64), MemoryMappedIoWindowError> {
    let Some(info) = super::machine::boot_info(crate::DTB.load(Ordering::Relaxed)) else {
        return Err(MemoryMappedIoWindowError::NoMemoryMap);
    };
    let spans = |span: &mut dyn FnMut(u64, u64)| {
        for i in 0..info.memmap_entries as usize {
            let Some(e) = super::machine::memory_map_entry(&info, i) else {
                break;
            };
            span(
                e.addr & !(PAGE_SIZE - 1),
                e.end().next_multiple_of(PAGE_SIZE),
            );
        }
    };
    let (lo, hi) = firmware_mmio_hole(spans);
    if let Some(tolud) = super::machine::top_of_low_dram()
        && tolud != lo
    {
        return Err(MemoryMappedIoWindowError::Disagreement { map: lo, tolud });
    }

    // Everything this kernel already knows decodes inside the hole. The APICs and VT-d's register
    // file are usually above it (on xenon all three are, and the hole ends at 0xf0000000), so most
    // of this list costs nothing on most machines; the framebuffer is the one that earns it, and it
    // earns it on the only real machine this port has met. xenon's aperture is at 0xd0000000, which
    // is the floor of its hole *exactly*, so a window that took the floor and did not ask would
    // have collided with the screen the panic would have been printed on.
    let window = window_in_hole(lo, hi, PCI_BAR_MAPPED, |avoid| {
        avoid(ecam.0, ecam.0 + ecam.1);
        if let Some(apic) = super::discovery::local_apic_phys() {
            avoid(apic, apic + PAGE_SIZE);
        }
        if let Some(io_apic) = super::discovery::io_apic_phys() {
            avoid(io_apic, io_apic + PAGE_SIZE);
        }
        for (base, size) in memory::vtd_regions().into_iter().flatten() {
            avoid(base, base + size);
        }
        for (base, size) in memory::amd_vi_regions().into_iter().flatten() {
            avoid(base, base + size);
        }
        if let Some((base, size)) = memory::framebuffer() {
            avoid(base, base + size);
        }
    });
    match window {
        Some(base) => Ok((base, PCI_BAR_MAPPED)),
        None => Err(MemoryMappedIoWindowError::NoRoom {
            lo,
            hi,
            size: PCI_BAR_MAPPED,
        }),
    }
}
