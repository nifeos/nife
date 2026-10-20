//! **Booting the progenitor**: loading it, measuring it and endowing it with the boot capability
//! set.
//!
//! Cut out of `user.rs` unchanged by milestone 872 (the kernel's user-mode file is split along its
//! seams), which left the parent an index.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

use super::*;
use crate::cap::{Rights, page_frame_cap, page_frame_run_cap, page_frame_run_len, rendezvous_cap};

/// **The archive entry the kernel enters as the first process**, on every architecture.
///
/// One name, one binary, one program (milestone 266). This used to be `init`, and it meant
/// `fixtures/src/hello.rs`'s `init_boot` role on aarch64 and `system_initializer` on riscv64: an alias
/// standing over two implementations of one job, which is DECISIONS §19's own failure mode and had
/// already been paid for once as a boot that reached userspace and printed nothing at all.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const PROGENITOR_ENTRY: &str = "progenitor";

/// The progenitor's stack, in pages (19d.2c): it loads whole ELFs with deep call chains, so its stack is
/// larger than an ordinary process's one page. Also the stack of `hello`'s init roles and the
/// riscv64 serial driver, which share this constant and are far shallower.
///
/// **Fourteen pages (56 KiB) since milestone 809 (the package client becomes a program)**, on
/// 2026-10-10 (UTC); twelve from the first measurement (2026-09-27), eight before. Nothing measured
/// it until `crate::progenitor_stack`'s gauge read peaks on `script/swish-check`. Debug, the build
/// `swish-check` and CI boot, is the deep one, because `system_initializer::boot`'s frame stays
/// live under the spawn service. The latest raise came with a second sender on the spawn loop (an
/// installer endpoint) and a rollback that compares two generations. Measured on fourteen, debug:
///
/// | | aarch64 | riscv64 | x86_64 |
/// |---|---|---|---|
/// | debug, at the prompt (twelve pages) | 19,000 | 18,976 | |
/// | debug, after `jig rollback` | **44,224** | **43,744** | **42,480** |
/// | spare of 57,344 | 13,120 | 13,600 | 14,864 |
///
/// The gauge's floor (`progenitor_stack::HEADROOM_FLOOR`, 8 KiB) is what fails `swish-check`.
/// notes/stack/progenitor-stack.md has the frames, the earlier tables, and why each was a raise
/// rather than a trim (milestone 696 (the spawn service runs outside `boot`'s frame) is the trim).
///
/// Name: ratified 2026-10-18 (calef: "Progenitors numbers are ratified."), replacing `INIT_STACK_PAGES`.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const PROGENITOR_STACK_PAGES: u64 = 14;
const _: () = assert!(PROGENITOR_STACK_PAGES <= address_space_map::MAX_STACK_PAGES);

