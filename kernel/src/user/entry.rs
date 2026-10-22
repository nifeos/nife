//! **Entering user mode**: the spawn description, the ELF child builders' loader, and the `run`
//! family that fakes a trap frame and returns into a program.
//!
//! Cut out of `user.rs` unchanged by milestone 872 (the kernel's user-mode file is split along its
//! seams), which left the parent an index.
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
/// aarch64: SGI 3, distinct from the scheduler's RESCHED (0) and the older endpoint SGIs (1, 2).
/// RISC-V has no software-generated interrupt a test can raise on itself at all (the SBI IPI
/// arrives down the *software*-interrupt arm, never touching `irq_route`), so it names the console
/// UART's own line, which is the one interrupt this ISA can assert by hand. That makes it the same
/// number as [`UART_RX_INTID`] there, deliberately; [`spawn_elf_child_builder`] binds the route
/// once and grants two capabilities naming it. See `sched::tests`' `DELIVERY_IRQ`, which reached
/// the same conclusion.
///
/// Name: ratified 2026-10-18 (calef), with the rule that took `init` out of nife's own names. It is
/// an SGI only on aarch64, which the name no longer claims.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
#[cfg(target_arch = "aarch64")]
pub const DELEGATED_TEST_INTERRUPT: u32 = 3;
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
#[cfg(target_arch = "riscv64")]
pub const DELEGATED_TEST_INTERRUPT: u32 = 10;
/// `x86_64` (milestone 161, updated by roadmap item 4): **the local APIC's self-IPI test vector**,
/// which puts this ISA on aarch64's side of the split rather than RISC-V's. The local APIC will
/// deliver a vector to its own CPU on demand through the ICR, so x86 needs no device to raise an
/// interrupt by hand, and `arch::x86_64::irq::raise_self_interrupt` is the mechanism. The intid for
/// such a source **is its vector**, which is why this is 0x22 and not a small number like the other
/// two: see `arch::x86_64::exceptions::x86_trap_body`'s self-IPI arm for the naming rule.
///
/// It was 4 (COM1's legacy IRQ) while the APIC was unbuilt and that arm's own comment said to
/// revisit this when it landed.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(target_arch = "x86_64")]
pub const DELEGATED_TEST_INTERRUPT: u32 = crate::arch::irq::SELF_TEST_VECTOR as u32;

