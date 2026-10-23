//! **The device starters**: each brings one device up at boot and hands back what `boot_progenitor`
//! grants the progenitor. See notes/progenitor-boot.md.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

use super::*;

/// **The USB keyboard, when this machine has an xHCI controller** (milestone 242 (USB host and
/// HID)): start its driver and say what it found.
///
/// `None`, having printed why, for a controller the kernel would not hand over or a driver that
/// failed; `None` silently with no controller. A report of no keyboard still grants the attach
/// endpoint, because the driver keeps watching its ports. The kernel starts this driver, not the
/// progenitor, because its pages are spawn-time mappings (notes/progenitor-boot.md). Name
/// provisional.
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput"
    ),
    allow(dead_code)
)]
pub(super) fn boot_usb_keyboard() -> Option<usb_keyboard_service::Wiring> {
    let image = program("usb_keyboard_driver")?;
    match usb_keyboard_service::start(image) {
        Ok(w) => {
            usb_keyboard_service::describe(&w.report);
            #[cfg(feature = "xhci_port_probe")]
            crate::xhci_port_probe::after_report();
            (w.report[0] != extensible_host_controller_interface::report::FAILED).then_some(w)
        }
        Err(why) => {
            usb_keyboard_service::describe_refusal(why);
            None
        }
    }
}

/// **The clock page the interactive boot hands the progenitor** (milestone 51 (wall-clock time)),
/// and what a machine with no clock looks like.
///
/// Always granted: with no `clock` program in the archive the page is zeroed, which reads as
/// `clock_contract::state::UNKNOWN`, so the progenitor's slot numbering never depends on the
/// machine. The progenitor gets `READ` only (DECISIONS §43 (reading the clock is a page)), so
/// nothing it builds can set the time.
pub(super) fn boot_clock_page() -> u64 {
    match program("clock") {
        Some(image) => {
            let wiring = clock_service::start(image);
            // Drain the service's blocking announcement, or it parks there forever and never serves
            // a proposal.
            let report = wiring.report;
            let _ = crate::sched::spawn(move || {
                crate::sched::ipc_receive(report);
                crate::sched::exit();
            });
            wiring.page_phys
        }
        // No `clock` program packed: a zeroed page, the honest unknown clock.
        None => crate::memory::alloc_zeroed()
            .expect("no frame for the clock page")
            .addr(),
    }
}

/// The confined transport, the completion interrupt, and the DMA page's physical base, for a
/// virtio device (the rng or the NIC) this kernel discovered and wired at boot. Returned rather
/// than stored, so the caller grants them at the slots it chooses.
pub(super) struct VirtioBootGrant {
    /// The `Virtio` capability's id (`crate::virtio::register`'s return value).
    pub(super) vid: usize,
    /// The device's completion interrupt, **routed** (`crate::sched::bind_irq`) but not
    /// **enabled**: aarch64's GIC enables inline and riscv64 pins the PLIC source to the boot hart
    /// (`notes/harts-and-pes.md`'s hart lottery), so the caller enables it beside `uart_irq`.
    pub(super) intid: u32,
    /// The DMA region's physical base. A driver writes physical addresses into virtio
    /// descriptors, no capability exposes one, and the progenitor's three `START` words are spent,
    /// so the base is also written into the page at [`VIRTIO_DMA_PHYS_OFFSET`] for the progenitor
    /// to read back and relay as the driver's `arg1`.
    pub(super) dma: u64,
}

/// **Discover and wire a virtio-rng device on the MMIO bus, for the interactive boot** (DECISIONS
/// §120 (a QEMU-only virtio-rng stopgap)'s 2026-08-26 amendment). `None` on a boot with none (real
/// hardware, or a run with `NIFE_RNG` unset), which the caller treats as an absence rather than a
/// failure.
///
/// The MMIO transport only, not PCIe. It does `entropy_service::start`'s kernel-side setup and then
/// hands the three capabilities back, because on this boot the progenitor builds the entropy
/// service. See notes/progenitor-boot.md.
pub(super) fn boot_virtio_rng_device() -> Option<VirtioBootGrant> {
    boot_virtio_mmio_device(crate::virtio::find_entropy_device()?)
}

