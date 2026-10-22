//! **Wiring for the USB keyboard driver** (milestone 242 (USB host and HID, because on commodity
//! hardware the keyboard is not a UART); notes/usb.md).
//!
//! `kernel/src/extensible_host_controller_interface.rs` found the controller, took it from the
//! firmware, drew the register window and confined the DMA region; this file decides what the
//! process that drives it is handed, and the whole of the confinement claim is in the [`Spawn`]
//! literal below. It is `non_volatile_memory_express_service`'s shape, one device over.
//!
//! # What the driver holds, and what it is denied
//!
//! Held:
//!
//! - slot 0, the controller's **interrupt** (`Irq`, WAIT and ACK);
//! - slot 1, an **attach** endpoint (READ): the one message it ever receives there carries the
//!   endpoint its keystrokes go to, delegated by the progenitor once the line discipline exists.
//!   It is how a driver the kernel starts before the progenitor ends up holding `WRITE` on an
//!   endpoint the progenitor creates, without the kernel ever naming that endpoint;
//! - slot 2, a **report** endpoint (WRITE): one bring-up report, which the kernel waits for;
//! - mapped: **the register pages** [`extensible_host_controller_interface::register_window`]
//!   chose, device-typed: every page of BAR0 but the MSI-X table's and pending-bit array's;
//! - mapped: **the work pages of its DMA region** (`dma::WORK_PAGES`), normal memory.
//!
//! Denied, each a decision:
//!
//! - **The MSI-X pages.** Whoever writes the table aims the interrupt message, and with interrupt
//!   remapping off an aimed message is any vector on any CPU.
//! - **The scratchpad pages.** They are the controller's private memory; the driver writes their
//!   addresses and never their contents.
//! - **PCI configuration space.** It cannot turn bus mastering on or off, move a BAR, or touch MSI.
//! - **Any physical memory outside its region, by DMA.** The IOMMU confines the controller to the
//!   region before the driver starts, and [`crate::extensible_host_controller_interface::bring_up`]
//!   refuses to start one at all where it cannot.
//! - **Every endpoint but three.** No budget, no filesystem, no console, no clock.
//!

//! # BUGS
//!
//! - **The attach handshake is a blocking send.** If the driver died between its report and its
//!   first receive on the attach endpoint, the progenitor's `SEND_CAP` there would block the boot.
//!   The driver receives on it immediately after reporting and does nothing else first, so the
//!   window is one system call wide; it is a window.
//! - **Enumeration after the report is silent.** A keyboard plugged in later is found and works,
//!   but a device refused later has nobody to say so to: the report endpoint has one message.
//!
//! Name: provisional. A `<program>_service`, as `non_volatile_memory_express_service` is, so its
//! name is the program's plus the suffix and carries no decision of its own.

use extensible_host_controller_interface::{MAX_REGISTER_PAGES, dma, report};

use super::*;
use crate::cap::{Rights, irq_cap, rendezvous_cap};
use crate::sched::RendezvousId;

/// Where the driver's register pages are mapped, page `n` of BAR0 at `REGS_VA + n * 4096`. Must
/// match `components/src/usb_keyboard_driver.rs`.
const REGS_VA: u64 = address_space_map::pair_page(0x0000_0000_0100_0000);

/// Where the driver's DMA work pages are mapped. Must match `components/src/usb_keyboard_driver.rs`.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0200_0000);

/// **Stack pages beyond the one every loaded program gets.** Enumeration is a call chain six
/// frames deep (attach, configure, transfer, wait, poll, the descriptor walk), and a debug build
/// spends more than one page on it: the first boot of this driver died with a data abort eight
/// words below its only stack page. Three more, mapped below it as ordinary frames, give it 16 KiB.
const EXTRA_STACK_PAGES: usize = 3;

/// The most mappings one spawn makes: every work page, every register page, and the stack.
const MAX_MAPS: usize = dma::WORK_PAGES as usize + MAX_REGISTER_PAGES as usize + EXTRA_STACK_PAGES;

/// What [`start`] hands back.
pub(crate) struct Wiring {
    /// The driver's one bring-up report: [`report::KEYBOARD`], [`report::NO_KEYBOARD`] or
    /// [`report::FAILED`] in word 0.
    pub report: [u64; 5],
    /// The attach endpoint. The progenitor is granted `WRITE | GRANT` on it, and delegates the
    /// line discipline's endpoint through it.
    pub attach: RendezvousId,
}

/// **Find, confine and hand over the xHCI controller, and wait for the driver's report.** `Err`
/// with [`crate::extensible_host_controller_interface::Absent::NoController`] on a machine with no
/// controller, which the caller says nothing about.
pub(crate) fn start(
    image: &'static [u8],
) -> Result<Wiring, crate::extensible_host_controller_interface::Absent> {
    let found = crate::extensible_host_controller_interface::bring_up()?;
    let words = found.handoff.pack();

    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(found.intid, irq_ep);
    // Enabled through the arch layer on all three, which is what the `Irq` capability's ACK uses
    // too: on riscv64 that is the source's stable round-robin hart (`target_context`), and a
    // source enabled on the boot hart's context instead would be moved by its first ACK. Measured,
    // not reasoned: pinned to `boot_s_context` the way the virtio-rng grant is, the riscv64 boot
    // took the first interrupt and no keystroke after it; `keyboard_service::wire_device` enables
    // this way and its keys arrive.
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
            &mut maps[..dma::WORK_PAGES as usize],
            DMA_VA,
            dma_phys,
            dma::WORK_PAGES as usize,
        );
        // The register pages the window chose, device-typed: a cached or reordered doorbell write
        // is a command the controller never hears about.
        let mut n = dma::WORK_PAGES as usize;
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

/// **The boot line for a report**, printed by the caller: one sentence a person at the bench can
/// act on, in the `  usb       :` column every boot line here uses.
pub(crate) fn describe(w: &[u64; 5]) {
    match w[0] {
        report::KEYBOARD => {
            let (port, speed, vendor, product) = report::keyboard_of(w[1]);
            crate::println!(
                "  usb       : a keyboard on port {port} ({}, {vendor:04x}:{product:04x}); its keys reach the shell",
                extensible_host_controller_interface::port::speed_name(speed),
            );
        }
        report::NO_KEYBOARD => {
            let (ports, connected, settled) = report::ports_of(w[1]);
            // An unsettled count is a moment in a port that was still changing, so it is not
            // printed as a fact about what is plugged in.
            let seen = if settled {
                Some((connected, ports))
            } else {
                None
            };
            match (report::refusal_of(w[2]), seen) {
                ((Some(step), port, detail), Some((connected, ports))) => crate::println!(
                    "  usb       : no keyboard yet ({connected} of {ports} ports in use; port {port}: {} ({detail})); one plugged in later is found",
                    step.describe(),
                ),
                ((Some(step), port, detail), None) => crate::println!(
                    "  usb       : no keyboard yet (the ports were still settling; port {port}: {} ({detail})); one plugged in later is found",
                    step.describe(),
                ),
                ((None, ..), Some((connected, ports))) => crate::println!(
                    "  usb       : no keyboard yet ({connected} of {ports} ports in use); one plugged in later is found"
                ),
                ((None, ..), None) => crate::println!(
                    "  usb       : no keyboard yet (the ports were still settling); one plugged in later is found"
                ),
            }
        }
        _ => match report::refusal_of(w[1]) {
            (Some(step), port, detail) => crate::println!(
                "  usb       : REFUSED. The USB keyboard driver stopped: {} (port {port}, {detail:#x}); keystrokes come over the UART only",
                step.describe(),
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
