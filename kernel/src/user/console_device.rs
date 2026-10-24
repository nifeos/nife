//! The console UART as the kernel grants it: its receive interrupt, its register page, and the
//! port range `x86_64` has instead of a page.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

/// The console UART's receive interrupt on QEMU `virt`, which the progenitor routes to the input
/// driver it builds. aarch64's PL011 is SPI 1 = INTID 33; RISC-V's NS16550 is PLIC source 10.
///
/// A fallback, not the answer: the boot paths ask the machine first ([`uart_irq_and_source`]), and
/// on the JH7110 UART0 is PLIC line 32 (notes/visionfive2.md, BUGS).
///
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
/// for in his #1255 review; recommended `UART_RECEIVE_INTID`; `INTID` is the GIC's own term and
/// stays.
#[cfg(target_arch = "aarch64")]
pub const UART_RX_INTID: u32 = 33;
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
/// for in his #1255 review; recommended `UART_RECEIVE_INTID`; `INTID` is the GIC's own term and
/// stays.
#[cfg(target_arch = "riscv64")]
pub const UART_RX_INTID: u32 = 10;
/// `x86_64`: COM1 is ISA IRQ 4, the legacy line. It is neither the IO APIC input it may be
/// remapped to (the ACPI MADT's overrides say, unread here) nor the IDT vector that input raises.
///
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
/// for in his #1255 review; recommended `UART_RECEIVE_INTID`; `INTID` is the GIC's own term and
/// stays.
#[cfg(target_arch = "x86_64")]
pub const UART_RX_INTID: u32 = 4;

/// The console UART's interrupt line and which source decided it: the machine's own answer when
/// it gave one (`memory::uart_irq`, a device tree on aarch64/riscv64 or ACPI on `x86_64`), else
/// [`UART_RX_INTID`], QEMU `virt`'s constant. The source string is for a bench transcript to
/// print, so the number's origin is diagnosable.
pub(crate) fn uart_irq_and_source() -> (u32, &'static str) {
    match crate::memory::uart_irq() {
        Some(n) => (n, "machine description"),
        None => (UART_RX_INTID, "QEMU-virt fallback; the machine did not say"),
    }
}

/// The console UART's registers, physically. aarch64 `virt` puts a PL011 at `0x0900_0000`; RISC-V
/// `virt` puts an NS16550 at `0x1000_0000`. The progenitor holds a device capability for it and delegates it
/// to the console and input drivers it builds. Matches `console::UART_PHYS`.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(target_arch = "aarch64")]
pub const UART_PHYS: u64 = 0x0900_0000;
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(target_arch = "riscv64")]
pub const UART_PHYS: u64 = crate::arch::machine::CONSOLE_UART_PHYS;
/// `x86_64` has no physical address for its console: COM1 is in the I/O port space, reached
/// through [`X86_COM1_PORT_BASE`] as a `PortRange` capability. Zero, which
/// [`machine_has_no_device_page_for_the_console`] reads so no fixture grants a device page here.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(target_arch = "x86_64")]
pub const UART_PHYS: u64 = 0;

/// COM1's I/O ports, `0x3F8..=0x3FF`. The progenitor is minted a `PortRange(0x3F8, 8)` capability
/// over exactly this range for the console and input drivers, enforced by the TSS I/O bitmap: the
/// port analogue of [`UART_PHYS`].
#[cfg(target_arch = "x86_64")]
pub const X86_COM1_PORT_BASE: u16 = 0x3F8;
/// The eight ports a 16550 occupies (data/interrupt-enable/FIFO-control/line-control/modem-control/
/// line-status/modem-status/scratch). A range because a device capability names what the hardware
/// names, the port-space twin of a `DeviceFrame` naming a page.
#[cfg(target_arch = "x86_64")]
pub const X86_COM1_PORT_COUNT: u16 = 8;

/// Is there no UART page a device capability can map on this machine? True on `x86_64`, where
/// [`UART_PHYS`] is zero ([§121 (what a device capability is when the device has no
/// page)](../../design/decisions/0121-port-io-capability.md)).
///
/// A fixture that went ahead anyway would map physical page zero, read real-mode vector bytes, and
/// report a green test for a device story x86 does not have.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn machine_has_no_device_page_for_the_console() -> bool {
    UART_PHYS == 0
}

/// The reason a fixture gives when [`machine_has_no_device_page_for_the_console`] is true.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const NO_UART_PAGE: &str = "this machine's console UART is in the I/O port space, so there is \
                                no page for a device capability to be a mapping of and no \
                                capability shape for a port yet (DECISIONS \u{a7}121)";