/// **An entropy service on the CPU's own seed instruction, for a boot with no virtio-rng**
/// (milestone 595 (the shell runs a `std` program)). Returns its request endpoint, which
/// [`boot_progenitor`] grants at slot 16, or `None`, said on the console.
///
/// The kernel confirms the instruction and builds the service, so detection stays in `arch/` and
/// works on aarch64 too. No instruction, or an all-zero first draw, is a refusal with no fallback
/// generator (DECISIONS §137 (a hardware TRNG with no published health-test claim) option A). On
/// `x86_64` the installer may already have taken the readiness report. See
/// notes/progenitor-boot.md.
pub(super) fn boot_instruction_entropy() -> Option<crate::sched::RendezvousId> {
    let image = program("entropy")?;
    let Some(w) = entropy_service::ensure(image, entropy_service::Bus::Instruction) else {
        crate::println!(
            "  entropy     : NONE. No virtio-rng device, and this CPU has no seed instruction \
             (RDSEED, RNDRRS), so nothing at the prompt can draw random bytes and there is no login."
        );
        return None;
    };
    if let Some(report) = w.wait_for_ready()
        && report[0] != entropy_contract::READY
    {
        crate::println!(
            "  entropy     : REFUSED. The seed instruction's first draw was all zeros ({:#x}), so \
             the service is condemned for this boot; nothing at the prompt can draw random bytes.",
            report[0]
        );
        return None;
    }
    crate::println!("  entropy     : the CPU's seed instruction; the progenitor serves it");
    Some(w.request)
}

/// **The same for the network card** (milestone 590 (the booted system starts its network stack)).
/// `None` with no virtio-net device on the MMIO bus: every real board, and every QEMU run with
/// `NIFE_NET` unset. The progenitor builds `net_stack` from these three and nothing else.
///
/// The MMIO NIC has no IOMMU in front of it, so its DMA is confined by the transport's shadow-ring
/// validator alone, not the SMMU of DECISIONS §20 (IOMMU-backed DMA isolation); the PCIe NIC sits
/// unclaimed on this boot.
pub(super) fn boot_virtio_net_device() -> Option<VirtioBootGrant> {
    boot_virtio_mmio_device(crate::virtio::find_net_device()?)
}

/// **A network stack over an `e1000e`, when this boot has one and no virtio-net** (milestone 198 (a
/// package manager)'s rung 3a, on milestone 494 (a driver for the network card a PC actually has)'s
/// driver). `None`, having said why, for an unproven part, a link that was down or a refused
/// bring-up; `None` silently with no such NIC, which is every `virt` boot.
///
/// The kernel builds it, for [`boot_usb_keyboard`]'s reason, and it is the wiring every `e1000e`
/// gate already runs. See notes/progenitor-boot.md.
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput"
    ),
    allow(dead_code)
)]
pub(super) fn boot_e1000e_network() -> Option<e1000e_service::Wiring> {
    use e1000e_service::{Absent, NotAtBoot};
    let image = program("net_stack")?;
    let name = |device| ::e1000e::model(device).unwrap_or("an unnamed part");
    match e1000e_service::start_for_boot(image) {
        Ok(w) => {
            crate::println!(
                "  network     : an e1000e ({}), link up; the kernel started net_stack on it, \
                 confined {}",
                name(w.device),
                if w.confined_by_iommu {
                    "by the IOMMU"
                } else {
                    "by its own arithmetic alone (no IOMMU owns it)"
                },
            );
            Some(w)
        }
        Err(NotAtBoot::Absent(Absent::NoController)) => None,
        Err(NotAtBoot::Unproven { device }) => {
            crate::println!(
                "  network     : NONE. An e1000e ({}, 8086:{device:04x}) is on the bus and left \
                 alone: no gate has driven that part yet (milestone 494's bench boot is how one \
                 does)",
                name(device),
            );
            None
        }
        Err(NotAtBoot::Absent(Absent::NoLink { device })) => {
            crate::println!(
                "  network     : NONE. The e1000e ({}) has no link, and a stack waiting for DHCP \
                 would hold the prompt back for ever",
                name(device),
            );
            None
        }
        Err(NotAtBoot::Absent(why)) => {
            crate::println!("  network     : REFUSED. The e1000e could not be brought up: {why:?}");
            None
        }
    }
}

