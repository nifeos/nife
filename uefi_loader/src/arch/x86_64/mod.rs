//! **`x86_64`: an `hvm_start_info`, and leaving long mode** (milestone 87 (the x86_64 bare-metal
//! machine)).
//!
//! The kernel's entry contract on this architecture is PVH's: 32-bit protected mode, paging off,
//! `eax` = `0x336EC578`, `ebx` = the physical address of an `hvm_start_info`. Everything here
//! produces that state from the one UEFI hands over. This is the code that was `main.rs` until the
//! loader learned a second and third architecture; it moved here unchanged in behaviour, and
//! notes/x86-uefi-boot.md is still the account of it.

use core::ptr;

use machine_discovery::framebuffer::{Framebuffer, PixelOrder};
use r_efi::efi::{self, ALLOCATE_ADDRESS, ALLOCATE_MAX_ADDRESS, BootServices, Handle, SystemTable};
use r_efi::protocols::graphics_output;
use uefi_loader::handoff::{
    CMDLINE_LEN, MEMMAP_ENTRY_LEN, MODULE_ENTRY_LEN, START_INFO_LEN, StartInfo, e820_kind,
    encode_memmap_entry, encode_module,
};

use crate::{
    MAP_SLACK_DESCRIPTORS, PAGE, Placed, SUCCESS, boot_services, say, say_conflict, say_decimal,
    say_span,
};

// The mode-switch trampoline. See leave_long_mode.s, which is where the interesting half of this
// file's job actually happens.
core::arch::global_asm!(include_str!("leave_long_mode.s"));

unsafe extern "C" {
    /// The trampoline's first instruction. Copied, not called where it is linked.
    static x86_leave_long_mode: [u8; 0];
    /// The 32-bit continuation, whose address in the *copy* is an argument to the trampoline.
    static x86_leave_long_mode_pmode32: [u8; 0];
    /// The three descriptors the trampoline loads.
    static x86_leave_long_mode_gdt: [u8; 0];
    /// The `lgdt` operand, whose 8-byte base this loader patches after the copy.
    static x86_leave_long_mode_gdtr: [u8; 0];
    /// One past the last byte to copy.
    static x86_leave_long_mode_end: [u8; 0];
}

/// **The ceiling on every allocation this loader makes.**
///
/// Everything the kernel is handed has to be nameable by a 32-bit instruction stream running with
/// paging off, which is the state `_start` is entered in. A `hvm_start_info` at 5 GiB would be a
/// pointer the kernel's trampoline literally cannot load.
pub(crate) const ALLOCATION_CEILING: u64 = 0xffff_ffff;

/// **The page a STARTUP IPI can name**, which the kernel's AP bring-up copies its real-mode
/// trampoline into.
///
/// It is `AP_TRAMPOLINE_PHYS` in `kernel/link-x86_64.ld` and `.ap_trampoline`'s link address in the
/// kernel image, so the two files name one number. It is not part of the kernel's `p_paddr` span
/// (that section is linked low and *loaded* beside `.rodata`), which is why the span allocation
/// does not cover it and this loader has to ask for it separately.
const AP_TRAMPOLINE_PHYS: u64 = 0x8000;

/// Where the boot command line sits inside page 0 of the handoff block.
///
/// 256 rather than 120 (the `hvm_start_info` plus the **two**-entry module list) so that a field
/// added to either does not silently start overwriting the command line; the page has four
/// kilobytes and there is nothing else to spend them on.
///
/// **It was 128 against a one-entry list**, which left exactly eight bytes of slack, and milestone
/// 198 (a package manager, and the trivial install that makes a second customer possible)'s rung 2a
/// added the second entry (this loader's own file). Doubling it costs nothing and puts the slack
/// back.
const CMDLINE_OFFSET: u64 = 256;

/// What this architecture reads from the firmware before anything is placed.
pub struct Found {
    rsdp: u64,
    screen: Option<Framebuffer>,
}

