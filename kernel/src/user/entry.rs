//! **Entering user mode**: the spawn description, the ELF child builders' loader, and the `run`
//! family that fakes a trap frame and returns into a program.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

use super::*;

/// A physical page to map into a new process's address space, at a chosen VA.
///
/// The frame is **not** owned by the process (it is shared, or it is device MMIO), so it is not
/// freed when the process dies. See [`AddressSpace::map_physical`].
#[derive(Clone, Copy)]
pub struct Mapping {
    pub va: u64,
    pub phys: u64,
    pub flags: Flags,
}

/// **Everything a new process is handed at birth.** Its world, made explicit.
///
/// A capability system has no ambient environment: no inherited file descriptors, no `PATH`, no
/// uid. So a process gets *exactly* what is in this struct and nothing else. The whole of what it
/// can do is a function of `arg0`, `grants`, and `maps`, and reading a `Spawn` literal tells you
/// the complete authority of the thing you are about to start.
pub struct Spawn<'a> {
    /// Lands in `x0` at `_start`. A tiny channel for "which role are you" that needs no
    /// capability, the way a real kernel hands a new process its argc.
    pub arg0: u64,
    /// Lands in `x1`. A second scalar the process needs before it can name anything: the virtio
    /// driver's DMA region physical address, which it must write into device descriptors and
    /// cannot discover, because a process only knows virtual addresses.
    pub arg1: u64,
    /// Lands in `x2`. The virtio driver's device registers sit at a sub-page offset (slots are
    /// 0x200 apart, pages are 0x1000), so we map the containing page and tell the driver where in
    /// it the slot begins.
    pub arg2: u64,
    /// Capabilities, granted into slots 0, 1, 2, ... in order.
    pub grants: &'a [crate::cap::Cap],
    /// Extra pages: a shared buffer, a device's registers. Mapped after the ELF's own segments.
    pub maps: &'a [Mapping],
}

/// The interrupt the kernel routes for the interrupt-delegation test (19d.2b), which
/// `interrupt_child_builder` delegates to the child it builds.
///
/// aarch64: SGI 3, distinct from the scheduler's RESCHED (0) and the endpoint SGIs (1, 2). RISC-V
/// has no software-generated interrupt a test can raise on itself (the SBI IPI arrives down the
/// software-interrupt arm, never touching `irq_route`), so it names the console UART's own line,
/// the same number as [`UART_RX_INTID`] there; [`spawn_elf_child_builder`] binds that route once
/// and grants two capabilities naming it.
///
/// Name: ratified 2026-10-18 (calef), with the rule that took `init` out of nife's own names. It is
/// an SGI only on aarch64, which the name no longer claims.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
#[cfg(target_arch = "aarch64")]
pub const DELEGATED_TEST_INTERRUPT: u32 = 3;
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
#[cfg(target_arch = "riscv64")]
pub const DELEGATED_TEST_INTERRUPT: u32 = 10;
/// `x86_64` (milestone 161): **the local APIC's self-IPI test vector**. The local APIC delivers a
/// vector to its own CPU on demand through the ICR (`arch::x86_64::irq::raise_self_interrupt`), so
/// this ISA is on aarch64's side of the split. The intid for such a source is its vector, hence
/// 0x22; see `arch::x86_64::exceptions::x86_trap_body`'s self-IPI arm.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(target_arch = "x86_64")]
pub const DELEGATED_TEST_INTERRUPT: u32 = crate::arch::irq::SELF_TEST_VECTOR as u32;

