//! **Where the kernel, the archive and the boot file land in physical memory**, and the
//! allocation below the ceiling every placement uses.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The kernel, placed: where to enter it and the physical range it occupies.
pub struct Placed {
    /// The physical address of `_start` ([`image::physical_entry`]).
    pub entry: u64,
    /// The first byte of the placed image, page-aligned.
    pub start: u64,
    /// One past the last, page-aligned.
    pub end: u64,
}

/// Place the embedded kernel by its physical addresses, zeroed first so every `.bss` and `NOLOAD`
/// section arrives zeroed.
pub(crate) fn place_kernel(
    table: &SystemTable,
    services: &BootServices,
) -> Result<Placed, &'static str> {
    // `elf::Elf::parse` validates everything before this loader moves a byte: bounds, overlap,
    // W^X, the entry point inside an executable segment.
    let kernel = image::parse(embedded::KERNEL)?;
    let (start, end) = image::physical_span(kernel.segments(), PAGE)
        .ok_or("the embedded kernel has no loadable segments")?;
    let entry = image::physical_entry(kernel.entry(), kernel.segments())
        .ok_or("the embedded kernel's entry point is in no segment")?;

    // `AllocateAddress`, not `AllocateMaxAddress`: the kernel is linked for exactly this range
    // (`kernel/link-<arch>.ld`'s `PHYS_START`) and there is nowhere else to put it. A firmware that
    // has something of its own there fails HERE, with the range and the offending descriptors
    // printed, rather than by silently overwriting whatever it was.
    let pages = ((end - start) / PAGE) as usize;
    let granted =
        boot_services::allocate_pages(services, ALLOCATE_ADDRESS, efi::LOADER_DATA, pages, start);
    if granted.is_none() {
        // **Name what is in the way before giving up** (milestone 195). "The firmware said no" is
        // the least actionable sentence a person standing at a machine can be handed.
        say_span(table, "uefi_loader: wanted ", start, end);
        say_conflict(table, services, start, end);
        return Err("the firmware would not give up the kernel's load range");
    }

    // SAFETY: the firmware just granted this whole range exclusively.
    unsafe { ptr::write_bytes(start as *mut u8, 0, (end - start) as usize) };

    // `Segment::data` is `p_filesz` bytes, which is empty for a `NOLOAD` section: they arrive as
    // address space to reserve rather than bytes to copy, and the zeroing above is what makes their
    // memory correct.
    for segment in kernel.segments() {
        // SAFETY: `physical_span` covers every segment, and the range it covers was just allocated;
        // the source is a slice of this binary's own `.rodata`. They cannot overlap: the source is
        // in the firmware's allocation for this image and the destination is a range the firmware
        // handed out separately.
        unsafe {
            ptr::copy_nonoverlapping(
                segment.data.as_ptr(),
                segment.paddr as *mut u8,
                segment.data.len(),
            );
        }
    }
    Ok(Placed { entry, start, end })
}

/// Copy the archive to memory below the architecture's ceiling, if this build has one.
pub(crate) fn place_archive(services: &BootServices) -> Result<Option<(u64, u64)>, &'static str> {
    if embedded::INITIAL_RAMDISK.is_empty() {
        return Ok(None);
    }
    let pages = (embedded::INITIAL_RAMDISK.len() as u64).div_ceil(PAGE) as usize;
    let base = allocate_below(services, pages, efi::LOADER_DATA)
        .ok_or("no memory below the kernel's reach for the userspace archive")?;
    // SAFETY: `pages` covers the archive and the range was just granted exclusively.
    unsafe {
        ptr::copy_nonoverlapping(
            embedded::INITIAL_RAMDISK.as_ptr(),
            base as *mut u8,
            embedded::INITIAL_RAMDISK.len(),
        );
    }
    Ok(Some((base, embedded::INITIAL_RAMDISK.len() as u64)))
}

/// **The removable-media path the firmware found this image at**, which is the path its own file is
/// still at.
///
/// One constant rather than a decoded device path; notes/x86-uefi-boot/loader-design.md records the
/// trade.
#[cfg(target_arch = "x86_64")]
pub(crate) const BOOT_FILE_PATH: &str = "\\EFI\\BOOT\\BOOTX64.EFI";

/// The largest boot file this loader will read back, and the size of one install boot slot (calef,
/// 2026-10-03 UTC: "keep 64 MiB with both fixes"). The two fixes: an install image is always a
/// release build with a stripped kernel (see `image::carries_debug_info`), and CI budgets the
/// release `BOOTX64.EFI` at a quarter of this (`bench/boot-file-size-x86_64.txt`, checked by
/// `script/boot-file-size-check`). The debug tour build is about 15 MiB and the test build about
/// 19 MiB, so the 64 MiB ceiling exists to refuse something that is not this file at all, rather
/// than spend the machine's RAM on it.
#[cfg(target_arch = "x86_64")]
pub(crate) const BOOT_FILE_MAX: u64 = 64 * 1024 * 1024;

/// **Seek here and the file protocol puts the position at the end**, which is the specification's
/// own way of asking a file how long it is without decoding an `EFI_FILE_INFO`. `r-efi` names no
/// constant for it.
#[cfg(target_arch = "x86_64")]
const FILE_POSITION_END: u64 = u64::MAX;