/// **Boot the system: load the progenitor and start it as the first process, on every architecture**
/// (milestone 166, which merged aarch64's `spawn_progenitor` boot half and `riscv64`/`x86_64`'s
/// `riscv_shell_boot` into this one body).
///
/// It loads [`PROGENITOR_ENTRY`], measures it under the trust root (milestone 22 phase B.1) and the
/// program-measurement table it will check its own loads against (milestone 104), builds its address
/// space (its segments, a deep stack, and the whole initial ramdisk mapped read-only so it can
/// parse and load the rest by name), endows it with the boot capability set, and starts it. From
/// those capabilities and nothing else, `crates/system_initializer` builds the console server, the
/// input driver, the line discipline and the shell out of the progenitor's own budget and wires
/// them together; the kernel touches none of it. It does not block: the progenitor and its children
/// run on the scheduler while the boot thread parks.
///
/// **The capability slot layout is the same on all three architectures** (milestone 166's point):
/// the root budget at slot 0, the console device at 1, the UART receive interrupt at 2, the clock
/// page at 3, the inert-configuration page at 4, the file service and its page at 5 and 6, the
/// virtio-rng trio at 7-9, the graphical terminal stack at 10-12, the virtio-net trio at 13-15, the
/// GPU and keyboard grants at 17-22, the machine statistics page at 23, the progenitor's own
/// address space at 28, the reboot object at 31 and the disk tree's pair at 32 and 33, each when
/// present.
/// `components/src/progenitor.rs`'s single `GRANTS` table reads exactly this.
///
/// **Only two differences are the hardware's, and only those are `#[cfg]`-gated**:
/// - **The console device (slot 1).** aarch64 and riscv64 grant the UART's registers as a
///   `DeviceFrame` (a page), `WRITE|GRANT` so the progenitor maps them into the console and input
///   drivers it builds. `x86_64` has no page for its console (COM1 is port I/O), so it grants a
///   `PortRange` over `0x3F8..=0x3FF` instead (milestone 299, DECISIONS §121 reversed 2026-09-15):
///   the progenitor delegates it with `CAP_INSERT` rather than `MAP_INTO`, and the kernel's TSS I/O
///   bitmap is what lets the drivers' `in`/`out` reach exactly these ports.
/// - **Arming the interrupt controller.** aarch64's GIC has no boot-hart lottery, so its UART line
///   is enabled inline before the thread is built and its virtio-rng source as its caps are inserted;
///   riscv64 enables the PLIC source and supervisor external interrupts only after the driver is
///   running, because the lottery forbids arming earlier (see [`VirtioBootGrant::intid`]); `x86_64`
///   arms nothing here, because it has no userspace input driver to feed until DECISIONS §149.
///
/// Returns the progenitor's thread, so the caller can say how it left.
///
/// Name: ratified 2026-09-15 (calef, this header): `boot` the verb, `progenitor` the noun it acts
/// on, greppable with [`PROGENITOR_ENTRY`]. Refused `boot_via_progenitor`
/// (its `via` disambiguated a separate aarch64 boot path that no longer exists).
// One caller, `boot.rs`'s `hand_over`, which all three architectures reach. The `allow` is kept for
// the configurations that do not: a `soak` or `job_mix` build replaces the hand-off with its own
// workload, and `test`/`bench` park before it.
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput",
        feature = "network_bench"
    ),
    allow(dead_code)
)]
pub fn boot_progenitor(archive: &'static [u8]) -> Result<crate::thread::ThreadId, LoadError> {
    // The UART receive interrupt line, from the machine's own description when it gave one
    // (`memory::uart_irq`), else QEMU virt's constant. The source is printed so a bench transcript
    // names where the number came from: a wrong PLIC source once cost a boot on the JH7110
    // (notes/visionfive2.md, BUGS).
    let (uart_irq, uart_irq_source) = uart_irq_and_source();
    crate::println!("  uart irq  : {uart_irq} ({uart_irq_source})");
    // The stride is the one fact about this UART the userspace drivers cannot see for themselves,
    // and a wrong one is a console that prints nothing at all, so the transcript states it.
    let uart_shape = crate::console::uart_register_shape();
    #[cfg(target_arch = "riscv64")]
    crate::println!(
        "  uart shape: registers {} byte(s) apart, {}-bit access (device tree), passed to the console and input drivers",
        uart_shape.offset(1),
        if uart_shape.is_word_access() { 32 } else { 8 },
    );

    let (ramdisk_start, ramdisk_len) = memory::initial_ramdisk_region().expect("no archive");
    let ramdisk_pages = ramdisk_len.div_ceil(FRAME_SIZE);
    let elf = measured_progenitor(archive)?;

    let aspace_name = progenitor_address_space(&elf, ramdisk_start, ramdisk_pages)?;

    // Route the UART receive interrupt to an endpoint; the input driver's Irq cap will WAIT on it.
    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(uart_irq, irq_ep);
    // aarch64's GIC has no boot-hart-lottery hazard, so its UART line is enabled here, the same
    // place and way the old `spawn_progenitor` armed it. riscv64 must wait until the driver is
    // running (the arming block after the start below). `x86_64` arms COM1's IRQ 4 here too, since
    // milestone 505 (an x86_64 input driver that never lets the core idle): the IO APIC entry goes
    // live now, and nothing raises the line until the input driver sets the 16550's receive enable.
    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    crate::arch::irq::enable(uart_irq);
    let build_region =
        crate::memory_region::create(12288).expect("no building budget for the progenitor");

    let thread_control_block_region = crate::memory_region::create(2).expect("no tcb region");
    let tid =
        crate::sched::create_thread_control_block(thread_control_block_region).expect("no tcb");
    grant_boot_slots(tid, build_region, uart_irq);
    let (fs_rights, fs_pool) = grant_file_service(tid);
    let virtio_rng = grant_entropy_source(tid);
    let (gpu, keyboard) = wire_graphics();
    grant_machine_slots(tid, aspace_name);
    grant_kernel_log(tid);
    grant_usb_keyboard(tid);
    grant_screen_terminal(tid, gpu.is_none());
    grant_graphics(tid, gpu.as_ref(), keyboard.as_ref());
    let virtio_net = grant_virtio_net(tid);
    grant_kernel_network_stack(tid, virtio_net.is_none());
    // **Say that this boot worked**, if a chooser started it (rung 2b of milestone 198's other
    // half). Here and not a line earlier or later, and the position is the mechanism: the
    // filesystem server above has mounted the installed disk and reported ready, which is as late
    // a criterion as this system can evaluate without a person. The installer stages in a transfer
    // window of its own (`install_service::confirm`), so nothing else on the disk constrains when.
    #[cfg(target_arch = "x86_64")]
    if fs_rights != 0 {
        install_service::confirm();
    }
    grant_disk_tree(tid, fs_pool);
    crate::sched::configure_thread_control_block(tid, elf.entry(), USER_STACK_TOP, aspace_name)
        .expect("configure");
    // x0 = the console UART's register shape, which the progenitor hands its console and input
    // drivers (`abi::uart`); x1 = the archive length; x2 = the file-service rights.
    crate::sched::start_thread_control_block(tid, [uart_shape.to_word(), ramdisk_len, fs_rights])
        .expect("start");

    // Arm the interrupt chain so the input driver's keystrokes flow. aarch64 already did its arming
    // inline above (the GIC's UART line before the thread was built, its virtio-rng source as the
    // caps were inserted), because the GIC has no boot-hart lottery. What is left here is the
    // hardware that must wait until the driver is running:
    //
    // riscv64: the source at the PLIC and supervisor external interrupts in `sie`. The input driver
    // arms the NS16550's own RX interrupt (its IER) when it starts, and re-arms the PLIC source
    // through its Irq cap's ACK.
    //
    // `x86_64`: nothing. There is no userspace input driver to feed until DECISIONS §149 chooses a
    // console, so arming COM1's line would deliver keystrokes to nobody.
    #[cfg(target_arch = "riscv64")]
    {
        crate::drivers::plic::enable(uart_irq, crate::arch::irq::boot_s_context());
        // The virtio-rng device's own source, pinned to the same boot-hart context for the same
        // reason (`notes/harts-and-pes.md`'s hart lottery; see [`VirtioBootGrant::intid`]'s own doc).
        if let Some(g) = &virtio_rng {
            crate::drivers::plic::enable(g.intid, crate::arch::irq::boot_s_context());
        }
        // The NIC's, for the same reason (milestone 590 (provisional)).
        if let Some(g) = &virtio_net {
            crate::drivers::plic::enable(g.intid, crate::arch::irq::boot_s_context());
        }
        crate::arch::exceptions::enable_external();
    }
    #[cfg(not(target_arch = "riscv64"))]
    let _ = (&virtio_rng, &virtio_net);
    Ok(tid)
}

