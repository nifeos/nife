# NVMe: the first non-virtio disk

The tree spells it `non_volatile_memory_express`, since calef's ruling of 2026-09-17
(DECISIONS §154) and the rename of 2026-09-18. `NVMe` stays in this page's prose, because that is
the specification's own name and the word the datasheet, the boot output and QEMU's `-device nvme`
all print; what moved is the crate, the kernel module, its `NonVolatileMemoryExpress` type, and the
program. A reader who greps `nvme` and finds nothing is looking for the expansion.

The storage half of milestone 53 (the board's own peripherals), decided by calef on 2026-08-15: NVMe first. The backup workload
(milestone 55 (Time Machine)) measures sustained sequential write and endurance, which SD media fails. And a real
PCIe device driver compounds toward the machines this project actually wants to
run on. Everything this project drove before this was a QEMU paravirtual device. An NVMe controller
is a real device family with a real specification, the same one in the M.2 drive on the desk. QEMU
emulates it faithfully enough that the driver written here is the driver the
board-side work will reuse.

## The shape

Three pieces, in the tree's usual split:

- **`crates/non_volatile_memory_express`**: the pure logic. Register field decode (CAP), command
  building (the 64-byte submission entries), and the submission/completion ring arithmetic with the
  phase tag. Doorbell addressing, PRP construction, IDENTIFY parsing, and (milestone 261 (the NVMe driver leaves the kernel)) the
  spawn handoff and the block-range check the EL0 data plane runs before it builds a command.
  Host-tested in milliseconds, eight Kani harnesses (`script/verify`), no MMIO anywhere in it.
- **`kernel/src/non_volatile_memory_express.rs`**: the admin plane's volatile half, at EL1.
  Reset, the admin rings by register, enable, IDENTIFY, Create I/O Queue.
  `NonVolatileMemoryExpress` takes the register window and one DMA region passed in (rule 2);
  `bring_up()` is the policy that finds the controller, allocates the region, and confines the
  device.
- **`components/src/non_volatile_memory_express.rs`**: the data plane's volatile half, at EL0
  since milestone 261. Copies commands into the I/O submission ring, rings the doorbell, polls the
  completion ring's phase tag, and serves `filesystem_contract::blk`. It shares the crate's name
  rather than carrying a `_server` suffix, which calef ruled on 2026-09-18: the obvious spelling is
  34 bytes against `nifefs`'s 32-byte `NAME_LEN`, and the program's own header has the argument.
- **`kernel/src/user/non_volatile_memory_express_service.rs`**: the wiring, and the whole of the
  confinement claim: what that process is handed and what it is refused, in one `Spawn` literal.
- **`kernel/src/pci.rs::find_nvme_device`**: enumeration and transport bring-up over the §18 PCIe
  machinery, matching the NVMe class code (`01:08:02`) rather than a vendor id, because the
  class triple is the one identity the spec requires of every controller, QEMU's included.

## How the protocol works, in one sitting

NVMe is a queue machine. The driver owns rings in its own DMA memory; the controller owns nothing
in the driver's space except what commands point it at.

1. **Commands** are 64-byte entries the driver writes into a **submission queue** (SQ), then
   announces by writing the ring's new tail index to that queue's doorbell register in BAR0.
2. The controller fetches the command (a DMA read of the SQ), does the work (DMA to or from the
   addresses in the command's PRP fields), and posts a 16-byte entry to the paired
   completion queue (CQ).
3. Every completion carries a phase tag that the controller flips each time it laps the ring.
   The ring starts zeroed, so the first lap writes tag 1, the second tag 0, and so on; the driver
   knows which tag means "fresh" and needs no shared index and no interrupt to spot a completion.
   `crates/non_volatile_memory_express`'s `CqState` owns that discipline and a Kani harness
   proves the flip happens
   exactly at the wrap.
4. Queue pair 0 (the admin queue) is created by plain register writes (`AQA`/`ASQ`/`ACQ`)
   while the controller is disabled; every other pair is created by admin commands (CQ first,
   because an SQ names its CQ at creation). Admin commands and I/O commands are the same wire
   shape through the same rings, so bring-up itself exercises the transfer machinery.

The driver serves whole 4096-byte filesystem blocks, `filesystem_contract::blk`'s unit, and exposes exactly
the blk-IPC verbs as methods: `read_block`, `write_block`, `size_bytes`. A block is one to eight
logical blocks depending on the namespace's LBA format (QEMU's default is 512-byte LBAs, so eight);
IDENTIFY tells the driver the format and `IdentifyNamespace::blocks_per` does the arithmetic.

## The confinement story, which is the interesting part

This is the first DMA device in the tree whose addresses the kernel cannot validate before the
device uses them. The virtio drivers run at EL0 behind a `Virtio` capability: the kernel owns the
queue addresses and checks every descriptor against the driver's DMA region before ringing the
device. NVMe's equivalent of a descriptor is the PRP inside a command the controller fetches from
driver-written memory, and nothing kernel-side parses commands on their way past.

What bounds the device instead is the IOMMU alone (milestone 16b): `bring_up` confines the
controller's requester id to its DMA region *before* the controller is enabled, so there
is no instant at which an enabled controller could reach other memory. All three test boots have an
IOMMU denying unlisted requester ids by default. They are aarch64's SMMUv3, riscv64's ratified
RISC-V IOMMU, and x86_64's VT-d, confirmed behind this driver 2026-08-25 per the evidence section of §86 (whether an NVMe
driver can leave the kernel). That cuts two ways. The confinement is real (an address outside the region faults
in hardware). And it is *mandatory*: an unconfined NVMe controller on these machines cannot fetch
its first command, so a boot where someone forgets the confine call fails loudly rather than running
unconfined.