/// **The two reads a kernel cannot make once the firmware is gone**: the ACPI root pointer, and the
/// screen (milestone 243).
///
/// Both are reads of something that already exists, so nothing later can invalidate them, and the
/// answers are in hand before the console these sentences are printed on goes away. **A machine
/// with no screen is not an error**: the `OptiPlex` with its serial module is that machine.
pub fn discover(table: &SystemTable, services: &BootServices) -> Result<Found, &'static str> {
    let rsdp = find_rsdp(table).ok_or("the firmware's configuration table has no ACPI RSDP")?;
    let screen = find_screen(services);
    match screen {
        Some(found) => {
            say_span(
                table,
                "uefi_loader: screen at ",
                found.base,
                found.base + found.span().unwrap_or(0) as u64,
            );
            say(table, "uefi_loader:   ");
            say_decimal(table, found.width);
            say(table, "x");
            say_decimal(table, found.height);
            say(table, ", stride ");
            say_decimal(table, found.stride);
            say(table, ", ");
            say(table, found.order.token());
            say(table, "\r\n");
        }
        None => say(
            table,
            "uefi_loader: no linear framebuffer; the kernel will have only a UART\r\n",
        ),
    }
    Ok(Found { rsdp, screen })
}

/// **Everything from "the kernel and the archive are placed" to "the kernel is running".**
/// Returns only on failure, and only while there is still a console to say so on.
// Every argument is a distinct fact this boot learned (the devices found, the images placed, which
// slot started us and from what medium), and the two `Option`s among them have independent
// meanings; a struct would carry the same nine names one call deeper.
#[allow(clippy::too_many_arguments)]
pub fn hand_over(
    handle: Handle,
    table: &SystemTable,
    services: &BootServices,
    found: Found,
    kernel: &Placed,
    module: Option<(u64, u64)>,
    boot_file: Option<(u64, u64)>,
    // **Which boot slot started this image**, when a chooser did, so the running system can say
    // the boot worked. `None` for a stick, a `-kernel` boot, and a chooser's own fallback to the
    // image in its file; all three are boots with nothing to confirm.
    from_slot: Option<u8>,
    // **Whether this boot came from the NVMe disk**, which is the only boot whose kernel may mount
    // it (`boot_slot::medium`; calef's ruling on PR #1652, 2026-10-04 UTC).
    on_nvme: bool,
) -> Result<(), &'static str> {
    reserve_ap_page(table, services);
    let mut handoff = allocate_handoff(services)?;
    let trampoline = place_trampoline(services)?;

    // Everything the kernel is handed has to be reachable from 32 bits. Checked here, with a
    // console still available, rather than discovered as a triple fault on somebody's desk.
    if kernel.entry >= ALLOCATION_CEILING || handoff.start_info_at >= ALLOCATION_CEILING {
        return Err("the kernel entry or the handoff landed above 4 GiB");
    }

    say(
        table,
        "uefi_loader: kernel placed, exiting boot services\r\n",
    );

    // --- The final map, then the point of no return ---
    //
    // NOTHING may allocate between these two calls. `ExitBootServices` compares the key against the
    // map's current generation and refuses if anything has changed, which is the specification
    // protecting the kernel from being handed a map that no longer describes the machine.
    // SAFETY: `raw_map_at` is `map_capacity` bytes of the handoff block `allocate_handoff` was
    // granted, reserved for this map and nothing else.
    let map = unsafe {
        boot_services::memory_map(
            services,
            handoff.raw_map_at as *mut core::ffi::c_void,
            handoff.map_capacity,
        )
    }
    .ok_or("the firmware's memory map grew between the two GetMemoryMap calls")?;
    let final_bytes = map.bytes;
    handoff.descriptor_size = map.descriptor_size;
    // SAFETY: `handle` is this image's, and `map.key` is from the `GetMemoryMap` just above with
    // nothing allocated since, which is the specification's condition for this call.
    if unsafe { (services.exit_boot_services)(handle, map.key) } != SUCCESS {
        return Err("ExitBootServices refused the map key");
    }

    // --- From here the firmware is gone. No console, no allocation, no going back. ---
    let written = write_memory_map(&handoff, final_bytes);
    let cmdline = write_cmdline(&found, from_slot, on_nvme, handoff.cmdline_at);
    write_start_info(&handoff, &found, module, boot_file, written, cmdline);
    paint_last_word(found.screen);

    // The trampoline was copied to an executable page below 4 GiB and its GDT pointer patched;
    // every argument is a physical address below 4 GiB, which is the contract stated at the top of
    // leave_long_mode.s. It does not return, so this expression's type is `!` and the `Ok` arm of
    // this function's signature is unreachable.
    (trampoline.enter)(
        kernel.entry,
        handoff.start_info_at,
        trampoline.gdtr,
        trampoline.pmode32,
    )
}

