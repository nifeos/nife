//! **The device starters**: each brings one device up at boot and hands back what `boot_progenitor`
//! grants the progenitor.
//!
//! Cut out of `user.rs` unchanged by milestone 872 (the kernel's user-mode file is split along its
//! seams), which left the parent an index.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

use super::*;

/// **The USB keyboard, when this machine has an xHCI controller** (milestone 242 (USB host and
/// HID)): start its driver and say what it found. `None`, having printed why, for a controller the
/// kernel would not hand over or a driver that failed; `None` silently for a machine with no
/// controller at all, which is every QEMU boot that attached none.
///
/// **A report of no keyboard still grants the attach endpoint**, because the driver keeps
/// watching its ports and a keyboard plugged in after boot is found: the controller is up, so there
/// is something to delegate to.
///
/// **The kernel starts this driver, not the progenitor**, and the reason is the mappings. The
/// driver is handed a dozen register pages and eleven DMA pages as spawn-time mappings, which a
/// process holds no name for and so can neither delegate nor revoke (`non_volatile_memory_express_service`'s
/// choice, for its reason). Built by the progenitor instead, each would be a capability in its
/// table, and a `DeviceFrame` names one page. Name provisional.
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

/// **The clock page the interactive boot hands the progenitor**, and the one place both ISAs agree on what a
/// machine with no clock looks like (milestone 51's wiring; `boot_progenitor`).
///
/// The grant is **unconditional**, and that is the design rather than an oversight. A zeroed page
/// reads as `clock_contract::state::UNKNOWN` (`a_zeroed_page_reads_as_unknown`), so a boot with no
/// `clock` program in its initial ramdisk hands the progenitor a page that honestly says "the
/// machine has no clock it believes" instead of no page at all. That keeps the slot numbering the
/// same on every boot, which matters more than it sounds: the progenitor's capability table is read
/// positionally, and a capability whose *slot* depends on what the machine turned out to have is a
/// wiring nobody can check by reading.
///
/// It is also the DECISIONS §43 split, delivered: the progenitor gets `READ` on a frame. Nothing on this path
/// can hand a child the writable mapping that would let it set the time, because the progenitor never had one.
pub(super) fn boot_clock_page() -> u64 {
    match program("clock") {
        Some(image) => {
            let wiring = clock_service::start(image);
            // The service publishes the RTC reading and *then* announces, with a blocking send. It
            // does not need to be drained for the page to be right, but an undrained announcement
            // parks the service inside it forever, so it would never serve a proposal. One thread
            // whose whole life is that receive costs nothing and leaves the propose endpoint live.
            let report = wiring.report;
            let _ = crate::sched::spawn(move || {
                crate::sched::ipc_receive(report);
                crate::sched::exit();
            });
            wiring.page_phys
        }
        // No `clock` program packed: allocate the page anyway and leave it zeroed. This is the
        // honest unknown clock, and it is the same state `date`'s test allocates deliberately.
        None => crate::memory::alloc_zeroed()
            .expect("no frame for the clock page")
            .addr(),
    }
}

/// The confined transport, the completion interrupt, and the DMA page's physical base, for a
/// virtio device (the rng, or since milestone 590 (provisional) the NIC) this kernel discovered and
/// wired at boot. Returned to the caller rather than
/// stored, because `crate::sched::grant`'s next-free-slot placement means the caller decides
/// exactly where these land relative to whatever else it has already granted.
pub(super) struct VirtioBootGrant {
    /// The `Virtio` capability's id (`crate::virtio::register`'s return value).
    pub(super) vid: usize,
    /// The device's completion interrupt. **Routed** (`crate::sched::bind_irq`) but not yet
    /// **enabled**: the two boards enable an interrupt source differently (aarch64's GIC inline,
    /// riscv64's PLIC pinned to the boot hart's own context via `boot_s_context`, not the
    /// hart-spreading `crate::arch::irq::enable` uses -- `notes/harts-and-pes.md`'s hart lottery
    /// is why, the same reasoning `uart_irq`'s identical two-step split in `boot_progenitor`
    /// already follows), so the caller does that part itself, the same place it already enables
    /// `uart_irq`.
    pub(super) intid: u32,
    /// The DMA region's physical base. `entropy.rs` (and `net_stack`) needs this as a plain value
    /// (it builds virtio
    /// ring descriptors, which are physical-address-based by the spec, not a fact any capability
    /// exposes), and there is no fourth `START` argument word to carry it across the kernel/progenitor
    /// boundary (`start_thread_control_block`'s own `[u64; 3]`, already spent on
    /// `role`/`initial_ramdisk_len`/`fs_rights`). So it travels the way the page's *contents*
    /// already do: written into the page itself at [`VIRTIO_DMA_PHYS_OFFSET`], which the progenitor
    /// reads back out once, after mapping the granted frame briefly, and relays to entropy's own
    /// `arg1` exactly the way it already relays `fs_rights`.
    pub(super) dma: u64,
}