Note the flag difference in the runners: the virtio PCI devices need `iommu_platform=on` or QEMU
silently routes their DMA around the IOMMU. The NVMe device model needs no flag, because a real
PCI device's DMA always goes through the PCI address space. One less thing to forget, and the
reason the runner comment says so at the attach line.

### The driver left the kernel, 2026-09-17

Milestone 261 built §86's option 2a, and the paragraph above is now about a *process* rather
than about the kernel. The split costs no new syscall surface at all, because NVMe 1.4 section 3.1 already
put a page boundary where the authority boundary belongs. The controller registers (`CC`,
`CSTS`, `AQA`, `ASQ`, `ACQ`) are below offset `1000h` and the first doorbell is at `1000h`. So the
kernel maps one page of BAR0 into the server and not the other, and the server holds no
`DeviceFrame`, no `PageFrame`, no `Irq` and no `Virtio` capability. Both windows arrive as
mappings installed before `_start` runs, the way milestone 159's TRNG driver's register page does.

What the split actually withholds, stated precisely because it is easy to overclaim. Creating
a queue is what names the physical address a ring lives at, in a PRP field of an admin command,
and the admin rings' own bases are the `ASQ` and `ACQ` registers. A process that can issue no
admin command and cannot name those registers cannot choose where any ring lives.

**What it does not withhold.** The IOMMU keys on the controller's requester id and bounds it to
the whole allocation, admin rings included, because the controller fetches from both halves. So a
server that computes a PRP backwards from its own base can make the controller overwrite the admin
ring. And with `CAP.DSTRD` = 0 the admin doorbells share the mapped page, so it can ring the admin
queue without being able to write what that queue holds. Both are denial of service against the
controller rather than an escape, §86's option 2a says so in those terms, and option 4's doorbell
validator is what would close them.

### What x86_64 needed that the other two did not

