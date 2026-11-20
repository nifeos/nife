---
status: NOT-STARTED
raised: 2026-09-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 392. A legacy virtio-mmio slot panics the kernel's scan, where skipping it is the answer

Filed 2026-09-10 as an unnumbered proposal by the lane that built milestone
306, which met this as a boot panic in a step that had nothing to do with virtio; numbered
2026-09-19 by milestone 433's drain of the proposal pile. Premise re-read against the tree on
2026-09-19 and still true: `kernel/src/virtual_io.rs` still reaches the `debug_assert_eq!` on
`REG_VERSION` at line 67, inside `find_by_device_id`'s walk, immediately after the `DeviceID` match
and before the device is returned. The message still reads "expected modern virtio-mmio", naming
neither the slot nor the runner flag. The runner bug that exposed it was fixed in both
runners at the time; the shape that turns a missing command-line flag into a kernel panic is
untouched. *(Number provisional until the merge queue lands it.)*

It is one `debug_assert_eq!` in `kernel/src/virtual_io.rs` and whatever the tree decides
should stand in its place, plus a test that a legacy slot is not returned.

**In brief.** `virtio::find_by_device_id` walks the mmio slots, matches on `DeviceID`, and then
`debug_assert_eq!`s that the slot's `VERSION` register is 2 (modern virtio). A slot that reports 1
is a device this kernel deliberately does not drive, and the honest answer to meeting one is the
same as meeting a slot of the wrong type: keep walking. What happens instead is a panic, in a
debug build, at whatever point in the boot happened to ask.

How it presented, which is the argument for changing it. On 2026-09-10 the riscv64 boot tour
grew a step that scans for a virtio-rng. Booted with `NIFE_RNG=1` and no `NIFE_DISK`, the runner
attached a legacy RNG. The `-global` that selects modern mmio lived inside the disk block; it was
fixed separately, in both runners. The kernel panicked with `expected modern virtio-mmio, left: 1,
right: 2` at a point in the tour that names no device and gives a reader nothing to go on. The
runner bug is fixed; the shape that turned a missing command-line flag into a kernel panic is not.

Why it survived. Every leg of the suite builds disks. The global was always present and no legacy
slot has ever existed on a machine this repository boots. The assertion has therefore never
fired in CI and never will; it fires only for someone assembling a QEMU command line by hand, which
is exactly the person least equipped to read it.

**What to weigh, because the assertion is not obviously wrong.** It was written as a tripwire for a
runner that had silently stopped selecting modern virtio, and that is a real failure worth catching.
A scan that skipped quietly would report "no device" where the truth is "a device this kernel
declines to drive". Milestone 145 (a test that needs hardware the boot doesn't have can say so)'s
`None` path already means the first thing. So the choice is
between skipping with a printed line that says which slot was declined and why, and keeping the
panic with a message that names the missing runner flag. The first is probably right (a kernel
should not die of a device it does not want) but the second is defensible and the difference is a
judgment about who the reader is.

**Blocked until it is answered:** nothing.

## Index row

`virtio::find_by_device_id` walks the mmio slots, matches on `DeviceID`, and then asserts in a debug
build that the slot's `VERSION` register is 2. A slot reporting 1 is a device this kernel
deliberately does not drive, and the honest answer to meeting one is the same as meeting a slot of
the wrong type: keep walking. What happens instead is a panic at whatever point in the boot happened
to ask, which on 2026-09-10 was the riscv64 tour's new entropy step, printing `expected modern
virtio-mmio, left: 1, right: 2` at a step that names no device. It survived because every leg of the
suite builds disks, so the `-global` selecting modern mmio was always present and no legacy slot has
ever existed on a machine this repository boots. The assertion has never fired in CI and never will.
It fires only for someone assembling a QEMU command line by hand. That is the person least equipped
to read it. The assertion is not obviously wrong, which is what the work has to weigh. It was a
tripwire for
a runner that had silently stopped selecting modern virtio, and a scan that skipped quietly reports
"no device" where the truth is "a device this kernel declines to drive", which is what milestone
145's `None` path already means. So the choice is between skipping with a printed line naming the
declined slot and keeping the panic with a message that names the missing runner flag.
