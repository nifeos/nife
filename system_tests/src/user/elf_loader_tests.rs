//! `elf_loader_tests`: the ELF loader. Forged images that ask for the kernel's addresses, a
//! writable executable page or room outside the image band are refused; a real image from the
//! initial ramdisk loads and runs at EL0.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

use core::sync::atomic::Ordering;

use super::test_programs::{loader_subject_image, spawn_bare};
use super::*;
use crate::arch::exceptions::{SVC_COUNT, USER_FAULTS};

/// Forge an ELF64 header by hand, so a test can ask for something no linker would emit.
///
/// A fixed buffer, since the kernel it tests has no heap (milestone 14 phase C): one ELF
/// header, one program header, sixteen bytes of code. The ELF names its own load address.
fn forged_elf(vaddr: u64, flags: u32) -> [u8; 136] {
    forged_elf_sized(vaddr, flags, 16)
}

/// [`forged_elf`], with the one segment claiming `memsz` bytes of memory for its sixteen bytes of
/// file: the zero-filled tail every `.bss` is, which is how a test asks for a large image without
/// carrying one.
fn forged_elf_sized(vaddr: u64, flags: u32, memsz: u64) -> [u8; 136] {
    const EHDR: usize = 64;
    const PHDR: usize = 56;
    let code: [u8; 16] = [0; 16];

    let mut out = [0u8; EHDR + PHDR + 16];
    out[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    out[4] = 2; // ELFCLASS64
    out[5] = 1; // little-endian
    out[6] = 1; // EV_CURRENT
    out[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    // This build's own machine, so the forgery gets past the machine check and reaches the
    // property under test rather than being refused for being foreign.
    out[18..20].copy_from_slice(&elf::NATIVE_MACHINE.to_le_bytes());
    out[24..32].copy_from_slice(&vaddr.to_le_bytes()); // e_entry
    out[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes()); // e_phoff
    out[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes());
    out[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum

    let p = EHDR;
    out[p..p + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
    out[p + 4..p + 8].copy_from_slice(&flags.to_le_bytes());
    out[p + 8..p + 16].copy_from_slice(&((EHDR + PHDR) as u64).to_le_bytes()); // p_offset
    out[p + 16..p + 24].copy_from_slice(&vaddr.to_le_bytes()); // p_vaddr
    out[p + 32..p + 40].copy_from_slice(&(code.len() as u64).to_le_bytes()); // p_filesz
    out[p + 40..p + 48].copy_from_slice(&memsz.to_le_bytes()); // p_memsz
    out
}

/// An ELF that names the base of the kernel's half as its load address is refused.
///
/// The refusal is by construction: the user `Mapper` is built with `Half::Low` and cannot express a
/// high address (`paging`'s `WrongHalf` guard, there since a host test found bits 63:48 are not
/// translated). `KERNEL_VA_BASE` rather than a constant makes it one attack on both ISAs: aarch64's
/// half starts at `0xffff_0000_0000_0000`, RISC-V Sv39's at `0xffff_ffc0_0000_0000`.
///
/// Falsification: replayable `system_tests/falsifications/user.elf_loader_tests.an_elf_that_asks_to_be_loaded_over_the_kernel_is_refused.patch`
#[test_case]
fn an_elf_that_asks_to_be_loaded_over_the_kernel_is_refused() {
    let image = forged_elf(mmu::KERNEL_VA_BASE, elf::PF_R | elf::PF_X);

    assert_eq!(
        load(&image, 0).err(),
        Some(LoadError::Unmappable(MapError::WrongHalf)),
        "the kernel agreed to map a user program on top of itself",
    );
}

/// And a binary asking for a page that is both writable and executable.
///
/// Caught in `crates/elf`, on the host, in microseconds. But assert it end-to-end too: the
/// value of the host test is that it is fast, not that it is the only line of defence.
///
/// Falsification: replayable `system_tests/falsifications/user.elf_loader_tests.an_elf_that_asks_for_a_writable_executable_page_is_refused.patch`
#[test_case]
fn an_elf_that_asks_for_a_writable_executable_page_is_refused() {
    let image = forged_elf(
        address_space_map::IMAGE_BASE,
        elf::PF_R | elf::PF_W | elf::PF_X,
    );

    assert_eq!(
        load(&image, 0).err(),
        Some(LoadError::NotLoadable(elf::Error::WritableAndExecutable)),
    );
}

/// **An image too large for the map is told it is too large** (milestone 206 (a program image has under 896 KiB), DECISIONS §171 (where a program image starts) option
/// A), with the two numbers a reader needs: where the image ends and where the stack begins.
///
/// Before 2026-09-26 this image would have been mapped page by page until the first page that
/// collided with the stack, and refused as `Unmappable(AlreadyMapped)`, which names an overlap and
/// not a size. The image here is one page past the band, which is the smallest image that must be
/// refused, and it is refused before a single frame is spent on it: the region it would need is
/// half a gigabyte, so reaching `AddressSpace::new` would have reported running out of memory.
#[test_case]
fn an_image_too_large_for_its_band_is_refused_as_too_large() {
    use address_space_map::{IMAGE, IMAGE_BASE, ImagePlacement, PAGE, STACK};

    let image = forged_elf_sized(IMAGE_BASE, elf::PF_R | elf::PF_X, IMAGE.bytes() + PAGE);

    assert_eq!(
        load(&image, 0).err(),
        Some(LoadError::Misplaced(ImagePlacement::TooLarge {
            image_end: IMAGE.end + PAGE,
            stack_base: STACK.start,
        })),
    );
}

/// **A program linked for the old layout is refused by name**, not loaded among pair pages it
/// would collide with at some later, unrelated address.
#[test_case]
fn an_image_linked_below_the_image_band_is_refused_as_outside_it() {
    let image = forged_elf(0x40_0000, elf::PF_R | elf::PF_X);

    assert_eq!(
        load(&image, 0).err(),
        Some(LoadError::Misplaced(
            address_space_map::ImagePlacement::OutsideBand {
                image_start: 0x40_0000,
                image_end: 0x40_1000,
            }
        )),
    );
}

/// **The old ceiling is gone**: an image of 2 MiB, more than twice the 896 KiB that used to run
/// into the stack, loads, and its last page is mapped where the image band says.
#[test_case]
fn an_image_past_the_old_ceiling_loads() {
    use address_space_map::IMAGE_BASE;
    const BYTES: u64 = 2 << 20;

    let image = forged_elf_sized(IMAGE_BASE, elf::PF_R | elf::PF_X, BYTES);
    let (space, entry) = load(&image, 0).expect("a 2 MiB image did not load");
    assert_eq!(entry, IMAGE_BASE);

    let last = space.while_installed(|| {
        mmu::translate_user(IMAGE_BASE + BYTES - page_frames::FRAME_SIZE).is_some()
    });
    drop(space);
    assert!(last, "the image's last page is not mapped");
}

/// Junk is refused, and refusing it does not take the kernel down.
#[test_case]
fn a_bad_binary_is_refused_rather_than_panicking() {
    assert!(load(b"#!/bin/sh\necho hi\n", 0).is_err());
    assert!(load(&[], 0).is_err());
    assert!(load(&[0u8; 4096], 0).is_err());
    // And we are still executing, which is the assertion.
}

/// The initial ramdisk is there, and it is the program we built, for **this** machine: `Elf::parse`
/// refuses a foreign `e_machine`, so parsing at all is half the assertion.
///
/// Falsification: unfalsified. Renamed from `the_initrd_holds_a_native_executable` on 2026-10-12
/// (UTC) under calef's ruling B on #1998; the body is unchanged and nobody has yet broken it to
/// watch it fail.
#[test_case]
fn the_initial_ramdisk_holds_a_native_executable() {
    let image = loader_subject_image();
    let e = elf::Elf::parse(image).expect("the initial ramdisk is not a loadable native ELF");

    assert_eq!(
        e.entry(),
        address_space_map::IMAGE_BASE,
        "linked somewhere unexpected"
    );

    // Three segments, and NONE of them writable-and-executable. Counted straight off the
    // iterator: the kernel this test rides in has no heap to collect into (milestone 14).
    assert!(
        e.segments().count() >= 3,
        "expected .text, .rodata and .data"
    );
    assert!(e.segments().any(|s| s.is_executable() && !s.is_writable()));
    assert!(e.segments().any(|s| s.is_writable() && !s.is_executable()));

    // And one of them has a .bss: memsz > filesz. If this is not true, the test below is
    // vacuous, and we would never know.
    assert!(
        e.segments().any(|s| s.memsz as usize > s.data.len()),
        "no segment has a .bss, so the zero-fill is untested",
    );
}

/// A separately compiled binary from the initial ramdisk runs at user mode and checks its image.
///
/// hello's `SELF_CHECK` role (one source, built for this machine) answers with a syscall if every
/// expectation holds, a trap if not, and no data crosses the boundary. A syscall and no fault means
/// `.text` ran, `.rodata` read, `.data` was copied, `.bss` was zeroed (the file lacks those bytes)
/// and the stack recursed eight frames.
///
/// Falsification: unfalsified. Renamed from
/// `a_real_elf_from_the_initrd_runs_at_el0_and_verifies_itself` on 2026-10-12 (UTC) under calef's
/// ruling B on #1998; the body is unchanged and nobody has yet broken it to watch it fail.
#[test_case]
fn a_real_elf_from_the_initial_ramdisk_runs_at_el0_and_verifies_itself() {
    let svc = SVC_COUNT.load(Ordering::Relaxed);
    let faults = USER_FAULTS.load(Ordering::Relaxed);

    spawn_bare(loader_subject_image(), 0, 0).expect("spawn failed");

    assert!(
        wait_for(|| SVC_COUNT.load(Ordering::Relaxed) > svc),
        "the program never reached its syscall",
    );
    assert_eq!(
        USER_FAULTS.load(Ordering::Relaxed),
        faults,
        "the program reached EL0 and then FAILED its own self-check: one of \
         .text/.rodata/.data/.bss/stack was not what the ELF asked for",
    );
}