Wiring the same driver onto `q35` (2026-08-25, decisions §86's evidence section) surfaced two bugs
that only a real (non-virtio) DMA device with a real PVH boot could have found, both fixed rather
than worked around:

- `kernel/src/pci.rs::place_bars` used to trust any nonzero BAR as "already placed, already
  mapped." True on the two device-tree architectures, where nothing runs before this kernel to place
  one. False on `x86_64`'s PVH boot, where QEMU resets `-device nvme`'s BAR0 to a live address of
  its own choosing. (It is attached directly to the root complex, with no firmware in between to
  reprogram it.) That address is unrelated to `PCI_BAR_PHYS`, the window `mmu::map_everything`
  actually mapped.
  `place_bars` now checks the existing address against that window rather than against zero.
- `kernel/src/memory.rs::bring_up_page_frames` and `kernel/src/arch/x86_64/mmu.rs`'s
  `map_firmware_regions` both sized themselves from `ram_regions()` alone (the e820 map's
  `usable` entries). Attaching VT-d and NVMe together grows the ACPI tables QEMU parks just above
  the top of guest memory. They grow enough that the `reserved` entry above them swallows the
  initrd's last few hundred bytes. The PVH loader placed those at a fixed offset below the top of
  memory sized
  for a smaller device set. The frame allocator's bitmap now widens to cover whatever `forbidden`
  reaches past RAM's own end, and the direct map now covers the initrd's recorded bounds
  explicitly, regardless of how the memmap classified the bytes.

Neither is `x86_64`-only in principle (a real UEFI machine, milestone 87, picks its own BAR
addresses too), which is why both fixes check against what is actually true rather than against
which architecture is running.

## EXAMPLES

Wire the server and move a block through it (this is the boot test, abridged; the full version is
`system_tests/src/user/non_volatile_memory_express_tests.rs`). Every one of these calls crosses a
rendezvous to an unprivileged process; the caller holds one endpoint and no device:

```rust
let disk = non_volatile_memory_express_service::ensure(
    program("non_volatile_memory_express").unwrap(),
)
.expect("an NVMe controller is attached");
// Against the geometry this boot was handed, never a constant: the same line holds for the
// runner's 8 MiB image and for a 256 GB namespace.
assert_eq!(disk.blk(blk::SIZE, 0) as u64, disk.size_bytes);

// SAFETY: a blk request is a CALL, so one side holds the buffer at a time.
unsafe { disk.transfer_block() }.fill(0x5a);
assert_eq!(disk.blk(blk::WRITE, 37), 0);   // eight 512-byte LBAs, one command, one PRP
unsafe { disk.transfer_block() }.fill(0);
assert_eq!(disk.blk(blk::READ, 37), 0);
assert!(unsafe { disk.transfer_block() }.iter().all(|b| *b == 0x5a));
```

Run the proof of all of it on all three architectures:

```sh
script/test            # the boot test runs in every leg; xtask attaches the controller
# The queue mechanics alone, on the host, in milliseconds.
cargo test -p non_volatile_memory_express
# The eight harnesses, ~seconds.
cargo kani -p non_volatile_memory_express
```

Poke at the controller interactively:

```sh
cargo xtask build && NIFE_NVME=target/nife-nvme.img cargo xtask run
# (write the image first: any size a controller will take, e.g. an 8 MiB zero file)
```

## What the test proves, and where

`system_tests/src/user/non_volatile_memory_express_tests.rs`'s
`a_confined_el0_process_serves_the_block_interface_end_to_end`, on all three architectures (§19 (architectural parity is a tenet);
x86_64 joined 2026-08-25, decisions §86's evidence section). The controller enumerates over ECAM and
comes up confined behind the SMMU (aarch64), the RISC-V IOMMU (riscv64), or VT-d (x86_64). It
answers IDENTIFY with the attached disk's exact size. Then an EL0 process serves SIZE, WRITE,
READ-back with byte-exact verification. A second block is written with a second pattern so that each
block reads back its own (the write landed where it said, not everywhere). There is a refusal of a
block outside the namespace, a flush count that moves, and a refusal of an opcode it has no verb
for. Every one of those is written against the geometry the boot was
handed rather than against the runner's image size, and none of them assumes what the disk held
beforehand. So the same test proves the same things on a 256 GB disk (milestone 318 (the NVMe boot test on real geometry)). It asserts the
IOMMU was active, which matters more than it did when the driver was the kernel. The IOMMU is now
the *whole* of what stops a compromised server reaching memory it was not
given.

This is not fatal risk 6's experiment, and nothing here should be read as one. Milestone 16b
already proved IOMMU-backed isolation against emulated silicon; risk 6's open clause is about a
real device at real speed, on xenon, photographed. The boot that takes it, its two preflights and
what each outcome means are [risk-6-bench-evening.md](risk-6-bench-evening.md).

"Confined" means the IOMMU unit that owns the controller is the one translating. Some unit being up
is not enough. On a machine with two VT-d units (a client Intel part: one for the GPU, one
catch-all), that is a weaker claim; see that page's first night-of condition. A controller the
driver refuses fails the test.

The parity note milestone 53 requires: what ships on all three architectures is this driver
against QEMU's `-device nvme`, on the two `virt` machines and on `q35`. The VisionFive 2's PLDA
XpressRICH root complex is a different host bridge (no `pci-host-ecam-generic` node, so
`find_nvme_device` truthfully reports nobody home on the board today). Driving it is the
board-side follow-up, and this driver is written to need only a working enumeration and a BAR from
it.

## The DMA-escape attacker and the client windows

The confinement test needs a driver that tries to escape, and the attacker is this exact server
differing by one field: the physical address it puts in a PRP. So the attack tests the IOMMU rather
than a straw man. It is the NVMe twin of `block_driver`'s two virtio attacker roles (milestone 261
(the NVMe driver leaves the kernel), milestone 202 (every confinement test is a ritual until
somebody breaks the confinement and watches it fail)). `start_dma_escape` in the kernel's wiring
writes a magic word and a victim's physical address into the transfer buffer before the server
runs. The server reads them before its readiness transfer, which reuses that page. The victim is a
raw number read out of shared memory, exactly the authority an escaping driver would fabricate from
nothing. The test's point is that holding the number buys the controller no reach the IOMMU did not
grant it. The role is compiled only with the `confinement_attackers` feature, which only `cargo
xtask test` turns on, so a shipped server has neither the check nor the attack. The zeroed buffer
of a real boot is not the only thing standing between it and the attack.

Each client stages in the transfer window its badge names (milestone 834 (ioping on nife and
Linux)). A client maps only its own window, and the server answers one request at a time, so two
clients staging at once touch different pages. A badge no client was given is refused before
anything is staged.

## BUGS

- What the driver does not withhold ("What it does not withhold", above): the IOMMU bounds the *controller* to a
  region that contains the admin rings, so the server can aim DMA at them, and with stride 0 it can ring the admin
  doorbell. Neither is an escape and both are option 4's to close.
- Nothing but the test serves it over blk IPC. `non_volatile_memory_express` speaks the four
  blk verbs and `block_roster` lists the controller (`TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS`,
  §193 (what a block-roster entry calls an NVMe disk)), so `disk_surveyor` sees the disk, but no
  program is handed its endpoint and the FS server's default remains virtio-blk.
- A transfer is one command per filesystem block, so a sixteen-block blk request is sixteen
  round trips where the virtio block server issues one. `prp_pair` refuses anything needing a PRP
  list, and a multi-page transfer needs one.
- The EL0 server cannot read `CSTS`, so a hung controller presents to it as a bounded-out poll
  answered `EIO` rather than as `CSTS.CFS`. The direct price of not mapping the controller register
  page, and a worse diagnostic than the kernel-resident driver gave.
- A doorbell stride above `non_volatile_memory_express::MAX_DSTRD` (8) is refused rather than
  served. Doorbell N sits at `0x1000 + N * (4 << DSTRD)`, so a wide stride scales the doorbell
  file past the one page of BAR0 the server is mapped. Kani found this, producing `DSTRD = 15` in
  under a second, which puts queue 1's submission tail at `0x41000`. A real controller reporting it
  would have met `MappedWindow`'s bounds check as a panic in the disk driver rather than a
  diagnosis. QEMU reports 0 and §86 quotes the spec calling 0 "the expected doorbell stride value"
  for hardware, so no controller this project has met would hit it, and one that did now fails
  loudly at bring-up naming itself.
- Polling only. Completions are spotted by phase-tag polls with a spin bound, not by the
  interrupt-as-message path the virtio drivers use. Right for QEMU (which completes synchronously
  inside the doorbell write) and for a boot test; wrong for a shared machine under load. The
  controller is created with IEN=0 and no MSI-X table is touched, so interrupts are additive later.
  It also keeps `Object::Irq` off the EL0 server's grant list, which is a smaller authority, and
  §86's interrupt finding is the reason not to reach for MSI-X casually. An MSI write is a memory
  write to an architecturally special address. This tree runs `intel-iommu` with no
  `intremap=on` and `gic-version=2` with no ITS, so nothing would confine one a userspace driver
  aimed. See `notes/confinement-claims.md`'s fifth claim.
- One command in flight per queue. `SqState` asserts rather than manages a full ring; queue
  depth is 16 to keep wraps exercised, not for parallelism. Milestone 55's storage bench will want
  real queue depth, and the ring arithmetic already supports it; the driver's completion loop does
  not.
- One namespace, PRP-only, no SGLs, no PRP lists. Transfers are exactly one filesystem block.
  All fine for the blk unit; a future bulk path (or a 8 KiB+ LBA format) needs PRP lists.
- **`bring_up` is not idempotent.** A second call re-confines the requester id (leaking the first
  domain's tables, as `iommu::confine` documents) and re-creates queues against a live controller,
  which the controller will refuse. Call it once; `non_volatile_memory_express_service::ensure`
  holds a once-per-boot flag
  for exactly this reason, and the boot test does everything in one case.
