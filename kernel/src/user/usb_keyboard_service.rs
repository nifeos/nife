//! Wiring for the USB keyboard driver, whose [`Spawn`] literal is the confinement claim: the
//! interrupt, attach and report endpoints, the register pages `register_window` chose, and the DMA
//! work pages. See notes/usb.md, "What the driver holds".
//!
//! # BUGS
//!
//! - The attach handshake is a blocking send: a driver that died between its report and its first
//!   receive there would block the progenitor's `SEND_CAP`, and the boot.
//! - A device refused after the report is refused silently: the report endpoint has one message.
//!
//! Name: provisional. A `<program>_service`, as `non_volatile_memory_express_service` is, so its
//! name is the program's plus the suffix and carries no decision of its own.

use extensible_host_controller_interface_contract::{MAX_REGISTER_PAGES, WORK_PAGES};
use usb_keyboard_report as report;

use super::*;
use crate::cap::{Rights, irq_cap, rendezvous_cap};
use crate::sched::RendezvousId;

/// Where the driver's register pages are mapped, page `n` of BAR0 at `REGS_VA + n * 4096`. Must
/// match `components/src/usb_keyboard_driver.rs`.
const REGS_VA: u64 = address_space_map::pair_page(0x0000_0000_0100_0000);

/// Where the driver's DMA work pages are mapped. Must match `components/src/usb_keyboard_driver.rs`.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0200_0000);

/// Stack pages beyond the one every loaded program gets. Enumeration is a call chain six frames
/// deep, and in a debug build one page overflowed by eight words; three more give it 16 KiB.
const EXTRA_STACK_PAGES: usize = 3;

/// The most mappings one spawn makes: every work page, every register page, and the stack.
const MAX_MAPS: usize = WORK_PAGES as usize + MAX_REGISTER_PAGES as usize + EXTRA_STACK_PAGES;

/// What [`start`] hands back.
pub(crate) struct Wiring {
    /// The driver's one bring-up report: [`report::KEYBOARD_FOUND`],
    /// [`report::NO_KEYBOARD_FOUND`] or [`report::BRING_UP_FAILED`] in word 0.
    pub report: [u64; 5],
    /// The attach endpoint. The progenitor is granted `WRITE | GRANT` on it, and delegates the
    /// line discipline's endpoint through it.
    pub attach: RendezvousId,
}

/// Find, confine and hand over the xHCI controller, and wait for the driver's report. `Err`
/// with [`crate::extensible_host_controller_interface::Absent::NoController`] on a machine with no
/// controller, which the caller says nothing about.
pub(crate) fn start(
    image: &'static [u8],
) -> Result<Wiring, crate::extensible_host_controller_interface::Absent> {
    let found = crate::extensible_host_controller_interface::bring_up()?;
    let words = found.handoff.pack();

    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(found.intid, irq_ep);
    // Through the arch layer, as the `Irq` capability's ACK is: on riscv64 a source enabled on
    // the boot hart's context instead is moved by its first ACK, and no keystroke after the first
    // arrives (measured). See notes/usb.md, "What the driver holds".
    crate::arch::irq::enable(found.intid);

    let attach = crate::sched::create_rendezvous();
    let report = crate::sched::create_rendezvous();

    let (bar0, dma_phys, registers) = (found.bar0, found.handoff.dma_phys, found.handoff.registers);
    let intid = found.intid;
    let mut stack = [0u64; EXTRA_STACK_PAGES];
    for frame in &mut stack {
        *frame = crate::memory::alloc_zeroed()
            .expect("no frame for the USB keyboard driver's stack")
            .addr();
    }
    crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; MAX_MAPS];
        // The work pages, contiguous, normal memory: the controller and the driver share ordinary
        // RAM there. The scratchpad pages after them are not mapped.
        super::fs_service::map_channel(
            &mut maps[..WORK_PAGES as usize],
            DMA_VA,
            dma_phys,
            WORK_PAGES as usize,
        );
        // The register pages the window chose, device-typed: a cached or reordered doorbell write
        // is a command the controller never hears about.
        let mut n = WORK_PAGES as usize;
        for page in 0..MAX_REGISTER_PAGES {
            if registers & (1 << page) != 0 {
                maps[n] = Mapping {
                    va: REGS_VA + page * FRAME_SIZE,
                    phys: bar0 + page * FRAME_SIZE,
                    flags: Flags::user_device(),
                };
                n += 1;
            }
        }
        for (k, &frame) in stack.iter().enumerate() {
            maps[n] = Mapping {
                va: USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE,
                phys: frame,
                flags: Flags::user_data(),
            };
            n += 1;
        }
        run(
            image,
            Spawn {
                arg0: words[0], // the DMA region's PHYSICAL base: every ring pointer is physical
                arg1: words[1], // its length in pages, and the register page mask
                arg2: words[2],
                grants: &[
                    irq_cap(intid),                        // slot 0: the controller's interrupt
                    rendezvous_cap(attach, Rights::READ),  // slot 1: receive where keystrokes go
                    rendezvous_cap(report, Rights::WRITE), // slot 2: the one bring-up report
                ],
                maps: &maps[..n],
            },
        )
    })
    .expect("could not spawn the USB keyboard driver");

    Ok(Wiring {
        report: crate::sched::ipc_receive(report),
        attach,
    })
}

