//! **A forged ELF cannot smuggle a hostile segment past the first one, or wrap its address range
//! over the kernel** (a ninth outsider pass at the confinement claim, claim 22; DECISIONS §15 (the
//! native ABI)).
//!
//! `elf_loader_tests` forges single-segment images. These forge what one segment cannot: two
//! `PT_LOAD`s, the first benign and the second the attack, against a loader that checks only the
//! first; and one segment whose `p_vaddr + p_memsz` wraps, against a half check that reads only the
//! base. Each is forged by hand (the loader has no heap; milestone 14 (kernel objects from untyped)
//! phase C) for this build's machine, so it is not refused as foreign; `load` is `exec`'s loader.
//!
//! Name: provisional (lane/pass-9-tests, 2026-10-14 UTC), for the module, the file and each
//! function. Cross-ISA: `load` builds a `Half::Low` user `Mapper` on each architecture and refuses
//! a high-half or wrapping address by construction, so the same forgeries run on every architecture
//! with an initial ramdisk, which is all three (DECISIONS §19 (architectural parity is a tenet)).

use super::*;

const EHDR: usize = 64;
const PHDR: usize = 56;

/// Write one ELF64 program header into `out` at `base`: a `PT_LOAD` with `flags`, loading `filesz`
/// bytes from `file_off` to `vaddr`, claiming `memsz` bytes of memory (the zero-filled `.bss` tail).
fn write_phdr(out: &mut [u8], base: usize, vaddr: u64, flags: u32, file_off: u64, memsz: u64) {
    out[base..base + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
    out[base + 4..base + 8].copy_from_slice(&flags.to_le_bytes());
    out[base + 8..base + 16].copy_from_slice(&file_off.to_le_bytes()); // p_offset
    out[base + 16..base + 24].copy_from_slice(&vaddr.to_le_bytes()); // p_vaddr
    out[base + 32..base + 40].copy_from_slice(&16u64.to_le_bytes()); // p_filesz (16 bytes of code)
    out[base + 40..base + 48].copy_from_slice(&memsz.to_le_bytes()); // p_memsz
}

/// Common ELF64 header for a forgery with `phnum` program headers. Entry at `entry`, the program
/// header table right after the ELF header.
fn write_ehdr(out: &mut [u8], entry: u64, phnum: u16) {
    out[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    out[4] = 2; // ELFCLASS64
    out[5] = 1; // little-endian
    out[6] = 1; // EV_CURRENT
    out[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    out[18..20].copy_from_slice(&elf::NATIVE_MACHINE.to_le_bytes()); // this build's machine
    out[24..32].copy_from_slice(&entry.to_le_bytes()); // e_entry
    out[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes()); // e_phoff
    out[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes()); // e_phentsize
    out[56..58].copy_from_slice(&phnum.to_le_bytes()); // e_phnum
}

/// A two-segment forgery: a benign first `PT_LOAD` and a second whose `(vaddr, flags)` is the
/// attack. Both carry sixteen bytes of file, placed after the two program headers.
fn forged_two_segments(v0: u64, f0: u32, v1: u64, f1: u32) -> [u8; EHDR + 2 * PHDR + 16] {
    let mut out = [0u8; EHDR + 2 * PHDR + 16];
    write_ehdr(&mut out, v0, 2);
    let code_off = (EHDR + 2 * PHDR) as u64;
    write_phdr(&mut out, EHDR, v0, f0, code_off, 16);
    write_phdr(&mut out, EHDR + PHDR, v1, f1, code_off, 16);
    out
}

/// A one-segment forgery that names `memsz` bytes of memory at `vaddr`, so a test can ask for a
/// span that wraps the address space without carrying it in the file.
fn forged_sized(vaddr: u64, flags: u32, memsz: u64) -> [u8; EHDR + PHDR + 16] {
    let mut out = [0u8; EHDR + PHDR + 16];
    write_ehdr(&mut out, vaddr, 1);
    write_phdr(&mut out, EHDR, vaddr, flags, (EHDR + PHDR) as u64, memsz);
    out
}

/// **A benign first segment does not buy a writable-executable second segment.**
///
/// The first `PT_LOAD` is an ordinary read-execute text at the image base, exactly what a real
/// program carries. The second asks for read-write-execute at the next page. A loader that enforced
/// W^X on the first segment it saw and then trusted the rest would accept this; the claim is that
/// every segment is checked, so the whole image is refused as `WritableAndExecutable`.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_benign_first_segment_does_not_smuggle_a_writable_executable_second() {
    let image = forged_two_segments(
        address_space_map::IMAGE_BASE,
        elf::PF_R | elf::PF_X,
        address_space_map::IMAGE_BASE + 0x2000,
        elf::PF_R | elf::PF_W | elf::PF_X,
    );

    assert_eq!(
        load(&image, 0).err(),
        Some(LoadError::NotLoadable(elf::Error::WritableAndExecutable)),
        "a second, writable-executable segment loaded behind a benign first one: the W^X check \
         did not reach every segment",
    );
}

/// **A benign first segment does not buy a second segment mapped over the kernel.**
///
/// The first `PT_LOAD` is benign text at the image base; the second names the kernel half's base,
/// which a loader that stopped checking after the first would map over the kernel. Either guard may
/// refuse it: that base (`0xffff_0000_0000_0000`) is far above the image band (near `0x40_0000`),
/// so the placement check (`Misplaced(TooLarge)`) fires before the mapper's `Half::Low` `WrongHalf`
/// guard runs. The entry is kept off the result so a success prints (`AddressSpace` lacks `Debug`).
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_benign_first_segment_does_not_smuggle_a_segment_over_the_kernel() {
    let image = forged_two_segments(
        address_space_map::IMAGE_BASE,
        elf::PF_R | elf::PF_X,
        mmu::KERNEL_VA_BASE,
        elf::PF_R | elf::PF_X,
    );

    let outcome = load(&image, 0).map(|(_space, entry)| entry);
    assert!(
        matches!(
            outcome,
            Err(LoadError::Unmappable(MapError::WrongHalf))
                | Err(LoadError::Misplaced(
                    address_space_map::ImagePlacement::TooLarge { .. }
                ))
        ),
        "a second segment named the kernel's half and the loader did not refuse it on either the \
         placement guard or the half guard: {outcome:?}",
    );
}

/// **A segment whose span wraps the address space is refused, not wrapped into the low half.**
///
/// `p_vaddr` is the last page of the 64-bit space and `p_memsz` is four pages, so a naive
/// `base .. base + memsz` wraps to a low-half range straddling the canonical hole. The base alone
/// is high-half, so `WrongHalf` is the floor; the oversized `memsz` tests that arithmetic on it
/// cannot turn the refusal into a low-half success.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_segment_whose_address_range_wraps_is_refused() {
    let vaddr = u64::MAX - page_frames::FRAME_SIZE + 1; // the last page of the space
    let image = forged_sized(vaddr, elf::PF_R | elf::PF_X, 4 * page_frames::FRAME_SIZE);

    // `AddressSpace` is not `Debug`; keep only the entry point so a success is printable.
    let outcome = load(&image, 0).map(|(_space, entry)| entry);
    assert!(
        outcome.is_err(),
        "a segment whose p_vaddr + p_memsz wraps the address space was loaded (entry {outcome:?}): \
         a wrap would map low-half pages the image never legitimately named",
    );
}
