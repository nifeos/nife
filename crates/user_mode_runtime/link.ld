/* An EL0 program's layout, shared by every program in `components/` and `fixtures/`.
 *
 * It lives beside the userspace runtime rather than in either package because both link
 * against it and `cargo:rustc-link-arg` is per-package, so each build script has to name it
 * (`components/build.rs`, `fixtures/build.rs`). It was `user/link.ld` until milestone 175 split
 * that package; milestone 73 had already refused to rename it on the ground that it is
 * genuinely shared, and two copies would be two places to be wrong about the base.
 *
 * The base is `address_space_map::IMAGE_BASE` (milestone 206 (a program image has under 896 KiB), DECISIONS §171 (where a program image starts) option D): the start of
 * the image band, which runs up to the stack band at 0x7F00_0000, so an image may be 496 MiB. It
 * was 0x400000 until 2026-09-26, which left 896 KiB before the stack, and `ripgrep` had to be
 * relinked by substitution to fit. A linker script cannot name a Rust constant, so the number is
 * written here and `crates/address_space_map`'s host test parses this line and fails if the two
 * differ. Everything above 2 GiB is out of reach for this line: x86_64 links with the small code
 * model, which puts every symbol under 0x8000_0000 (see that crate's docs).
 *
 * The kernel is at 0xffff_..., in TTBR1. A user program is at 0x0000_..., in TTBR0, and the
 * hardware picks between them from bits 63:48 of the address. So a user program can be linked
 * at a low address with no fear of colliding with anything: the kernel is not *in* its address
 * space in any sense it can observe. See notes/higher-half.md.
 *
 * PHDRS is explicit, and that is the point. Left to itself the linker will happily put .text
 * and .rodata in one PT_LOAD, sharing a page, and our loader refuses that (Error::SegmentsOverlap).
 * More importantly, being explicit is what lets us hand the kernel three segments with three
 * different permissions, so that W^X is a property of the file and not just of our good manners.
 */

ENTRY(_start)

PHDRS
{
    text   PT_LOAD FLAGS(5);   /* R+X  */
    rodata PT_LOAD FLAGS(4);   /* R    */
    data   PT_LOAD FLAGS(6);   /* R+W  */
    note   PT_NOTE FLAGS(4);   /* R, and inside `rodata`: see .note.nife below */
}

SECTIONS
{
    . = 0x60000000;

    .text : ALIGN(4096) {
        *(.text._start)         /* the entry point goes first, so `entry` is legible */
        *(.text .text.*)
    } :text

    /* The `rodata` segment starts on a page of its own even when .rodata is empty, which a
     * program that carries a note and has no constants (`least_authority_demo`) is. The linker drops
     * an empty output section and its ALIGN with it, so without this line .note.nife would follow
     * .text into the same page and the loader would refuse the program with SegmentsOverlap: the
     * .bss rule below, one segment earlier. Found by milestone 597 on all three targets. */
    . = ALIGN(4096);
    .rodata : ALIGN(4096) {
        *(.rodata .rodata.*)
    } :rodata

    /* **The program's manifest, as an ELF note** (milestone 597 (a program carries its manifest in
     * an ELF note), provisional; DECISIONS §197 (a package is one archive file) option M2, ruled
     * 2026-09-26). `capability_manifest_note::carry!` puts one note in `.note.nife.manifest`, and this keeps
     * it twice over: in the `rodata` PT_LOAD, so its bytes are in the file and covered by the
     * digest the progenitor vouches on, and in a PT_NOTE of its own, which is how the shell and the
     * progenitor find it (`crates/elf`'s `Elf::note`) without section headers, which `strip` may
     * delete from a program that still runs.
     *
     * KEEP, because nothing references the note and the linker would otherwise collect it. It is
     * named before the /DISCARD/ below, and the first rule that matches an input section is the one
     * that places it, so every other `.note*` a toolchain adds is still thrown away. A program that
     * carries no note gets an empty section here and a PT_NOTE with no notes in it, which reads as
     * "no manifest". */
    .note.nife : ALIGN(4) {
        KEEP(*(.note.nife.*))
    } :rodata :note

    .data : ALIGN(4096) {
        *(.data .data.*)

        /* The GOT, placed here on purpose and BEFORE .bss (milestone 161).
         *
         * Left unnamed, the linker appends orphan sections after the last one it was told about,
         * which is .bss. .got has file contents and .bss does not, so the `data` PT_LOAD's
         * p_filesz then stretches past .bss to cover it, p_memsz == p_filesz, and the segment has
         * no zero-fill tail at all. Two things break quietly: the loader has nothing to zero, so
         * .bss is whatever bytes happened to be in the file, and the kernel's own
         * `the_initial_ramdisk_holds_a_native_executable` test loses the memsz > filesz it asserts on,
         * which is how this was found.
         *
         * It shows up on x86_64 and not on the other two. The GOT is not something this project
         * asked for: all three targets use the static relocation model and none of these programs
         * is position-independent, but LLVM's x86-64 backend still routes a few references through
         * a GOT entry, and the aarch64 and RISC-V backends produce none for the same source. So
         * this is one line that costs nothing where it is not needed and prevents a silent hole
         * where it is. */
        *(.got .got.plt)
    } :data

    /* .bss is in the `data` segment but occupies NO space in the file. That is what makes
     * p_memsz > p_filesz, and the loader has to zero the difference. See crates/elf.
     *
     * ALIGN(4096) here is not tidiness, it is the same rule the sections above follow, and it was
     * missing. A program with .bss and an EMPTY .data (nothing initialized and non-zero) gets its
     * .data collapsed by the linker, so the `data` PT_LOAD begins at .bss, which without this
     * alignment starts wherever .rodata happened to end. The two segments then share a page and the
     * loader refuses the program with SegmentsOverlap. Found by milestone 57's disk_surveyor, whose
     * 44 KiB of table buffers are all bss; every earlier program happened to have some .data. */
    .bss : ALIGN(4096) {
        *(.bss .bss.*)
        *(COMMON)
    } :data

    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