/// Read the progenitor out of the archive and check it against the trust root and the program
/// measurements before anything is built from it.
fn measured_progenitor(archive: &'static [u8]) -> Result<Elf<'static>, LoadError> {
    let fs = nifefs::Fs::parse(archive).expect("initial ramdisk is not a nifefs archive");
    let init_bytes = fs
        .read(PROGENITOR_ENTRY)
        .unwrap_or_else(|| panic!("archive has no '{PROGENITOR_ENTRY}' program"));
    // Measured boot (milestone 22 phase B.1): the progenitor is this board's boot program too, so
    // it is in the trust root under its own name and checked here, before its address space is
    // built.
    crate::trust::require(PROGENITOR_ENTRY, init_bytes);
    // And the table it measures the six boot components and every spawnable program against
    // (milestone 104). This is the boot path that genuinely uses it: `crates/system_initializer` is
    // the same code aarch64's the progenitor runs, so the two boards extend the chain by the same lines.
    crate::trust::require_program_measurements(&fs);
    Elf::parse(init_bytes).map_err(LoadError::NotLoadable)
}

/// Build the progenitor's address space (segments, stack, timebase page, initial ramdisk) and
/// register it, returning its name.
fn progenitor_address_space(
    elf: &Elf<'_>,
    ramdisk_start: u64,
    ramdisk_pages: u64,
) -> Result<u64, LoadError> {
    // system_initializer's address space: its segments, a deep stack (it runs an ELF loader that builds three
    // children), and the whole archive mapped read-only so it can load them by name.
    //
    // `log_pages_for(ramdisk_pages)` is the term that was not here before 2026-09-21: the
    // archive's pages are mapped with `map_physical`, which records now, and a record is paid for
    // out of this space's own region like the page tables beside it. It is the largest single such
    // window in the tree (the aarch64 archive is a few thousand pages), so it is the one place the
    // cost is visible rather than lost in `AS_OVERHEAD`'s slack. See
    // `crate::revoke::log_pages_for`.
    let content: u64 = elf
        .segments()
        .map(|seg| {
            let (s, e) = seg.page_range(FRAME_SIZE);
            (e - s) / FRAME_SIZE
        })
        .sum::<u64>()
        + 1
        + ramdisk_pages / 512
        + crate::revoke::log_pages_for(ramdisk_pages)
        + PROGENITOR_STACK_PAGES
        + 8;
    let mut space =
        AddressSpace::new(content).ok_or(LoadError::Unmappable(MapError::OutOfPageFrames))?;
    map_segments(&mut space, elf)?;
    for k in 0..PROGENITOR_STACK_PAGES {
        let page = space
            .map_new(USER_STACK_VA - k * FRAME_SIZE, Flags::user_data())
            .map_err(LoadError::Unmappable)?;
        // Painted so the kernel can say how deep this stack has ever been: see
        // `crate::progenitor_stack`, and `script/swish-check`, which fails on too little headroom.
        crate::progenitor_stack::paint_page(k, page);
    }
    // The timebase page, which [`load`] maps for every process it builds and a hand-built
    // address space has to map for itself (see [`map_timebase_page`] for the six call sites
    // that each found this as a page fault). The progenitor reads the clock like any program does.
    #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
    map_timebase_page(&mut space).map_err(LoadError::Unmappable)?;
    for i in 0..ramdisk_pages {
        space
            .map_physical(
                INITIAL_RAMDISK_VA + i * FRAME_SIZE,
                ramdisk_start + i * FRAME_SIZE,
                Flags::user_rodata(),
                crate::revoke::PageMapSource::NoCapability,
            )
            .map_err(LoadError::Unmappable)?;
    }
    Ok(readopt_user_address_space(space).expect("register the progenitor's address space"))
}

/// Slots 0 to 4, granted on every boot: the root budget, the console device, the UART interrupt,
/// the clock page and the inert-configuration page.
fn grant_boot_slots(tid: crate::thread::ThreadId, build_region: u64, uart_irq: u32) {
    // slot 0: the delegable root budget (milestone 31), GRANT included so the progenitor can split
    // off a budget for the shell and hand it on; rights only narrow downward. slot 1: the console
    // device (see below), WRITE|GRANT so the progenitor delegates it into the console and input
    // drivers. slot 2: the UART Irq, READ|GRANT so it can delegate it to input.
    let s0 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::memory_region_root_cap(build_region),
        None,
    )
    .expect("insert budget");
    assert_eq!(s0, 0);
    // Slot 1: the console device. On aarch64 and riscv64 it is the UART's registers as a
    // `DeviceFrame` (a page); on `x86_64` the console is port I/O with no page, so it is a
    // `PortRange` over COM1's eight ports instead. See this function's doc for the full split.
    #[cfg(not(target_arch = "x86_64"))]
    let uart_slot = crate::cap::device_frame_cap(UART_PHYS, Rights::WRITE.union(Rights::GRANT));
    // **On `x86_64` the console is a port range, not a page** (milestone 299, DECISIONS §121
    // reversed 2026-09-15). COM1's 16550 lives at I/O ports `0x3F8..=0x3FF`, so `uart_dev` names a
    // `PortRange` capability rather than a device page: the progenitor holds it with `GRANT` and
    // delegates it (`CAP_INSERT`, not `MAP_INTO`) into the console and input drivers it builds, and
    // the kernel's TSS I/O bitmap is what lets their `in`/`out` reach exactly these ports.
    #[cfg(target_arch = "x86_64")]
    let uart_slot = crate::cap::port_range_cap(
        X86_COM1_PORT_BASE,
        X86_COM1_PORT_COUNT,
        Rights::WRITE.union(Rights::GRANT),
    );
    let s1 = crate::sched::thread_control_block_insert_cap(tid, uart_slot, Some(1))
        .expect("insert uart device");
    assert_eq!(s1, 1);
    let s2 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::irq_cap_rights(uart_irq, Rights::READ.union(Rights::GRANT)),
        Some(2),
    )
    .expect("insert uart irq");
    assert_eq!(s2, 2);
    // The clock page (slot 3), read-only, ahead of the filesystem pair so its number is the same on
    // every boot. `READ` is DECISIONS §43's split at this boundary: the progenitor can endow a reader and holds
    // nothing that could set the time. See [`boot_clock_page`].
    let s3 = crate::sched::thread_control_block_insert_cap(
        tid,
        page_frame_cap(boot_clock_page(), Rights::READ.union(Rights::GRANT)),
        Some(3),
    )
    .expect("insert the clock page");
    assert_eq!(s3, 3);
    // The inert-configuration page (slot 4), `clock`'s twin (milestone 47, DECISIONS §111): ahead
    // of the filesystem pair for the identical reason, its slot must not depend on whether a disk
    // was attached. See [`boot_config_page`].
    let s4 = crate::sched::thread_control_block_insert_cap(
        tid,
        page_frame_cap(boot_config_page(), Rights::READ.union(Rights::GRANT)),
        Some(4),
    )
    .expect("insert the config page");
    assert_eq!(s4, 4);
}

