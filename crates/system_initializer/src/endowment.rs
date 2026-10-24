//! The progenitor's endowment: `BootEndowment`, the capabilities the kernel granted it by slot, and
//! `MountedTreeGrant`, the optional second directory grant. `lib.rs` re-exports both by name.
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

/// **The capabilities the kernel granted the progenitor, by slot.** Data the boot entry states
/// rather than code this crate repeats, so a board that grants a different layout says so in one
/// place instead of in this crate's logic.
///
/// The order comes from `kernel::user::boot_progenitor`, one function on all three
/// architectures, so `components/src/progenitor.rs` states it once with no `cfg`. A slot the
/// kernel did not grant holds nothing, and [`boot`](crate::boot) probes it.
pub struct BootEndowment {
    /// The construction budget, held `WRITE | GRANT`: everything this system is made of.
    pub untyped: u64,
    /// The UART's registers, a device capability to map into the console and input drivers.
    pub uart_dev: u64,
    /// The UART receive interrupt, an `Irq` capability to delegate into the input driver.
    pub uart_irq: u64,
    /// **The wall clock**: a `PageFrame` capability with `READ` alone, granted on every boot ahead
    /// of the filesystem pair so its slot never moves. With no clock service it is a zeroed page,
    /// which reads as `clock_contract::state::UNKNOWN`. Handed on, `READ` alone, to a child whose
    /// manifest declares a clock and to the shell for `time`, so nothing spawned can set the time
    /// (DECISIONS §43 (reading the clock is a page, setting it is a page you may write)). The
    /// shell's view is this frame handed on, not a second grant, so no board states the slot twice.
    pub clock_page: u64,
    /// **The inert-configuration page** (DECISIONS §111 (inert configuration is a validated page)):
    /// a `PageFrame` capability with `READ` alone, granted on every boot ahead of the filesystem
    /// pair, like [`clock_page`](BootEndowment::clock_page). The kernel assembles the boot's fixed
    /// defaults (`TZ=UTC`, `LANG=C`, `TERM=dumb`) into it before the progenitor exists
    /// (`kernel::user::boot_config_page`). Handed on,
    /// `READ` alone, only to a child whose manifest declares
    /// [`grant_plan::Manifest::configuration`].
    pub config_page: u64,
    /// **The filesystem, when this boot has one** (milestone 50). The kernel wires the block server
    /// and the FS server before it starts us and grants the service endpoint plus the page its
    /// clients share with it. The rights that endpoint carries arrive in `fs_rights`, and **0 means
    /// this boot attached no RedoxFS disk**, in which case these two slots hold nothing at all.
    pub fs_ep: u64,
    /// The page the file service's clients share with it; see [`fs_ep`](BootEndowment::fs_ep).
    pub fs_page: u64,
    /// **A virtio-rng device, when this boot has one** (DECISIONS §120 (a QEMU-only virtio-rng
    /// stopgap for the interactive boot)). The confined transport; `WRITE | GRANT`, so this process
    /// can delegate it to an entropy service it builds. **Absent** on real hardware or a run with
    /// `NIFE_RNG` unset. [`boot`](crate::boot) probes for it, because all three `START` words are
    /// spent (`role`, `initial_ramdisk_len`, `fs_rights`).
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
    /// **`display_terminal`'s served endpoint, on a boot whose terminal is a screen beside the
    /// serial console**: the kernel put `display_terminal` on the firmware's framebuffer
    /// (`kernel::user::boot_screen_terminal`), and the UART is still the console and the keystroke
    /// source. [`boot`](crate::boot) hands this to the console server, which writes every byte to
    /// both. `WRITE | GRANT`. **Absent** on every other boot, and never beside a virtio-gpu.
    pub disp_term_ep: u64,
    /// The physical page shared with that `display_terminal`, written before an `OPERATION_WRITE` on
    /// [`disp_term_ep`](BootEndowment::disp_term_ep), and absent exactly when that is. `READ |
    /// WRITE | GRANT`.
    pub disp_term_page: u64,
    /// **The gpu's scanout, when this boot has a virtio-gpu**: one `PageFrame` capability naming
    /// the surface run, `READ | WRITE | GRANT`, which a `graphical_terminal` session's
    /// `display_terminal` is built on. It is the tail of [`gpu_dma`](BootEndowment::gpu_dma)'s run,
    /// minted separately because nothing lets a holder narrow a run, and the terminal must not
    /// reach the driver's rings in the first page. Absent with no GPU, like the rest of the gpu's
    /// four.
    pub gpu_surface: u64,
    /// **A virtio-gpu, when this boot has one**: the confined transport, `WRITE | GRANT`, the
    /// [`virtio_rng`](BootEndowment::virtio_rng) trio's shape plus
    /// [`gpu_surface`](BootEndowment::gpu_surface). Nothing is built from it at boot; the spawn
    /// service keeps it and its three siblings and builds each `graphical_terminal` session's
    /// drivers from them, and the shell holds none. **Absent** with no GPU on the bus; deleted at
    /// boot when no session can be built. `kernel::user::boot_progenitor` says why it is granted.
    pub gpu: u64,
    /// The gpu's completion interrupt, `READ | GRANT`, routed and enabled by the kernel.
    pub gpu_irq: u64,
    /// The gpu's whole DMA region: one `PageFrame` capability naming the run (DECISIONS §102 (a Frame names a run of pages)), rings
    /// and control buffers in the first page and the surface after it. `READ | WRITE | GRANT`. The
    /// region carries its own physical base at `abi::virtio::DMA_PHYS_OFFSET`, which `gpu_driver`
    /// reads itself, so this process never maps it.
    pub gpu_dma: u64,
    /// **A virtio keyboard, when this boot has a gpu too**: the confined transport, `WRITE |
    /// GRANT`, the [`virtio_rng`](BootEndowment::virtio_rng) trio's shape. The spawn service keeps
    /// it beside the gpu's four and builds a session's `keyboard_driver` from it. **Absent** on
    /// every real board, and then a session reads keystrokes from the boot's line discipline over
    /// the UART. Only ever granted beside [`gpu`](BootEndowment::gpu).
    pub keyboard: u64,
    /// The keyboard's event interrupt, `READ | GRANT`.
    pub keyboard_irq: u64,
    /// The keyboard's one DMA page, `READ | WRITE | GRANT`, carrying its own physical base at
    /// `abi::virtio::DMA_PHYS_OFFSET` as [`gpu_dma`](BootEndowment::gpu_dma) does.
    pub keyboard_dma: u64,
    /// **A virtio-net device, when this boot has one.** The confined transport, `WRITE | GRANT`,
    /// the [`virtio_rng`](BootEndowment::virtio_rng) trio's shape exactly: this process delegates
    /// it into the `net_stack` it builds and keeps nothing. **Absent** on every real board today
    /// and on any QEMU run with `NIFE_NET` unset, and [`boot`](crate::boot) probes for it the way
    /// it probes for the rng.
    pub virtio_net: u64,
    /// The network card's interrupt, `READ | GRANT`; see [`virtio_net`](BootEndowment::virtio_net)
    /// and, for why it arrives unenabled on riscv64, [`virtio_rng_irq`](BootEndowment::virtio_rng_irq).
    pub virtio_net_irq: u64,
    /// The network card's DMA page, with its own physical base written at its last eight bytes;
    /// [`virtio_rng_dma`](BootEndowment::virtio_rng_dma)'s shape and reason. `READ | WRITE | GRANT`.
    pub virtio_net_dma: u64,
    /// **An entropy service the kernel already built on the CPU's seed instruction** (`RDSEED` on
    /// `x86_64`, `RNDRRS` on aarch64), when this boot has no virtio-rng. The request endpoint,
    /// `WRITE | GRANT`, already proven: the kernel waited for the service's readiness report and
    /// grants nothing on a refusal (`kernel::user::boot_instruction_entropy`). **Never both** this
    /// and the virtio-rng trio. **Absent** on riscv64, on a CPU without the instruction, and with
    /// `NIFE_RNG` set; [`boot`](crate::boot) probes it.
    ///
    /// Name: provisional, lane `milestone/595-x86-std`, 2026-09-26. `fs_ep`'s shape, one service over.
    pub entropy_ep: u64,
    /// **The machine statistics page** (DECISIONS §225 (`free` sees the machine and your share)
    /// part 2): a `PageFrame` capability with `READ | GRANT` to the frame the kernel keeps its
    /// machine-wide counters in
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
    /// **A network stack the kernel already built on an `e1000e` NIC**: its `Stack` endpoint with
    /// every right, the same endpoint to everything downstream as the virtio path's. The kernel
    /// builds this server for `kernel::user::boot_usb_keyboard`'s reason (spawn-time mappings no
    /// capability names). **Never both** this and [`virtio_net`](BootEndowment::virtio_net).
    /// **Absent** on every `virt` boot, on a NIC with no link, and on a part no gate has driven,
    /// such as the I219 that milestone 494 (a driver for the network card a PC actually has) is
    /// for. [`boot`](crate::boot) probes.
    ///
    /// Name: provisional, milestone 198's lane, 2026-10-05 (UTC).
    pub net_stack_ep: u64,
    /// The endpoint `net_stack`'s one report arrives on, `READ`: the DHCP lease, which [`boot`](crate::boot)
    /// receives and then deletes this, exactly as it does for a stack it built. Granted with
    /// [`net_stack_ep`](BootEndowment::net_stack_ep) and never without it.
    ///
    /// Name: provisional, milestone 198's lane, 2026-10-05 (UTC).
    pub net_stack_report: u64,
    /// **The reboot object** (DECISIONS §251 (restarting the machine is a kernel object the
    /// progenitor hands out)), `WRITE | GRANT`: the one capability
    /// on the machine that may restart it, granted on every boot. [`boot`](crate::boot) never invokes it. The
    /// spawn service keeps it and places `WRITE` in a child whose manifest declares
    /// [`grant_plan::Manifest::reboot`], which syncs for itself through its sync-only
    /// capability. The shell holds none.
    ///
    /// Name: provisional, milestone 805's lane, 2026-10-06 (UTC).
    pub reboot: u64,
    /// **Capabilities the kernel granted that the interactive system never uses**, deleted before
    /// anything is built. Empty on riscv64; on aarch64 the kernel's report endpoint and a test SGI,
    /// which exist because that boot path is shared with the test roles.
    pub for_test_roles: &'static [u64],
}

/// **The shell's second mounted tree**: a second file service the kernel granted, which the boot
/// shell holds at `grant_plan::spawnproto::MOUNTED_TREE_SLOT` and sees at `/disk`
/// (`grant_plan::spawnproto::MOUNTED_TREE_NAME`). Passed to [`boot`](crate::boot) beside
/// [`BootEndowment`] rather than folded into it, because it is the mounted tree's whole description
/// and a boot without one passes `None`.
///
/// On the real boot it is the disk tree, the whole NVMe namespace as one read-only file served with
/// no cache (calef, 2026-10-18 21:36 UTC, "Yes, read-only at prompt"), and both slots are empty on a
/// machine with no NVMe disk, which [`boot`](crate::boot) probes.
#[derive(Clone, Copy)]
pub struct MountedTreeGrant {
    /// The service's endpoint, `WRITE | GRANT`. The shell is placed `WRITE`; the spawn service keeps
    /// this copy and badges it per job, as it does [`BootEndowment::fs_ep`].
    pub ep: u64,
    /// The service's client-window pool, one run, as [`BootEndowment::fs_page`] is. Its window 0 is
    /// the first file service's window 0 frame, so the shell's one page serves both trees.
    pub pool: u64,
}
