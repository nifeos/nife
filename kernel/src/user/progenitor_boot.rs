//! **Booting the progenitor**: loading it, measuring it and endowing it with the boot capability
//! set. See notes/progenitor-boot.md.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

use super::*;
use crate::cap::{Rights, page_frame_cap, page_frame_run_cap, page_frame_run_len, rendezvous_cap};

/// **The archive entry the kernel enters as the first process**, on every architecture: one name,
/// one binary, one program (milestone 266).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const PROGENITOR_ENTRY: &str = "progenitor";

/// The progenitor's stack, in pages (19d.2c): it loads whole ELFs with deep call chains, so its
/// stack is larger than an ordinary process's one page. The ELF child builders and the riscv64
/// serial driver share this constant and are far shallower.
///
/// Fourteen pages (56 KiB), from `crate::progenitor_stack`'s measured peaks on
/// `script/swish-check`. Debug is the deep build, at about 44 KiB after `jig rollback` on all three
/// architectures, and the gauge's floor (`progenitor_stack::HEADROOM_FLOOR`, 8 KiB) is what fails
/// `swish-check`. notes/stack/progenitor-stack.md has the frames and the tables.
///
/// Name: ratified 2026-10-18 (calef: "Progenitors numbers are ratified."), replacing `INIT_STACK_PAGES`.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const PROGENITOR_STACK_PAGES: u64 = 14;
const _: () = assert!(PROGENITOR_STACK_PAGES <= address_space_map::MAX_STACK_PAGES);

/// **Boot the system: load the progenitor and start it as the first process, on every
/// architecture.**
///
/// `archive` is the initial ramdisk, which must hold [`PROGENITOR_ENTRY`]. This measures the
/// progenitor, builds its address space (its segments, a deep stack and the whole archive
/// read-only), endows it with the boot capability set and starts it, without blocking. The slot
/// layout is one on all three architectures (notes/progenitor-boot.md has it).
///
/// Returns the progenitor's thread, or the [`LoadError`] that refused its image.
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
    // From the machine's description when it gave one, else QEMU virt's constant. The source is
    // printed so a bench transcript names it (a wrong PLIC source cost a boot on the JH7110).
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
    // aarch64's GIC has no boot-hart lottery, so its line is armed here; riscv64's waits until the
    // driver runs. On `x86_64` the IO APIC entry goes live now, and nothing raises the line until
    // the input driver sets the 16550's receive enable.
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
    // Say that this boot worked, if a chooser started it (milestone 198's rung 2b). Here and no
    // earlier: the filesystem server above has mounted the installed disk and reported ready, the
    // latest criterion this system can evaluate without a person.
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

    // riscv64 arms only once the driver is running, because the boot-hart lottery forbids arming
    // earlier (`notes/harts-and-pes.md`, [`VirtioBootGrant::intid`]): the UART's PLIC source, the
    // virtio-rng device's and the NIC's, all pinned to the boot hart, then external interrupts.
    #[cfg(target_arch = "riscv64")]
    {
        crate::drivers::plic::enable(uart_irq, crate::arch::irq::boot_s_context());
        if let Some(g) = &virtio_rng {
            crate::drivers::plic::enable(g.intid, crate::arch::irq::boot_s_context());
        }
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
    // The progenitor is this board's boot program, so it is in the trust root under its own name
    // and checked before its address space is built (milestone 22 phase B.1).
    crate::trust::require(PROGENITOR_ENTRY, init_bytes);
    // And the table it measures every program it loads against (milestone 104).
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
    // Segments, a deep stack, the archive's page tables and its mapping log (the archive is the
    // largest window `map_physical` records: `crate::revoke::log_pages_for`), and slack.
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
    // [`load`] maps the timebase page for the processes it builds; a hand-built space maps its own.
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
///
/// The budget carries `GRANT` so the progenitor can split budgets off for its children. The console
/// device is the UART's register page on aarch64 and riscv64 and a `PortRange` over COM1's eight
/// ports on `x86_64`. The clock and configuration pages are `READ | GRANT` (DECISIONS §43, §111)
/// and sit ahead of the conditional file-service pair, so their slots never move.
fn grant_boot_slots(tid: crate::thread::ThreadId, build_region: u64, uart_irq: u32) {
    let s0 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::memory_region_root_cap(build_region),
        None,
    )
    .expect("insert budget");
    assert_eq!(s0, 0);
    #[cfg(not(target_arch = "x86_64"))]
    let uart_slot = crate::cap::device_frame_cap(UART_PHYS, Rights::WRITE.union(Rights::GRANT));
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
    let s3 = crate::sched::thread_control_block_insert_cap(
        tid,
        page_frame_cap(boot_clock_page(), Rights::READ.union(Rights::GRANT)),
        Some(3),
    )
    .expect("insert the clock page");
    assert_eq!(s3, 3);
    let s4 = crate::sched::thread_control_block_insert_cap(
        tid,
        page_frame_cap(boot_config_page(), Rights::READ.union(Rights::GRANT)),
        Some(4),
    )
    .expect("insert the config page");
    assert_eq!(s4, 4);
}