/// Slots 5 and 6, the file service and its shared page, when this boot has a filesystem; returns
/// the rights the progenitor is told the endpoint holds (0 for none) and the client-window pool's
/// physical base.
fn grant_file_service(tid: crate::thread::ThreadId) -> (u64, Option<u64>) {
    // The file service (slot 5) and the page its clients share with it (slot 6), when this boot has
    // a filesystem (milestone 50). GRANT on both, because the progenitor's job with them is to delegate: it
    // narrows the endpoint into the shell and maps the frame into its address space. `a2` carries
    // the rights the endpoint holds, which is also how the progenitor is told there is one at all. `None` is
    // the ordinary case for a run with no RedoxFS disk attached.
    match program("redoxfs_server").and_then(|redoxfs_server| {
        fs_service::root_directory(fs_service::blk_server_image(), redoxfs_server)
    }) {
        Some((file_ep, file_shared)) => {
            let s5 = crate::sched::thread_control_block_insert_cap(
                tid,
                rendezvous_cap(file_ep, Rights::WRITE.union(Rights::GRANT)),
                Some(5),
            )
            .expect("insert the file service");
            assert_eq!(s5, 5);
            let s6 = crate::sched::thread_control_block_insert_cap(
                tid,
                // **The whole client-window pool, one run** (milestone 599 (a frame per filesystem
                // client channel), calef's option-4 ruling of 2026-09-27): every window, in the
                // slot that held window 0's page. The progenitor never maps it into a client; it
                // slices one window per client with `abi::page_frame::SLICE` and deletes the slice
                // once the client is built.
                page_frame_run_cap(
                    file_shared,
                    page_frame_run_len(fs_service::FILE_POOL_PAGES),
                    Rights::WRITE.union(Rights::GRANT),
                ),
                Some(6),
            )
            .expect("insert the shared file page");
            assert_eq!(s6, 6);
            (filesystem_contract::dir::ALL, Some(file_shared))
        }
        None => (0, None),
    }
}