/// **Spawn one of milestone 19d (run a real workload)'s and 19e's ELF child builders**, the archive
/// entry `entry` (milestone 19d.2c; the boot half of this function became [`boot_progenitor`] at
/// milestone 166 (one boot loader, reached two inconsistent ways)).
///
/// A builder is a userspace parent: it reads its child out of the archive by name, builds it out of
/// an ELF it parsed and reports. There are six (`report_child_builder`, `device_child_builder`,
/// `interrupt_child_builder`, `console_server_builder`, `least_authority_demo_builder` and
/// `coremark_builder`, all in `fixtures/`), and every one gets the same bare 19d world at fixed
/// slots: the building budget (slot 0), the kernel's `report` endpoint (slot 1, `WRITE|GRANT` so a
/// builder can report and endow a child on it), the console UART's registers (slot 2), the 19d.2b
/// test interrupt (slot 3), and the UART receive interrupt (slot 4). Each builder takes what it
/// uses from that and ignores the rest. `x0` is 0, since no builder dispatches on it.
///
/// It returns a [`holding::Holding`] over the thread and its building budget, so a test finished
/// with it can hand **2048 frames** back: these tests reserve 8 MiB each and the measured aarch64
/// boot spent 12289 frames on them, **42% of everything the suite never returned**
/// (notes/frames.md).
///
/// **The kernel enters `entry` itself, so `entry` must be in the trust root**, which is why the six
/// builders are in `sealed_pair::BOOT_PROGRAMS` beside the progenitor. An `entry` the archive does
/// not hold panics here, and one the trust root does not name halts in [`crate::trust::require`],
/// so a test that names a program this function cannot enter fails rather than waiting on a
/// report nobody sends.
///
/// **This is not the boot path.** [`boot_progenitor`] is, on all three architectures, and hands the
/// progenitor its own slot layout; this function only ever enters a builder and grants only the
/// five capabilities builders use.
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

    // Route the test interrupt (19d.2b) BEFORE spawning the builder: the test raises the SGI as soon
    // as this returns, and an interrupt that fires before it is routed is dropped ("unexpected
    // interrupt"), not queued. Setting up the route here means the fire is counted on the routed
    // endpoint even though the built child is not yet waiting; the child's WAIT drains it.
    crate::sched::bind_irq(DELEGATED_TEST_INTERRUPT, crate::sched::create_rendezvous());
    crate::arch::irq::enable(DELEGATED_TEST_INTERRUPT);
    // And the UART receive interrupt (19d.2c), which an input driver waits on. Route and enable it
    // here, so a builder could delegate the Irq cap to one. The
    // number is the machine's (uart_irq_and_source; on the JH7110 the QEMU constant armed the wrong
    // PLIC source, see its doc), and the line names the source so a transcript is diagnosable. On
    // QEMU RISC-V the discovered line and DELEGATED_TEST_INTERRUPT are the SAME source (see
    // DELEGATED_TEST_INTERRUPT), and binding it twice would leave the first endpoint routed to
    // nothing while the test waits on it, so bind once and grant twice.
    let (uart_rx_intid, uart_irq_source) = uart_irq_and_source();
    crate::println!("  uart irq  : {uart_rx_intid} ({uart_irq_source})");
    if uart_rx_intid != DELEGATED_TEST_INTERRUPT {
        crate::sched::bind_irq(uart_rx_intid, crate::sched::create_rendezvous());
        crate::arch::irq::enable(uart_rx_intid);
    }

    // Read and MEASURE the builder here, before the thread is created (milestone 22 (trusted init)
    // phase B.1): the check has to be the thing that decides whether a thread is created at all,
    // not something the new thread does to itself. `trust::require` halts on a mismatch, so past
    // this line the bytes are the ones this kernel image was built against. The program-measurement
    // table (milestone 104 (the measurement continues past init)) is required for the same reason:
    // the whole archive is mapped into the builder.
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

    // **The building budget is carved here, not inside the thread**, so the caller has a name for
    // it and can reclaim it: a large untyped a builder retypes its child's address space, frames
    // and TCB from. Carving it out here changes nothing about what the builder gets; it changes who
    // can name it afterwards, the whole difference between 8 MiB spent and 8 MiB lent. See
    // notes/frames.md.
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
        // A region big enough for the builder's own segments, the initial ramdisk's page tables,
        // and slack: a builder loads whole ELFs with deep call chains.
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
        let mut space = AddressSpace::new(content).expect("no memory for the builder");
        map_segments(&mut space, &elf).expect("could not lay out the builder");
        // A multi-page stack: a builder that parses and builds a child ELF has deep call chains
        // (the loader loop, copy_from_slice, the elf parser), so one page overflows. Map
        // PROGENITOR_STACK_PAGES down from USER_STACK_TOP; the entry sp is unchanged (USER_STACK_TOP).
        for k in 0..PROGENITOR_STACK_PAGES {
            space
                .map_new(USER_STACK_VA - k * FRAME_SIZE, Flags::user_data())
                .expect("could not map the builder's stack");
        }
        #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
        map_timebase_page(&mut space).expect("could not map the builder's timebase page");

        // Map the initial ramdisk, one page at a time, read-only. These are reserved RAM pages the
        // frame allocator does not own, so this maps rather than allocates. A builder parses the
        // archive here to find its child.
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
        // slot 0: the delegable root budget. A builder narrows and hands budgets to the children it
        // builds, so the root carries GRANT (milestone 31). Rights only narrow downward from here.
        crate::sched::grant(crate::cap::memory_region_root_cap(build_region))
            .expect("grant untyped");
        // slot 1: the kernel's report endpoint, WRITE|GRANT so a builder can report a result and endow
        // a child to send on it. This slot is the boot layout's slot 1 too, which is exactly why
        // [`boot_progenitor`] once carried a report endpoint it never used: the shared function.
        crate::sched::grant(crate::cap::rendezvous_cap(
            report,
            crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant report");
        // slot 2: the UART registers, so a builder can build a driver and hand it the registers
        // (19d.2). WRITE (device access) | GRANT (delegate to the driver).
        //
        // **On x86_64 `UART_PHYS` is zero and this grants a device capability over physical page
        // zero**, which is a foot gun and is marked as one rather than designed away (AGENTS.md's
        // ladder: an exception must say it is an exception). The slot is positional, so declining to
        // grant here would renumber the interrupts below and every builder that names them. Nothing
        // reaches it today: every fixture that would map it asks
        // `machine_has_no_device_page_for_the_console()` first and skips.
        crate::sched::grant(crate::cap::device_frame_cap(
            UART_PHYS,
            crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant uart device");
        // slot 3: the 19d.2b test interrupt, so `interrupt_child_builder` can build an interrupt-driven
        // driver and hand it the Irq cap. The route was set up above, before the spawn; this only
        // grants the cap (a per-thread act). READ (WAIT/ACK) | GRANT (delegate).
        crate::sched::grant(crate::cap::irq_cap_rights(
            DELEGATED_TEST_INTERRUPT,
            crate::cap::Rights::READ.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant test irq");
        // slot 4: the UART receive interrupt, for an input driver a builder could build (19d.2c).
        // The same discovered number the route above was bound with, or the cap would name a source
        // no endpoint serves.
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
    // What this process is about to have mapped into it beyond its own image: the `Spawn` windows,
    // a device run, and every `PageFrame` run it is granted, since a process maps a granted run
    // into itself and pays its own log for each page (a 1920x1080 surface is 2025 records, twelve
    // log pages). See [`load`] for why the number is taken here rather than asked of each caller.
    //
    // **Both kinds of grant**: the runs still to come from `spawn.grants`, and the runs the spawn
    // closure already placed at fixed slots with `grant_at`, which is how every screen and GPU
    // spawn hands over its surface (`grants: &[]`). Counting only the first leaves the surface
    // out, and the log runs dry partway through mapping it: at 1920x1080 the framebuffer driver is
    // refused page 1029 of 2025 and dies with `E_SURFACE`.
    let granted_runs: u64 = spawn
        .grants
        .iter()
        .map(|g| g.object.mappable_pages())
        .sum::<u64>()
        + crate::sched::current_mappable_pages();
    let windowed = spawn.maps.len() as u64 + device.as_ref().map_or(0, |d| d.pages) + granted_runs;
    let (mut space, entry) = match load(image, windowed) {
        Ok(v) => v,
        Err(e) => {
            crate::println!();
            crate::println!("  refused to load a user program: {e}");
            crate::println!("  the kernel is fine.");
            crate::sched::exit();
        }
    };

    // The extra pages go in BEFORE we hand the address space off: a shared message buffer, or a
    // device's MMIO for a driver. This is the line that puts a UART into a userspace process.
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

    // HAND IT ITS WORLD. Granted in order, so slot 0 is `grants[0]`, and reading the caller's
    // `Spawn` literal tells you the entire authority of the process. There is no path it can
    // say, no uid it can be. A capability system's "environment" is not a variable, it is this.
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

/// Drop to EL0 at `entry`, on a fresh stack, with `arg0` in `x0`. Never returns.
///
/// `arg0` reaches `_start` as its first argument (AAPCS64 puts it in `x0`). It is how the kernel
/// tells one binary which of several roles to play, the way a real kernel hands a new process
/// its argc/argv. See the console server, which is the same ELF as its client with a different
/// `arg0`.
/// Drop the **current** thread to EL0 at `entry` on `user_sp`, no arguments (milestone 19c.3).
/// The entry path for a thread started through the TCB object surface, which runs on the freshly
/// scheduled thread rather than the one that called `START`. The address space is already
/// installed (the context switch that scheduled us in used our `space` field). This is `enter_at`
/// with a caller-chosen stack and zero args; `enter_at` is now the exec wrapper over it.
pub(crate) fn enter_at_on_current(entry: u64, user_sp: u64, arg0: u64, arg1: u64, arg2: u64) -> ! {
    enter_frame(entry, user_sp, arg0, arg1, arg2)
}

fn enter_at(entry: u64, arg0: u64, arg1: u64, arg2: u64) -> ! {
    enter_frame(entry, USER_STACK_TOP, arg0, arg1, arg2)
}

fn enter_frame(entry: u64, user_sp: u64, arg0: u64, arg1: u64, arg2: u64) -> ! {
    // THE TRAPFRAME IS NOT AN ORDINARY LOCAL, and this cost us an afternoon.
    //
    // It must sit at the TOP OF THIS THREAD'S KERNEL STACK, because that is where the hardware
    // will look for it. `enter_userspace` does `mov sp, x0`, and `exception_restore` leaves
    // SP_EL1 = x0 + 272 across the `eret`. So when the user traps back in, `SAVE_CONTEXT`
    // subtracts 272 and rebuilds the frame **at exactly this address**. It had better be
    // writable, and it had better be a stack.
    //
    // The first version wrote `enter_userspace(&TrapFrame { .. })`, and every field of that
    // struct is a compile-time constant, so Rust CONST-PROMOTED IT INTO .rodata. The kernel
    // set SP_EL1 to read-only memory, and the user's first `svc` faulted trying to write its
    // own trap frame there. See notes/userspace.md: the kernel then walked `sp` DOWNWARD
    // through .rodata and the whole of .text, 272 bytes and one fault at a time, until it fell
    // out of the bottom of the image into writable RAM and could finally tell us.
    let top = crate::sched::current_kernel_stack_top()
        .expect("a user thread needs a kernel stack of its own to be trapped onto");

    // **The frame goes at the very top of the kernel stack, on both ISAs, and it must be ABOVE the
    // live `sp`.** Everything below `sp` belongs to somebody else: a callee's frame, and on a trap
    // the 288/272 bytes the vector subtracts from `sp` to build its own frame. An object parked
    // there is not stored, it is lent.
    //
    // RISC-V used to compute this from the live `sp` instead (`(current_sp().min(top) - size) & !15`),
    // because its TCB entry path is shallow and a frame at the top would have overlapped this
    // function's own stack. That traded a deterministic overlap for an intermittent one, and
    // milestone 71 caught it: `current_sp()` is a real call at opt-level 0, so it returned
    // `sp - 16`, which put the frame at `sp - 304` while `trap.s` builds an S-mode trap frame at
    // `sp - 288`. The two differ by exactly 16 bytes, so the user frame's `x[2]` (the user `sp`)
    // sat precisely on the trap frame's `x[0]` slot, which `trap.s` writes as a literal zero. Any
    // timer interrupt taken between building the frame and consuming it therefore rewrote the whole
    // frame: user `sp` read 0 every time, `sepc` read whatever `t5` held, and `sstatus` read the
    // trap's `scause` (whose UXL bits are 0, an illegal U-mode XLEN). When `t5` happened to be 0 the
    // `sepc == 0` guard in `enter_user` fired; when it did not, the thread `sret`ed to a garbage PC,
    // died on its first instruction, and never answered whoever was waiting on it, which is a
    // lost-wakeup hang with no guard message. See notes/riscv-port.md.
    //
    // The shallow-path problem the old code was avoiding is real, and the fix for it is a
    // reservation rather than a moving target: `user_entry_trampoline` (both ISAs) drops `sp` by a
    // frame's worth before the first Rust frame exists, so this region is off-limits to the entry
    // path by construction. See arch/*/context.s.
    //
    // `thread_trampoline` deliberately does NOT reserve, and the asymmetry is the point. Only the
    // TCB path can be shallow; the exec path reaches here through `run` and the ELF loader, so its
    // frames are always far below the stack top, and reserving would spend a frame's worth on every
    // kernel thread to insure against a depth that cannot happen. If that ever stops being true, the
    // assertion below is what says so.
    let slot = top - size_of::<TrapFrame>() as u64;
    let frame = slot as *mut TrapFrame;

    // And prove it, rather than trusting the reasoning above. This is one check, once per
    // exec, against a bug whose symptom is a nested fault storm that eats the kernel image.
    assert!(
        mmu::translate(frame as u64).is_some_and(|(_, f)| f.is_writable()),
        "the user's TrapFrame at {frame:p} is not in writable memory",
    );

    // **The invariant the milestone 71 fault violated**, checked rather than reasoned about. A slot
    // at or below the live `sp` is one a callee or a trap will build over, and the old RISC-V
    // placement failed this on the very first user entry. Necessary rather than sufficient: this
    // function's own frame sits *above* `sp` and is not covered, which is what the trampoline
    // reservation handles. Cheap enough to keep: one comparison per exec.
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

// --- the test programs the kernel loads by name ---
//
// **There used to be five hand-assembled programs here**, three aarch64 and two RISC-V, written as
// `global_asm!` machine code in `.rodata` and copied into a user page by a one-page loader. They
// were honest milestone-7a scaffolding: there was no ELF loader and no filesystem to load from, so
// the "binary" rode inside the kernel image.
//
// They are gone (milestone 19's user-test port). The behaviours are ordinary (yield twice, read a
// forbidden address, spin forever), the toolchain builds them for both targets, and the initial
// ramdisk already delivers thirty other programs, so the scaffolding had outlived its reason twice
// over: once when 7c shipped the ELF loader, and again when the second ISA turned "one hand-written
// program" into "two hand-written programs, forever". Keeping them would have meant hand-assembling
// every one of them a second time to run the same tests on RISC-V.
//
// What replaced each:
//
//   - aarch64 `hello`, riscv `USER_HELLO` (yield, yield)  -> `kernel_test_subject`, role `KERNEL_TEST_SUBJECT_ROUND_TRIP`
//   - aarch64 `outlaw`  (read a kernel address)           -> `kernel_test_subject`, role `KERNEL_TEST_SUBJECT_READ_KERNEL`
//   - aarch64 `spin`    (loop, no syscall, no stack)      -> the `interrupt_ignorer` binary (DECISIONS §24)
//   - riscv `USER_REPORTER` (invoke a cap, SEND a word)   -> `riscv_least_authority_demo`, which builds a
//     process from the same parts and runs a real ELF through them
//
// This also removed `exec`, the one-page raw-machine-code loader they needed. Every program the
// kernel runs now arrives as an ELF.

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
