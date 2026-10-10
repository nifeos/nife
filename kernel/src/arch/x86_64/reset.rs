//! **A kernel-initiated cold reboot, `x86_64`** (milestone 249 (the boot lottery is sampled by a person walking to the board)'s parity half).
//!
//! A PC has no one firmware call that owns its power the way PSCI does on aarch64 or SBI SRST on
//! RISC-V. It has three reset mechanisms of different vintages, and a kernel that wants to come
//! back tries them strongest-first and says which one it is on before each, because a successful
//! one never returns and the console line before it is the only record:
//!
//! 1. **The FADT's reset register**, when the table offers one (`RESET_REG_SUP`). This is the route
//!    firmware vouches for, and on every Intel chipset this project has seen it names port `0xCF9`
//!    anyway; it is tried first because a machine that puts it somewhere else is exactly the
//!    machine on which the guess in step 2 is wrong.
//! 2. **Port `0xCF9`, the chipset's Reset Control register**, written `0x02` then `0x0E`: system
//!    reset, then CPU reset with full reset. Full reset (bit 3) is what makes it *cold*: the
//!    platform drops power to the rails briefly rather than only pulling the CPU's reset line, which
//!    is the difference that matters on a board whose firmware does not survive a warm reset
//!    (radon's lesson, milestone 249's block).
//! 3. **The 8042 keyboard controller's reset pulse**, command `0xFE` to port `0x64`. The oldest
//!    route and the one most likely to be emulated by a platform that has nothing else.
//!
//! Each gets a tenth of a second to take effect before the next is tried. If all three come back,
//! [`reboot`] returns and the caller says so; nothing here halts, because the caller (the rebooting
//! soak, or since milestone 805 the reboot object's method) still has a running machine to report
//! on.
//!
//! # BUGS
//!
//! - **A memory-space FADT reset register is not attempted.** It would need a device mapping made
//!   at the moment of the reset, and no `x86_64` machine this project owns or emulates uses one (q35
//!   and xenon both say system I/O). It is logged and skipped, so the fallback still runs.
//! - **Only the first route has ever reset anything.** q35's FADT offers `0xCF9 <- 0x0F`, so
//!   `script/soak-test --reboot --arch x86_64` resets on attempt 1 and never reaches 2 or 3. Both
//!   fallbacks compile, print, and have never executed; a QEMU proof of either needs a machine
//!   whose FADT does not offer that route, and no runner here starts one. xenon's first bench
//!   reboot is where attempt 1 meets real firmware; the fallbacks stay unproven until a machine
//!   without the FADT route turns up.
//! - **Proven under QEMU `q35` only.** Whether xenon's `0xCF9` full reset brings the machine all
//!   the way back through its firmware to a netboot is the bench's first question; a reset the
//!   firmware does not survive is not a reboot. See milestone 249's block.
//! - **Nothing here quiesces devices first.** A reset through `0xCF9` resets the platform, so a DMA
//!   engine mid-transfer is cut off with everything else; that is the point of a cold reset. Since
//!   milestone 805 (`reboot` at the prompt) this is also called on a machine holding a filesystem
//!   open, through the reboot object (kernel/src/reboot.rs). What makes that safe is not here:
//!   `reboot` syncs the writable filesystem before it invokes the object, so the block servers are
//!   idle by the time this runs, and nothing else on the machine writes to persistent storage.

use core::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use machine_discovery::acpi::{ResetRegister, ResetSpace};

use super::port::{in8, out8, out32};
use crate::println;

/// The FADT reset register's address, or 0 for "none", written once by [`record`] during the ACPI
/// walk and read once by [`reboot`]. Atomics rather than a lock for `isa`'s reason: a value written
/// once before any other core exists does not want a lock rank.
static FADT_RESET_ADDRESS: AtomicU64 = AtomicU64::new(0);
/// ACPI's address-space id for [`FADT_RESET_ADDRESS`] (0 memory, 1 I/O, 2 PCI config).
static FADT_RESET_SPACE: AtomicU8 = AtomicU8::new(0);
/// The byte to write there.
static FADT_RESET_VALUE: AtomicU8 = AtomicU8::new(0);

/// Remember what the FADT said, for [`reboot`]. Called from `machine::read_acpi`.
pub fn record(reset: Option<ResetRegister>) {
    let Some(r) = reset else { return };
    FADT_RESET_SPACE.store(
        match r.space {
            ResetSpace::Memory => 0,
            ResetSpace::Io => 1,
            ResetSpace::PciConfig => 2,
            ResetSpace::Other(id) => id,
        },
        Ordering::Relaxed,
    );
    FADT_RESET_VALUE.store(r.value, Ordering::Relaxed);
    FADT_RESET_ADDRESS.store(r.address, Ordering::Relaxed);
}

/// The chipset's Reset Control register (Intel's `RST_CNT`), on every PCH since the ICH.
const RESET_CONTROL_PORT: u16 = 0xcf9;
/// `RST_CNT` bit 1, System Reset: selects a platform reset rather than an INIT when bit 2 fires.
const RESET_CONTROL_SYSTEM: u8 = 0x02;
/// `RST_CNT` bits 1, 2 and 3: system reset, reset CPU (the edge that fires it), full reset (drop
/// the rails, which is what makes it cold).
const RESET_CONTROL_COLD: u8 = 0x0e;
/// The 8042's command and status port.
const KBC_PORT: u16 = 0x64;
/// Status bit 1: the controller's input buffer is full, so a command written now would be lost.
const KBC_INPUT_FULL: u8 = 0x02;
/// The 8042 command that pulses the CPU reset line.
const KBC_PULSE_RESET: u8 = 0xfe;
/// The legacy PCI configuration mechanism's address and data ports, for a PCI-config FADT register.
const PCI_CONFIG_ADDRESS: u16 = 0xcf8;
const PCI_CONFIG_DATA: u16 = 0xcfc;

