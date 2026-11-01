//! **Everything between "the firmware started us" and "the kernel is running"**: the entry
//! point, the load in the one order that works, the device tree handed on, and leaving boot
//! services.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Where the firmware starts us. The name is UEFI's default entry point for the UEFI targets, and
/// the one `cargo xtask`'s riscv64 build names as the image's entry.
#[unsafe(no_mangle)]
pub(crate) extern "efiapi" fn efi_main(handle: Handle, system_table: *mut SystemTable) -> Status {
    // SAFETY: the firmware's own contract for an EFI application entry point: it passes a valid
    // system table that outlives the call, and `boot_services` is non-null until
    // `ExitBootServices`.
    let table = unsafe { &*system_table };
    // SAFETY: as above.
    let services = unsafe { &*table.boot_services };

    say(table, "nife uefi_loader: milestone 87\r\n");

    // A UEFI error status, so the firmware reports the failure and moves to the next boot option
    // rather than presenting a blank screen.
    const LOAD_ERROR: Status = Status::LOAD_ERROR;

    let Err(reason) = load(handle, table, services) else {
        // `load` hands the machine to the kernel and never comes back, so this cannot run.
        return LOAD_ERROR;
    };
    say(table, "uefi_loader: ");
    say(table, reason);
    say(table, "\r\n");
    LOAD_ERROR
}

/// Everything between "the firmware started us" and "the kernel is running", in the one order that
/// works. Returns only on failure, and only while there is still a console to say so on.
pub(crate) fn load(
    handle: Handle,
    table: &SystemTable,
    services: &BootServices,
) -> Result<(), &'static str> {
    // --- 0. Which image should be running at all ---
    //
    // **Before anything is placed**, because the chooser may hand the machine to a different copy
    // of this binary, and that copy needs the kernel's fixed physical range free. An installed
    // machine whose slots are all unbootable, a stick, and a `-kernel` boot all fall through this
    // and boot the image in this file, which is the answer to "what if both slots are bad".
    #[cfg(target_arch = "x86_64")]
    let from_slot = chooser::started_from_slot(handle, services);
    #[cfg(target_arch = "x86_64")]
    match from_slot {
        Some(slot) => {
            // A chooser started us. Say which slot, and do not become a chooser in turn.
            say(table, "uefi_loader: started from boot slot ");
            say_decimal(table, u32::from(slot));
            say(table, "\r\n");
        }
        None => chooser::choose(handle, table, services),
    }

    // --- 1. What this architecture needs from the firmware, read before anything moves ---
    let found = arch::discover(table, services)?;

    // --- 2. The kernel, at the physical addresses its linker script chose ---
    let kernel = place_kernel(table, services)?;

    // --- 3. The userspace archive, if this build has one ---
    let module = place_archive(services)?;

    // --- 4. This file's own bytes, so the running system can install itself ---
    //
    // An image a chooser started has no file to read back: `LoadImage` from a buffer leaves the
    // child's device handle null. That narrows an installed machine to not installing itself onto
    // a second disk, which is `chooser`'s BUGS, and it is also what keeps an installed machine
    // from offering to wipe itself once per boot.
    #[cfg(target_arch = "x86_64")]
    let boot_file = if from_slot.is_some() {
        None
    } else if image::carries_debug_info(embedded::KERNEL) {
        // **A debug image cannot be installed**, because the wrong state is not expressible: with
        // no boot file the kernel offers no install at all (calef, 2026-10-03 UTC, the install boot
        // slots). A slot is a fixed 64 MiB and the debug kernel alone is 5.8 MB of DWARF. The
        // release path (`cargo xtask stick`, `install-boot`) strips the kernel before embedding it.
        say(
            table,
            "uefi_loader: this kernel carries debug info; a debug image is not offered for install\r\n",
        );
        None
    } else {
        place_boot_file(handle, table, services)
    };
    #[cfg(not(target_arch = "x86_64"))]
    let boot_file = None;

    // The slot number, on the one architecture that has a chooser to produce it. Carried across
    // the handoff so a booted system can say the boot worked, which is the half of the two-slot
    // rollback that rung 2b left unbuilt: see `boot_slot::cmdline`.
    #[cfg(not(target_arch = "x86_64"))]
    let from_slot: Option<u8> = None;

    // Whether the kernel may mount the internal disk: only when this boot came from it. See
    // `chooser::booted_from_nvme`.
    #[cfg(target_arch = "x86_64")]
    let on_nvme = chooser::booted_from_nvme(handle, services, from_slot);
    #[cfg(not(target_arch = "x86_64"))]
    let on_nvme = false;

    // --- 5. The architecture's handover, which ends in the kernel or in an error ---
    arch::hand_over(
        handle, table, services, found, &kernel, module, boot_file, from_slot, on_nvme,
    )
}