/// Slots 7 to 9 for a virtio-rng device, or slot 16 for the instruction entropy service when there
/// is none; returns the virtio-rng grant for the interrupt arming after the start.
fn grant_entropy_source(tid: crate::thread::ThreadId) -> Option<VirtioBootGrant> {
    // The virtio-rng device (slots 7-9, always, even without a disk), when this boot has one
    // (DECISIONS §120's 2026-08-26 amendment). GRANT on all three so system_initializer can
    // delegate them onward to an entropy service it builds, the same shape every other device
    // authority here already takes. `None` exactly as the filesystem pair can be:
    // system_initializer's own probe (`invoke` on an ungranted slot answers `NoSuchSlot`) is what
    // tells it apart from a real one. See [`boot_virtio_rng_device`] for what wiring it costs and
    // why it hands back an un-enabled interrupt.
    //
    // **Explicit slots, not `None`'s first-free** (`thread_control_block_insert_cap`'s own second
    // sense, `notes/abi.md` §4's "the emptiness is load-bearing"), and this is not a style choice:
    // the filesystem pair above is itself conditional, and first-free numbering would silently
    // shift these three down by two on the (ordinary) boot that has virtio-rng but no attached
    // disk. Explicit targets keep slots 7-9 the virtio-rng trio's own regardless of what the
    // filesystem pair did or did not consume. Fixed past the pair's own max reach (slot 6), not
    // slot 5, because the inert-configuration page (slot 4) shifted that pair down by one.
    let virtio_rng = boot_virtio_rng_device();
    if let Some(g) = &virtio_rng {
        let s7 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::virtio_cap_rights(g.vid, Rights::WRITE.union(Rights::GRANT)),
            Some(7),
        )
        .expect("insert the virtio-rng transport");
        assert_eq!(s7, 7);
        let s8 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::irq_cap_rights(g.intid, Rights::READ.union(Rights::GRANT)),
            Some(8),
        )
        .expect("insert the virtio-rng interrupt");
        assert_eq!(s8, 8);
        let s9 = crate::sched::thread_control_block_insert_cap(
            tid,
            page_frame_cap(
                g.dma,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            Some(9),
        )
        .expect("insert the virtio-rng DMA page");
        assert_eq!(s9, 9);
        // aarch64's GIC has no hart lottery, so its virtio-rng source is enabled here, inline,
        // exactly as the old `spawn_progenitor` did before the thread started. riscv64's is armed at
        // the PLIC in the block after the start below; `x86_64` arms nothing.
        #[cfg(target_arch = "aarch64")]
        crate::arch::irq::enable(g.intid);
    }
    // **Or the CPU's own seed instruction** (slot 16, milestone 595 (provisional)), when there is
    // no virtio-rng: an entropy service the *kernel* built and proved, granted as its request
    // endpoint, the way the file service in slot 5 is. See [`boot_instruction_entropy`] for why the
    // kernel builds this one rather than the progenitor, and why the grant is never both.
    if virtio_rng.is_none()
        && let Some(request) = boot_instruction_entropy()
    {
        let s16 = crate::sched::thread_control_block_insert_cap(
            tid,
            rendezvous_cap(request, Rights::WRITE.union(Rights::GRANT)),
            Some(16),
        )
        .expect("insert the instruction entropy service");
        assert_eq!(s16, 16);
    }
    virtio_rng
}

/// Wire the virtio-gpu and virtio keyboard when present, and say what this boot has.
fn wire_graphics() -> (
    Option<display_service::GpuDevice>,
    Option<keyboard_service::KeyboardDevice>,
) {
    // **The graphical terminal stack's raw materials** (milestone 600 (provisional); milestone 177 (wire the graphical terminal stack into the real interactive boot)
    // built the stack itself here), when a virtio-gpu is attached: the gpu's confined transport,
    // interrupt and DMA run, and the surface run inside it, in slots 17, 18, 19 and 12. Nothing
    // is built from them at boot (milestone 632 (provisional), calef's 2026-09-30 ruling: the
    // boot stays the minimal UART system and graphics is launched from the swish prompt); the
    // progenitor hands all seven to the shell, which holds them until a `graphical_terminal` session's spawn
    // hands them back for the drivers to be built from, exactly as it hands the machine
    // statistics page to a session that may delegate it.
    //
    // **Why the kernel grants rather than builds.** Milestone 177 spawned the stack kernel-side
    // because a virtio-gpu "needed eleven capability-table slots, one `PageFrame` per DMA page".
    // DECISIONS §102 (a Frame names a run of pages) had already given `PageFrame` a page count,
    // and milestone 142 (a text display good enough that people use it instead of a GUI) made the gpu's
    // region one run and one capability (`display_service::wire_device`), so the premise had
    // expired: the gpu is four capabilities here, and the keyboard three.
    //
    // **Two frame capabilities over one region, deliberately.** The driver gets the whole run and
    // the terminal only the surface after its first page, which is the split
    // `display_service::start_terminal` already makes; nothing lets a holder narrow a run, so the
    // kernel mints both. `None` with no GPU on the bus, or no `gpu_driver` or `display_terminal`
    // in the archive: no device is wired for programs this boot could never run.
    let gpu = if program("gpu_driver").is_some() && program("display_terminal").is_some() {
        display_service::wire_device()
    } else {
        None
    };
    // **And a virtio keyboard, when this boot has a gpu too** (slots 20-22), the rng trio's
    // shape exactly. `None` is milestone 192 (a keyboard on real silicon)'s option A, not an
    // absence: a `graphical_terminal` session then takes its keystrokes from the boot's own UART line
    // discipline, at launch rather than at boot. Wired only beside a GPU, because a keyboard
    // with no screen has no terminal to type into on this boot.
    let keyboard = if gpu.is_some() && program("keyboard_driver").is_some() {
        keyboard_service::wire_device()
    } else {
        None
    };
    if gpu.is_some() {
        crate::println!(
            "  graphics  : a virtio-gpu and {}; the spawn service holds the grants, a `graphical_terminal` launch builds from them",
            if keyboard.is_some() {
                "a virtio keyboard"
            } else {
                "no keyboard (a graphical terminal session's keystrokes come over the UART)"
            }
        );
    }
    (gpu, keyboard)
}

