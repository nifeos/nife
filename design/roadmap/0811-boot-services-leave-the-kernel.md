---
status: NOT-STARTED
raised: 2026-10-07
milestone_dependencies: 22, 23, 166, 600
decision_dependencies: 19, 26, 32, 86, 102, 121, 130, 159
machine_requirements: none
specific_machine: none
needs_person: no
---
# 811. The boot services leave the kernel: the kernel starts only the progenitor

*(Title and slug are drafts.)*

calef, 2026-10-07 (UTC): *"'move the boot service starters from the kernel to the progenitor' seems
like a milestone that's part of making nife ready for customers."*

The purpose: on a real boot the kernel builds one process, the progenitor, and hands it the device
capabilities. Every driver and server after that is built by userspace and has a supervisor. A
process the kernel starts has no parent that can restart it. §159 (lab machines upgrade like user
machines, and only a new kernel needs a reboot) was amended on 2026-10-07 by PR #1805: upgrading a
userspace component does not reboot the machine. A kernel-started process cannot meet that, because
nothing can restart or update it short of a reboot. Today that includes the file server.

On principle 1 this sits on the customer path. A customer's machine whose file server dies has to be
rebooted, and so does one taking a package update to its disk driver. That is the gap between "runs"
and "runs it" for any first customer, whoever it turns out to be.

## What exists, checked 2026-10-07 (UTC)

Read from `main` at PR #1804's merge; line numbers are from that commit. "Real boot" means a
build with none of `test`, `system_tests`, `bench` or the bench boots' features. A "tour" entry
runs only on the default build, which has no `shell` feature, before the hand-over on that
architecture.

### Started by the kernel on a real boot, inside `boot_progenitor`