/// **The page the kernel's AP bring-up needs, asked for by name.** A refusal is reported and the
/// boot carries on single-core.
fn reserve_ap_page(table: &SystemTable, services: &BootServices) {
    // A STARTUP IPI names a physical PAGE below 1 MiB (`vector << 12`), so `arch::x86_64::ap_boot`
    // copies its real-mode trampoline to a fixed low address that `kernel/link-x86_64.ld` picks at
    // link time (`AP_TRAMPOLINE_PHYS`), and the firmware must not be using it. **A refusal is not
    // fatal**: a single-core boot beats no boot, and the kernel brings up secondaries only when
    // asked, so this says what it found (the one line that explains `smp: 1 core(s) online` on a
    // machine with eight) and carries on.
    let granted = boot_services::allocate_pages(
        services,
        ALLOCATE_ADDRESS,
        efi::LOADER_DATA,
        1,
        AP_TRAMPOLINE_PHYS,
    );
    if granted.is_none() {
        say_span(
            table,
            "uefi_loader: WARNING no AP trampoline page at ",
            AP_TRAMPOLINE_PHYS,
            AP_TRAMPOLINE_PHYS + PAGE,
        );
        say_conflict(
            table,
            services,
            AP_TRAMPOLINE_PHYS,
            AP_TRAMPOLINE_PHYS + PAGE,
        );
        say(
            table,
            "uefi_loader: secondary cores will not be brought up\r\n",
        );
    }
}

/// The handoff block's addresses, and the memory-map sizes the final `GetMemoryMap` reuses.
struct Handoff {
    map_capacity: usize,
    entry_capacity: usize,
    descriptor_size: usize,
    start_info_at: u64,
    module_list_at: u64,
    cmdline_at: u64,
    memmap_at: u64,
    raw_map_at: u64,
}

/// Size the memory map, then allocate everything the handoff needs in one block below 4 GiB.
fn allocate_handoff(services: &BootServices) -> Result<Handoff, &'static str> {
    // `write_memory_map` strides by the firmware's own `descriptor_size`; see `memory_map_size`.
    let (map_bytes, descriptor_size) =
        boot_services::memory_map_size(services).ok_or("the firmware reported no memory map")?;

    let map_capacity = map_bytes + MAP_SLACK_DESCRIPTORS * descriptor_size;
    let entry_capacity = map_capacity / descriptor_size;

    // One allocation, three regions, so there is one failure to report and one address to print:
    //   page 0                    the `hvm_start_info` and the one-entry module list
    //   [entries)                 the PVH memory map this loader writes
    //   [raw)                     the firmware's own memory map, read once and never handed on
    let entries_bytes = (entry_capacity * MEMMAP_ENTRY_LEN) as u64;
    let handoff_pages = (PAGE
        + entries_bytes.next_multiple_of(PAGE)
        + (map_capacity as u64).next_multiple_of(PAGE))
        / PAGE;
    let handoff_base = boot_services::allocate_pages(
        services,
        ALLOCATE_MAX_ADDRESS,
        efi::LOADER_DATA,
        handoff_pages as usize,
        ALLOCATION_CEILING,
    )
    .ok_or("no memory below 4 GiB for the boot handoff")?;
    let start_info_at = handoff_base;
    let module_list_at = handoff_base + START_INFO_LEN as u64;
    // **The boot command line lives in the same page**, after the 56-byte structure and the 32-byte
    // module list, which together end at 88. It is at most `Framebuffer::MAX_LEN` plus a NUL and
    // there are four kilobytes here, so it costs no allocation and adds no failure path: one more
    // `AllocatePages` for sixty bytes would be one more thing that can be refused on somebody's
    // firmware, in the middle of the one sequence in this loader whose order is rigid.
    let cmdline_at = handoff_base + CMDLINE_OFFSET;
    let memmap_at = handoff_base + PAGE;
    let raw_map_at = memmap_at + entries_bytes.next_multiple_of(PAGE);
    Ok(Handoff {
        map_capacity,
        entry_capacity,
        descriptor_size,
        start_info_at,
        module_list_at,
        cmdline_at,
        memmap_at,
        raw_map_at,
    })
}

