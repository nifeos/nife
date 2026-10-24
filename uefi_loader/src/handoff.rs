//! **Building the handoff the kernel already knows how to read.**
//!
//! The kernel's `x86_64` boot handoff is PVH's `hvm_start_info`: one physical pointer to a
//! structure carrying the memory map, the ACPI root pointer and any loaded modules, decoded by
//! `machine_discovery::x86_64` and consumed by `arch::x86_64::machine`. So this loader synthesises
//! an `hvm_start_info` rather than inventing a UEFI handoff, and the kernel cannot tell which
//! loader started it: one structure, one decoder, one set of tests, and no second boot path to
//! diverge on hardware nobody can attach a debugger to (milestone 87; notes/x86-uefi-boot.md).

//! # This module is the *writer* for `machine_discovery::x86_64`'s *reader*
//!
//! And the tests below say so literally: every encoder here is checked by decoding its output with
//! that crate. Neither side carries its own copy of the layout, which is the same rule
//! `byte_sink_contract`, `grant_plan` and `clock_contract` are held to.

//! # BUGS
//!
//! - **`EfiLoaderData` is reported as reserved and stays that way** (milestone 195 did not reclaim
//!   it): it holds the `hvm_start_info`, its memory map, the module list, the archive and the
//!   kernel image, so reclaiming it would let the allocator hand out the map it is reading. The
//!   cost is each one's page padding, measured at two memory sizes in notes/x86-uefi-boot.md.
//! - **The map is passed through in firmware order, neither sorted nor coalesced.** UEFI promises
//!   neither, and `machine_discovery::x86_64`'s own `BUGS` records that its consumer is where such
//!   a check belongs.

/// `hvm_start_info`, version 1: the size this loader always writes.
pub const START_INFO_LEN: usize = 56;

/// One `hvm_memmap_table_entry`.
pub const MEMMAP_ENTRY_LEN: usize = 24;

/// One `hvm_modlist_entry`.
pub const MODULE_ENTRY_LEN: usize = 32;

/// The magic at offset 0, and the value the kernel is handed in `eax`. "xEn3" little-endian.
pub const MAGIC: u32 = 0x336e_c578;

/// The structure version this loader writes. **1, not 0**, because version 0 stops before the
/// memory-map fields and a version-0 handoff means "this kernel cannot find its RAM here".
pub const VERSION: u32 = 1;

/// E820 type 1: ordinary RAM, the only kind a frame allocator may hand out.
pub(crate) const E820_RAM: u32 = 1;
/// E820 type 2: somebody else owns it.
pub(crate) const E820_RESERVED: u32 = 2;
/// E820 type 3: ACPI tables, reclaimable after they have been read.
pub(crate) const E820_ACPI_RECLAIMABLE: u32 = 3;
/// E820 type 4: ACPI non-volatile storage.
pub(crate) const E820_ACPI_NVS: u32 = 4;
/// E820 type 5: RAM the firmware found faulty.
pub(crate) const E820_UNUSABLE: u32 = 5;
/// E820 type 7: byte-addressable persistent memory.
pub(crate) const E820_PERSISTENT: u32 = 7;

/// **Translate one UEFI memory type into the E820 type PVH's map carries.**
///
/// The direction of every judgement call here is the same one
/// `machine_discovery::x86_64::MemoryKind::from_raw` documents for unknown types: **claiming less
/// RAM than exists costs a few megabytes, and claiming more corrupts something.** A UEFI type this
/// function has never heard of is reserved, not RAM.
pub const fn e820_kind(efi_type: u32) -> u32 {
    use r_efi::efi as t;
    match efi_type {
        t::CONVENTIONAL_MEMORY => E820_RAM,
        // **Boot-services memory is free RAM the moment `ExitBootServices` returns**, and the UEFI
        // specification says so rather than leaving it to be inferred: those two types describe the
        // firmware's own code and data for the boot phase, and the boot phase is over. Linux
        // reclaims them and so does every other UEFI loader; milestone 195 measured 26 MiB of a
        // 2 GiB machine here, and the same firmware wants nearly as much of a 256 MiB one.
        //
        // **The one thing that makes this safe is that nothing of ours is in them.** Every
        // allocation this loader makes asks for `LOADER_CODE` or `LOADER_DATA`, deliberately, and
        // those stay reserved below. A loader that had taken `AllocatePages` defaults would be
        // freeing its own handoff here.
        t::BOOT_SERVICES_CODE | t::BOOT_SERVICES_DATA => E820_RAM,
        // **And this loader's own code, dead by the time the kernel reads this map**: the PE image
        // the firmware loaded, whose embedded kernel and archive were copied OUT of it before
        // `ExitBootServices` (9 MiB for the tour build, 19 MiB for the test build, which a 256 MiB
        // machine cannot spare), and the one-shot mode-switch trampoline. `LOADER_DATA` is NOT
        // here, and the asymmetry is the mechanism: everything the kernel reads later asks for
        // `LOADER_DATA`, and only the trampoline asks for `LOADER_CODE`, because firmware sets the
        // execute-disable bit on data.
        t::LOADER_CODE => E820_RAM,
        t::ACPI_RECLAIM_MEMORY => E820_ACPI_RECLAIMABLE,
        t::ACPI_MEMORY_NVS => E820_ACPI_NVS,
        t::UNUSABLE_MEMORY => E820_UNUSABLE,
        t::PERSISTENT_MEMORY => E820_PERSISTENT,
        // Everything else, this loader's own allocations included. See this module's BUGS section
        // for what that still costs and why the rest of it is not a type question.
        _ => E820_RESERVED,
    }
}