/// Slots 23, 28 and 31, granted on every boot: the machine statistics page, the progenitor's own
/// address space and the reboot object.
fn grant_machine_slots(tid: crate::thread::ThreadId, aspace_name: u64) {
    // **The machine statistics page** (slot 23, milestone 126 (the `procps` package), DECISIONS §225
    // (`free` sees the machine and your share) part 2), the config page's shape: a frame the kernel
    // keeps its machine-wide counters in, granted unconditionally so its slot never moves, and
    // `READ | GRANT` so the progenitor can hand it to the boot shell and let nobody write it.
    //
    // Slot 23 because every slot below it is named by a grant some boot makes, and the page is
    // granted on every boot. It was the kernel's fault slot until calef raised the table to 32 on
    // 2026-09-27 for exactly this (`crate::cap::CAPABILITY_TABLE_SLOTS`). See
    // `crate::machine_statistics`.
    let s23 = crate::sched::thread_control_block_insert_cap(
        tid,
        page_frame_cap(
            crate::machine_statistics::page_phys(),
            Rights::READ.union(Rights::GRANT),
        ),
        Some(23),
    )
    .expect("insert the machine statistics page");
    assert_eq!(s23, 23);
    // **The progenitor's own address space** (slot 28, §249 (a running address space stays
    // nameable), its 2026-10-05 amendment; field name `own_space`, provisional). `WRITE` alone, so
    // the progenitor can `UNMAP` each scratch page it filled for a child once the page is in the
    // child (milestone 95 (an unmap primitive)), and `MAP_INTO` its own space, which `PageFrame::MAP`
    // already let it do. No `GRANT`: nothing else is ever handed authority over the progenitor's
    // memory, and a right it does not hold is one it cannot pass on by mistake. No `ENUMERATE`
    // either, because nothing it does needs to list itself.
    //
    // Granted on every boot so its slot never moves, past the USB keyboard's conditional slot 27 for
    // the reason every conditional group above gives. It names `aspace_name`, which `CONFIGURE`
    // below no longer retires (§249's option A), so this capability goes on resolving while the
    // progenitor runs and goes dead the moment its thread is reaped.
    let s28 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::address_space_cap(aspace_name, Rights::WRITE),
        Some(28),
    )
    .expect("insert the progenitor's own address space");
    assert_eq!(s28, 28);
    // **The reboot object** (slot 31, milestone 805 (`reboot` at the prompt), DECISIONS §251
    // (restarting the machine is a kernel object the progenitor hands out)): the one capability
    // that may restart the machine, minted here and nowhere else. `WRITE | GRANT`: the progenitor
    // never invokes it, but it places `WRITE` in the one child whose manifest declares `reboot`,
    // and delegation only narrows, so the right it hands on has to be one it holds (a `GRANT`-only
    // grant here made every `reboot` spawn fail, found by the first run that typed `reboot`).
    // The method itself checks no right, as §251 says. Granted on every boot so its slot
    // never moves, past `net_stack_report`'s conditional slot 30 for the reason every group above
    // gives. Field name `reboot`, provisional.
    let s31 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::reboot_capability(Rights::WRITE.union(Rights::GRANT)),
        Some(31),
    )
    .expect("insert the reboot object");
    assert_eq!(s31, 31);
}

/// Slots 32 and 33, the disk tree and its client-window pool, when this machine has an NVMe disk
/// (milestone 834 (ioping on nife and Linux)): the installed system's second mounted tree (calef, 2026-10-18 21:36 UTC, "Yes,
/// read-only at prompt"). `WRITE | GRANT` on both for the file service's reason at slots 5 and 6:
/// the progenitor narrows the endpoint into the shell and slices the pool, one window per client.
/// Past the reboot object's slot 31, so no earlier slot moves. Only beside a file service, whose
/// window 0 the shell's one page is and the disk tree's window 0 becomes.
fn grant_disk_tree(tid: crate::thread::ThreadId, fs_pool: Option<u64>) {
    let Some(tree) = fs_pool.and_then(super::disk_tree_service::start) else {
        return;
    };
    let s32 = crate::sched::thread_control_block_insert_cap(
        tid,
        rendezvous_cap(tree.request, Rights::WRITE.union(Rights::GRANT)),
        Some(32),
    )
    .expect("insert the disk tree");
    assert_eq!(s32, 32);
    let s33 = crate::sched::thread_control_block_insert_cap(
        tid,
        page_frame_run_cap(
            tree.pool,
            page_frame_run_len(super::disk_tree_service::POOL_PAGES as u64),
            Rights::WRITE.union(Rights::GRANT),
        ),
        Some(33),
    )
    .expect("insert the disk tree's client windows");
    assert_eq!(s33, 33);
}