/// **The mode-switch trampoline, in a page the firmware will let us execute.**
fn place_trampoline(services: &BootServices) -> Result<Trampoline, &'static str> {
    // `LOADER_CODE` rather than `LOADER_DATA`, and that is not tidiness: firmware with a memory
    // protection policy (OVMF has one, and so does every recent vendor firmware) sets the
    // execute-disable bit on data allocations, and the first instruction of the copy would fault
    // with boot services still live and nothing watching.
    //
    // Below 4 GiB for the same reason everything else is: the second half of this blob executes
    // with paging off, where a linear address is a physical one.
    let trampoline_base = boot_services::allocate_pages(
        services,
        ALLOCATE_MAX_ADDRESS,
        efi::LOADER_CODE,
        1,
        ALLOCATION_CEILING,
    )
    .ok_or("no executable page below 4 GiB for the mode-switch trampoline")?;
    copy_trampoline(trampoline_base)
}

/// Translate the firmware's final memory map into the PVH one, after `ExitBootServices`. Makes no
/// firmware call and no allocation; returns how many entries it wrote.
fn write_memory_map(handoff: &Handoff, final_bytes: usize) -> usize {
    let descriptors = final_bytes / handoff.descriptor_size;
    let written = descriptors.min(handoff.entry_capacity);
    for i in 0..written {
        // SAFETY: `raw_map_at` holds `final_bytes` of descriptors and `i < final_bytes /
        // descriptor_size`. Read unaligned because the firmware chooses `descriptor_size` and
        // nothing promises the stride keeps 8-byte alignment.
        let descriptor = unsafe {
            ptr::read_unaligned(
                (handoff.raw_map_at as usize + i * handoff.descriptor_size)
                    as *const efi::MemoryDescriptor,
            )
        };
        let bytes = encode_memmap_entry(
            descriptor.physical_start,
            descriptor.number_of_pages * PAGE,
            e820_kind(descriptor.r#type),
        );
        // SAFETY: `written <= entry_capacity`, which is what the region was sized for.
        unsafe {
            ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                (handoff.memmap_at as usize + i * MEMMAP_ENTRY_LEN) as *mut u8,
                MEMMAP_ENTRY_LEN,
            );
        }
    }
    written
}