/// **The booted system's stack over radon's Ethernet port** (milestone 53 (the board's own
/// peripherals: network and storage on real silicon)), [`boot_e1000e_network`]'s twin. Returns the
/// `Stack` endpoint and the lease endpoint, or `None` having said why. While
/// `designware_ethernet_service::PROVEN_ON_SILICON` is false it touches nothing: radon's boot is the
/// one every other bench session depends on, and a bring-up that hung would take the prompt with it.
/// With no cable in the port it costs the five seconds the link wait allows, then says `NONE`.
#[cfg(target_arch = "riscv64")]
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput"
    ),
    allow(dead_code)
)]
pub(super) fn boot_designware_network()
-> Option<(crate::sched::RendezvousId, crate::sched::RendezvousId)> {
    use designware_ethernet_service::NotStarted;
    let port = crate::memory::jh7110_ethernet()?;
    if !designware_ethernet_service::PROVEN_ON_SILICON {
        crate::println!(
            "  network     : NONE. The JH7110's Ethernet port at {:#x} is described and left as \
             the firmware left it: no bench boot has proved this driver yet (milestone 53's \
             runbook, notes/designware-ethernet.md, is how one does)",
            port.port.base,
        );
        return None;
    }
    let image = program("net_stack")?;
    match designware_ethernet_service::start_net_server(image, socket_contract::NO_LISTEN_GRANT) {
        Ok(w) => {
            crate::println!(
                "  network     : the JH7110's Ethernet port, link up; the kernel started net_stack \
                 on it, confined by its own arithmetic alone (no IOMMU on this SoC)"
            );
            Some((w.stack, w.report))
        }
        Err(NotStarted::NoLink) => {
            crate::println!(
                "  network     : NONE. The JH7110's Ethernet port has no link, and a stack \
                 waiting for DHCP would hold the prompt back for ever"
            );
            None
        }
        Err(why) => {
            crate::println!("  network     : REFUSED. The JH7110's Ethernet port: {why:?}");
            None
        }
    }
}

/// The shared body of [`boot_virtio_rng_device`] and [`boot_virtio_net_device`]: a zeroed DMA frame
/// with its own physical base written at [`VIRTIO_DMA_PHYS_OFFSET`], the interrupt routed but not
/// enabled, and the transport registered with the kernel, confined to that one frame.
fn boot_virtio_mmio_device(d: crate::virtio::VirtioMmioDevice) -> Option<VirtioBootGrant> {
    // Zeroed first, so no stale descriptor or buffer content is visible to the device or to the
    // driver's own first read.
    let dma = crate::memory::alloc_contiguous_zeroed(1)
        .expect("no DMA frame for a boot virtio device")
        .addr();
    // Its physical base goes in the page's tail ([`VIRTIO_DMA_PHYS_OFFSET`]).
    write_dma_phys(dma);
    // Routed, not yet enabled: [`VirtioBootGrant::intid`].
    crate::sched::bind_irq(d.intid, crate::sched::create_rendezvous());
    let vid = crate::virtio::register(
        crate::virtio::Transport::Mmio {
            mmio_phys: d.mmio_phys,
        },
        dma,
        FRAME_SIZE,
        None,
    );
    Some(VirtioBootGrant {
        vid,
        intid: d.intid,
        dma,
    })
}

