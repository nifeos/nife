//! **riscv64: the firmware's device tree, the boot hart, and the Linux RISC-V boot state.**
//!
//! The kernel's entry contract is the one OpenSBI's jump and U-Boot's `booti` meet
//! (`kernel/src/arch/riscv64/boot.s`): S-mode, **paging off**, `a0` = this hart's id, `a1` = the
//! physical address of the device tree. UEFI runs this loader in S-mode too, possibly with paging
//! on (an identity map), and it is the only one of the three architectures where the kernel needs
//! a number the firmware must be asked for: which hart it is running on. UEFI answers that with
//! `RISCV_EFI_BOOT_PROTOCOL` (read from EDK2's header, [`RiscvBootProtocol`]); a firmware
//! without it is asked through the device tree's `/chosen/boot-hartid`, which U-Boot writes.
//!
//! **There is no `riscv64` UEFI target in rustc** (`rustc --print target-list`, 2026-09-19, lists
//! `aarch64-`, `i686-` and `x86_64-unknown-uefi` only). This module is therefore compiled for
//! `riscv64imac-unknown-none-elf` as a position-independent executable, and `cargo xtask` turns
//! the ELF into a PE/COFF image (`xtask/src/portable_executable.rs`); see notes/boot-stick.md.
//!
//! # BUGS
//!
//! - **The kernel is linked for `0x8020_0000`** (`kernel/link-riscv64.ld`), which is QEMU `virt`'s
//!   RAM and, by the `booti` header's deliberate `text_offset`, radon's. A board whose RAM does not
//!   contain that address cannot take this payload, and the loader says so with the memory map.
//! - **Everything the kernel reads early must be below 3 GiB** ([`ALLOCATION_CEILING`]): the boot
//!   table maps the first three gigapages and nothing above.

use r_efi::efi::{self, BootServices, Handle, Status, SystemTable};

use crate::{
    Placed, SUCCESS, configuration_table, copy_device_tree, exit_boot_services, say, say_decimal,
};

/// **`RISCV_EFI_BOOT_PROTOCOL`** (`gRiscVEfiBootProtocolGuid`, read from EDK2's
/// `UefiCpuPkg/UefiCpuPkg.dec`, 2026-09-19), which answers the one question RISC-V has that the
/// other two architectures do not: which hart is this. The kernel's entry contract wants it in
/// `a0`, and nothing a UEFI application runs in tells it otherwise.
///
/// Written here because `r-efi` 7.1.0 does not define it: it is RISC-V's own protocol, not in the
/// UEFI specification proper.
const RISCV_BOOT_PROTOCOL_GUID: efi::Guid = efi::Guid::from_fields(
    0xccd1_5fec,
    0x6f73,
    0x4eec,
    0x83,
    0x95,
    &[0x3e, 0x69, 0xe4, 0xb9, 0x40, 0xbf],
);

/// The protocol's interface: a revision and one function (`RiscVBootProtocol.h` in EDK2).
#[repr(C)]
struct RiscvBootProtocol {
    /// `RISCV_EFI_BOOT_PROTOCOL_REVISION`, 0x00010000 for the first.
    revision: u64,
    /// `GetBootHartId(this, &mut hart)`.
    get_boot_hart_id: unsafe extern "efiapi" fn(*mut RiscvBootProtocol, *mut usize) -> Status,
}

core::arch::global_asm!(include_str!("leave_uefi.s"));

unsafe extern "C" {
    /// `leave_uefi.s`: paging off, then `a0` = `hart`, `a1` = `device_tree`, jump.
    fn riscv64_leave_uefi(entry: u64, hart: u64, device_tree: u64) -> !;
}

/// **The ceiling on everything the kernel reads before its own page tables are up.** The boot table
/// in `kernel/src/arch/riscv64/mmu.rs` identity-maps gigapages 0, 1 and 2, so the last byte it
/// reaches is `0xbfff_ffff`.
pub(crate) const ALLOCATION_CEILING: u64 = 0xbfff_ffff;

/// The firmware's device tree and the boot hart, found before anything moves.
pub struct Found {
    device_tree: *const u8,
    hart: u64,
}

/// Find the device tree and ask which hart this is.
pub fn discover(table: &SystemTable, services: &BootServices) -> Result<Found, &'static str> {
    let device_tree: *const u8 = configuration_table(table)
        .iter()
        .find(|entry| entry.vendor_guid == efi::DTB_TABLE_GUID)
        .map(|entry| entry.vendor_table.cast())
        .ok_or("the firmware offers no device tree")?;

    // Which source answered is printed, because on a board it is the first thing to know when the
    // kernel comes up on the wrong hart: U-Boot has offered each of these at different versions.
    let hart = if let Some(hart) = boot_hart_from_protocol(services) {
        say(
            table,
            "uefi_loader: boot hart from RISCV_EFI_BOOT_PROTOCOL: ",
        );
        hart
    } else if let Some(hart) = boot_hart_from_device_tree(device_tree) {
        say(table, "uefi_loader: boot hart from /chosen/boot-hartid: ");
        hart
    } else {
        return Err(
            "the firmware says neither through RISCV_EFI_BOOT_PROTOCOL nor /chosen/boot-hartid \
             which hart this is",
        );
    };
    say_decimal(table, hart as u32);
    say(table, "\r\n");
    Ok(Found { device_tree, hart })
}