/// Spin for `millis` of wall clock. Only on the way to a reset, where yielding would hand the core
/// to a workload that is about to stop existing.
fn wait_ms(millis: u64) {
    let hz = super::timer::frequency();
    let start = super::timer::now();
    while super::timer::now().wrapping_sub(start) < hz / 1000 * millis {
        core::hint::spin_loop();
    }
}

/// **Reset the machine, cold, trying every route a PC has, and return only if none worked.**
///
/// The arch contract `soak::draw_again` calls on all three architectures: one line per attempt,
/// prefixed with `marker`, printed before the attempt. See the module header for the order and why.
/// Each attempt drains the console first, because milestone 592 (radon's cold reboot dies in
/// OpenSBI's PMIC write) found that a reset takes the machine with it, and an un-drained line is a
/// line the transcript never shows.
///
/// Name: provisional (milestone 249): calef names public items.
pub fn reboot(marker: &str) -> abi::Error {
    let address = FADT_RESET_ADDRESS.load(Ordering::Relaxed);
    let value = FADT_RESET_VALUE.load(Ordering::Relaxed);
    match (address, FADT_RESET_SPACE.load(Ordering::Relaxed)) {
        (0, _) => println!(
            "{marker} attempt 1 of 3 skipped: the FADT offers no reset register on this machine"
        ),
        (port, 1) if port <= u64::from(u16::MAX) => {
            println!(
                "{marker} attempt 1 of 3: FADT reset register, I/O port {port:#x} <- {value:#04x}"
            );
            // SAFETY: the port and value firmware's FADT names for exactly this purpose.
            // Drain first (milestone 592, radon's cold reboot dies in OpenSBI's PMIC write): a
            // reset register write takes the machine with it, and the line above is still in the
            // transmitter otherwise.
            crate::console::drain();
            unsafe { out8(port as u16, value) };
            wait_ms(100);
        }
        (packed, 2) => {
            let device = (packed >> 32) & 0x1f;
            let function = (packed >> 16) & 0x7;
            let offset = packed & 0xff;
            println!(
                "{marker} attempt 1 of 3: FADT reset register, PCI config 00:{device:02x}.{function} \
                 offset {offset:#x} <- {value:#04x}"
            );
            // SAFETY: the legacy configuration mechanism, addressing the bus-0 register the FADT
            // names for this purpose. The address write selects the dword; the data write lands on
            // the byte within it.
            // Drain first, for the FADT I/O-port twin's reason (milestone 592).
            crate::console::drain();
            unsafe {
                out32(
                    PCI_CONFIG_ADDRESS,
                    0x8000_0000
                        | (device << 11) as u32
                        | (function << 8) as u32
                        | (offset & 0xfc) as u32,
                );
                out8(PCI_CONFIG_DATA + (offset & 3) as u16, value);
            }
            wait_ms(100);
        }
        (address, space) => println!(
            "{marker} attempt 1 of 3 skipped: the FADT reset register is in address space {space} \
             at {address:#x}, which this kernel does not write (see arch/x86_64/reset.rs BUGS)"
        ),
    }

    println!(
        "{marker} attempt 2 of 3: chipset reset control, port {RESET_CONTROL_PORT:#x} <- \
         {RESET_CONTROL_SYSTEM:#04x} then {RESET_CONTROL_COLD:#04x} (full reset)"
    );
    // SAFETY: `RST_CNT`, whose only effect is the reset being asked for. Read-modify-write so the
    // reserved bits keep what firmware left in them, which is what Intel's datasheets ask.
    // Drain first (milestone 592): the writes below pulse the chipset into reset immediately.
    crate::console::drain();
    unsafe {
        let keep = in8(RESET_CONTROL_PORT) & !RESET_CONTROL_COLD;
        out8(RESET_CONTROL_PORT, keep | RESET_CONTROL_SYSTEM);
        wait_ms(1);
        out8(RESET_CONTROL_PORT, keep | RESET_CONTROL_COLD);
    }
    wait_ms(100);

    println!(
        "{marker} attempt 3 of 3: 8042 reset pulse, port {KBC_PORT:#x} <- {KBC_PULSE_RESET:#04x}"
    );
    // SAFETY: the keyboard controller's status read and its reset command. Bounded wait for the
    // input buffer, because a machine with no 8042 reads 0xff forever.
    // Drain first (milestone 592): the pulse is the last route and takes the machine with it too.
    crate::console::drain();
    unsafe {
        let mut bound = 10_000u32;
        while in8(KBC_PORT) & KBC_INPUT_FULL != 0 && bound > 0 {
            core::hint::spin_loop();
            bound -= 1;
        }
        out8(KBC_PORT, KBC_PULSE_RESET);
    }
    wait_ms(100);

    println!("{marker} all three reset routes returned: this machine did not reset");
    // Every route was tried (the fallbacks are never skipped), so this is never "no mechanism".
    abi::Error::ResetDidNotHappen
}