/// **Discover and wire a virtio-rng device on the MMIO bus, for the interactive boot's own use**
/// (DECISIONS §120's 2026-08-26 amendment: "grant the QEMU-only virtio-rng stopgap"). `None` on a
/// boot with no such device: real hardware (milestone 55's actual target has no virtio-rng at all,
/// §120's own text), or a run with `NIFE_RNG` unset. The whole chain past this point treats that
/// exactly as "this boot has no filesystem" is already treated by [`boot_progenitor`]: an absence
/// the caller can act on, not a failure.
///
/// **Only the MMIO transport**, unlike `entropy_service::start`'s own test-harness wiring, which
/// also offers PCIe: a first cut scoped to what an interactive boot actually needs, on the same
/// "a minimal device surface for the boot a person actually meets" posture already named for the
/// GPU/keyboard/NVMe flags (`components/src/login.rs`'s own BUGS, before this amendment). Widening to
/// PCIe (behind the IOMMU) is real follow-on, not invented here.
///
/// Mirrors `kernel::user::entropy_service::start`'s own kernel-side setup (device discovery, a
/// zeroed DMA frame, the interrupt route, `crate::virtio::register`) up to the point that function
/// spawns the service itself: this one hands the three capabilities back for the **caller** to
/// grant and delegate, because on the interactive boot the caller is the progenitor, not the kernel, and the progenitor
/// is the one that builds the entropy service: `crates/system_initializer`'s own ELF loader, the
/// tree's only one (milestone 96), and that crate's own header says why a second loader would be
/// the wrong shape.
pub(super) fn boot_virtio_rng_device() -> Option<VirtioBootGrant> {
    boot_virtio_mmio_device(crate::virtio::find_entropy_device()?)
}

/// **An entropy service on the CPU's own seed instruction, for a boot with no virtio-rng**
/// (milestone 595 (provisional); promoted from the proposal
/// `the-x86-64-progenitor-serves-entropy-from-rdseed`). The request endpoint, which
/// [`boot_progenitor`] grants at slot 16, or `None`, and a `None` is said on the console.
///
/// **The kernel confirms the instruction exists, not the progenitor**, which is the question that
/// proposal left open. Three things decided it, and none of them is effort:
///
/// - It is what the tree already does. `entropy_service::is_instruction_backend_available` reads
///   the feature bit from `arch::isa`'s boot record, and `components/src/entropy.rs` says outright
///   that it trusts its spawner's choice of mode. The installer (`install_service`) takes the same
///   service the same way on the booted `x86_64` path.
/// - It is the only answer that works on aarch64 too. `ID_AA64ISAR0_EL1` is not readable at EL0,
///   so a progenitor that ran `CPUID` itself would be an `x86_64` special case with no aarch64 twin
///   (DECISIONS §19). Here the function is arch-neutral: aarch64 with `FEAT_RNG` and no
///   `NIFE_RNG` takes this path as well, and riscv64 answers `None` from the same predicate.
/// - Detection stays in `kernel/src/arch/` (AGENTS.md's rule 1), and the progenitor stays free of
///   `cfg(target_arch)`.
///
/// **The kernel builds the service rather than telling the progenitor to**, because a fact with
/// no capability has no slot to travel in: the progenitor's three `START` words are spent
/// (`system_initializer::BootEndowment::virtio_rng`'s doc), and a flag in a page
/// would be a second format for one bit. A built service is a capability, and a probe on its slot
/// is how the progenitor already tells every optional grant from an absent one. `fs_ep` is the
/// precedent: the kernel wires the file service before the progenitor exists and grants its
/// endpoint.
///
/// **A refusal, not weaker bytes.** No instruction, or a first draw of all zeros
/// (`entropy_contract::readiness`), and this returns `None` with a sentence saying so. There is no
/// fallback to `RDRAND`/`RNDR` (DRBG output, which `entropy.rs`'s header refuses) and no software
/// generator. The bytes served are the instruction's own, unmixed and not health-tested after the
/// first draw: option A of DECISIONS §137 (a hardware TRNG with no published health-test claim),
/// which is what every other backend here does. Choosing B or C is calef's.
///
/// **`ready` may already be taken.** `entropy_service::ensure` hands the readiness report to
/// whoever wired the service first, and on `x86_64` that can be the installer earlier in this boot.
/// The installer does not read the verdict either, so a condemned service would answer
/// `NO_ENTROPY` to every request rather than serve anything; the progenitor's own first draw (the
/// login password) is then what fails, and it builds no login stack.
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