/// Print the boot line for report `w`: one sentence a person at the bench can
/// act on, in the `  usb       :` column every boot line here uses.
pub(crate) fn describe(w: &[u64; 5]) {
    match w[0] {
        report::KEYBOARD_FOUND => {
            let (port, speed, vendor, product) = report::keyboard_report_of(w[1]);
            crate::println!(
                "  usb       : a keyboard on port {port} ({}, {vendor:04x}:{product:04x}); its keys reach the shell",
                extensible_host_controller_interface::port::speed_name(speed),
            );
        }
        report::NO_KEYBOARD_FOUND => {
            let (ports, connected, settled) = report::ports_report_of(w[1]);
            // An unsettled count is a moment in a port that was still changing, so it is not
            // printed as a fact about what is plugged in.
            let seen = if settled {
                Some((connected, ports))
            } else {
                None
            };
            match (report::failure_report_of(w[2]), seen) {
                ((Some(step), port, detail), Some((connected, ports))) => crate::println!(
                    "  usb       : no keyboard yet ({connected} of {ports} ports in use; port {port}: {} ({detail})); one plugged in later is found",
                    step.sentence(),
                ),
                ((Some(step), port, detail), None) => crate::println!(
                    "  usb       : no keyboard yet (the ports were still settling; port {port}: {} ({detail})); one plugged in later is found",
                    step.sentence(),
                ),
                ((None, ..), Some((connected, ports))) => crate::println!(
                    "  usb       : no keyboard yet ({connected} of {ports} ports in use); one plugged in later is found"
                ),
                ((None, ..), None) => crate::println!(
                    "  usb       : no keyboard yet (the ports were still settling); one plugged in later is found"
                ),
            }
        }
        _ => match report::failure_report_of(w[1]) {
            (Some(step), port, detail) => crate::println!(
                "  usb       : REFUSED. The USB keyboard driver stopped: {} (port {port}, {detail:#x}); keystrokes come over the UART only",
                step.sentence(),
            ),
            (None, ..) => crate::println!(
                "  usb       : REFUSED. The USB keyboard driver reported {:#x}; keystrokes come over the UART only",
                w[0]
            ),
        },
    }
}

/// The boot line for a controller the kernel refused to hand over.
pub(crate) fn describe_refusal(why: crate::extensible_host_controller_interface::Absent) {
    use crate::extensible_host_controller_interface::Absent;
    match why {
        Absent::NoController => {}
        Absent::Window(w) => crate::println!(
            "  usb       : REFUSED. A register the driver needs shares a page with the controller's MSI-X table ({w:?})"
        ),
        Absent::TooManyScratchpads(n) => crate::println!(
            "  usb       : REFUSED. The controller wants {n} scratchpad pages, more than one page of pointers names"
        ),
        Absent::NoMemory(pages) => crate::println!(
            "  usb       : REFUSED. No contiguous run of {pages} pages for the controller's DMA region"
        ),
        Absent::Unconfined => crate::println!(
            "  usb       : REFUSED. The IOMMU does not confine the USB controller, so no driver may hold it; keystrokes come over the UART only"
        ),
    }
}