/// **The inert-configuration page the interactive boot hands the progenitor** (milestone 47
/// (navigation and naming), DECISIONS §111 (inert configuration is a read-only page)):
/// [`boot_clock_page`]'s twin, minus the service.
///
/// Always granted, for the clock page's reason. The page is assembled once, into a frame nothing
/// else can see, and only then handed over, so it needs no seqlock (`environment_contract`). The
/// values are the least-assuming defaults the kernel's `std` harness uses
/// (`system_tests/src/user/std_service.rs`). See notes/progenitor-boot.md.
pub(super) fn boot_config_page() -> u64 {
    let bytes = environment_contract::PageBuilder::new()
        .tz("UTC")
        .expect("UTC is not a recognized environment_contract::domain::KNOWN_TZ member")
        .lang("C")
        .expect("C is not a recognized environment_contract::domain::KNOWN_LANG member")
        .term("dumb")
        .expect("dumb is not a recognized environment_contract::domain::KNOWN_TERM member")
        .build();
    // Zeroed, so a previous occupant's bytes are not visible through the tail past `PAGE_BYTES`.
    let phys = crate::memory::alloc_zeroed()
        .expect("no frame for the config page")
        .addr();
    // SAFETY: `phys` names that frame, direct-mapped and owned by nobody else yet, and `bytes` is
    // `PAGE_BYTES` long, far under `FRAME_SIZE`, so the copy does not run past the frame.
    unsafe {
        core::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            mmu::phys_to_virt(phys) as *mut u8,
            bytes.len(),
        );
    }
    phys
}

/// **The shell's terminal on the screen the firmware left running** (milestone 198's rung 1b):
/// `display_terminal` on that screen, served by `framebuffer_driver`. Returns what the console
/// server needs to write every byte there as well as to the UART.
///
/// The order is the handover: the programs are found, [`crate::console::yield_screen`] stops the
/// kernel painting, and only then is the driver spawned, so there are never two painters. A refusal
/// after the yield leaves the screen blank and the boot on the UART. `None` on aarch64 and riscv64
/// until milestone 157 (real display output on the board) tells the console about a screen. See
/// notes/progenitor-boot.md. **Name provisional.**
pub(super) fn boot_screen_terminal() -> Option<display_service::TerminalWiring> {
    let driver = program("framebuffer_driver")?;
    let terminal = program("display_terminal")?;
    // A framebuffer in kernel memory (`ramfb`'s, in `screen.rs`'s `.bss`) is not handed away, as
    // mapping it would give a driver a window onto kernel statics. Checked before the yield, so a
    // refusal leaves the kernel painting.
    let screen = crate::console::peek_screen()?;
    if crate::screen::is_kernel_memory(&screen) {
        crate::println!(
            "  screen    : kept by the kernel; its framebuffer is kernel memory, not an aperture"
        );
        return None;
    }
    let screen = crate::console::yield_screen()?;
    crate::println!(
        "  screen    : handed to a userspace terminal; the kernel writes the UART alone"
    );
    let w = display_service::start_screen_terminal(driver, terminal, screen)?;
    let [tag, geometry, ..] = crate::sched::ipc_receive(w.driver_report);
    assert_eq!(
        tag,
        graphics_contract::status::UP,
        "the framebuffer driver did not come up ({tag:#x})",
    );
    let [tag, cells, ..] = crate::sched::ipc_receive(w.term_report);
    assert_eq!(
        tag,
        video_terminal::status::TERM_UP,
        "the display terminal did not come up ({tag:#x})",
    );
    // The driver reports its surface in surface pixels; at the screen's scale that is the whole
    // screen (`display_service::start_screen_terminal`), which is what this line says.
    let scale =
        screen_console::ScreenConsole::scale_for_cell(screen.width, video_terminal::CELL_W) as u64;
    crate::println!(
        "  screen    : {}x{} pixels of it at scale {scale} served by framebuffer_driver, a {}x{} \
         terminal on it",
        (geometry & 0xffff_ffff) * scale,
        (geometry >> 32) * scale,
        cells & 0xffff_ffff,
        cells >> 32,
    );
    Some(w)
}
