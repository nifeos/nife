# Booting the progenitor: the boot capability set

The kernel's last act at boot is `user::boot_progenitor` (`kernel/src/user/progenitor_boot.rs`). It
loads the progenitor, measures it, builds its address space, grants it a fixed table of
capabilities and starts it. From those capabilities and nothing else, `crates/system_initializer`
builds the console server, the input driver, the line discipline and the shell out of the
progenitor's own budget. This note carries the design of the
kernel's half: the slot layout, why each grant has the rights it has, and which devices the kernel
starts itself. How the progenitor loads its children is in
[the progenitor and loading](progenitor-and-loading.md).

## What the kernel does, in order

1. It reads the progenitor out of the archive and checks it against the trust root under its own
   name, before its address space exists (milestone 22 (trusted init) phase B.1). The progenitor is this board's
   boot program, so it is in the trust root like any boot program. Then it requires the
   program-measurement table the progenitor checks every load against (milestone 104 (the measurement continues past init)).
2. It builds the address space: the segments, a stack of `PROGENITOR_STACK_PAGES`, and the
   timebase page where the architecture has one. The whole initial ramdisk is mapped read-only at
   `INITIAL_RAMDISK_VA`, so the progenitor can parse the archive and load the rest by name. The
   archive's pages are mapped with `map_physical`, which records each mapping in this space's own
   log. That window is the largest in the tree (the aarch64 archive is a few thousand pages), so
   its log pages are budgeted at the call site with `crate::revoke::log_pages_for`. Each stack page
   is painted, so `crate::progenitor_stack` can say how deep the stack has been.
3. It routes the UART receive interrupt to an endpoint, creates the thread, and grants the slots.
4. It configures and starts the thread with three `START` words: the console UART's register shape
   (`abi::uart`), the archive's length, and the file service's rights (0 for none).
5. It arms the interrupts that must wait for the driver to run, and returns without blocking.

The UART's interrupt number comes from the machine's own description when it gave one, else QEMU
virt's constant. The boot prints it with its source, because a wrong PLIC source once cost a boot on
the JH7110 ([the VisionFive 2](visionfive2.md), BUGS). On riscv64 it also prints the UART's register
stride and access width. The stride is the one fact about this UART the userspace drivers cannot see
for themselves, and a wrong one is a console that prints nothing.

## The slot layout

The layout is the same on all three architectures. `components/src/progenitor.rs`'s `GRANTS`
table reads it.

| slot | what | rights | granted |
|---|---|---|---|
| 0 | the root building budget (12,288 pages) | the root's, `GRANT` included | always |
| 1 | the console device | `WRITE \| GRANT` | always |
| 2 | the UART receive interrupt | `READ \| GRANT` | always |
| 3 | the clock page | `READ \| GRANT` | always |
| 4 | the inert-configuration page | `READ \| GRANT` | always |
| 5, 6 | the file service endpoint and its client-window pool | `WRITE \| GRANT` | with a filesystem |
| 7 to 9 | virtio-rng: transport, interrupt, DMA page | `GRANT` on each | with a virtio-rng |
| 10, 11 | the firmware-screen terminal: endpoint, output page | `GRANT` on each | with a UEFI screen and no GPU |
| 12 | the gpu's surface run | `READ \| WRITE \| GRANT` | with a virtio-gpu |
| 13 to 15 | virtio-net: transport, interrupt, DMA page | `GRANT` on each | with a virtio-net NIC |
| 16 | the instruction entropy service's request endpoint | `WRITE \| GRANT` | with no virtio-rng |
| 17 to 19 | the gpu's transport, interrupt and DMA run | `GRANT` on each | with a virtio-gpu |
| 20 to 22 | the virtio keyboard's transport, interrupt, DMA page | `GRANT` on each | with a gpu and a keyboard |
| 23 | the machine statistics page | `READ \| GRANT` | always |
| 24 to 26 | the kernel's log ring, cursor page, notification | ring `READ`, the rest `READ \| WRITE`; `GRANT` on each | when allocated |
| 27 | a USB keyboard driver's attach endpoint | `WRITE \| GRANT` | with an xHCI controller |
| 28 | the progenitor's own address space | `WRITE` | always |
| 29, 30 | a kernel-built network stack: `Stack` endpoint, lease endpoint | all rights; `READ` | with an `e1000e` or radon's port, and no virtio-net |
| 31 | the reboot object | `WRITE \| GRANT` | always |
| 32, 33 | the disk tree's endpoint and client-window pool | `WRITE \| GRANT` | with an NVMe disk beside a file service |