/// Write the boot command line into page 0 of the handoff block, after `ExitBootServices`.
/// Returns its address, or 0 when there is nothing to put on it.
fn write_cmdline(found: &Found, from_slot: Option<u8>, on_nvme: bool, cmdline_at: u64) -> u64 {
    // The command line, NUL-terminated, in the page reserved for it above. Written after
    // `ExitBootServices` like everything else in this section: it names memory this loader already
    // owns and needs no firmware call, so there is nothing here that could invalidate the map key.
    // **A command line whenever there is anything to put on it**, which since rung 2b of milestone
    // 198 is not the same as "whenever there is a screen": an installed machine with a serial
    // console and no framebuffer still has to be told which slot it came from, or it can never
    // confirm the boot and every upgrade to it reverts a few boots later.
    let mut token = [0u8; CMDLINE_LEN];
    // The one writer, so what this places is decoded in a host test by the kernel's own parsers;
    // `handoff::cmdline` says what each token is and what `screen_hold` adds.
    let written =
        uefi_loader::handoff::cmdline(found.screen.as_ref(), from_slot, on_nvme, &mut token);
    if written == 0 {
        0
    } else {
        // SAFETY: `CMDLINE_OFFSET + CMDLINE_LEN` is inside page 0 of the handoff block, which was
        // allocated and zeroed above, and `written + 1` is at most `CMDLINE_LEN`.
        unsafe { ptr::copy_nonoverlapping(token.as_ptr(), cmdline_at as *mut u8, written + 1) };
        cmdline_at
    }
}

/// Write the module list and the `hvm_start_info` into page 0 of the handoff block, after
/// `ExitBootServices`.
fn write_start_info(
    handoff: &Handoff,
    found: &Found,
    module: Option<(u64, u64)>,
    boot_file: Option<(u64, u64)>,
    written: usize,
    cmdline: u64,
) {
    // **Module 0 is the archive and module 1 is this loader's own file, in that order, and the
    // order is the contract.** `arch::x86_64::machine::initial_ramdisk` reads module 0 and has
    // since milestone 87; `machine::boot_file` reads module 1 and is milestone 198 (a package
    // manager, and the trivial install that makes a second customer possible)'s rung 2a. A boot
    // with no archive therefore hands over **no** modules at all rather than sliding the boot file
    // into slot 0, which would be read as an archive and fail the measurement that protects it.
    let modules: [Option<(u64, u64)>; 2] = match module {
        Some(_) => [module, boot_file],
        None => [None, None],
    };
    let module_count = modules.iter().filter(|m| m.is_some()).count() as u32;

    let info = StartInfo {
        rsdp: found.rsdp,
        modules: if module_count > 0 {
            handoff.module_list_at
        } else {
            0
        },
        module_count,
        memmap: handoff.memmap_at,
        memmap_entries: written as u32,
        cmdline,
    };
    for (index, entry) in modules.iter().enumerate() {
        let Some((addr, size)) = *entry else { continue };
        let bytes = encode_module(addr, size);
        // SAFETY: the module list sits inside page 0 of the handoff block, after the 56-byte
        // structure, and is two 32-byte entries ending well before `CMDLINE_OFFSET`.
        unsafe {
            ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                (handoff.module_list_at + (index * MODULE_ENTRY_LEN) as u64) as *mut u8,
                MODULE_ENTRY_LEN,
            );
        }
    }
    let bytes = info.encode();
    // SAFETY: page 0 of the handoff block, 56 bytes.
    unsafe {
        ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            handoff.start_info_at as *mut u8,
            START_INFO_LEN,
        );
    }
}

/// **The last word anything says before the kernel exists** (milestone 243's early-boot half):
/// clear the screen and paint one sentence, after `ExitBootServices`.
fn paint_last_word(screen: Option<Framebuffer>) {
    // From `ExitBootServices` to the kernel's `attach_screen` nothing can speak on a machine with
    // no serial port, and a fault in between is a silent triple fault. The window cannot be
    // narrated (its code is 32-bit and knows no screen), but it can be bounded: the screen is
    // cleared and given one sentence, so the five things a person at a monitor can be looking at
    // are told apart (`uefi_loader::screen` has the ladder, as does notes/serial-less-output.md).
    // Painted AFTER `ExitBootServices`: before it the screen is the firmware's console and writing
    // underneath would race its scrolling, and the aperture survives the call (milestone 243 (a
    // machine with no serial port)): what ends is the firmware's *console*, not the *display*.
    if let Some(screen) = screen
        && let Some(span) = screen.span()
    {
        // SAFETY: `screen` came from this firmware's own `EFI_GRAPHICS_OUTPUT_PROTOCOL` and
        // `find_screen` checked its span against the aperture size the firmware reported. Boot
        // services are gone, so no firmware code is drawing there any more, and the kernel has not
        // started, so nothing else is either: this loader is the only writer in this instant. The
        // loader runs identity-mapped, so the physical base is the address.
        let pixels = unsafe { core::slice::from_raw_parts_mut(screen.base as *mut u8, span) };
        let _ = uefi_loader::screen::paint_handoff(screen, pixels);
    }
}

