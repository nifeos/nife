---
status: NOT-STARTED
raised: 2026-11-23
milestone_dependencies: 1062, 1068
decision_dependencies: 86, unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1123. The NVMe driver learns of a completion by interrupt, not by polling

The number, title and slug are provisional. The branch that wrote this block is
`roadmap/tgoskits-prior-art`.

Today both halves of the NVMe path poll. The `nifeos/non_volatile_memory_express` crate leaves the
wait to its caller, and its `Command::create_io_completion_queue` writes `IEN=0` and vector 0. The
user-space driver in `components/src/non_volatile_memory_express_data_plane.rs` spins on the phase
tag of the completion ring for up to `SPIN_BOUND` polls, and its admin twin does the same. Each
names the polling in its `BUGS`, and `notes/non-volatile-memory-express.md` gives the reason: a
user-space driver that could aim an MSI had nothing to confine it.

This milestone moves completion to interrupts. The controller signals by MSI-X, one vector per I/O
queue pair where the device and the machine offer enough vectors, and by INTx where they do not.
The driver sleeps on its interrupt and drains the completion ring when it wakes.

What drives it: a shared machine under load. A spinning data plane burns a core for every command,
and the notes already call polling wrong for that case. What it saves: the core the spin holds,
and the first step toward more than one command in flight per queue.

Reuse: the kernel's MSI-X and MSI programming in `kernel/src/pci.rs` (`function_interrupt`,
`program_msix`, `program_msi`, `intx_intid`), the `Interrupt` capability and its `WAIT` and
`ACKNOWLEDGE` operations, the VT-d remapping that milestone 1068 (the kernel turns interrupt
remapping on where an IOMMU offers it) built, and the crate's ring arithmetic. No third-party
crate: the prior art below is a design to read, not a dependency.

## What reaches a user-space driver today

The kernel already delivers device interrupts to user-space drivers. The virtio-pci transports and
the xHCI driver hold an `Interrupt` capability. `WAIT` blocks until the line fires, the kernel masks
it, and `ACKNOWLEDGE` unmasks it once the driver has quieted the device.

- Routing. `function_interrupt` in `kernel/src/pci.rs` asks the architecture for an MSI vector. On
  x86_64 `alloc_msi_vector` hands one out, and where VT-d offers remapping the vector is a remapped
  entry that only the function's requester id may raise. On aarch64 and riscv64 `alloc_msi_vector`
  answers `None`, and the function takes its INTx line through the bus 0 swizzle.
- One vector per function. `program_msix` writes MSI-X table entry 0 and nothing past it, and
  `alloc_msi_vector` reserves one vector per call. Nothing hands a function a block of vectors.
- The MSI-X table stays out of reach. The kernel maps the NVMe admin process two pages of BAR0
  (`REGISTER_PAGES`), so a controller that puts its table later in the BAR never exposes it.

## What is missing

- The kernel never routes the NVMe function's interrupt. `pci::find_nvme_device` returns BAR0 and
  the requester id and calls no interrupt routing, and neither NVMe process receives an
  `Interrupt` capability. The data plane's capability contract lists `Interrupt` as denied.
- More than one vector per function. One vector per I/O queue pair needs as many table entries,
  as many reserved vectors and, under VT-d, as many remapping entries, each bound to the one
  requester id.
- Interrupt remapping off x86_64 VT-d. Milestone 1068 steps 2 to 4 (AMD-Vi, the riscv64 MSI page
  table, the aarch64 GICv3 ITS) remain open. Until they land, aarch64 and riscv64 take INTx, and
  x86_64 under AMD-Vi grants an MSI that nothing confines.
- A way to mask an INTx line at the controller. NVMe masks its INTx through `INTMS` and `INTMC`, at
  `0x0c` and `0x10` of the register page. The data plane holds only the doorbell page, by design,
  so it cannot reach either register. The kernel's mask between `WAIT` and `ACKNOWLEDGE` may cover
  it, since a level line stays masked until the driver acknowledges, but nobody has checked that
  against a controller that holds the line.
- A wait on two things at once. The data plane blocks on its request endpoint for the next client
  request. With interrupts it must also wake for a completion. `WAIT` on an `Interrupt` blocks on
  that interrupt alone, so a driver with one thread waits on one or the other.
- The crate's surface. `create_io_completion_queue` takes no interrupt-enable flag and no vector,
  and `registers` names no `INTMS` or `INTMC`.

## Decisions this milestone needs before it builds

Each of these is a new system call, a new wire surface or a new capability, so each is the
architect's. This block recommends none of them.

1. How a driver receives more than one vector for one function. One `Interrupt` capability per
   vector, a capability naming a range, or one capability with a queue index in `WAIT`'s reply.
