//! **Forged ELF images the cited tests do not shape** (an eighth outsider pass at the confinement
//! claim, claim 22).
//!
//! The cited tests refuse a segment *starting* in the kernel half and flags of exactly `R|W|X`.
//! Here: segment arithmetic that overflows, an in-band segment whose `memsz` reaches the kernel
//! half, a benign first header with a second naming the kernel, and `W|X` dressed in vendor OS
//! bits. `load` must refuse each with the error that names why, and the kernel keeps executing.
//! Cross-ISA over each ISA's `Mapper` (DECISIONS §19 (architectural parity is a tenet)). See
//! notes/confinement-claims/the-eighth-pass-tests.md.

use super::*;

/// Forge a one-segment ELF64 header by hand, `elf_loader_tests::forged_elf_sized`'s shape with the
/// `memsz` knob this pass's wraparound case needs.
fn forged(vaddr: u64, flags: u32, memsz: u64) -> [u8; 136] {
    const EHDR: usize = 64;
    const PHDR: usize = 56;
    let code: [u8; 16] = [0; 16];

    let mut out = [0u8; EHDR + PHDR + 16];
    out[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    out[4] = 2;
    out[5] = 1;
    out[6] = 1;
    out[16..18].copy_from_slice(&2u16.to_le_bytes());
    out[18..20].copy_from_slice(&elf::NATIVE_MACHINE.to_le_bytes());
    out[24..32].copy_from_slice(&vaddr.to_le_bytes());
    out[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes());
    out[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes());
    out[56..58].copy_from_slice(&1u16.to_le_bytes());

    let p = EHDR;
    out[p..p + 4].copy_from_slice(&1u32.to_le_bytes());
    out[p + 4..p + 8].copy_from_slice(&flags.to_le_bytes());
    out[p + 8..p + 16].copy_from_slice(&((EHDR + PHDR) as u64).to_le_bytes());
    out[p + 16..p + 24].copy_from_slice(&vaddr.to_le_bytes());
    out[p + 32..p + 40].copy_from_slice(&(code.len() as u64).to_le_bytes());
    out[p + 40..p + 48].copy_from_slice(&memsz.to_le_bytes());
    out
}

/// Forge a **two**-segment ELF64: the first benign and inside the band, the second naming
/// `second_vaddr`. A loader that inspects only the first program header, or stops sizing at the
/// first segment, passes this image.
fn forged_two_segment(second_vaddr: u64) -> [u8; 192] {
    const EHDR: usize = 64;
    const PHDR: usize = 56;
    let code: [u8; 16] = [0; 16];

    let mut out = [0u8; EHDR + 2 * PHDR + 16];
    out[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    out[4] = 2;
    out[5] = 1;
    out[6] = 1;
    out[16..18].copy_from_slice(&2u16.to_le_bytes());
    out[18..20].copy_from_slice(&elf::NATIVE_MACHINE.to_le_bytes());
    out[24..32].copy_from_slice(&address_space_map::IMAGE_BASE.to_le_bytes());
    out[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes());
    out[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes());
    out[56..58].copy_from_slice(&2u16.to_le_bytes());

    for (i, &vaddr) in [address_space_map::IMAGE_BASE, second_vaddr]
        .iter()
        .enumerate()
    {
        let p = EHDR + i * PHDR;
        out[p..p + 4].copy_from_slice(&1u32.to_le_bytes());
        out[p + 4..p + 8].copy_from_slice(&(elf::PF_R | elf::PF_X).to_le_bytes());
        out[p + 8..p + 16].copy_from_slice(&((EHDR + 2 * PHDR) as u64).to_le_bytes());
        out[p + 16..p + 24].copy_from_slice(&vaddr.to_le_bytes());
        out[p + 32..p + 40].copy_from_slice(&(code.len() as u64).to_le_bytes());
        out[p + 40..p + 48].copy_from_slice(&(code.len() as u64).to_le_bytes());
    }
    out
}

/// **A segment whose end arithmetic overflows the address space is refused as an overflow**, not
/// wrapped into a small image that loads.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn an_elf_whose_segment_arithmetic_overflows_is_refused_as_an_overflow() {
    let image = forged(u64::MAX - 15, elf::PF_R | elf::PF_X, 16);

    assert_eq!(
        load(&image, 0).err(),
        Some(LoadError::NotLoadable(elf::Error::AddressOverflow)),
        "a segment whose vaddr plus memsz overflows u64 was accepted (or wrapped): the loader \
         trusted arithmetic a hostile image did",
    );
}

/// **A segment that starts inside the band and claims to reach the kernel half is refused.**
///
/// `memsz` is the `.bss` trick: sixteen bytes of file, and a claimed memory size that runs from
/// the image base to one page past the kernel half's start. No addition overflows, so the refusal
/// has to come from the image placement or the half check, not from an accident of arithmetic.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn an_elf_that_reaches_from_its_band_into_the_kernel_half_is_refused() {
    let reach =
        mmu::KERNEL_VA_BASE.wrapping_sub(address_space_map::IMAGE_BASE) + page_frames::FRAME_SIZE;
    let image = forged(address_space_map::IMAGE_BASE, elf::PF_R | elf::PF_X, reach);

    let refused = load(&image, 0).err();
    assert!(
        refused.is_some(),
        "an image claiming to reach from its band into the kernel's half was loaded",
    );
    assert!(
        !matches!(refused, Some(LoadError::Unmappable(_))),
        "the refusal was an accident of a single page rather than the band or half check: {refused:?}",
    );
}

/// **A forged image whose second program header names the kernel is refused.**
///
/// The first segment is what every loading test here trusts: in band, `R|X`, sixteen bytes. The
/// refusal is the band check's, the stronger fact: the second segment stretches the image's span
/// past every limit before a page is spent, and `image_end` names the kernel-half page it aimed
/// at. A loader that inspected only the first program header would have reported success.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn an_elf_whose_second_segment_names_the_kernel_is_refused() {
    let image = forged_two_segment(mmu::KERNEL_VA_BASE);

    assert_eq!(
        load(&image, 0).err(),
        Some(LoadError::Misplaced(
            address_space_map::ImagePlacement::TooLarge {
                image_end: mmu::KERNEL_VA_BASE + page_frames::FRAME_SIZE,
                stack_base: address_space_map::STACK.start,
            }
        )),
        "the loader accepted (or silently ignored) a second segment aimed at the kernel's half: \
         only the first program header was inspected",
    );
}

/// **`W|X` dressed in vendor OS bits is still `W|X`.**
///
/// The standard says `PF_MASKOS` bits are ignored; a check written as equality against a known
/// flag *set* rather than as membership would let `R|W|X|0x00ff_0000` through as an unrecognized
/// and therefore unchecked combination.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn an_elf_dressing_writable_executable_in_vendor_bits_is_refused() {
    let image = forged(
        address_space_map::IMAGE_BASE,
        elf::PF_R | elf::PF_W | elf::PF_X | 0x00ff_0000,
        16,
    );

    assert_eq!(
        load(&image, 0).err(),
        Some(LoadError::NotLoadable(elf::Error::WritableAndExecutable)),
        "OS/vendor flag bits turned a writable executable request into one the loader stopped \
         checking",
    );
}