| starter | call site | spawns | arch | what the process is handed |
|---|---|---|---|---|
| `fs_service::root_directory` | `boot_progenitor` (`user/progenitor_boot.rs`) | `block_driver` (`spawn_block_server`, `fs_service/wiring.rs`), `redoxfs_server` (`spawn_fs_server`, `fs_service/wiring.rs`) | all | virtio: a `Virtio` cap, an `Irq`, DMA pages as spawn-time mappings; the FS server gets a `MemoryRegion` budget and the channel pages |
| the NVMe arm of the above | `nvme_disk` (`fs_service/wiring.rs`), only when booted from NVMe | `non_volatile_memory_express` (§86's data plane) | all (exercised on `x86_64`) | BAR0's doorbell page and a DMA run, as mappings; the kernel keeps the admin plane |
| `boot_clock_page` | `boot_progenitor`; starter in `user/device_starters.rs` | `clock` (`clock_service.rs:58`) | all | the clock page, and the RTC page as a mapping on aarch64 and riscv64; on `x86_64` a CMOS reading as an argument (§130) |
| `boot_instruction_entropy` | `boot_progenitor`; starter in `user/device_starters.rs` | `entropy` in instruction mode | `x86_64`, aarch64 with `FEAT_RNG`; riscv64 answers `None` | two endpoints; no device |
| `boot_usb_keyboard` | `boot_progenitor`; starter in `user/device_starters.rs` | `usb_keyboard_driver` | all | an `Irq`, about a dozen xHCI register pages and eleven DMA pages as mappings |
| `boot_screen_terminal` | `boot_progenitor`; starter in `user/device_starters.rs` | `framebuffer_driver`, `display_terminal` | all, UEFI boots only | the firmware screen's aperture |
| `boot_e1000e_network` | `boot_progenitor`; starter in `user/device_starters.rs` | `net_stack` over an `e1000e` | all (a PCIe part; exercised on `x86_64`) | BAR0's two queue pages and an 18-page DMA region as mappings |
| `boot_designware_network` | `boot_progenitor`; starter in `user/device_starters.rs` | `net_stack` over radon's GMAC | riscv64 | nothing until `PROVEN_ON_SILICON` |
| `install_service::confirm` | `boot_progenitor`; in `user/install_service.rs` | `system_installer` (confirm role) | `x86_64` | one disk's NVMe endpoint |

### Started by the kernel on a real boot, outside `boot_progenitor`

| starter | call site | spawns | arch |
|---|---|---|---|
| `install_service::offer` | `kernel/src/lib.rs:1051` | `non_volatile_memory_express`, `entropy`, `system_installer`, `mkfs` | `x86_64`, a boot from a file only |
| `x86_userspace_demo` | `lib.rs:943`; the demo is in `user/boot_demos.rs` | two hand-assembled children | `x86_64`, every boot |
| tour: `console_service::start` | `lib.rs:2134` | `console` | aarch64 |
| tour: `virtio_service::start`, `start_pci` | `lib.rs:2207`, `lib.rs:2244` | the virtio and PCIe block demos | aarch64 |
| tour: `kernel_test_subject` | `lib.rs:2267`, `lib.rs:1328` | `kernel_test_subject` | aarch64, riscv64 |
| tour: `memory_region_service::start` | `lib.rs:2304` | `memory_region_depleter` | aarch64 |
| tour: `riscv_least_authority_demo`, `riscv_uart_driver_demo` | `lib.rs:1419`, `lib.rs:1520` | demo programs | riscv64 |
| tour: `entropy_service::ensure` (MMIO, JH7110) | `lib.rs:1665`, `lib.rs:1721` | `entropy`, `jh7110_entropy` | riscv64 |

### Already granted, not started

The kernel already hands the progenitor raw device authority for the UART (slots 1-2), virtio-rng
(7-9) and virtio-net (13-15, milestone 590 (the booted system starts its network stack)). It does
the same for virtio-gpu and the virtio keyboard (12, 17-22, milestone 600 (the graphical terminal
stack is built in userspace)). The progenitor builds those services itself. That is the shape every
row above moves to.

### Test and bench only

`credential_service`, `compositor_service`, `disk_service`, `identity_provisioner_service`,
`login_service`, `ntp_service`, `raw_mode_service`, `rmle_service`, `keyboard_service::start`, the
rest of `display_service` and `fs_service`, and `holding`. Their module declarations in
`kernel/src/user.rs`, which is the index now, carry `allow(dead_code)` outside `system_tests`. The bench boots
reach `e1000e_service` (`network_bench.rs:48`), `designware_ethernet_service`
(`network_bench.rs:193`), `designware_mobile_storage_service` (`storage_bench.rs:296`),
`non_volatile_memory_express_service` (`disk_throughput.rs:43`) and `fs_service` (`bench/filesystem.rs`). This milestone leaves them where they are: a harness that wires a driver by hand is measuring
the driver, not the boot.

## Can the progenitor receive what each one needs?

| device authority | capability kind | exists? | needed by |
|---|---|---|---|
| virtio transport, interrupt, DMA run | `Virtio`, `Irq`, `PageFrame` run (§102) | yes, granted today for rng, net, gpu, keyboard | `block_driver` |
| file-service channel pages, a budget | `PageFrame` run, `MemoryRegion` | yes | `redoxfs_server` |
| one MMIO page | `DeviceFrame` | yes | RTC for `clock`, NVMe doorbell, e1000e's two queue pages |
| a DMA run for a PCIe device | `PageFrame` run | yes (unverified that the IOMMU binding survives a grant rather than a kernel-built mapping) | NVMe, e1000e, xHCI |
| several contiguous MMIO pages | `DeviceFrame` names one page | no | xHCI's register window |
| a firmware aperture | `PageFrame` run (`display_service.rs:121`) | yes | `framebuffer_driver` |
| "the CPU has a seed instruction" | none; a fact, not an object | no slot for a fact (`boot_instruction_entropy`'s doc, in `user/device_starters.rs`) | `entropy` in instruction mode |
| CMOS wall clock on `x86_64` | none; ports stay kernel-resident (§121 (what a device capability is when the device has no page: x86 port I/O)) | the reading can travel in the clock page | `clock` |

Two gaps are real. A multi-page device window has no capability, and the progenitor's capability
table is full: all 32 slots are named in `components/src/progenitor.rs`'s `GRANTS`. Moving the rows
frees seven (5, 10, 11, 16, 27, 29, 30) and needs about eleven. Fork 2's ruling answers both: a
`DeviceFrame` page count and a 64-slot table.

The progenitor can already map a capability into a child without the child holding it. So the
confinement each kernel starter argues for (the driver holds no name for its window, so it cannot
delegate it) survives the move. The supervisor holds the name, which is also what lets it re-map the
window into a restarted driver.

## The work, in order

1. Supervision on a real boot, by `root_supervisor`'s tree (fork 3). Today nothing starts
   `root_supervisor`, `spawner` or `sub_server_supervisor` on any boot; only
   `system_tests/src/user/authority_tests.rs` does (recorded by PR #1796's proposal, the init
   package holds only what init does). Without this, moving a starter changes who built it and not
   whether it can restart.
2. `block_driver` and `redoxfs_server` on the virtio path. Every capability exists. The file
   endpoint is created by the supervisor, not the server, so a client's capability outlives a
   restart (milestone 23 (live replacement)'s "a supervisor restores the service with no authority
   it did not already hold"). `fs_service::start_crash` and `recover_crash` (both in
   `fs_service/crash.rs`) already prove a respawned server recovers the same disk. Slots 5 and 6 become the device
   trio.
3. The NVMe arm, then `install_service::confirm`. Moving confirm into the progenitor retires the
   foot gun its own header names (`install_service.rs:325`): the single-user window now holds
   because one process sequences both, not because the kernel is single-threaded.
4. `clock`: the RTC page as a `DeviceFrame`; on `x86_64` the kernel writes its CMOS reading (§130
   (how the kernel-resident CMOS RTC reaches the userspace clock service)) into the clock page
   before the grant.
5. `entropy` in instruction mode: the fact travels in the inert-configuration page (§111 (inert
   configuration is a read-only page)), and the kernel stops building the service.
6. `net_stack` over `e1000e`, then over the GMAC once `PROVEN_ON_SILICON` is true.
7. The screen terminal.
8. The xHCI keyboard, last, because it needs the multi-page device window.
9. `install_service::offer` into the progenitor, and the tour behind a non-default `tour` feature
   (fork 1).

Drivers and the file server go first because they are what §159 and #1805 need restartable. A
display or a keyboard that dies is visible to the person at the machine. A file server that dies
takes every program with it.

## Parity

All three architectures move together, per §19 (architectural parity), and the slot layout stays
identical (milestone 166 (one boot loader, reached two inconsistent ways)). aarch64 and riscv64 take
the virtio file-service path under QEMU. `x86_64` takes it under QEMU and the NVMe path on xenon.
riscv64's GMAC row is inert until radon proves it. aarch64 has no silicon here until argon, so its
exit is QEMU's. The tour rows differ per architecture today; by fork 1's ruling
none of them is on a default build afterwards.

## Dependencies

- PR #1796's finding: no real boot starts the supervision tree. Work item 1 closes it.
- Milestone 23 (live replacement), PARTIAL. Restart is this milestone; live swap of a driver is 23's
  and is not required for the exit.
- PR #1805's proposal (lab machines update themselves) lists "the kernel still starts `block_driver`
  and `redoxfs_server` itself" as a gap. This milestone is that gap.
- The kernel's capability minting: a `DeviceFrame` page count and a 64-slot table (fork 2).
- §26 (the fault endpoint: thread death becomes a message) is how a supervisor learns of a death.
  §32 (a supervisor may collect a corpse without being able to build one) is how it collects one.
  §26 also says the kernel never relaunches anything, which is why the kernel cannot be the
  restarter.

## Rulings

calef ruled all three forks on PR #1807 on 2026-10-07 (UTC), each recorded there by the maintainer.

### Fork 1, ruled A: the kernel keeps no starter

Ruled 14:19 UTC: *"Yes"* to option A. The kernel keeps no service starter on any default build. The
tour's demonstrations and `x86_userspace_demo` move behind a non-default `tour` feature, and the
x86 install offer moves into the progenitor. The offer's argument (ask before the authority to wipe
exists) holds there too, because the progenitor holds the disk capabilities and need not delegate
them until a person answers. Refused: keeping the tour on the default build and proving the exit on
the `shell` build only, which proves the claim on a build no image ships; and keeping the install
offer in the kernel. Prior art (recalled): seL4's kernel starts only the root task, and Genode's
core starts only `init`. This also answers the question milestone 267 (the milestone tour is three
things wearing one name) left open about the tour's console server, for the default build.

### Fork 2, ruled A now: fixed slots, a `DeviceFrame` page count, a 64-slot table

Ruled 14:20 UTC: *"Yes"* to option A now. Device authority reaches the progenitor in fixed slots, as
every grant does today. `DeviceFrame` gains a page count, as §102 (a frame names a run of pages)
gave `PageFrame` one, and `CAPABILITY_TABLE_SLOTS` grows to 64 (calef raised it from 16 to 32 on
2026-09-27). The new method's semantics owe a `design/decisions/` record when it is built. Option B
(seL4-style device regions plus a read-only boot-information page) is recorded as its own later
milestone, for when devices number in dozens: see the candidate below. Refused: a kernel-served
endpoint that hands out devices by kind, because it puts a protocol server in the kernel.

### Fork 3, ruled B: `root_supervisor`'s tree supervises drivers

Ruled 14:23 UTC: *"B"*. The progenitor starts `root_supervisor`'s tree from milestone 22 (trusted
init). `spawner` builds each program and `sub_server_supervisor` restarts it, holding no memory
itself. This closes the gap PR #1796 found: nothing starts the tree on a real boot. Refused: the
progenitor restarting what it built, whose only argument was effort; and a driver manager per
device class. Prior art (recalled): MINIX 3's reincarnation server.