2. How a driver waits on a request and a completion together. The options include binding an
   interrupt to an endpoint (the kernel's `bind_interrupt` already does this for the progenitor's
   UART), one thread per queue pair, or a notification object of the seL4 kind.
3. Who holds the interrupt, the admin process or the data plane, and whether the admin process
   hands it over as it hands over the doorbell page. The admin queue wants its own vector too.
4. The crate's new public names for the interrupt-enable flag, the vector argument and the two mask
   registers. Each ships provisional and goes to a name review.

## Prior art

- tgoskits `drivers/blk/nvme-driver` (MIT), written against NVMe base specification 2.2, read at
  `rcore-os/tgoskits@b6aa8e5d4`. In MSI-X mode it reserves vector 0 for the admin queue and gives
  each I/O queue pair one fixed vector, up to 64. With fewer than two vectors it unwinds MSI-X and
  builds a single-queue INTx controller. Its hard handler only acknowledges the source and sets a
  bit. A task drains the completion ring, and in INTx mode it masks vector 0 with `INTMS` and
  unmasks through `INTMC`. This milestone follows that split of vectors and that fallback.
- What not to copy from it. `Nvme::new` calls `mmio_api::init`, and tgoskits `memory/mmio-api`
  keeps its accessor in a process-wide `static mut MMIO_OP`. The first caller sets it and later
  callers lose their own without an error. The flag `INIT` flips before the write, so a second
  caller can read the accessor before the first caller has stored it, and nothing checks that
  setup finished. Here each process holds its windows from its startup page, and no global stands
  between a driver and its registers.
- Linux `drivers/nvme/host/pci.c` asks for one vector per CPU plus one for the admin queue through
  `pci_alloc_irq_vectors`, shares the admin vector with I/O queue 1, and falls back to one vector.
- SPDK's user-space driver polls by design and never takes an interrupt. Its throughput is the
  argument for keeping a polled mode, which this milestone keeps where polling wins.

Where nife differs: the kernel owns the MSI-X table and the remapping table, so a driver never
programs a vector. It names one only through the capability the kernel granted.

A concrete example. Under QEMU's `q35` with `intel-iommu` and `intremap=on`, the kernel reserves two
vectors for the NVMe function and writes MSI-X entries 0 and 1. It grants the admin process the
first and the data plane the second. The data plane rings the submission doorbell, sleeps in
`WAIT`, wakes on vector 1, drains every fresh completion, writes the completion head doorbell and
calls `ACKNOWLEDGE`.

## The work, in order

1. The architect rules the four decisions above.
2. The crate takes the interrupt-enable flag and the vector in `create_io_completion_queue`, and
   names `INTMS` and `INTMC`. The work lands in `nifeos/non_volatile_memory_express` through its own
   queue, and nife's pin moves after it.
3. The kernel routes the NVMe function's interrupts: as many MSI-X entries as the ruling allows,
   or INTx where the machine has no MSI. It grants the result in the capabilities the ruling chose.
4. The admin process creates each I/O completion queue with interrupts enabled and the vector it
   holds, and the data plane sleeps on its interrupt in place of the spin.
5. A test that a completion that never comes still ends as `EIO`, now bounded by a timeout in place
   of the spin count.

## Done when

- The crate's Kani harnesses and its three cargo-fuzz targets (`identify_namespace`, `completion`,
  `capabilities`) still run and pass, with a harness for each new field the crate writes.
- nife's proofs in `driver_process_contract::non_volatile_memory_express` and the falsification
  `a_confined_el0_server_cannot_dma_outside_its_region` still pass.
- A QEMU system test on each architecture runs reads and writes through the NVMe data plane with
  completion by interrupt: MSI-X on x86_64, INTx on aarch64 and riscv64. The test fails if the
  driver falls back to the spin.
- Where milestone 1068 has landed for a machine, a component that holds no grant for the vector
  cannot raise it, shown in the shape of milestone 202 (every confinement test is a ritual until
  somebody breaks the confinement and watches it fail).

## Follow-on

- **Proposed.** More than one command in flight per queue, which interrupts make worth having
  and this milestone does not build.
- **Milestone 1068.** Remapping on AMD-Vi, riscv64 and aarch64, without which an MSI on those
  machines is unconfined.

## BUGS

- The claim about the kernel mask covering a held INTx line is a reading of `WAIT`'s documentation,
  not a run.
- The tgoskits facts come from one read of its `dev` branch at the commit above.

## Index row

The NVMe path polls for every completion, which spends a core per command on a loaded machine.
This block moves completion to MSI-X with one vector per queue pair, and to INTx where MSI is
absent. It names the kernel support that exists and what is missing: more than one vector per
function, remapping off VT-d, and a wait on a request and a completion together. The new surface
waits on four rulings.