The table has 64 slots (`crate::cap::CAPABILITY_TABLE_SLOTS`), and the ones above 33 are the
progenitor's own to fill.

### Why every slot is explicit

The progenitor reads its table positionally. A capability whose slot depends on what the machine
turned out to have is wiring nobody can check by reading. So every grant names its slot
(`thread_control_block_insert_cap(.., Some(n))`) rather than taking the first free one, and asserts
where it landed. `notes/abi.md` section 4 calls this "the emptiness is load-bearing". First-free
numbering would shift the virtio-rng trio down by two on the ordinary boot that has a virtio-rng
and no disk. Each conditional group sits past the highest slot of every group before it, for the
same reason.

An optional grant that is absent is an empty slot. The progenitor tells it apart by probing:
`invoke` on an ungranted slot answers `NoSuchSlot`.

The pages granted on every boot are granted even when there is nothing to say. With no `clock`
program in the archive, the clock page is a zeroed frame, which reads as
`clock_contract::state::UNKNOWN` (`a_zeroed_page_reads_as_unknown`): the honest "the machine has no
clock it believes". The configuration page is always the least-assuming defaults (`UTC`, `C`,
`dumb`), the same ones the kernel's `std` harness uses (`system_tests/src/user/std_service.rs`).
There is no shell-held configuration to pass instead yet; the shape it would take is
design/roadmap/0047-navigation-and-naming/environment.md's "inheritance with visibility".

### The rights, slot by slot

- The root budget carries `GRANT`, so the progenitor can split off a budget for the shell and hand
  it on (milestone 31 (a capability shell)). Rights only narrow downward from here.
- The console device is `WRITE | GRANT`, delegated into the console and input drivers. On aarch64
  and riscv64 it is the UART's registers as a `DeviceFrame`, which the progenitor maps into those
  drivers. `x86_64` has no page for its console: COM1's 16550 is I/O ports `0x3F8..=0x3FF`. So
  there it is a `PortRange` (milestone 299 (the x86 port-range capability), DECISIONS §121 (x86 port I/O) reversed on 2026-09-15). The progenitor
  delegates it with `CAP_INSERT` rather than `MAP_INTO`, and the TSS I/O bitmap lets the drivers'
  `in` and `out` reach exactly those ports.
- The clock page is `READ` (DECISIONS §43 (reading the clock is a page)'s split): the progenitor can endow a reader and holds
  nothing that could set the time.
- The file service's endpoint and pool carry `GRANT`, because the progenitor's job with them is to
  delegate. It narrows the endpoint into the shell. The pool is every client window as one run, in
  the slot that once held window 0's page (milestone 599 (a frame per filesystem client channel), calef's option-4 ruling of 2026-09-27).
  The progenitor never maps the run into a client: it slices one window per client with
  `abi::page_frame::SLICE` and deletes the slice once the client is built.
- The device trios (virtio-rng, virtio-net, the gpu, the keyboard) carry `GRANT` on each
  capability, because the progenitor delegates them into the service it builds and deletes its own
  copies.
- The statistics page is `READ | GRANT` (milestone 126 (the `procps` package), DECISIONS §225 (`free` sees the machine and your share) part 2): the progenitor
  hands it to the boot shell, and nobody can write it.