/// **Spawn one of the ELF child builders**, the archive entry `entry`, in the bare world every
/// builder gets (milestone 19d.2c).
///
/// `image` is the initial ramdisk and `report` the endpoint granted at slot 1. `entry` must be in
/// the archive and in the trust root (`sealed_pair::BOOT_PROGRAMS`): otherwise this panics or halts,
/// rather than leaving a test waiting on a report nobody sends. Slots 0 to 4 hold the building
/// budget, `report`, the console UART's registers, the test interrupt and the UART receive
/// interrupt. This is not the boot path; [`boot_progenitor`] is. See notes/progenitor-boot.md.
/// Returns a [`holding::Holding`] over the thread and its 2048-frame building budget, to hand back.
///
/// Name: ratified 2026-10-18 (calef), with the rule that took `init` out of nife's own names.
/// Refused keeping `spawn_hello`: `hello` was the one binary all six builders were roles of, and
/// that binary is gone.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn spawn_elf_child_builder(
    image: &'static [u8],
    entry: &'static str,
    report: crate::sched::RendezvousId,
) -> holding::Holding {
    let (initial_ramdisk_start, initial_ramdisk_len) =
        memory::initial_ramdisk_region().expect("no initial ramdisk to hand the builder");
    let initial_ramdisk_pages = initial_ramdisk_len.div_ceil(FRAME_SIZE);

    // Routed before the builder exists: the test raises it as soon as this returns, and an
    // interrupt that fires unrouted is dropped, not queued. The child's WAIT drains the count.
    crate::sched::bind_irq(DELEGATED_TEST_INTERRUPT, crate::sched::create_rendezvous());
    crate::arch::irq::enable(DELEGATED_TEST_INTERRUPT);
    // And the UART receive interrupt (19d.2c), the machine's own number, printed with its source.
    // On QEMU RISC-V it is the same source as DELEGATED_TEST_INTERRUPT, and binding it twice would
    // leave the first endpoint routed to nothing, so bind once and grant twice.
    let (uart_rx_intid, uart_irq_source) = uart_irq_and_source();
    crate::println!("  uart irq  : {uart_rx_intid} ({uart_irq_source})");
    if uart_rx_intid != DELEGATED_TEST_INTERRUPT {
        crate::sched::bind_irq(uart_rx_intid, crate::sched::create_rendezvous());
        crate::arch::irq::enable(uart_rx_intid);
    }

    // Measured before the thread exists, so the check decides whether one is created at all
    // (milestone 22 phase B.1); `trust::require` halts on a mismatch. The measurement table too
    // (milestone 104), because the whole archive is mapped into the builder.
    let boot_fs = match nifefs::Fs::parse(image) {
        Ok(fs) => fs,
        Err(e) => {
            crate::println!("  archive is not a nifefs image: {e:?}");
            crate::sched::exit();
        }
    };
    let Some(builder_bytes) = boot_fs.read(entry) else {
        panic!("the initial ramdisk has no '{entry}' program to enter");
    };
    crate::trust::require(entry, builder_bytes);
    crate::trust::require_program_measurements(&boot_fs);

    // Carved here rather than inside the thread, so the caller can name it and reclaim it
    // (notes/frames.md).
    let build_region =
        crate::memory_region::create(12288).expect("no building budget for the builder");

    let tid = crate::sched::spawn(move || {
        let elf = match Elf::parse(builder_bytes) {
            Ok(e) => e,
            Err(e) => {
                crate::println!("  the {entry} image is not loadable: {e:?}");
                crate::sched::exit();
            }
        };
        // The builder's segments, the archive's page tables and mapping log, a deep stack, and slack.
        let content: u64 = elf
            .segments()
            .map(|seg| {
                let (start, end) = seg.page_range(FRAME_SIZE);
                (end - start) / FRAME_SIZE
            })
            .sum::<u64>()
            + 1
            + initial_ramdisk_pages / 512
            + crate::revoke::log_pages_for(initial_ramdisk_pages)
            + PROGENITOR_STACK_PAGES
            + 8;
        crate::sched::defer_kill();
        let mut space = AddressSpace::new(content).expect("no memory for the builder");
        map_segments(&mut space, &elf).expect("could not lay out the builder");
        // A builder's loader and parser have deep call chains, so one page overflows. The entry sp
        // stays USER_STACK_TOP.
        for k in 0..PROGENITOR_STACK_PAGES {
            space
                .map_new(USER_STACK_VA - k * FRAME_SIZE, Flags::user_data())
                .expect("could not map the builder's stack");
        }
        #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
        map_timebase_page(&mut space).expect("could not map the builder's timebase page");

        // Reserved RAM the frame allocator does not own, so mapped rather than allocated.
        for i in 0..initial_ramdisk_pages {
            space
                .map_physical(
                    INITIAL_RAMDISK_VA + i * FRAME_SIZE,
                    initial_ramdisk_start + i * FRAME_SIZE,
                    Flags::user_rodata(),
                    crate::revoke::PageMapSource::NoCapability,
                )
                .expect("could not map the initial ramdisk into the builder");
        }

        crate::sched::adopt_address_space(space);
        crate::sched::allow_kill();
        // Slots 0 to 4 in order: `grant` takes the next free slot, and builders name them by number.
        crate::sched::grant(crate::cap::memory_region_root_cap(build_region))
            .expect("grant untyped");
        crate::sched::grant(crate::cap::rendezvous_cap(
            report,
            crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant report");
        // On x86_64 `UART_PHYS` is zero, so this grants a device capability over physical page
        // zero: a foot gun, marked rather than designed away. The slot is positional, so skipping
        // it would renumber the interrupts after it; every fixture that would map it asks
        // `machine_has_no_device_page_for_the_console()` first and skips.
        crate::sched::grant(crate::cap::device_frame_cap(
            UART_PHYS,
            crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant uart device");
        crate::sched::grant(crate::cap::irq_cap_rights(
            DELEGATED_TEST_INTERRUPT,
            crate::cap::Rights::READ.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant test irq");
        // The number the route above was bound with, or the cap names a source no endpoint serves.
        crate::sched::grant(crate::cap::irq_cap_rights(
            uart_rx_intid,
            crate::cap::Rights::READ.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant uart rx irq");

        // A builder has no filesystem, so `x2` (the file-service rights the boot path passes) is 0.
        enter_frame(elf.entry(), USER_STACK_TOP, 0, initial_ramdisk_len, 0)
    })
    .expect("could not spawn the builder");

    let mut held = holding::Holding::new();
    held.add_thread(tid);
    held.add_region(build_region);
    held
}

/// **A run of contiguous device pages mapped into a new process before it starts**, for a window
/// too large to spell as [`Mapping`]s (the shell on the firmware screen: a screen's aperture is a
/// thousand pages or more, and the kernel has no heap to build a slice that long in).
///
/// Always device-typed and writable, because the one thing that needs it is a driver's view of a
/// device's memory, and write-combining when [`Self::write_combining`] says so. Like every [`Spawn::maps`] entry the process holds no *name* for it: it cannot
/// map it again, delegate it, or revoke it, which is the property `non_volatile_memory_express_service`
/// and milestone 159's TRNG driver chose spawn-time mappings for. **Name provisional.**
#[derive(Clone, Copy)]
pub(crate) struct DeviceRun {
    /// Where the first page lands in the new process.
    pub va: u64,
    /// The first page's physical address. Page-aligned.
    pub phys: u64,
    /// How many pages. The intermediate page tables come out of the address space's own
    /// `AS_OVERHEAD`, so a caller bounds this (`display_service`'s `MAX_APERTURE_PAGES`).
    pub pages: u64,
    /// **Map it write-combining** (`paging::Flags::user_write_combining`) rather than as
    /// registers. True only for memory the process writes and never reads, which on this tree is
    /// one framebuffer aperture: a register window combined would lose the order of its stores.
    /// A required field with no default, so a second caller has to say which it is. Name:
    /// provisional (the screen terminal lane, 2026-10-04).
    pub write_combining: bool,
}

/// Load the initial ramdisk program and become it, handed the world described by `spawn`. Never
/// returns.
pub fn run(image: &[u8], spawn: Spawn) -> ! {
    run_with(image, spawn, None, None)
}

/// [`run`], with one [`DeviceRun`] mapped as well. Never returns. **Name provisional.**
pub(crate) fn run_with_device_run(image: &[u8], spawn: Spawn, device: DeviceRun) -> ! {
    run_with(image, spawn, Some(device), None)
}

/// [`run`], and the process also holds its own address space, `WRITE`, at `slot` (§255 (each
/// socket is its own capability)): the network stack's spawn, so it can `UNMAP` a closed socket's
/// page. `supervision_protocol::CHILDS_OWN_SPACE` is the progenitor's twin. Never
/// returns. Name provisional.
pub fn run_with_own_space(image: &[u8], spawn: Spawn, slot: u64) -> ! {
    run_with(image, spawn, None, Some(slot))
}

fn run_with(image: &[u8], spawn: Spawn, device: Option<DeviceRun>, own_space: Option<u64>) -> ! {
    // Every window this process will have mapped beyond its image: the `Spawn` windows, a device
    // run, and every granted `PageFrame` run, both those still in `spawn.grants` and those the spawn
    // closure already placed with `grant_at`. See [`load`] and notes/user-address-spaces.md.
    let granted_runs: u64 = spawn
        .grants
        .iter()
        .map(|g| g.object.mappable_pages())
        .sum::<u64>()
        + crate::sched::current_mappable_pages();
    let windowed = spawn.maps.len() as u64 + device.as_ref().map_or(0, |d| d.pages) + granted_runs;
    crate::sched::defer_kill();
    let (mut space, entry) = match load(image, windowed) {
        Ok(v) => v,
        Err(e) => {
            crate::println!();
            crate::println!("  refused to load a user program: {e}");
            crate::println!("  the kernel is fine.");
            crate::sched::exit();
        }
    };

    // The extra pages go in before the address space is handed off.
    for m in spawn.maps {
        space
            .map_physical(
                m.va,
                m.phys,
                m.flags,
                crate::revoke::PageMapSource::NoCapability,
            )
            .expect("could not map a Spawn page into the new address space");
    }
    if let Some(d) = device {
        for k in 0..d.pages {
            space
                .map_physical(
                    d.va + k * FRAME_SIZE,
                    d.phys + k * FRAME_SIZE,
                    if d.write_combining {
                        Flags::user_write_combining()
                    } else {
                        Flags::user_device()
                    },
                    crate::revoke::PageMapSource::NoCapability,
                )
                .expect("could not map a device run into the new address space");
        }
    }

    let name = crate::sched::adopt_address_space(space);
    crate::sched::allow_kill();

    // In order, so slot k is `grants[k]`: the `Spawn` literal is the process's whole authority.
    for &granted in spawn.grants {
        crate::sched::grant(granted).expect("no free capability slot");
    }
    if let Some(slot) = own_space {
        crate::sched::grant_at(
            slot,
            crate::cap::address_space_cap(name, crate::cap::Rights::WRITE),
        )
        .expect("the own-space slot was already occupied");
    }

    enter_at(entry, spawn.arg0, spawn.arg1, spawn.arg2)
}

/// Drop the **current** thread to user mode at `entry` on `user_sp`, with `arg0` to `arg2` in the
/// first three argument registers. Never returns.
///
/// The entry path for a thread started through the TCB object surface, which runs on the freshly
/// scheduled thread with its address space already installed by the context switch.
pub(crate) fn enter_at_on_current(entry: u64, user_sp: u64, arg0: u64, arg1: u64, arg2: u64) -> ! {
    enter_frame(entry, user_sp, arg0, arg1, arg2)
}

fn enter_at(entry: u64, arg0: u64, arg1: u64, arg2: u64) -> ! {
    enter_frame(entry, USER_STACK_TOP, arg0, arg1, arg2)
}

/// Build the user-entry trap frame at the top of this thread's kernel stack and return into user
/// mode at `entry` on `user_sp`. Never returns.
///
/// The frame must be at the top, because the next trap rebuilds it there, and above the live `sp`,
/// because a callee or a trap builds below it. Both are asserted. See notes/userspace.md and
/// notes/riscv-port.md.
fn enter_frame(entry: u64, user_sp: u64, arg0: u64, arg1: u64, arg2: u64) -> ! {
    // Not a local: `enter_userspace` does `mov sp, x0`, and the next trap's `SAVE_CONTEXT` rebuilds
    // the frame at exactly this address, so it must be writable stack, never `.rodata`.
    let top = crate::sched::current_kernel_stack_top()
        .expect("a user thread needs a kernel stack of its own to be trapped onto");

    // Only the TCB entry path can be shallow, so only `user_entry_trampoline` drops `sp` by a frame
    // before the first Rust frame; `thread_trampoline` reaches here through `run` and the ELF
    // loader, far below the top. If that stops being true, the second assertion below says so.
    let slot = top - size_of::<TrapFrame>() as u64;
    let frame = slot as *mut TrapFrame;

    // One check per exec, against a fault storm that walks `sp` through the kernel image.
    assert!(
        mmu::translate(frame as u64).is_some_and(|(_, f)| f.is_writable()),
        "the user's TrapFrame at {frame:p} is not in writable memory",
    );

    // Necessary, not sufficient: this function's own frame above `sp` is covered by the
    // trampoline's reservation.
    assert!(
        slot >= crate::arch::current_sp(),
        "the user's TrapFrame at {frame:p} is below the live sp: a callee frame or a trap frame \
         will be built over it",
    );

    // SAFETY: `frame` is 16-byte-aligned writable kernel stack (a KernelStack top is page
    // aligned and TrapFrame is a multiple of 16), the user code and stack are mapped, and the
    // user address space is installed. `arch` owns the register layout: we ask for a user-entry
    // frame and hand it back to `arch` to make the jump (notes/riscv-port.md, leak #3).
    unsafe {
        frame.write(TrapFrame::for_user_entry(
            entry,
            user_sp,
            [arg0, arg1, arg2],
        ));
        enter_user(frame)
    }
}

/// The `kernel_test_subject` program's roles (`fixtures/src/kernel_test_subject.rs`), passed in the
/// first argument register.
///
/// `ROUND_TRIP` yields twice and exits: two syscalls from user mode, where the second can only
/// happen if the return from the first genuinely put the thread back at EL0/U-mode.
///
/// Name: provisional, minted 2026-10-22 (UTC) by lane maintainer/rename-outlaw: the program's
/// ratified name and the role.
// The tests use it on both ISAs; of the two boot tours only RISC-V's has a syscall-count step.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const KERNEL_TEST_SUBJECT_ROUND_TRIP: u64 = 0;

/// `READ_KERNEL` reads the address handed to it in the second argument register, which is what makes
/// the program portable: the kernel's own memory lives at a different virtual address on each ISA,
/// and the caller knows which. See `tests::a_user_program_cannot_read_a_kernel_address`.
///
/// Name: provisional, minted 2026-10-22 (UTC) by lane maintainer/rename-outlaw: the program's
/// ratified name and the role.
// The tour uses it, and the shell/bench boots skip the tour.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const KERNEL_TEST_SUBJECT_READ_KERNEL: u64 = 1;

/// `BUILD_WITH_NOTHING` tries to mint a page, a message and a kernel object on slots it was never
/// granted, and reports the three errnos, which is claim 23's zero-authority corner: with no
/// untyped budget in the table, every path to building must answer "nothing there".
///
/// Name: provisional, minted 2026-10-22 (UTC) by lane maintainer/rename-outlaw: the program's
/// ratified name and the role.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const KERNEL_TEST_SUBJECT_BUILD_WITH_NOTHING: u64 = 2;