/// **The same for the network card** (milestone 590 (provisional), the booted system starts its
/// network stack; promoted from the proposal `the-booted-system-has-no-network`). `None` on a boot
/// with no virtio-net device on the MMIO bus: every real board today, and every QEMU run with
/// `NIFE_NET` unset. The progenitor builds `net_stack` from these three and nothing else, exactly
/// as it builds entropy from the rng's three.
///
/// **The MMIO NIC, not the PCIe one**, for [`boot_virtio_rng_device`]'s reason, and with a cost
/// that one does not carry: the MMIO NIC has no IOMMU in front of it, so the confinement of this
/// device's DMA is the transport's shadow-ring validator alone, not the SMMU of DECISIONS §20
/// (IOMMU-backed DMA isolation: one seam, two arch drivers). The runners attach both NICs under
/// `NIFE_NET`; the PCIe one sits unclaimed on this boot. See
/// milestone 590's block for why that is recorded rather than fixed here.
pub(super) fn boot_virtio_net_device() -> Option<VirtioBootGrant> {
    boot_virtio_mmio_device(crate::virtio::find_net_device()?)
}

/// **A network stack over an `e1000e`, when this boot has one and no virtio-net** (milestone 198
/// (a package manager), whose rung 3a fetch had run on aarch64 and riscv64 only; milestone 494 (a
/// driver for the network card a PC actually has) built the driver and named this as its first
/// follow-on). `None`, having said why, for a part no gate has driven, a link that was down, or a
/// bring-up the driver refused; `None` silently with no such NIC, which is every `virt` boot.
///
/// **The kernel builds this one, not the progenitor**, for [`boot_usb_keyboard`]'s reason: the
/// server is handed BAR0's two queue pages and an eighteen-page DMA region as spawn-time mappings,
/// which it holds no name for and so can neither delegate nor revoke
/// (`e1000e_service`'s header). Built by the progenitor, each page would be a capability in its
/// table. It is also the wiring every `e1000e` gate already runs, so the booted system's stack is
/// the tested one rather than a second copy of it.
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
    // The physical base is written into the tail of the same page, at an offset neither driver's
    // ring-and-buffer layout reaches (see [`VIRTIO_DMA_PHYS_OFFSET`]'s own doc).
    write_dma_phys(dma);
    // Routed, not yet enabled; see [`VirtioBootGrant::intid`]'s own doc for why enabling is the
    // caller's job.
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