fn boot_hart_from_protocol(services: &BootServices) -> Option<u64> {
    let mut interface: *mut core::ffi::c_void = core::ptr::null_mut();
    let mut guid = RISCV_BOOT_PROTOCOL_GUID;
    // SAFETY: a boot service, before `ExitBootServices`. A null registration asks for the first
    // handle carrying the protocol; the firmware reads the GUID and writes only `interface`.
    let found =
        unsafe { (services.locate_protocol)(&mut guid, core::ptr::null_mut(), &mut interface) };
    if found != SUCCESS || interface.is_null() {
        return None;
    }
    let protocol = interface.cast::<RiscvBootProtocol>();
    let mut hart = 0usize;
    // SAFETY: `LocateProtocol` succeeded, so the firmware asserts `protocol` is this interface.
    let status = unsafe { ((*protocol).get_boot_hart_id)(protocol, &mut hart) };
    (status == SUCCESS).then_some(hart as u64)
}

fn boot_hart_from_device_tree(device_tree: *const u8) -> Option<u64> {
    // SAFETY: the firmware installed this table; the header states its length.
    let header = unsafe { core::slice::from_raw_parts(device_tree, 8) };
    let total = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
    // The same ceiling `DeviceTreeBlob::from_ptr` applies, for the same reason: the header's
    // length is believed before anything is checked, and a lying one must not become a slice.
    if total > device_tree_blob::MAX_TOTALSIZE {
        return None;
    }
    // SAFETY: as above, and `total` is now bounded.
    let bytes = unsafe { core::slice::from_raw_parts(device_tree, total) };
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(bytes).ok()?;
    let value = tree.node_prop(b"chosen", b"boot-hartid").ok()??;
    match value.len() {
        4 => Some(u64::from(u32::from_be_bytes(value.try_into().ok()?))),
        8 => Some(u64::from_be_bytes(value.try_into().ok()?)),
        _ => None,
    }
}

/// Exit boot services and enter the kernel. Returns only on failure, with a console.
// The argument list is the x86_64 `hand_over`'s, kept in step on purpose: one signature per arch
// arm of the same seam, with the parameters this architecture does not use yet carried under
// their own names (see the two `None`s below).
#[allow(clippy::too_many_arguments)]
pub fn hand_over(
    handle: Handle,
    table: &SystemTable,
    services: &BootServices,
    found: Found,
    kernel: &Placed,
    module: Option<(u64, u64)>,
    // **Always `None` here**, and that is a recorded gap rather than an unused parameter.
    // `main.rs`'s `place_boot_file` reads this loader's own file so the running system can install
    // itself, and hands it over as a second PVH module. A device-tree handoff has `/chosen` with
    // one initial ramdisk and no second slot, so there is nowhere to put it; see that function's
    // BUGS and milestone 198 (a package manager, and the trivial install that makes a second
    // customer possible)'s rung 2a, which is an x86_64 claim for this reason.
    _boot_file: Option<(u64, u64)>,
    // **Always `None` here**, and that is a scope note (DECISIONS
    // §19 (architectural parity is a tenet; the targets are aarch64, riscv64, and x86_64)) rather than an unused parameter. The chooser that would set it is
    // `uefi_loader::chooser`, which is x86_64 only for the same reason the install offer is: an
    // installed machine on this architecture is what rung 2b of milestone 198 built, and the
    // device-tree architectures have no installed disk to choose slots on yet. When they do, this
    // parameter is where the number arrives.
    _from_slot: Option<u8>,
    // The boot medium token (`boot_slot::medium`) is x86_64 only, like the chooser.
    _on_nvme: bool,
) -> Result<(), &'static str> {
    if kernel.end > ALLOCATION_CEILING + 1 {
        return Err("the kernel is linked above what its own boot table reaches");
    }
    let device_tree = copy_device_tree(services, found.device_tree, module)?;
    say(
        table,
        "uefi_loader: kernel placed, exiting boot services\r\n",
    );
    exit_boot_services(handle, services)?;
    // SAFETY: the kernel, the archive and the device tree are placed below the ceiling, and
    // `leave_uefi.s` documents the state it leaves, which is the one OpenSBI's jump produces.
    unsafe { riscv64_leave_uefi(kernel.entry, found.hart, device_tree) }
}

/// Park the hart with `wfi`, which QEMU turns into a real host-thread sleep (AGENTS.md).
pub fn park() -> ! {
    loop {
        // SAFETY: waits for an interrupt; no other effect.
        unsafe { core::arch::asm!("wfi", options(nomem, nostack)) };
    }
}
