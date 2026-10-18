//! The progenitor's endowment: `BootEndowment`, the capabilities the kernel granted it by slot, and
//! `SecondDirGrant`, the optional second directory grant. `lib.rs` re-exports both by name. Moved
//! out of `lib.rs` unchanged but for visibility by milestone 843 (the system-initializer crate root
//! is split along its seams), under §279 (the system-initializer crate root splits into private
//! modules).
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

/// **The capabilities the kernel granted the progenitor, by slot.** Data the boot entry states
/// rather than code this crate repeats, so a board that grants a different layout says so in one
/// place instead of in this crate's logic.
///
/// The order comes from `kernel::user::boot_progenitor`, which has been one function on all three
/// architectures since milestone 166, and that is why `components/src/progenitor.rs` states it
/// once with no `cfg`. Until then aarch64's loader was shared with milestone 19d's test roles and
/// granted a report endpoint and a test interrupt this system has no use for, which displaced
/// every slot after the first; see [`for_test_roles`](BootEndowment::for_test_roles).
pub struct BootEndowment {
    /// The construction budget, held `WRITE | GRANT`: everything this system is made of.
    pub untyped: u64,
    /// The UART's registers, a device capability to map into the console and input drivers.
    pub uart_dev: u64,
    /// The UART receive interrupt, an `Irq` capability to delegate into the input driver.
    pub uart_irq: u64,
    /// **The wall clock** (milestone 51's wiring): a `PageFrame` capability with `READ` and nothing
    /// else, granted ahead of the filesystem pair so its slot is the same on every boot, whether or
    /// not a disk was attached, and granted **unconditionally**: a boot with no clock service hands
    /// us a zeroed page, which reads as `clock_contract::state::UNKNOWN` and is the honest answer for a
    /// machine that does not know the time. The progenitor hands it on only to a child whose manifest declares
    /// a clock, and hands on `READ`, so nothing spawned from this prompt can set the time
    /// (DECISIONS §43).
    ///
    /// **The shell holds a narrowed view of this same frame** (milestone 86), mapped at
    /// `SH_CLOCK_VA`, which is what `time <command>` measures with. There is deliberately no
    /// second field for it: the shell's clock is not a separate kernel grant but this one handed on,
    /// so a field would ask each board to state the same slot number twice with nothing checking
    /// that the two agree. `READ` and no `GRANT` there too, so the shell can read the wall clock and
    /// can hand one to nothing it spawns.
    pub clock_page: u64,
    /// **The inert-configuration page** (milestone 47's environment-variable fork, DECISIONS §111):
    /// a `PageFrame` capability with `READ` and nothing else, granted ahead of the filesystem pair
    /// for [`clock_page`](BootEndowment::clock_page)'s own reason (so its slot is the same whether
    /// or not a disk was attached), and granted **unconditionally** for the same reason too: this
    /// boot's fixed defaults (`TZ=UTC`, `LANG=C`, `TERM=dumb`) are assembled once, before the progenitor
    /// exists, so every boot hands the progenitor a real, validated page rather than making the slot's
    /// presence depend on some other component having started (`kernel::user::boot_config_page`,
    /// the same shape as `boot_clock_page`, minus the service: nothing here runs, so there is no
    /// readiness handshake to wait on, only a page to assemble and write once).
    ///
    /// The progenitor hands it on only to a child whose manifest declares [`grant_plan::Manifest::configuration`],
    /// and hands on `READ`, so nothing spawned from this prompt can change what a shell hands its
    /// own children.
    pub config_page: u64,
    /// **The filesystem, when this boot has one** (milestone 50). The kernel wires the block server
    /// and the FS server before it starts us and grants the service endpoint plus the page its
    /// clients share with it. The rights that endpoint carries arrive in `fs_rights`, and **0 means
    /// this boot attached no RedoxFS disk**, in which case these two slots hold nothing at all.
    pub fs_ep: u64,
    /// The page the file service's clients share with it; see [`fs_ep`](BootEndowment::fs_ep).
    pub fs_page: u64,
    /// **A virtio-rng device, when this boot has one** (DECISIONS §120's 2026-08-26 amendment:
    /// "grant the QEMU-only virtio-rng stopgap"). The confined transport; `WRITE | GRANT`, so this
    /// process can delegate it to an entropy service it builds. **Absent** on real hardware
    /// (milestone 55's actual target has none) or a run with `NIFE_RNG` unset, in which case this
    /// slot holds nothing at all, the same "0/empty means absent" shape [`fs_ep`](BootEndowment::fs_ep)
    /// already carries; [`boot`](crate::boot) probes for it (`invoke`'s own `NoSuchSlot` on an ungranted slot)
    /// rather than being told, because there is no fourth `START` argument word left to tell it
    /// with (`role`, `initial_ramdisk_len`, `fs_rights` already spend all three).
    pub virtio_rng: u64,
    /// The device's completion interrupt; see [`virtio_rng`](BootEndowment::virtio_rng). `READ |
    /// GRANT`, routed by the kernel but left unenabled on riscv64 (`kernel::user::VirtioBootGrant`'s
    /// own doc: the board's hart-lottery hazard means only the kernel, which knows the true boot
    /// hart, may enable it, and it already did before granting this).
    pub virtio_rng_irq: u64,
    /// **The DMA page the kernel wrote `direct_memory_access_phys` into**, at its own last eight bytes; see
    /// [`virtio_rng`](BootEndowment::virtio_rng). `READ | WRITE | GRANT`: this process maps it to
    /// read that value back out (entropy needs its own DMA region's physical base as a plain
    /// value; no capability exposes one), then delegates the same frame to the entropy service it
    /// builds, unread by anything else in between.
    pub virtio_rng_dma: u64,
    /// **`display_terminal`'s own served endpoint, on a boot whose terminal is a screen beside the
    /// serial console** (the shell on the firmware screen, milestone 198's rung 1b). The kernel put
    /// `display_terminal` on the firmware's framebuffer through `framebuffer_driver`
    /// (`kernel::user::boot_screen_terminal`), and the UART is still the console and the keystroke
    /// source. [`boot`](crate::boot) hands this to the console server, which writes every byte to both. `WRITE |
    /// GRANT`. **Absent** (holds nothing) on every other boot, and [`boot`](crate::boot) probes for it.
    ///
    /// It was the virtio-gpu stack's endpoint too until milestone 600 (provisional): that stack is
    /// not built at boot at all any more (milestone 632 (provisional) keeps the boot the minimal
    /// UART system and launches graphics from the prompt), so this endpoint names a firmware
    /// screen's terminal and nothing else.
    pub disp_term_ep: u64,
    /// The physical page shared with that `display_terminal`, written before an `OPERATION_WRITE` on
    /// [`disp_term_ep`](BootEndowment::disp_term_ep), and absent exactly when that is. `READ |
    /// WRITE | GRANT`.
    pub disp_term_page: u64,
    /// **The gpu's scanout, when this boot has a virtio-gpu** (milestone 600 (provisional)): one
    /// `PageFrame` capability naming the surface run, `READ | WRITE | GRANT`, which [`boot`](crate::boot)
    /// places in the shell (with the gpu's other three) for the user at the prompt to delegate
    /// into a `graphical_terminal` session (milestone 632 (provisional)); the session's `display_terminal` is
    /// built on it. It is the tail of [`gpu_dma`](BootEndowment::gpu_dma)'s run, minted separately
    /// by the kernel because nothing lets a holder narrow a run, and the terminal must not reach
    /// the driver's rings in the first page. Absent with no GPU, like the rest of the gpu's four.
    pub gpu_surface: u64,
    /// **A virtio-gpu, when this boot has one** (milestone 600 (provisional); milestone 177 (wire the graphical terminal stack into the real interactive boot) built
    /// the stack in the kernel until then): the confined transport, `WRITE | GRANT`, the
    /// [`virtio_rng`](BootEndowment::virtio_rng) trio's shape plus
    /// [`gpu_surface`](BootEndowment::gpu_surface). Nothing is built from it at boot (milestone
    /// 632 (provisional), calef's 2026-09-30 ruling: the boot stays the minimal UART system);
    /// [`boot`](crate::boot) places it and its three siblings in the shell, which holds them until a `graphical_terminal`
    /// session's spawn hands them back for the drivers to be built from. **Absent** with no GPU
    /// on the bus, or no `gpu_driver` or `display_terminal` in the archive; [`boot`](crate::boot) probes for
    /// it. `kernel::user::boot_progenitor` says why these are granted rather than built.
    pub gpu: u64,
    /// The gpu's completion interrupt, `READ | GRANT`, routed and enabled by the kernel.
    pub gpu_irq: u64,
    /// The gpu's whole DMA region: one `PageFrame` capability naming the run (DECISIONS §102 (a Frame names a run of pages)), rings
    /// and control buffers in the first page and the surface after it. `READ | WRITE | GRANT`. The
    /// region carries its own physical base at `abi::virtio::DMA_PHYS_OFFSET`, which `gpu_driver`
    /// reads itself, so this process never maps it.
    pub gpu_dma: u64,
    /// **A virtio keyboard, when this boot has a gpu too** (milestone 600 (provisional)): the
    /// confined transport, `WRITE | GRANT`, the [`virtio_rng`](BootEndowment::virtio_rng) trio's
    /// shape. Not built from at boot; placed in the shell beside the gpu's four (milestone 632
    /// (provisional)), and a `graphical_terminal` session's `keyboard_driver` is built from it when the user
    /// delegates it at spawn. **Absent** on a boot with no keyboard, which is milestone 192's
    /// option A and every real board: a session then takes its keystrokes from the boot's own line
    /// discipline over the UART. Only ever granted beside [`gpu`](BootEndowment::gpu).
    pub keyboard: u64,
    /// The keyboard's event interrupt, `READ | GRANT`.
    pub keyboard_irq: u64,
    /// The keyboard's one DMA page, `READ | WRITE | GRANT`, carrying its own physical base at
    /// `abi::virtio::DMA_PHYS_OFFSET` as [`gpu_dma`](BootEndowment::gpu_dma) does.
    pub keyboard_dma: u64,
    /// **A virtio-net device, when this boot has one** (milestone 590 (provisional), the booted
    /// system starts its network stack; promoted from the proposal
    /// `the-booted-system-has-no-network`). The confined transport, `WRITE | GRANT`, the
    /// [`virtio_rng`](BootEndowment::virtio_rng) trio's shape exactly: this process delegates it
    /// into the `net_stack` it builds and keeps nothing. **Absent** on every real board today and
    /// on any QEMU run with `NIFE_NET` unset, and [`boot`](crate::boot) probes for it the way it probes for the
    /// rng.
    pub virtio_net: u64,
    /// The network card's interrupt, `READ | GRANT`; see [`virtio_net`](BootEndowment::virtio_net)
    /// and, for why it arrives unenabled on riscv64, [`virtio_rng_irq`](BootEndowment::virtio_rng_irq).
    pub virtio_net_irq: u64,
    /// The network card's DMA page, with its own physical base written at its last eight bytes;
    /// [`virtio_rng_dma`](BootEndowment::virtio_rng_dma)'s shape and reason. `READ | WRITE | GRANT`.
    pub virtio_net_dma: u64,
    /// **An entropy service the kernel already built on the CPU's own seed instruction**
    /// (`RDSEED` on `x86_64`, `RNDRRS` on aarch64), when this boot has no virtio-rng (milestone 595
    /// (provisional), the `x86_64` half; promoted from the proposal
    /// `the-x86-64-progenitor-serves-entropy-from-rdseed`). The request endpoint, `WRITE | GRANT`,
    /// which is what [`boot`](crate::boot) would otherwise have made itself from the
    /// [`virtio_rng`](BootEndowment::virtio_rng) trio, already proven: the kernel waited for the
    /// service's readiness report and grants nothing on a refusal
    /// (`kernel::user::boot_instruction_entropy`, which also says why the kernel builds this one).
    ///
    /// **Never both.** The kernel grants this only when it granted no virtio-rng, so a boot has
    /// one entropy source or none. **Absent** on every riscv64 boot (neither instruction exists
    /// there), on a CPU without the instruction, and on any QEMU run with `NIFE_RNG` set; [`boot`](crate::boot)
    /// probes it the way it probes [`fs_ep`](BootEndowment::fs_ep).
    ///
    /// Name: provisional, lane `milestone/595-x86-std`, 2026-09-26. `fs_ep`'s shape, one service over.
    pub entropy_ep: u64,
    /// **The machine statistics page** (milestone 126, DECISIONS §225 (`free` sees the machine and your share) part 2): a `PageFrame`
    /// capability with `READ | GRANT` to the frame the kernel keeps its machine-wide counters in
    /// (`crates/machine_statistics_contract`). Granted unconditionally, like
    /// [`config_page`](BootEndowment::config_page), so its slot never moves. [`boot`](crate::boot) hands it to
    /// the boot prompt's shell when [`GRANT_MACHINE_PAGE`](crate::GRANT_MACHINE_PAGE) says the owner allows it, and keeps no
    /// copy; the shell sends it back with the spawn request of a program that declares
    /// [`grant_plan::Manifest::machine`] (`spawnproto::MACHINE_BIT`).
    ///
    /// Name: provisional, milestone 126's `free` lane, 2026-09-26.
    pub machine_page: u64,
    /// **The kernel's ring** (milestone 342 (the kernel and the `console` server drive one UART
    /// from two address spaces), calef's ruling F): a `READ | GRANT` capability to the run of
    /// frames the kernel writes its lines into (`system_log_contract::kernel_ring`). [`boot`](crate::boot) maps
    /// it read-only into the system log service and keeps no copy. Empty on a boot whose kernel
    /// could not allocate it. Name: provisional, milestone 342's lane, 2026-10-03.
    pub kernel_ring: u64,
    /// The kernel ring's cursor page, `READ | WRITE | GRANT`: mapped read-write into the log
    /// service, which writes how far it has read. Name: provisional, as above.
    pub kernel_ring_cursor: u64,
    /// The notification the kernel signals when it appends to its ring, `READ | WRITE | GRANT`:
    /// bound to the log service's thread before it starts. Name: provisional, as above.
    pub kernel_ring_notification: u64,
    /// **A USB keyboard driver's attach endpoint** (milestone 242 (USB host and HID)), `WRITE |
    /// GRANT`, when the kernel started `usb_keyboard_driver` on this machine's xHCI controller.
    /// [`boot`](crate::boot) delegates `WRITE` on the line discipline's endpoint through it, the one message the
    /// driver ever receives there, and deletes its own copy: from then on the driver's keystrokes
    /// reach the terminal exactly as `input`'s do, and nothing downstream can tell the two apart.
    /// **Absent** on a machine with no controller or one the kernel refused; [`boot`](crate::boot) probes.
    ///
    /// Name: provisional, milestone 242's lane, 2026-10-04.
    pub usb_keyboard_attach: u64,
    /// **This process's own address space** (§249 (a running address space stays nameable), its
    /// 2026-10-05 amendment ruling the slot): an `AddressSpace` capability with `WRITE` alone, no
    /// `GRANT`, granted on every boot. [`boot`](crate::boot) hands it to `supervision_protocol` before it builds
    /// anything, so every page the loader fills for a child is given up with `UNMAP` the moment it
    /// is in the child, and the progenitor keeps no window onto a boot server's memory (milestone
    /// 95 (an unmap primitive, and the mappings init never lets go)). Kept for the life of the
    /// process, because the spawn service builds a job's pages the same way. Never delegated:
    /// without `GRANT` it cannot be.
    ///
    /// Name: provisional, milestone 95's §249 lane, 2026-10-05 (UTC).
    pub own_space: u64,
    /// **A network stack the kernel already built on an `e1000e` NIC** (milestone 198 (a package
    /// manager), rung 3a on `x86_64`): its `Stack` endpoint with every right, which is what
    /// [`boot`](crate::boot)'s own retype of that endpoint carries on the virtio path, so from here on the two
    /// are the same endpoint to everything downstream. The kernel builds this server rather than
    /// granting its raw materials for `kernel::user::boot_usb_keyboard`'s reason: twenty pages of
    /// spawn-time mappings, which no capability names. **Never both** this and
    /// [`virtio_net`](BootEndowment::virtio_net): the kernel grants this only on a boot with no
    /// virtio-net. **Absent** on every `virt` boot, on a NIC with no link, and on a part no gate
    /// has driven: xenon's I219, until the bench boot of milestone 494 (a driver for the network
    /// card a PC actually has). [`boot`](crate::boot) probes.
    ///
    /// Name: provisional, milestone 198's lane, 2026-10-05 (UTC).
    pub net_stack_ep: u64,
    /// The endpoint `net_stack`'s one report arrives on, `READ`: the DHCP lease, which [`boot`](crate::boot)
    /// receives and then deletes this, exactly as it does for a stack it built. Granted with
    /// [`net_stack_ep`](BootEndowment::net_stack_ep) and never without it.
    ///
    /// Name: provisional, milestone 198's lane, 2026-10-05 (UTC).
    pub net_stack_report: u64,
    /// **The reboot object** (milestone 805 (`reboot` at the prompt), DECISIONS §251 (restarting
    /// the machine is a kernel object the progenitor hands out)), `WRITE | GRANT`: the one capability
    /// on the machine that may restart it, granted on every boot. [`boot`](crate::boot) never invokes it. The
    /// spawn service keeps it and places `WRITE` in a child whose manifest declares
    /// [`grant_plan::Manifest::reboot`], which syncs for itself through its sync-only
    /// capability. The shell holds none.
    ///
    /// Name: provisional, milestone 805's lane, 2026-10-06 (UTC).
    pub reboot: u64,
    /// **Capabilities the kernel granted that the interactive system never uses**, deleted with the
    /// device authority once the drivers exist.
    ///
    /// Empty on riscv64. On aarch64 it is the kernel's report endpoint and the milestone-19d.2b test
    /// SGI, both of which exist because that boot path is shared with the test roles: nothing
    /// receives on the report here, and no interactive component waits on that interrupt. A progenitor
    /// that kept them would be keeping delegable authority for no reason, which is the same kind of
    /// thing the construction budget is.
    pub for_test_roles: &'static [u64],
}