/// The firmware's configuration table as a slice.
pub(crate) fn configuration_table(table: &SystemTable) -> &[ConfigurationTable] {
    if table.configuration_table.is_null() {
        return &[];
    }
    // SAFETY: the firmware promises `number_of_table_entries` valid entries at
    // `configuration_table`, and they outlive boot services.
    unsafe { core::slice::from_raw_parts(table.configuration_table, table.number_of_table_entries) }
}

/// **Copy the firmware's device tree below the ceiling, naming the archive in `/chosen`**, and
/// putting the loader's two digests in `/chosen/bootargs` beside it (`embedded::LOADER_DIGESTS`,
/// milestone 695 (the kernel trusts two digests handed to it by the loader)). aarch64 and riscv64
/// both do this, identically.
#[cfg(not(target_arch = "x86_64"))]
pub(crate) fn copy_device_tree(
    services: &BootServices,
    source: *const u8,
    module: Option<(u64, u64)>,
) -> Result<u64, &'static str> {
    // SAFETY: the firmware installed this table and it outlives boot services; the header's first
    // eight bytes are magic and total size, and the full length is read from them before the rest.
    let header = unsafe { core::slice::from_raw_parts(source, 8) };
    let total = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
    // SAFETY: as above, now for the length the header states.
    let src = unsafe { core::slice::from_raw_parts(source, total) };
    let words = crate::embedded::LOADER_DIGESTS.as_bytes();
    let capacity = device_tree_patch::output_len_with_boot_arguments(src, words)
        .map_err(device_tree_patch::Error::reason)?;
    let pages = (capacity as u64).div_ceil(PAGE) as usize;
    let base = allocate_below(services, pages, efi::LOADER_DATA)
        .ok_or("no memory below the kernel's reach for the device tree")?;
    // SAFETY: `pages` pages at `base` were just granted exclusively.
    let out = unsafe { core::slice::from_raw_parts_mut(base as *mut u8, pages * PAGE as usize) };
    match module {
        Some((start, len)) => {
            device_tree_patch::with_initial_ramdisk_and_boot_arguments(
                src,
                start,
                start + len,
                words,
                out,
            )
            .map_err(device_tree_patch::Error::reason)?;
        }
        None => out[..total].copy_from_slice(src),
    }
    Ok(base)
}

/// **Exit boot services when the caller has nothing to read from the final map.**
///
/// The device-tree architectures hand the kernel the firmware's tree, not its memory map, so the
/// map is fetched only because `ExitBootServices` needs its key. One pool allocation, sized with
/// slack, then `GetMemoryMap` and `ExitBootServices` back to back. The specification allows one
/// retry after a refused key (the firmware may have changed the map by timer activity), and this
/// takes it.
#[cfg(not(target_arch = "x86_64"))]
pub(crate) fn exit_boot_services(
    handle: Handle,
    services: &BootServices,
) -> Result<(), &'static str> {
    let (bytes, descriptor_size) =
        boot_services::memory_map_size(services).ok_or("the firmware reported no memory map")?;
    let capacity = bytes + MAP_SLACK_DESCRIPTORS * descriptor_size;
    let buffer = boot_services::allocate_pool(services, capacity)
        .ok_or("no memory for the firmware's memory map")?;
    for _ in 0..2 {
        // SAFETY: `buffer` is `capacity` bytes from the firmware's pool, used for nothing else.
        let map = unsafe { boot_services::memory_map(services, buffer, capacity) }
            .ok_or("the firmware's memory map grew between the two GetMemoryMap calls")?;
        // SAFETY: `handle` is this image's, and `map.key` is from the `GetMemoryMap` just above
        // with nothing allocated since, which is the specification's condition for this call.
        if unsafe { (services.exit_boot_services)(handle, map.key) } == SUCCESS {
            return Ok(());
        }
    }
    Err("ExitBootServices refused the map key twice")
}