/// Where the copied trampoline's three interesting addresses ended up.
struct Trampoline {
    /// Its first instruction, called with the System V convention.
    enter: extern "sysv64" fn(u64, u64, u64, u64) -> !,
    // The `lgdt` operand, argument three.
    gdtr: u64,
    // The 32-bit continuation, argument four.
    pmode32: u64,
}

/// Copy `leave_long_mode.s` into `base` and patch the one address it cannot know at link time.
fn copy_trampoline(base: u64) -> Result<Trampoline, &'static str> {
    let start = (&raw const x86_leave_long_mode).cast::<u8>();
    let end = (&raw const x86_leave_long_mode_end).cast::<u8>();
    let len = end as usize - start as usize;
    if len > PAGE as usize {
        return Err("the mode-switch trampoline no longer fits in one page");
    }

    let offset =
        |symbol: *const [u8; 0]| base + (symbol.cast::<u8>() as usize - start as usize) as u64;
    let gdt = offset(&raw const x86_leave_long_mode_gdt);
    let gdtr = offset(&raw const x86_leave_long_mode_gdtr);
    let pmode32 = offset(&raw const x86_leave_long_mode_pmode32);

    // SAFETY: the source is this image's own `.text`, the destination is a page the firmware just
    // granted exclusively, and the length was checked against the page above.
    unsafe { ptr::copy_nonoverlapping(start, base as *mut u8, len) };

    // The GDT pointer's base is the only field in the blob whose correct value depends on where the
    // copy landed. It sits two bytes into the descriptor, after the 16-bit limit, and is therefore
    // unaligned by construction: `lgdt` does not care, and `write_unaligned` is why this is not a
    // fault on the first boot.
    // SAFETY: `gdtr` is inside the page just written, and the 8 bytes at `gdtr + 2` are the base
    // field `leave_long_mode.s` reserved with `.quad 0`.
    unsafe { ptr::write_unaligned((gdtr as usize + 2) as *mut u64, gdt) };

    Ok(Trampoline {
        // SAFETY: `base` now holds the trampoline's first instruction, and the signature is the
        // register contract leave_long_mode.s documents.
        enter: unsafe {
            core::mem::transmute::<u64, extern "sysv64" fn(u64, u64, u64, u64) -> !>(base)
        },
        gdtr,
        pmode32,
    })
}

/// **Where the ACPI root pointer comes from on a real machine.**
///
/// The 2.0 entry is preferred because its RSDP is revision 2 or later and carries the 64-bit
/// `xsdt_address`; the 1.0 entry is the fallback, and `machine_discovery::acpi` reads whichever
/// root the revision says is there. This is what makes a UEFI boot hand the kernel a **non-zero**
/// `rsdp`, where QEMU's PVH loader leaves it zero and the kernel falls back to scanning the BIOS
/// area for `"RSD PTR "`.
fn find_rsdp(table: &SystemTable) -> Option<u64> {
    let mut fallback = None;
    for entry in crate::configuration_table(table) {
        if entry.vendor_guid == efi::ACPI_20_TABLE_GUID {
            return Some(entry.vendor_table as u64);
        }
        if entry.vendor_guid == efi::ACPI_10_TABLE_GUID {
            fallback = Some(entry.vendor_table as u64);
        }
    }
    fallback
}