/// Slots 24 to 26: the kernel's log ring, its cursor page and its notification.
fn grant_kernel_log(tid: crate::thread::ThreadId) {
    // **The kernel's ring, its cursor page and its notification** (slots 24 to 26, milestone 342
    // (the kernel and the `console` server drive one UART from two address spaces), calef's
    // ruling F): the ring read-only so the log service can copy kernel lines out and never write
    // them, the cursor read-write so it can say how far it has read, and the notification the
    // kernel signals on append, which the progenitor binds to the service's thread. All three
    // carry `GRANT`, because the progenitor is the spawner and not the reader. Empty on a boot that
    // could not allocate them, which keeps the kernel's old behaviour: every line direct.
    crate::kernel_log::publish();
    if let Some((ring, cursor, notification)) = crate::kernel_log::grants() {
        for (capability, slot, what) in [
            (
                page_frame_run_cap(
                    ring,
                    core::num::NonZeroU64::new(system_log_contract::kernel_ring::PAGES as u64)
                        .expect("the ring has pages"),
                    Rights::READ.union(Rights::GRANT),
                ),
                24,
                "the kernel's ring",
            ),
            (
                page_frame_cap(
                    cursor,
                    Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
                ),
                25,
                "the kernel ring's cursor page",
            ),
            (
                crate::cap::notification_cap(
                    notification,
                    Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
                ),
                26,
                "the kernel ring's notification",
            ),
        ] {
            let s = crate::sched::thread_control_block_insert_cap(tid, capability, Some(slot))
                .unwrap_or_else(|_| panic!("insert {what}"));
            assert_eq!(s, slot, "{what} landed in the wrong slot");
        }
    }
}

/// Slot 27: a USB keyboard's attach endpoint, when the kernel started one.
fn grant_usb_keyboard(tid: crate::thread::ThreadId) {
    // **A USB keyboard** (slot 27, milestone 242 (USB host and HID)): the attach endpoint of a
    // driver the kernel already started on the machine's xHCI controller, `WRITE | GRANT`, so the
    // progenitor can delegate the line discipline's endpoint through it once it has built one and
    // then delete its own copy. Past the kernel ring's floor (slot 26) for the reason every
    // conditional group above gives. Empty on a machine with no controller, one the kernel refused,
    // or one whose driver reported a failure (nobody would receive the delegation). See
    // [`boot_usb_keyboard`].
    let usb_keyboard = boot_usb_keyboard();
    if let Some(k) = &usb_keyboard {
        let s27 = crate::sched::thread_control_block_insert_cap(
            tid,
            rendezvous_cap(k.attach, Rights::WRITE.union(Rights::GRANT)),
            Some(27),
        )
        .expect("insert the USB keyboard's attach endpoint");
        assert_eq!(s27, 27);
    }
}