/// **The inert-configuration page the interactive boot hands the progenitor** (milestone 47's
/// environment-variable fork, DECISIONS §111; `boot_progenitor`).
/// [`boot_clock_page`]'s twin, minus the service: nothing here runs, so there is nothing to spawn
/// and nothing to wait for a report from. The page is assembled once, into a frame nothing else
/// can see, and only then handed to the progenitor; see `environment_contract`'s own docs for why that
/// ordering needs no seqlock.
///
/// The grant is **unconditional**, [`boot_clock_page`]'s own reason: a fixed slot on every boot,
/// whether or not anything downstream ever declares wanting the page, is what lets the progenitor's
/// capability table stay positional. The values are the conservative universal defaults this
/// tree's kernel test harness for `std` programs already uses
/// (`system_tests/src/user/std_service.rs`): "nothing configured this program's locale or terminal, so
/// tell it the least assuming thing" is the honest baseline, the same posture `boot_clock_page`
/// takes for a machine with no RTC. There is no shell-held default config set yet to pass instead
/// (the "inheritance with visibility" shape design/roadmap/0047-navigation-and-naming/environment.md names);
/// this is the fixed default until one exists.
pub(super) fn boot_config_page() -> u64 {
    let bytes = environment_contract::PageBuilder::new()
        .tz("UTC")
        .expect("UTC is not a recognized environment_contract::domain::KNOWN_TZ member")
        .lang("C")
        .expect("C is not a recognized environment_contract::domain::KNOWN_LANG member")
        .term("dumb")
        .expect("dumb is not a recognized environment_contract::domain::KNOWN_TERM member")
        .build();
    // Zeroed before the assembled bytes are written, so nothing left behind by a previous
    // occupant of this physical page is visible through the reserved tail past `PAGE_BYTES`
    // (`ConfigPage` only ever reads the first `PAGE_BYTES`, but a frame's contents are otherwise
    // unspecified until written; the same shape `std_service::start_on_full` uses).
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

/// **The shell's terminal on the screen the firmware left running** (the shell on the firmware
/// screen, milestone 198's rung 1b; `design/roadmap/` has its block).
///
/// Milestone 243 (a machine with no serial port) put the *kernel's* boot tour on a UEFI machine's
/// framebuffer. Since milestone 299 (the serial console becomes a userspace driver) the console is
/// a userspace process that writes COM1, so on a PC with no serial port the tour scrolled past and
/// the prompt appeared nowhere. This puts `display_terminal` on that same screen, served by
/// `framebuffer_driver`, and returns what the progenitor needs to hand the console server so that
/// it writes every byte to the screen as well as to the UART.
///
/// **The order is the handover, and it is the point of the function.** The programs are found
/// first, so a build that lacks one leaves the kernel painting rather than a blank screen. Then
/// [`crate::console::yield_screen`] clears the screen and stops the kernel's `print!` from painting
/// it, under the console lock, and only then is the driver spawned. So there is no moment with two
/// painters, and after this the kernel's own lines (the progenitor's exit, a user fault report) go
/// to the UART alone, including the two this function prints: a line printed on the screen just
/// before the yield would be cleared by it before anybody could read it.
///
/// If the wiring refuses after the yield (a screen too large to map, [`display_service`]'s
/// `MAX_APERTURE_PAGES`), the screen is left blank rather than handed back: the boot goes on over
/// the UART exactly as a machine with no screen does. Recorded here rather than papered over with a
/// second handover path, because the refusal is a bound no screen in the fleet is near.
///
/// Readiness is drained here, the idiom the kernel-built virtio-gpu stack used before milestone 600
/// (provisional) moved it into the progenitor: when this returns, the driver and the terminal are
/// running and the terminal has painted its blank grid.
///
/// Arch-neutral, and `None` on aarch64 and riscv64 today only because nothing there tells the
/// console about a screen: milestone 157 (real display output on the board), the U-Boot
/// `simple-framebuffer` discovery, is what would, and then this function needs no change. **Name
/// provisional.**
pub(super) fn boot_screen_terminal() -> Option<display_service::TerminalWiring> {
    let driver = program("framebuffer_driver")?;
    let terminal = program("display_terminal")?;
    // **A screen this kernel owns the memory of is not handed away** (milestone 243).
    //
    // The handover's whole shape assumes a UEFI aperture: a BAR on a display adapter, memory no
    // part of this kernel is otherwise in, whose physical range `display_service` maps into a
    // userspace driver. `ramfb` broke that assumption on the two `virt` boards, where the
    // framebuffer is `kernel/src/screen.rs`'s own `.bss` and mapping its range into a driver would
    // hand a userspace process a window onto kernel statics.
    //
    // Checked before the yield rather than after, so a refusal leaves the kernel still painting
    // rather than leaving a screen cleared and unclaimed. It is a range test rather than a flag, so
    // milestone 157's U-Boot aperture (outside the kernel image, like the UEFI one) passes without
    // anybody having to remember to set anything.
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
