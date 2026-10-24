//! What an xHCI's ports read, and when: an instrument behind the `xhci_port_probe` feature.
//!
//! [`before_handoff`] prints the controller and every `PORTSC` as the firmware left them, and
//! [`after_report`] every `PORTSC` after the driver's scan, then each port that moved, out to five
//! seconds. It only reads, and stalls the boot about five seconds. See notes/bench-boots.md.
//!
//! Name: provisional, minted 2026-10-21 (UTC) by maintainer/xenon-usb-no-ports.

use core::sync::atomic::{AtomicU64, Ordering};

use extensible_host_controller_interface::{Capabilities, regs};

/// Where the controller's registers are mapped, from [`before_handoff`] to [`after_report`]; 0
/// for no controller.
static REGS_VA: AtomicU64 = AtomicU64::new(0);

/// Extended capability id: Supported Protocol (xHCI 1.2 section 7.2).
const EXTENDED_SUPPORTED_PROTOCOL: u8 = 2;

/// Milliseconds after the report at which the ports are read again.
const SAMPLES_MS: [u64; 9] = [1, 10, 50, 100, 250, 500, 1000, 2000, 5000];

fn read(regs_va: u64, off: u64) -> u32 {
    // SAFETY: the caller's offsets are capability, operational, runtime or extended-capability
    // registers inside BAR0, which `mmu::map_everything` maps as device memory.
    unsafe { core::ptr::read_volatile((regs_va + off) as *const u32) }
}

fn capabilities_of(regs_va: u64) -> Capabilities {
    let r = |off| read(regs_va, off);
    Capabilities::decode(
        r(regs::CAPLENGTH),
        r(regs::HCSPARAMS1),
        r(regs::HCSPARAMS2),
        r(regs::HCCPARAMS1),
        r(regs::DBOFF),
        r(regs::RTSOFF),
    )
}

fn millis() -> u64 {
    crate::arch::timer::now() * 1000 / crate::arch::timer::frequency()
}

/// Step 1: the controller as the firmware left it.
pub(crate) fn before_handoff(regs_va: u64, bar_bytes: u64) {
    let capabilities = capabilities_of(regs_va);
    REGS_VA.store(regs_va, Ordering::Relaxed);
    let op = capabilities.operational;
    crate::println!(
        "xhci-probe: firmware's  USBCMD={:#010x} USBSTS={:#010x} HCSPARAMS1={:#010x} HCCPARAMS1={:#010x} (PPC={})",
        read(regs_va, op + regs::USBCMD),
        read(regs_va, op + regs::USBSTS),
        read(regs_va, regs::HCSPARAMS1),
        read(regs_va, regs::HCCPARAMS1),
        (read(regs_va, regs::HCCPARAMS1) >> 3) & 1,
    );
    let mut at = capabilities.extended;
    for _ in 0..64 {
        if at == 0 || at + 16 > bar_bytes {
            break;
        }
        let head = read(regs_va, at);
        match head as u8 {
            regs::EXTENDED_LEGACY_SUPPORT => crate::println!(
                "xhci-probe: firmware's  USBLEGSUP={head:#010x} USBLEGCTLSTS={:#010x}",
                read(regs_va, at + 4)
            ),
            EXTENDED_SUPPORTED_PROTOCOL => {
                let ports = read(regs_va, at + 8);
                crate::println!(
                    "xhci-probe: protocol    USB {}.{:x}: ports {}..={}",
                    head >> 24,
                    (head >> 16) & 0xff,
                    ports & 0xff,
                    (ports & 0xff) + ((ports >> 8) & 0xff) - 1,
                );
            }
            _ => {}
        }
        let next = u64::from((head >> 8) & 0xff) * 4;
        if next == 0 {
            break;
        }
        at += next;
    }
    dump("firmware's ", regs_va, &capabilities, None);
}

/// Step 2: the ports after the driver's reset and scan, and how they move over five seconds.
pub(crate) fn after_report() {
    let regs_va = REGS_VA.load(Ordering::Relaxed);
    if regs_va == 0 {
        return;
    }
    let capabilities = capabilities_of(regs_va);
    let mut last = [0u32; 256];
    let start = millis();
    dump("at report  ", regs_va, &capabilities, Some(&mut last));
    for ms in SAMPLES_MS {
        while millis() < start + ms {
            core::hint::spin_loop();
        }
        let mut label = [b' '; 11];
        let mut w = Cursor(&mut label, 0);
        let _ = core::fmt::write(&mut w, format_args!("+{ms}ms"));
        dump(
            core::str::from_utf8(&label).unwrap_or("?"),
            regs_va,
            &capabilities,
            Some(&mut last),
        );
    }
}

/// One line of controller state, then one line per port: every port when `last` is `None`,
/// otherwise only those whose `PORTSC` differs from `last` (which is updated).
fn dump(when: &str, regs_va: u64, capabilities: &Capabilities, mut last: Option<&mut [u32; 256]>) {
    crate::println!(
        "xhci-probe: {when} USBSTS={:#010x} IMAN={:#010x} ERDP={:#010x}",
        read(regs_va, capabilities.operational + regs::USBSTS),
        read(regs_va, capabilities.interrupter(regs::IMAN)),
        read(regs_va, capabilities.interrupter(regs::ERDP)),
    );
    let mut connected = 0;
    for n in 1..=capabilities.max_ports {
        let sc = read(regs_va, capabilities.portsc(n));
        connected += sc & 1;
        if let Some(last) = last.as_deref_mut() {
            let moved = last[n as usize] != sc || when.starts_with("at report");
            last[n as usize] = sc;
            if !moved {
                continue;
            }
        }
        crate::println!(
            "xhci-probe: {when} port {n:2} PORTSC={sc:#010x} CCS={} PED={} PP={} PLS={} speed={} CSC={} PRC={} PLC={}",
            sc & 1,
            (sc >> 1) & 1,
            (sc >> 9) & 1,
            (sc >> 5) & 0xf,
            (sc >> 10) & 0xf,
            (sc >> 17) & 1,
            (sc >> 21) & 1,
            (sc >> 22) & 1,
        );
    }
    crate::println!(
        "xhci-probe: {when} {connected} of {} connected",
        capabilities.max_ports
    );
}

/// A fixed buffer `core::fmt::Write` fills, for a label without an allocator.
struct Cursor<'a>(&'a mut [u8; 11], usize);

impl core::fmt::Write for Cursor<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            if self.1 < self.0.len() {
                self.0[self.1] = b;
                self.1 += 1;
            }
        }
        Ok(())
    }
}