/// The fields of `hvm_start_info` this loader fills in. Everything else is zero.
#[derive(Clone, Copy, Default)]
pub struct StartInfo {
    /// **The ACPI RSDP**, taken from the UEFI configuration table.
    ///
    /// This is the field that makes a UEFI boot different from a PVH one in a way the kernel can
    /// see. QEMU's PVH loader leaves it zero, so `arch::x86_64::machine::find_rsdp` falls back to
    /// scanning the BIOS area for `"RSD PTR "`; real firmware hands it over, so the non-zero path
    /// runs. Milestone 87's brief called that path out by name: it had never executed.
    pub rsdp: u64,
    /// Where the module list is, or 0 when there is no initial ramdisk.
    pub modules: u64,
    /// How many modules. 0 or 1 here.
    pub module_count: u32,
    /// Where the memory map is.
    pub memmap: u64,
    /// How many entries it has.
    pub memmap_entries: u32,
    /// **Where the NUL-terminated boot command line is**, or 0 when there is nothing to say. It
    /// carries `machine_discovery::framebuffer`'s description of the screen the firmware was
    /// drawing on (milestone 243), which is what keeps a machine with no serial port from being
    /// silent, and the other words [`cmdline`] lists. A command line rather than a new structure
    /// field, deliberately: `hvm_start_info` is Xen's format, versioned by Xen, and an appended
    /// field is a fork of somebody else's layout that looks exactly like the real thing to whoever
    /// reads it next.
    pub cmdline: u64,
}

impl StartInfo {
    /// Lay the structure out, little-endian, exactly as Xen's `start_info.h` specifies it.
    pub fn encode(&self) -> [u8; START_INFO_LEN] {
        let mut out = [0u8; START_INFO_LEN];
        put_u32(&mut out, 0, MAGIC);
        put_u32(&mut out, 4, VERSION);
        // offset 8: flags. Xen defines SecureBoot (bit 0) and SecureBootEnabled (bit 1); this
        // loader does not report them, because it does not verify anything and a flag claiming
        // otherwise would be a lie the kernel has no way to check.
        put_u32(&mut out, 12, self.module_count);
        put_u64(&mut out, 16, self.modules);
        put_u64(&mut out, 24, self.cmdline);
        put_u64(&mut out, 32, self.rsdp);
        put_u64(&mut out, 40, self.memmap);
        put_u32(&mut out, 48, self.memmap_entries);
        out
    }
}

/// Lay out one `hvm_memmap_table_entry`.
pub fn encode_memmap_entry(addr: u64, size: u64, kind: u32) -> [u8; MEMMAP_ENTRY_LEN] {
    let mut out = [0u8; MEMMAP_ENTRY_LEN];
    put_u64(&mut out, 0, addr);
    put_u64(&mut out, 8, size);
    put_u32(&mut out, 16, kind);
    out
}

/// Lay out one `hvm_modlist_entry`.
pub fn encode_module(addr: u64, size: u64) -> [u8; MODULE_ENTRY_LEN] {
    let mut out = [0u8; MODULE_ENTRY_LEN];
    put_u64(&mut out, 0, addr);
    put_u64(&mut out, 8, size);
    // offset 16: this module's own command line, and offset 24 is reserved. Neither is used.
    out
}