## Candidate milestone

- Not numbered: device authority as seL4-style device regions plus a read-only boot-information
  page naming each device's kind, slot and interrupt, so the progenitor reads a page instead of
  knowing a slot number per device. A new boot ABI, wanted once devices number in dozens (fork 2's
  ruling). Prior art (recalled): seL4's `BootInfo` frame; Genode's platform driver.

## Exit

A test proves both, on aarch64, riscv64 and `x86_64` under QEMU, on the default build and the
`shell` build:

1. The kernel spawns only the progenitor. The kernel counts every user process it builds (the
   `run`/`run_with` path, `user/entry.rs`) and prints the count at the hand-over;
   `script/swish-check` requires it to read 1 at the prompt.
2. `redoxfs_server` dies and its supervisor restarts it. The test arms the server to fault (the
   arming `fs_service::start_crash` uses, carried by the progenitor's spawn). The transcript shows
   the death and the restart, and a `cat` at the prompt afterwards succeeds through the shell's
   original capability.

Reuse: the supervision tree is `root_supervisor`, `spawner` and `sub_server_supervisor`
unchanged in shape. The restart test reuses `fs_service`'s crash arming and its recovery proof.
Nothing is written that the tree already has.

## BUGS

- The NVMe admin plane stays in the kernel by §86 (whether an NVMe driver can leave the kernel), so
  a restarted NVMe server inherits whatever state a crashed one left the I/O queue pair in. Whether
  the kernel must reset the pair before a restart is unmeasured.
- A restart loses requests in flight. A client sees an error, not a retry; that is milestone 23's
  contract to settle, not this one's.

## Index row

The kernel stops starting the file server, the disk driver and a dozen other services on a real
boot, and hands the device capabilities to the progenitor instead. Everything userspace runs then
has a supervisor, so the file server can die and come back without a reboot, which is what §159's
updates without a reboot need first. It is on the customer path.