/// **A second, disjoint directory capability for the shell** (milestone 154's "wiring a second
/// grant into the real boot", design/roadmap/0154-multi-directory-namespace.md), passed to [`boot`](crate::boot)
/// alongside [`BootEndowment`] rather than folded into it: this is not a kernel grant like every
/// field above, it is something [`boot`](crate::boot) itself constructs (a second `fs_subtree_caretaker`,
/// narrowing the same file service [`BootEndowment::fs_ep`] already names) out of capabilities the
/// kernel already granted.
///
/// **`None` at every real entry point today.** The mechanism here is real and reachable through
/// the one function the real progenitor calls (`components/src/progenitor.rs`), not a synthetic
/// kernel-side test harness. What it does
/// not decide is *what* the second subtree should be: [DECISIONS
/// §126](../../../design/decisions/0126-two-directory-cwd.md) named that a boot-time policy
/// question reserved for an architect, so no shipped boot enables it. A second, separate gap:
/// nothing yet tells the shell process *which* label and cspace slot this landed at (the `START`
/// ABI's three words are already spoken for by the role, the argument, and the clock slot), so a
/// shell built with one of these today would hold a capability its own `Nav` has no way to learn
/// about. Provisional shape.
#[derive(Clone, Copy)]
pub struct SecondDirGrant {
    /// One component under the image root, the same shape a `DirGrant`'s `name` already takes.
    pub name: &'static str,
    /// The `filesystem_contract::dir` rights the caretaker asks for on its descent.
    pub rights: u64,
}