- The log ring is read-only, so the log service can copy kernel lines out and never write them. The
  cursor page is read-write, so it can say how far it has read, and the kernel signals the
  notification on append (milestone 342 (one UART from two address spaces), calef's ruling F). The progenitor binds the notification to
  the service's thread. A boot that could not allocate them grants none, and every kernel line goes
  direct to the UART.
- The USB keyboard's attach endpoint is `WRITE | GRANT`. The progenitor delegates the line
  discipline's endpoint through it once it has built one, then deletes its own copy.
- The progenitor's own address space is `WRITE` alone (§249 (a running address space stays nameable) and its 2026-10-05 amendment). That
  lets it `UNMAP` each scratch page it filled for a child once the page is in the child (milestone
  95), and `MAP_INTO` its own space, which `PageFrame::MAP` already let it do. It has no `GRANT`,
  because nothing else is ever handed authority over the progenitor's memory. It has no `ENUMERATE`,
  because nothing it does lists itself. The capability names a space `CONFIGURE` does not retire
  (§249's option A), so it resolves while the progenitor runs and dies when its thread is reaped.
  The progenitor's field for it, `own_space`, is a provisional name, and so is `reboot` below.
- The kernel-built network stack's `Stack` endpoint carries every right, as the progenitor's own
  retype of it does on the virtio path. Its lease endpoint is `READ`. The progenitor receives the
  lease itself, exactly as from a stack it built, so both paths reach the prompt through one line of
  code.
- The reboot object is the one capability that may restart the machine, and it is minted here and
  nowhere else (milestone 805 (`reboot` at the prompt), DECISIONS §251 (restarting the machine is a kernel object)). The progenitor never invokes it, but it places
  `WRITE` in the one child whose manifest declares `reboot`. Delegation only narrows, so the
  progenitor has to hold `WRITE` itself: a `GRANT`-only reboot object makes every `reboot` spawn
  fail. The method itself checks no right, as §251 says.
- The disk tree is the installed system's second mounted tree, read-only at the prompt (milestone
  834; calef, 2026-10-18 21:36 UTC: "Yes, read-only at prompt"). It is granted only beside a file
  service, whose window 0 is the shell's one page and becomes the disk tree's window 0.

## Arming the interrupts

Two hardware differences are `#[cfg]`-gated: the console device, above, and when an interrupt
source is armed.

- aarch64's GIC has no boot-hart lottery, so the UART line is enabled before the thread is built,
  and each virtio source as its capabilities are inserted.
- `x86_64` arms COM1's IRQ 4 at the same place (milestone 505 (an x86_64 input driver that never lets the core idle)). The IO APIC entry goes live then,
  and nothing raises the line until the input driver sets the 16550's receive enable.
- riscv64 arms only after the progenitor starts, because the lottery forbids arming earlier
  ([harts and PEs](harts-and-pes.md)). The UART's PLIC source and each virtio device's source are
  enabled on the boot hart's own context (`boot_s_context`), not through the hart-spreading
  `crate::arch::irq::enable`, and then supervisor external interrupts. The input driver arms the
  NS16550's own receive interrupt when it starts, and re-arms the PLIC source through its Irq
  capability's `ACK`.

The virtio starters therefore hand back an interrupt that is routed but not enabled
(`VirtioBootGrant::intid`), and the caller enables it beside the UART's.

## The installed boot's confirmation

On `x86_64`, when a boot chooser started this boot, `boot_progenitor` calls
`install_service::confirm` to say the boot worked (milestone 198 (a package manager)'s rung 2b). The position is the
mechanism. The filesystem server granted just before has mounted the installed disk and reported
ready, which is as late a criterion as the system can evaluate without a person. The installer
stages in a transfer window of its own, so nothing else on the disk constrains when.

## Which devices the kernel starts itself

Most device authority goes to the progenitor raw, and the progenitor builds the driver. The kernel
starts a driver itself only where the progenitor could not build it as well.

- The USB keyboard driver (milestone 242 (USB host and HID)) is handed a dozen register pages and eleven DMA pages as
  spawn-time mappings. A process holds no name for a spawn-time mapping, so it can neither delegate
  nor revoke one; `non_volatile_memory_express_service` chose them for that reason. Built by the
  progenitor, each page would be a capability in its table, and a `DeviceFrame` names one page.
- The `e1000e` stack (milestone 198's rung 3a, on milestone 494 (a driver for the network card a PC actually has)'s driver) is handed BAR0's two queue
  pages and an eighteen-page DMA region the same way (`e1000e_service`'s header). It is also the
  wiring every `e1000e` gate already runs, so the booted system's stack is the tested one. A part no
  gate has driven is left alone, and so is a NIC with no link, since a stack waiting for DHCP would
  hold the prompt back forever.
- radon's Ethernet stack (milestone 53 (the board's own peripherals)) is the `e1000e` path's twin on riscv64. While
  `designware_ethernet_service::PROVEN_ON_SILICON` is false it touches nothing: radon's boot is the
  one every other bench session depends on.
- The instruction entropy service (milestone 595 (the shell runs a `std` program)) serves the CPU's seed instruction (`RDSEED`,
  `RNDRRS`) when there is no virtio-rng. The kernel confirms the instruction exists for three
  reasons. First, it is what the tree already does. `entropy_service::is_instruction_backend_available`
  reads `arch::isa`'s boot record, and `components/src/entropy.rs` trusts its spawner's choice of
  mode. It is the only answer that works on aarch64, where `ID_AA64ISAR0_EL1` is not readable at
  EL0. And detection stays in `kernel/src/arch/`, with the progenitor free of `cfg(target_arch)`.
  The kernel builds the service rather than telling the progenitor to, because a fact with no
  capability has no slot to travel in. The three `START` words are spent, and a flag in a page
  would be a second format for one bit.
