//! **Which devices this driver drives**: the Intel vendor ID, the device IDs it knows, and the
//! model name each one reports.

/// Intel's PCI vendor id.
pub const VENDOR_INTEL: u16 = 0x8086;

/// The device ids this driver claims, with what each is. Read from Linux's `hw.h` on 2026-10-04.
///
/// **Deliberately narrow.** The 82574L is what QEMU's `-device e1000e` presents, and proves the
/// sequence. The I219 entries are the Sunrise Point (SPT) generation only, which is what an `OptiPlex`
/// 7050 carries (Dell's specification sheet: "Integrated Intel i219-LM"). xenon's PCI survey
/// settled which: `00:1f.6 8086:15e3`, the I219-LM5 (bench/xenon-2026-10-04/). The other SPT rows
/// stay because FreeBSD runs one sequence for the whole class. Later PCH generations (Cannon Lake onward,
/// dozens of ids) need the bench to prove one before the family is widened, because the
/// generation-specific workarounds are exactly what this crate does not do (`BUGS`).
pub const DEVICE_IDS: &[(u16, &str)] = &[
    (0x10d3, "82574L (QEMU's e1000e)"),
    (0x156f, "I219-LM (SPT)"),
    (0x1570, "I219-V (SPT)"),
    (0x15b7, "I219-LM2 (SPT-H)"),
    (0x15b8, "I219-V2 (SPT-H)"),
    (0x15b9, "I219-LM3 (LBG)"),
    (0x15d7, "I219-LM4 (SPT)"),
    (0x15d8, "I219-V4 (SPT)"),
    (0x15e3, "I219-LM5 (SPT)"),
    (0x15d6, "I219-V5 (SPT)"),
];

/// Is `vendor:device` a part this driver claims? See [`DEVICE_IDS`] for why the list is short.
pub fn is_supported(vendor: u16, device: u16) -> bool {
    vendor == VENDOR_INTEL && DEVICE_IDS.iter().any(|&(id, _)| id == device)
}

/// What a claimed device id is, for a boot line; `None` for one this driver does not claim.
pub fn model(device: u16) -> Option<&'static str> {
    DEVICE_IDS
        .iter()
        .find(|&&(id, _)| id == device)
        .map(|&(_, m)| m)
}