/// **How many bytes the kernel's boot command line may occupy**, NUL included: the one place the
/// line's vocabulary is added up, though its tokens belong to two crates.
///
/// `Framebuffer::MAX_LEN` is the screen token; then a space and
/// `machine_discovery::framebuffer::SCREEN_HOLD`, which the `screen_hold` feature adds for
/// milestone 445 (the screen check stops sampling and starts asking), counted whether or not that
/// feature is on so a caller's buffer has one length rather than two; then
/// `boot_slot::cmdline::MAX_LEN`, the slot token an installed machine's chooser writes and every
/// other boot leaves off.
pub const CMDLINE_LEN: usize = machine_discovery::framebuffer::Framebuffer::MAX_LEN
    + 1
    + machine_discovery::framebuffer::SCREEN_HOLD.len()
    + 1
    + boot_slot::cmdline::MAX_LEN
    + 1
    + boot_slot::medium::MAX_LEN
    + 1;

/// **Write the kernel's boot command line**, NUL-terminated; returns its length without the NUL,
/// or 0 when there is nothing to say (no screen, no chooser) and the caller writes none. Up to four
/// words, in order: the screen (milestone 243; `None` with no graphics output); the slot key
/// `boot_slot::cmdline::KEY` and a digit, **only** when a chooser started this image;
/// `boot_slot::medium::NVME` when this boot came from the NVMe disk; and `SCREEN_HOLD` under the
/// `screen_hold` feature. Written here so a host test reads it back with the kernel's own parsers;
/// notes/x86-uefi-boot/loader-design.md has each word's contract.
///
/// # Panics
/// If `out` is shorter than [`CMDLINE_LEN`].
#[must_use]
pub fn cmdline(
    screen: Option<&machine_discovery::framebuffer::Framebuffer>,
    from_slot: Option<u8>,
    on_nvme: bool,
    out: &mut [u8],
) -> usize {
    assert!(out.len() >= CMDLINE_LEN, "cmdline needs CMDLINE_LEN bytes");
    #[allow(unused_mut)]
    let mut n = 0usize;

    if let Some(screen) = screen {
        n += screen.encode(out);
    }

    if let Some(slot) = from_slot {
        let at = if n == 0 {
            n
        } else {
            out[n] = b' ';
            n + 1
        };
        let written = boot_slot::cmdline::encode(slot, &mut out[at..]);
        // Zero means the number did not fit a digit, which no two-slot disk produces. Leaving the
        // separator off rather than writing a dangling space keeps the line exactly what the
        // parsers see.
        if written > 0 {
            n = at + written;
        }
    }

    if on_nvme {
        if n > 0 {
            out[n] = b' ';
            n += 1;
        }
        n += boot_slot::medium::encode_nvme(&mut out[n..]);
    }

    #[cfg(feature = "screen_hold")]
    {
        if n > 0 {
            out[n] = b' ';
            n += 1;
        }
        let word = machine_discovery::framebuffer::SCREEN_HOLD.as_bytes();
        out[n..n + word.len()].copy_from_slice(word);
        n += word.len();
    }

    if n == 0 {
        return 0;
    }
    out[n] = 0;
    n
}

fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], at: usize, value: u64) {
    bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use machine_discovery::x86_64 as pvh;

    use super::*;

    /// **The command line the kernel reads back is the one this wrote**, whichever build produced
    /// it, which is this module's rule applied to the one field that is not a fixed-width number.
    ///
    /// The package turns `screen_hold` on for its own tests (a dev-dependency on itself, see
    /// `Cargo.toml`), so the assertion below covers the gate's build; the screen token is asserted
    /// either way, because it is the half no build may lose.
    #[test]
    fn the_kernel_reads_back_the_command_line_this_writes() {
        use machine_discovery::framebuffer::{Framebuffer, PixelOrder, has_screen_hold};

        let screen = Framebuffer {
            base: 0x8000_0000,
            width: 1280,
            height: 800,
            stride: 5120,
            order: PixelOrder::Bgrx,
        };
        let mut out = [0u8; CMDLINE_LEN];
        let n = cmdline(Some(&screen), None, false, &mut out);
        assert_eq!(
            out[n], 0,
            "the line is NUL-terminated at the length returned"
        );
        let line = core::str::from_utf8(&out[..n]).expect("the writer emits ASCII");
        assert_eq!(
            Framebuffer::parse(line),
            Some(screen),
            "the kernel's own parser reads the screen back"
        );
        assert_eq!(
            has_screen_hold(line),
            cfg!(feature = "screen_hold"),
            "the hold token is present exactly when the feature that writes it is on"
        );
    }

    /// The point of every test in this file: what this loader **writes** is decoded by the crate
    /// the kernel **reads** with, so the two cannot drift apart without a host test failing in
    /// milliseconds.
    #[test]
    fn what_the_loader_writes_is_what_the_kernel_reads() {
        let info = StartInfo {
            rsdp: 0x7fee_0014,
            modules: 0x0100_0000,
            module_count: 1,
            memmap: 0x0100_1000,
            memmap_entries: 42,
            cmdline: 0x0100_2000,
        };
        let decoded =
            pvh::BootInfo::parse(&info.encode()).expect("the kernel's decoder accepts it");

        assert_eq!(
            decoded.version, 1,
            "version 0 has no memory-map fields at all"
        );
        assert_eq!(decoded.rsdp, 0x7fee_0014);
        assert_eq!(decoded.modules, 0x0100_0000);
        assert_eq!(decoded.module_count, 1);
        assert_eq!(decoded.memmap, 0x0100_1000);
        assert_eq!(decoded.memmap_entries, 42);
        assert_eq!(
            decoded.cmdline, 0x0100_2000,
            "the field milestone 243 started writing, and offset 24 is the only place it can be"
        );
    }

    /// **The whole of milestone 243's handoff, end to end, in one host test.** The loader describes
    /// a screen, encodes it into the command line, writes the command line's address into the
    /// structure, and the kernel's own decoder gets the pointer back out. Everything except the
    /// physical memory the pointer names is exercised here, in milliseconds, on a machine with no
    /// framebuffer at all.
    #[test]
    fn a_screen_survives_the_whole_handoff() {
        use machine_discovery::framebuffer::{Framebuffer, PixelOrder};

        let screen = Framebuffer {
            base: 0x8000_0000,
            width: 800,
            height: 600,
            stride: 3200,
            order: PixelOrder::Bgrx,
        };
        // The loader's own buffer, at the address it would have allocated for it.
        let mut line = [0u8; Framebuffer::MAX_LEN];
        let written = screen.encode(&mut line);

        let info = StartInfo {
            cmdline: 0x0200_0000,
            ..StartInfo::default()
        };
        let decoded = pvh::BootInfo::parse(&info.encode()).expect("a valid structure");
        assert_eq!(decoded.cmdline, 0x0200_0000);

        // What the kernel does once it has followed that pointer.
        let text = core::str::from_utf8(&line[..written]).expect("ASCII");
        assert_eq!(Framebuffer::parse(text), Some(screen));
    }

    /// **The slot number survives the handoff on a machine with no screen**, which is the shape
    /// this line could not carry before: the caller used to write no command line at all when
    /// there was no framebuffer, so a serial-only installed machine would have reached its kernel
    /// with nothing to confirm and reverted every upgrade.
    #[test]
    fn a_slot_number_reaches_the_kernel_with_or_without_a_screen() {
        use machine_discovery::framebuffer::{Framebuffer, PixelOrder};

        let screen = Framebuffer {
            base: 0x8000_0000,
            width: 800,
            height: 600,
            stride: 3200,
            order: PixelOrder::Bgrx,
        };

        for with_screen in [None, Some(&screen)] {
            for slot in [0u8, 1] {
                let mut out = [0u8; CMDLINE_LEN];
                let n = cmdline(with_screen, Some(slot), true, &mut out);
                assert_eq!(out[n], 0, "NUL-terminated at the length returned");
                let line = core::str::from_utf8(&out[..n]).expect("ASCII");
                assert_eq!(
                    boot_slot::cmdline::parse(line),
                    Some(slot),
                    "the kernel's own parser reads the slot back from {line:?}"
                );
                assert_eq!(
                    Framebuffer::parse(line),
                    with_screen.copied(),
                    "and the screen token is untouched by the one beside it"
                );
            }
        }
    }

    /// **Every word on the line is separated by exactly one space, and none leads or trails.**
    /// Milestone 326 (turn a mutation score upward), 2026-09-24: a slot the token cannot spell (past one digit) and a build whose
    /// only word is the hold token were the two ways to a stray space, and neither was asserted.
    #[test]
    fn no_word_is_written_with_a_stray_space() {
        use machine_discovery::framebuffer::{Framebuffer, PixelOrder, SCREEN_HOLD};

        let screen = Framebuffer {
            base: 0x8000_0000,
            width: 800,
            height: 600,
            stride: 3200,
            order: PixelOrder::Bgrx,
        };
        let line = |screen: Option<&Framebuffer>, slot: Option<u8>| {
            let mut out = [0u8; CMDLINE_LEN];
            let n = cmdline(screen, slot, false, &mut out);
            String::from_utf8(out[..n].to_vec()).unwrap()
        };
        let hold = if cfg!(feature = "screen_hold") {
            format!(" {SCREEN_HOLD}")
        } else {
            String::new()
        };
        assert_eq!(
            line(Some(&screen), Some(10)),
            format!("screen=0x80000000,800,600,3200,bgrx{hold}"),
            "slot 10 has no token, and leaves no space behind"
        );
        if cfg!(feature = "screen_hold") {
            assert_eq!(
                line(None, None),
                SCREEN_HOLD,
                "the hold token alone, unindented"
            );
        }
    }

    /// **A boot with nothing to say writes no command line**, so an ordinary `-kernel` boot and a
    /// stick are byte-for-byte what they were before the slot token existed.
    ///
    /// Skipped under `screen_hold`, which is a build that always has a word to write; the feature
    /// is one gate's and never a machine's.
    #[test]
    #[cfg(not(feature = "screen_hold"))]
    fn no_screen_and_no_chooser_is_an_empty_line_rather_than_a_blank_one() {
        let mut out = [0xAAu8; CMDLINE_LEN];
        assert_eq!(cmdline(None, None, false, &mut out), 0);
        assert_eq!(out[0], 0xAA, "nothing was written, not even a terminator");
    }

    /// The boot medium reaches the kernel's own parser, and a stick's line does not carry it: the
    /// second half is G2 of the live stick proposal, since without the token the kernel leaves the
    /// internal disk alone.
    #[test]
    fn the_boot_medium_is_on_the_line_only_for_a_boot_from_the_nvme_disk() {
        for (slot, on_nvme) in [(None, true), (Some(0), true), (None, false)] {
            let mut out = [0u8; CMDLINE_LEN];
            let n = cmdline(None, slot, on_nvme, &mut out);
            let line = core::str::from_utf8(&out[..n]).expect("ASCII");
            assert_eq!(boot_slot::medium::is_nvme(line), on_nvme, "{line:?}");
            assert_eq!(boot_slot::cmdline::parse(line), slot, "{line:?}");
            // No separator before the first token or after the last.
            assert_eq!(line.trim(), line, "{line:?}");
        }
    }

    /// **Every token this writer can emit fits [`CMDLINE_LEN`]**, asserted rather than added up by
    /// a reader, because the constant is the one place the line's vocabulary is summed and the
    /// tokens belong to two different crates.
    #[test]
    fn the_longest_line_this_writer_can_produce_fits_its_own_buffer() {
        use machine_discovery::framebuffer::{Framebuffer, PixelOrder};

        // The widest screen token this encoder can produce, by the fields that are printed in
        // full: the assertion is against `MAX_LEN`, which is that crate's own claim.
        let screen = Framebuffer {
            base: u64::MAX,
            width: u32::MAX,
            height: u32::MAX,
            stride: u32::MAX,
            order: PixelOrder::Bgrx,
        };
        let mut out = [0u8; CMDLINE_LEN];
        let n = cmdline(Some(&screen), Some(9), true, &mut out);
        assert!(
            n < CMDLINE_LEN,
            "{n} bytes and a NUL must fit {CMDLINE_LEN}"
        );
        assert_eq!(out[n], 0);
        // Nothing is left over that nobody accounted for: every token but the screen's counts at
        // its exact length, the screen's at that crate's own rounded-up `MAX_LEN`, and the NUL
        // once. The line above carries `screen-hold`, because Cargo.toml's self-dependency builds
        // this package's tests with that feature on.
        let screen_len = screen.encode(&mut [0u8; Framebuffer::MAX_LEN]);
        assert_eq!(
            CMDLINE_LEN,
            n + (Framebuffer::MAX_LEN - screen_len) + 1,
            "CMDLINE_LEN is the sum of its tokens and the NUL"
        );
    }

    /// The two crates agree on the structure's length, which is the fact that would silently break
    /// if either side gained a field.
    #[test]
    fn the_two_sides_agree_on_the_entry_sizes() {
        assert_eq!(MEMMAP_ENTRY_LEN, pvh::MEMMAP_ENTRY_LEN);
        assert_eq!(MODULE_ENTRY_LEN, pvh::MODULE_ENTRY_LEN);
        assert_eq!(MAGIC, pvh::MAGIC);
    }

    #[test]
    fn a_memory_map_entry_round_trips_through_the_kernels_decoder() {
        let bytes = encode_memmap_entry(0x10_0000, 0x0fe0_0000, E820_RAM);
        let entry = pvh::memory_entry(&bytes, 0).expect("one entry, in bounds");
        assert_eq!(entry.addr, 0x10_0000);
        assert_eq!(entry.size, 0x0fe0_0000);
        assert_eq!(entry.kind, pvh::MemoryKind::Ram);
        assert!(entry.is_usable_ram());
    }

    /// Indexing works across a *pair* of entries, which is what catches a stride mistake that a
    /// one-entry map cannot: the same trap `machine_discovery`'s own module tests call out.
    #[test]
    fn a_two_entry_map_strides_correctly() {
        let mut bytes = [0u8; MEMMAP_ENTRY_LEN * 2];
        bytes[..MEMMAP_ENTRY_LEN].copy_from_slice(&encode_memmap_entry(0, 0x1000, E820_RESERVED));
        bytes[MEMMAP_ENTRY_LEN..]
            .copy_from_slice(&encode_memmap_entry(0x10_0000, 0x2000, E820_RAM));

        assert_eq!(pvh::memory_entry(&bytes, 0).unwrap().addr, 0);
        assert_eq!(pvh::memory_entry(&bytes, 1).unwrap().addr, 0x10_0000);
        assert!(!pvh::memory_entry(&bytes, 0).unwrap().is_usable_ram());
        assert!(pvh::memory_entry(&bytes, 1).unwrap().is_usable_ram());
        assert!(
            pvh::memory_entry(&bytes, 2).is_none(),
            "past the end is None"
        );
    }

    #[test]
    fn a_module_entry_round_trips_through_the_kernels_decoder() {
        let bytes = encode_module(0x0200_0000, 4_400_000);
        let module = pvh::module(&bytes, 0).expect("one module, in bounds");
        assert_eq!(module.addr, 0x0200_0000);
        assert_eq!(module.size, 4_400_000);
    }

    /// The classification, stated as a table so a reader can check it against the UEFI
    /// specification without reading the `match`.
    #[test]
    fn every_uefi_memory_type_lands_where_it_should() {
        use r_efi::efi as t;
        assert_eq!(e820_kind(t::CONVENTIONAL_MEMORY), E820_RAM);
        assert_eq!(e820_kind(t::ACPI_RECLAIM_MEMORY), E820_ACPI_RECLAIMABLE);
        assert_eq!(e820_kind(t::ACPI_MEMORY_NVS), E820_ACPI_NVS);
        assert_eq!(e820_kind(t::UNUSABLE_MEMORY), E820_UNUSABLE);
        assert_eq!(e820_kind(t::PERSISTENT_MEMORY), E820_PERSISTENT);

        // Free the moment `ExitBootServices` returns (milestone 195), and asserted beside the
        // loader's own types below so the pair reads as the one decision it is.
        assert_eq!(e820_kind(t::BOOT_SERVICES_CODE), E820_RAM);
        assert_eq!(e820_kind(t::BOOT_SERVICES_DATA), E820_RAM);
        assert_eq!(e820_kind(t::LOADER_CODE), E820_RAM);

        for reserved in [
            t::RESERVED_MEMORY_TYPE,
            t::LOADER_DATA,
            t::RUNTIME_SERVICES_CODE,
            t::RUNTIME_SERVICES_DATA,
            t::MEMORY_MAPPED_IO,
            t::MEMORY_MAPPED_IO_PORT_SPACE,
            t::PAL_CODE,
        ] {
            assert_eq!(e820_kind(reserved), E820_RESERVED, "type {reserved}");
        }
    }

    /// The direction of the unknown-type default, asserted rather than left to the reader: a type
    /// UEFI adds after this was written must not become RAM by accident.
    #[test]
    fn a_memory_type_this_loader_has_never_heard_of_is_not_ram() {
        assert_eq!(e820_kind(0xdead_beef), E820_RESERVED);
        assert_eq!(e820_kind(u32::MAX), E820_RESERVED);
    }
}
