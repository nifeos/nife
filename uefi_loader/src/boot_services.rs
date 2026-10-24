//! **The boot services this loader calls more than once, as safe functions.**
//!
//! `r-efi` declares every firmware function pointer `unsafe extern "efiapi" fn`, correctly: Rust
//! cannot see what the firmware does with its arguments. Each function here makes one such call
//! with arguments built inside it, so its safety argument is written once rather than at every
//! caller. One condition is not in the types: a [`BootServices`] table is only callable until
//! `ExitBootServices` succeeds. Every caller holds it only in the code that runs before that call.
//!
//! Name: provisional (milestone 784 (reuse what the tree wrote), 2026-10-20 UTC). It names the
//! UEFI table these functions call through.

use core::ffi::c_void;
use core::ptr;

use r_efi::efi::{self, AllocateType, BootServices, MemoryType};

use crate::SUCCESS;

/// The memory map's current size in bytes and the firmware's descriptor stride, from a sizing
/// call with no buffer, or `None` when the firmware reports a map this loader cannot read.
///
/// **Never stride the map by `size_of::<MemoryDescriptor>()`.** `GetMemoryMap` reports its own
/// `descriptor_size`, which the specification lets be larger than the structure and real firmware
/// does make larger; every walk here strides by the size this returns, and a size smaller than the
/// structure is refused. The sizing call is expected to fail with `EFI_BUFFER_TOO_SMALL`, and the
/// size is checked rather than the status: success for a zero-length buffer would be an empty
/// machine.
pub(crate) fn memory_map_size(services: &BootServices) -> Option<(usize, usize)> {
    let mut bytes = 0usize;
    let mut key = 0usize;
    let mut descriptor_size = 0usize;
    let mut version = 0u32;
    // SAFETY: a boot service (see this module's header). A null buffer asks only for the size,
    // which the firmware writes with the key, stride and version into these locals.
    unsafe {
        (services.get_memory_map)(
            &mut bytes,
            ptr::null_mut(),
            &mut key,
            &mut descriptor_size,
            &mut version,
        )
    };
    (bytes != 0 && descriptor_size >= size_of::<efi::MemoryDescriptor>())
        .then_some((bytes, descriptor_size))
}

/// What a filled memory map says about itself.
pub(crate) struct MapRead {
    /// Bytes of descriptors written.
    pub(crate) bytes: usize,
    /// The key `ExitBootServices` must be handed, valid until anything allocates.
    pub(crate) key: usize,
    /// The stride between descriptors. See [`memory_map_size`].
    pub(crate) descriptor_size: usize,
    /// `EFI_MEMORY_DESCRIPTOR_VERSION` as the firmware reports it.
    pub(crate) version: u32,
}

/// Fill `capacity` bytes at `buffer` with the memory map, or `None` when it does not fit.
///
/// # Safety
/// `buffer` must be valid for writes of `capacity` bytes and not otherwise in use.
pub(crate) unsafe fn memory_map(
    services: &BootServices,
    buffer: *mut c_void,
    capacity: usize,
) -> Option<MapRead> {
    let mut read = MapRead {
        bytes: capacity,
        key: 0,
        descriptor_size: 0,
        version: 0,
    };
    // SAFETY: a boot service (see this module's header); `buffer` holds `capacity` bytes by the
    // caller's contract, and `read.bytes` says so.
    let status = unsafe {
        (services.get_memory_map)(
            &mut read.bytes,
            buffer.cast(),
            &mut read.key,
            &mut read.descriptor_size,
            &mut read.version,
        )
    };
    (status == SUCCESS).then_some(read)
}

/// `bytes` from the firmware's pool, as `LOADER_DATA`, or `None` when it refuses.
pub(crate) fn allocate_pool(services: &BootServices, bytes: usize) -> Option<*mut c_void> {
    let mut buffer: *mut c_void = ptr::null_mut();
    // SAFETY: a boot service (see this module's header); it writes only `buffer`.
    let status = unsafe { (services.allocate_pool)(efi::LOADER_DATA, bytes, &mut buffer) };
    (status == SUCCESS && !buffer.is_null()).then_some(buffer)
}

/// Hand a pool buffer back.
///
/// # Safety
/// `buffer` must have come from [`allocate_pool`] and must not be used afterwards.
pub(crate) unsafe fn free_pool(services: &BootServices, buffer: *mut c_void) {
    // SAFETY: a boot service (see this module's header), with a buffer the caller's contract says
    // is the firmware's own pool allocation and no longer in use.
    unsafe { (services.free_pool)(buffer) };
}

/// `AllocatePages`: with `ALLOCATE_ADDRESS`, exactly `base`; with `ALLOCATE_MAX_ADDRESS`, at or below
/// it. The base the firmware granted, or `None` when it refuses.
pub(crate) fn allocate_pages(
    services: &BootServices,
    how: AllocateType,
    kind: MemoryType,
    pages: usize,
    base: u64,
) -> Option<u64> {
    let mut base = base;
    // SAFETY: a boot service (see this module's header). **`base` is in/out**: the address or the
    // ceiling going in, the allocated base coming out, and nothing else is written.
    let status = unsafe { (services.allocate_pages)(how, kind, pages, &mut base) };
    (status == SUCCESS).then_some(base)
}

/// `HandleProtocol`: the interface `handle` carries for `guid`, or `None` when it carries none.
/// Only the x86_64 loader asks a handle for a protocol: it reads its own file and its disks.
#[cfg(target_arch = "x86_64")]
pub(crate) fn handle_protocol(
    services: &BootServices,
    handle: efi::Handle,
    guid: efi::Guid,
) -> Option<*mut c_void> {
    let mut guid = guid;
    let mut interface: *mut c_void = ptr::null_mut();
    // SAFETY: a boot service (see this module's header). The firmware reads the GUID, though the
    // signature says `*mut`, and writes only `interface`; a handle it does not know is an error
    // status, not undefined behavior.
    let status = unsafe { (services.handle_protocol)(handle, &mut guid, &mut interface) };
    (status == SUCCESS && !interface.is_null()).then_some(interface)
}