/// **Ask the firmware where the screen is** (milestone 243): one `LocateProtocol` call and three
/// field reads. It does **not** set a video mode: the firmware already chose one that works on this
/// monitor, and a mode set here would have to be told to the kernel through the same channel
/// anyway.
///
/// `None` on every unhappy answer: no protocol, a null `mode` or `info`, a `PixelBltOnly` adapter
/// (no linear framebuffer, only a boot-services `Blt` call gone by the time the kernel runs), a
/// `PixelBitMask` adapter (masks `machine_discovery::framebuffer` cannot express), or a geometry
/// whose arithmetic does not close. A machine with no screen is the `OptiPlex`, and not an error.
fn find_screen(services: &BootServices) -> Option<Framebuffer> {
    let mut interface: *mut core::ffi::c_void = ptr::null_mut();
    let mut guid = graphics_output::PROTOCOL_GUID;
    // SAFETY: a boot service, before `ExitBootServices`. A null registration asks for the first
    // handle carrying the protocol; the firmware reads the GUID and writes only `interface`. On a
    // machine with two display adapters that is the firmware's choice of console rather than ours,
    // and this loader has no basis for a better one.
    let found = unsafe { (services.locate_protocol)(&mut guid, ptr::null_mut(), &mut interface) };
    if found != SUCCESS || interface.is_null() {
        return None;
    }
    // SAFETY: `LocateProtocol` returned success, so the firmware asserts this is an
    // `EFI_GRAPHICS_OUTPUT_PROTOCOL` and that it outlives boot services.
    let gop = unsafe { &*interface.cast::<graphics_output::Protocol>() };
    if gop.mode.is_null() {
        return None;
    }
    // SAFETY: as above; `Mode` is a required field of the protocol and non-null was just checked.
    let mode = unsafe { &*gop.mode };
    if mode.info.is_null() {
        return None;
    }
    // SAFETY: as above. Read unaligned is not needed: the firmware allocates this structure and the
    // specification gives it natural alignment.
    let info = unsafe { &*mode.info };
    // **`pixels_per_scan_line` is the stride, not `horizontal_resolution`.** Firmware may pad each
    // row, and a writer that multiplies by the width paints a picture that shears down the screen.
    let order = match info.pixel_format {
        graphics_output::PIXEL_BLUE_GREEN_RED_RESERVED_8_BIT_PER_COLOR => PixelOrder::Bgrx,
        graphics_output::PIXEL_RED_GREEN_BLUE_RESERVED_8_BIT_PER_COLOR => PixelOrder::Rgbx,
        _ => return None,
    };
    let found = Framebuffer {
        // **This address survives `ExitBootServices`**: it is a BAR aperture on the display
        // adapter, not firmware memory. What ends is the firmware's console, not the display.
        base: mode.frame_buffer_base,
        width: info.horizontal_resolution,
        height: info.vertical_resolution,
        // The firmware reports the stride in PIXELS and every consumer of it wants bytes. This is
        // the one multiplication in the whole path and getting it wrong shears the picture.
        stride: info.pixels_per_scan_line.checked_mul(4)?,
        order,
    };
    // The kernel's console indexes this with a `usize` computed from the geometry, so a geometry
    // whose span does not close is refused here rather than trusted there.
    found.span()?;
    // And the aperture the firmware reports has to actually hold what the geometry claims. A
    // firmware that reported a stride larger than its own framebuffer would have this loader
    // handing the kernel a licence to write past the end of a BAR.
    if found.span()? > mode.frame_buffer_size {
        return None;
    }
    Some(found)
}

/// Park the core. `hlt` rather than a bare spin for the reason AGENTS.md gives about `wfi`: a
/// halted core should not burn a real one. Under QEMU this is the difference between 0% and 100% of
/// a host thread.
pub fn park() -> ! {
    loop {
        // SAFETY: `hlt` at CPL 0, with no effect other than parking this core until an interrupt.
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    }
}