/// Slots 5 and 6, the file service and its client-window pool, when this boot has a filesystem
/// (milestone 50).
///
/// Both carry `GRANT`: the progenitor narrows the endpoint into the shell, and slices one window per
/// client out of the pool with `abi::page_frame::SLICE` (milestone 599). Returns the endpoint's
/// rights, which `a2` tells the progenitor (0 for no file service, the ordinary case with no RedoxFS
/// disk attached), and the pool's physical base.
fn grant_file_service(tid: crate::thread::ThreadId) -> (u64, Option<u64>) {
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
///
/// The trio carries `GRANT` so the progenitor can delegate it to the entropy service it builds; its
/// probe of an ungranted slot (`NoSuchSlot`) is how it tells an absent device. The slots are
/// explicit rather than first-free, because the file-service pair before them is conditional. See
/// [`boot_virtio_rng_device`], [`boot_instruction_entropy`] and notes/progenitor-boot.md.
fn grant_entropy_source(tid: crate::thread::ThreadId) -> Option<VirtioBootGrant> {
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
        // aarch64's GIC has no hart lottery, so its source is armed here; riscv64's after the start.
        #[cfg(target_arch = "aarch64")]
        crate::arch::irq::enable(g.intid);
    }
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
///
/// Nothing is built from them at boot (milestone 632): the progenitor hands the grants to the
/// shell, and a `graphical_terminal` launch builds the drivers. No gpu is wired without `gpu_driver`
/// and `display_terminal` in the archive, and no keyboard without a gpu beside it; a graphical
/// session's keystrokes then come over the UART (milestone 192's option A). See
/// notes/progenitor-boot.md.
fn wire_graphics() -> (
    Option<display_service::GpuDevice>,
    Option<keyboard_service::KeyboardDevice>,
) {
    let gpu = if program("gpu_driver").is_some() && program("display_terminal").is_some() {
        display_service::wire_device()
    } else {
        None
    };
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

/// Slots 23, 28 and 31, granted on every boot so they never move: the machine statistics page, the
/// progenitor's own address space and the reboot object.
///
/// The statistics page is `READ | GRANT` (DECISIONS §225). The address space is `WRITE` alone
/// (§249), so the progenitor can `UNMAP` the scratch pages it filled for a child and can pass the
/// right to nobody. The reboot object is `WRITE | GRANT` (DECISIONS §251): the progenitor never
/// invokes it, but it must hold the `WRITE` it places in the one child whose manifest declares
/// `reboot`. See notes/progenitor-boot.md.
fn grant_machine_slots(tid: crate::thread::ThreadId, aspace_name: u64) {
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
    let s28 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::address_space_cap(aspace_name, Rights::WRITE),
        Some(28),
    )
    .expect("insert the progenitor's own address space");
    assert_eq!(s28, 28);
    let s31 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::reboot_capability(Rights::WRITE.union(Rights::GRANT)),
        Some(31),
    )
    .expect("insert the reboot object");
    assert_eq!(s31, 31);
}

/// Slots 32 and 33, the disk tree and its client-window pool, when this machine has an NVMe disk
/// (milestone 834): the installed system's second mounted tree, read-only at the prompt.
/// `WRITE | GRANT` on both, for the file service's reason at slots 5 and 6. Only beside a file
/// service, whose window 0 the disk tree's window 0 becomes.
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

/// Slots 24 to 26: the kernel's log ring, its cursor page and its notification (milestone 342).
///
/// The ring is read-only, so the log service can copy kernel lines out and never write them; the
/// cursor is read-write, so it can say how far it has read; the kernel signals the notification on
/// append. All three carry `GRANT`, because the progenitor is the spawner and not the reader. A boot
/// that could not allocate them grants nothing, and every kernel line goes direct.
fn grant_kernel_log(tid: crate::thread::ThreadId) {
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

/// Slot 27: a USB keyboard's attach endpoint, when the kernel started one (milestone 242).
///
/// `WRITE | GRANT`, so the progenitor can delegate the line discipline's endpoint through it and
/// then delete its own copy. Empty with no controller, a refused one, or a driver that reported a
/// failure. See [`boot_usb_keyboard`].
fn grant_usb_keyboard(tid: crate::thread::ThreadId) {
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

/// Slots 10 and 11, a terminal on the firmware's screen when there is no GPU (milestone 198's rung
/// 1b): the terminal's endpoint and its output page. Empty on every boot whose console has no
/// screen, which is every boot but a UEFI one. See [`boot_screen_terminal`].
fn grant_screen_terminal(tid: crate::thread::ThreadId, no_gpu: bool) {
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

/// Slots 13 to 15 for a virtio-net device (milestone 590); returns the grant for the interrupt
/// arming after the start.
///
/// The virtio-rng trio's shape: explicit slots, and `GRANT` on all three because the progenitor
/// delegates them into the `net_stack` it builds and deletes its own copies. Empty on every real
/// board and on any run with `NIFE_NET` unset. See [`boot_virtio_net_device`].
fn grant_virtio_net(tid: crate::thread::ThreadId) -> Option<VirtioBootGrant> {
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
        // aarch64 arms inline, riscv64 at the PLIC after the start, as for the virtio-rng source.
        #[cfg(target_arch = "aarch64")]
        crate::arch::irq::enable(g.intid);
    }
    virtio_net
}

/// Slots 29 and 30: a network stack the kernel built itself, when there is no virtio-net NIC.
///
/// On an `e1000e` (milestone 198's rung 3a), or on riscv64 on radon's Ethernet port (milestone 53,
/// gated by `designware_ethernet_service::PROVEN_ON_SILICON`). The `Stack` endpoint carries every
/// right, as the progenitor's own retype of it does on the virtio path, and the lease endpoint
/// `READ`: the progenitor receives the lease itself, so both paths reach the prompt the same way.
/// Never both. See [`boot_e1000e_network`].
fn grant_kernel_network_stack(tid: crate::thread::ThreadId, no_virtio_net: bool) {
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