- The screen terminal (milestone 198's rung 1b) puts `display_terminal` on the screen the firmware
  left running, served by `framebuffer_driver`, so a PC with no serial port shows the prompt.

The virtio-rng and virtio-net devices are only discovered and wired: a zeroed DMA frame with its own
physical base written at `VIRTIO_DMA_PHYS_OFFSET`, the interrupt routed, and the transport
registered and confined to that frame. Only the MMIO transport is used, a first cut scoped to what
an interactive boot needs; PCIe behind the IOMMU is follow-on work. The progenitor builds the
entropy service and `net_stack` from the three capabilities, with `crates/system_initializer`'s ELF
loader, the tree's only one (milestone 96 (one init)).

A driver writes physical addresses into virtio descriptors, and no capability exposes one. The DMA
page's base is therefore written into the page itself. The progenitor maps the granted frame
briefly, reads the base back out once, and relays it as the driver's `arg1`, the way it relays
`fs_rights`.

The refusal of a seed instruction that fails its first draw, and the screen handover's order, are
in [the device details](progenitor-boot/device-details.md).

### The graphics grants

Nothing is built from the gpu and keyboard at boot (milestone 632 (graphics on demand), calef's 2026-09-30 ruling): the
boot stays the minimal UART system, and graphics is launched from the swish prompt. The progenitor
hands the grants to the shell, which holds them until a `graphical_terminal` session's spawn hands
them back for the drivers to be built from, as it does with the statistics page.

The gpu is four capabilities and the keyboard three. DECISIONS §102 (a Frame names a run of pages) gives a `PageFrame` a page
count, and milestone 142 (a text display good enough that people use it) made the gpu's region one run and one capability
(`display_service::wire_device`). The driver gets the whole run and the terminal only the surface
after its first page, which is the split `display_service::start_terminal` makes. Nothing lets a
holder narrow a run, so the kernel mints both frame capabilities over the one region.

A keyboard is wired only beside a gpu, because a keyboard with no screen has no terminal to type
into. With no virtio keyboard, a graphical session takes its keystrokes from the boot's UART line
discipline (milestone 192 (a keyboard on real silicon)'s option A).

## The ELF child builders

The test suite's six ELF child builders get a bare five-slot world of their own from
`user::spawn_elf_child_builder`, which is not the boot path. [The child builders](progenitor-boot/child-builders.md)
has their slots and why the building budget is lent rather than spent.

## The stack

`PROGENITOR_STACK_PAGES` is fourteen pages. Debug, the build `script/swish-check` and CI boot, is
the deep one, because `system_initializer::boot`'s frame stays live under the spawn service.
[The progenitor's stack](stack/progenitor-stack.md) has the measurements and the frames.