/// **Read this loader's own file off its volume and place it for the kernel** as a second module
/// (milestone 198 (a package manager), rung 2a): the running system has no other copy, and the
/// archive cannot carry the file that contains it. So kernel and archive travel as the one file
/// that seals them (`build.rs`'s `refuse_an_unsealed_pair`). A failure costs the ability to
/// install, not the boot: a `None` says so and carries on (notes/x86-uefi-boot/loader-design.md).
///
/// # BUGS
/// - **The path is a constant**, not `loaded_image::Protocol::file_path`: the fallback path only.
/// - **x86_64 only**: a device tree's `/chosen` has one initial ramdisk and no second module.
/// - **The file is read whole into RAM**, about 9 MiB for the tour build.
#[cfg(target_arch = "x86_64")]
pub(crate) fn place_boot_file(
    handle: Handle,
    table: &SystemTable,
    services: &BootServices,
) -> Option<(u64, u64)> {
    let Some(interface) =
        boot_services::handle_protocol(services, handle, loaded_image::PROTOCOL_GUID)
    else {
        say(
            table,
            "uefi_loader: no loaded-image protocol; cannot install\r\n",
        );
        return None;
    };
    // SAFETY: the firmware answered `HandleProtocol` with a pointer to its own
    // `EFI_LOADED_IMAGE_PROTOCOL` for this image handle, which outlives boot services.
    let loaded = unsafe { &*interface.cast::<loaded_image::Protocol>() };
    // `device_handle` is read and `image_base`/`image_size` deliberately are not, though they look
    // like the easy answer. They describe the image as *loaded*: sections placed at their RVAs, with
    // section alignment between them. Writing those bytes to a disk would produce something that is
    // not this file and that no firmware promises to start.

    let Some(interface) = boot_services::handle_protocol(
        services,
        loaded.device_handle,
        simple_file_system::PROTOCOL_GUID,
    ) else {
        say(
            table,
            "uefi_loader: the boot volume has no filesystem protocol; cannot install\r\n",
        );
        return None;
    };
    // SAFETY: as above, now for the volume this image came from.
    let filesystem = interface.cast::<simple_file_system::Protocol>();

    let mut root: *mut file::Protocol = ptr::null_mut();
    // SAFETY: `filesystem` is the firmware's own protocol interface, and `open_volume` is its
    // first method.
    if unsafe { ((*filesystem).open_volume)(filesystem, &mut root) } != SUCCESS || root.is_null() {
        say(table, "uefi_loader: could not open the boot volume\r\n");
        return None;
    }

    let mut path = [0u16; 64];
    let units = utf16(BOOT_FILE_PATH, &mut path);
    let mut file: *mut file::Protocol = ptr::null_mut();
    // SAFETY: `root` is the firmware's open volume; `path` is NUL-terminated within its own buffer,
    // and `Open` only reads it though its signature says `*mut`.
    let opened = unsafe {
        ((*root).open)(
            root,
            &mut file,
            path[..units].as_mut_ptr(),
            file::MODE_READ,
            0,
        )
    };
    if opened != SUCCESS || file.is_null() {
        say(table, "uefi_loader: no ");
        say(table, BOOT_FILE_PATH);
        say(table, " on the boot volume; cannot install\r\n");
        // SAFETY: `root` is open and `close` is its own method.
        unsafe { ((*root).close)(root) };
        return None;
    }

    // Seek to the end and ask where that was, which is the specification's own way of asking a
    // file how long it is without decoding an `EFI_FILE_INFO` and its variable-length name.
    let mut length = 0u64;
    // SAFETY: `file` is open; both calls are its own methods and `length` is ours.
    let sized = unsafe {
        ((*file).set_position)(file, FILE_POSITION_END) == SUCCESS
            && ((*file).get_position)(file, &mut length) == SUCCESS
            && ((*file).set_position)(file, 0) == SUCCESS
    };
    if !sized || length == 0 || length > BOOT_FILE_MAX {
        say(table, "uefi_loader: could not size the boot file\r\n");
        // SAFETY: both handles are open.
        unsafe {
            ((*file).close)(file);
            ((*root).close)(root);
        }
        return None;
    }

    let pages = length.div_ceil(PAGE) as usize;
    let Some(base) = allocate_below(services, pages, efi::LOADER_DATA) else {
        say(table, "uefi_loader: no memory for the boot file\r\n");
        // SAFETY: both handles are open.
        unsafe {
            ((*file).close)(file);
            ((*root).close)(root);
        }
        return None;
    };

    // One call is allowed to return short, so read until the file is in or the firmware stops
    // making progress. A silent short read here would put a truncated boot file on somebody's disk.
    let mut done = 0u64;
    while done < length {
        let mut want = (length - done) as usize;
        // SAFETY: `pages` covers `length` bytes at `base`, granted exclusively above, and `done` is
        // below `length`.
        let status =
            unsafe { ((*file).read)(file, &mut want, (base + done) as *mut core::ffi::c_void) };
        if status != SUCCESS || want == 0 {
            break;
        }
        done += want as u64;
    }
    // SAFETY: both handles are open and `close` is each one's own method.
    unsafe {
        ((*file).close)(file);
        ((*root).close)(root);
    }
    if done != length {
        say(
            table,
            "uefi_loader: the boot file read short; cannot install\r\n",
        );
        return None;
    }

    say_span(table, "uefi_loader: boot file at ", base, base + length);
    Some((base, length))
}

/// `AllocatePages(AllocateMaxAddress)` under [`arch::ALLOCATION_CEILING`], which is the highest
/// physical address the kernel can reach before its own page tables are up.
pub(crate) fn allocate_below(services: &BootServices, pages: usize, kind: u32) -> Option<u64> {
    boot_services::allocate_pages(
        services,
        ALLOCATE_MAX_ADDRESS,
        kind,
        pages,
        arch::ALLOCATION_CEILING,
    )
}