/// Slots 10 and 11: a terminal on the firmware's screen, when there is no GPU.
fn grant_screen_terminal(tid: crate::thread::ThreadId, no_gpu: bool) {
    // **Or a terminal on the screen the firmware left running** (the shell on the firmware screen,
    // milestone 198's rung 1b), when there is no GPU: slots 10 and 11, the terminal's endpoint and
    // its output page. (They were the graphical stack's slots too, until milestone 600
    // (provisional) moved that stack's construction into the progenitor.) `None` on every machine whose
    // console has no screen, which is every boot but a UEFI one today. See
    // [`boot_screen_terminal`].
    let screen = if no_gpu { boot_screen_terminal() } else { None };
    if let Some(t) = &screen {
        let s10 = crate::sched::thread_control_block_insert_cap(
            tid,
            rendezvous_cap(t.term, Rights::WRITE.union(Rights::GRANT)),
            Some(10),
        )
        .expect("insert the screen terminal's endpoint");
        assert_eq!(s10, 10);
        let s11 = crate::sched::thread_control_block_insert_cap(
            tid,
            page_frame_cap(
                t.out,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            Some(11),
        )
        .expect("insert the screen terminal's output page");
        assert_eq!(s11, 11);
    }
}

/// Slots 12 and 17 to 22: the gpu's and keyboard's raw materials, for a `graphical_terminal` launch
/// to build from.
fn grant_graphics(
    tid: crate::thread::ThreadId,
    gpu: Option<&display_service::GpuDevice>,
    keyboard: Option<&keyboard_service::KeyboardDevice>,
) {
    let insert = |granted, slot: u64, what: &str| {
        let s = crate::sched::thread_control_block_insert_cap(tid, granted, Some(slot))
            .unwrap_or_else(|_| panic!("insert {what}"));
        assert_eq!(s, slot, "{what} landed in the wrong slot");
    };
    if let Some(g) = gpu {
        insert(
            page_frame_run_cap(
                g.surface,
                display_service::GPU_SURFACE_RUN,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            12,
            "the gpu's surface run",
        );
        insert(
            crate::cap::virtio_cap_rights(g.vid, Rights::WRITE.union(Rights::GRANT)),
            17,
            "the gpu transport",
        );
        insert(
            crate::cap::irq_cap_rights(g.intid, Rights::READ.union(Rights::GRANT)),
            18,
            "the gpu interrupt",
        );
        insert(
            page_frame_run_cap(
                g.dma,
                display_service::GPU_DMA_RUN,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            19,
            "the gpu's DMA run",
        );
    }
    if let Some(k) = keyboard {
        insert(
            crate::cap::virtio_cap_rights(k.vid, Rights::WRITE.union(Rights::GRANT)),
            20,
            "the keyboard transport",
        );
        insert(
            crate::cap::irq_cap_rights(k.intid, Rights::READ.union(Rights::GRANT)),
            21,
            "the keyboard interrupt",
        );
        insert(
            page_frame_cap(
                k.dma,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            22,
            "the keyboard's DMA page",
        );
    }
}

/// Slots 13 to 15 for a virtio-net device; returns the grant for the interrupt arming after the
/// start.
fn grant_virtio_net(tid: crate::thread::ThreadId) -> Option<VirtioBootGrant> {
    // **The network card** (slots 13-15, milestone 590 (provisional)), when this boot has a
    // virtio-net device on the MMIO bus: the virtio-rng trio's shape exactly, three slots past the
    // graphical stack's own, explicit for the same reason those are (every conditional grant
    // before these would otherwise shift them). GRANT on all three, because the progenitor's only
    // use for them is to delegate them into the `net_stack` it builds and then delete its own
    // copies; see `crates/system_initializer`'s network block. `None` on every real board today and
    // on any run with `NIFE_NET` unset, and the progenitor's probe tells that apart the way it
    // tells a missing virtio-rng apart. See [`boot_virtio_net_device`].
    let virtio_net = boot_virtio_net_device();
    if let Some(g) = &virtio_net {
        let s13 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::virtio_cap_rights(g.vid, Rights::WRITE.union(Rights::GRANT)),
            Some(13),
        )
        .expect("insert the virtio-net transport");
        assert_eq!(s13, 13);
        let s14 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::irq_cap_rights(g.intid, Rights::READ.union(Rights::GRANT)),
            Some(14),
        )
        .expect("insert the virtio-net interrupt");
        assert_eq!(s14, 14);
        let s15 = crate::sched::thread_control_block_insert_cap(
            tid,
            page_frame_cap(
                g.dma,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            Some(15),
        )
        .expect("insert the virtio-net DMA page");
        assert_eq!(s15, 15);
        // aarch64 arms inline, riscv64 at the PLIC after the start below: the virtio-rng
        // source's split, for its reason.
        #[cfg(target_arch = "aarch64")]
        crate::arch::irq::enable(g.intid);
    }
    virtio_net
}

/// Slots 29 and 30: a network stack the kernel built itself, when there is no virtio-net NIC.
fn grant_kernel_network_stack(tid: crate::thread::ThreadId, no_virtio_net: bool) {
    // **Or a stack the kernel already built on an `e1000e`** (slots 29 and 30, milestone 198 (a
    // package manager): rung 3a on x86_64), when there is no virtio-net NIC: the `Stack` endpoint,
    // `READ | WRITE | GRANT` as the progenitor's own retype of it is on the virtio path, and the
    // endpoint the lease arrives on, `READ`. The progenitor receives the lease itself, exactly as
    // it does from a stack it built, so both paths reach the prompt through one line of code. Never
    // both: a boot with the virtio trio leaves these empty. See [`boot_e1000e_network`] for why the
    // kernel builds this one.
    //
    // **Or one built on radon's Ethernet port** (milestone 53 (the board's own peripherals: network
    // and storage on real silicon)), in the same two slots, when there is neither. Gated by
    // `designware_ethernet_service::PROVEN_ON_SILICON`, which one bench boot has set.
    let kernel_stack = if no_virtio_net {
        let e1000e = boot_e1000e_network().map(|w| (w.stack, w.report));
        #[cfg(target_arch = "riscv64")]
        let e1000e = e1000e.or_else(boot_designware_network);
        e1000e
    } else {
        None
    };
    if let Some((stack, report)) = kernel_stack {
        let s29 = crate::sched::thread_control_block_insert_cap(
            tid,
            rendezvous_cap(stack, Rights::ALL),
            Some(29),
        )
        .expect("insert the kernel-built stack's endpoint");
        assert_eq!(s29, 29);
        let s30 = crate::sched::thread_control_block_insert_cap(
            tid,
            rendezvous_cap(report, Rights::READ),
            Some(30),
        )
        .expect("insert the kernel-built stack's lease endpoint");
        assert_eq!(s30, 30);
    }
}
