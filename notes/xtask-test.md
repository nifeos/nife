# The test command: what `cargo xtask test` runs, and why in that order

Name: provisional, minted 2026-10-24 (UTC) by lane comments/929-xtask-03. Named for the command it
describes, `cargo xtask test`, which `script/test` wraps.

This note carries the design behind `xtask/src/suite.rs`. The code keeps each constraint beside the
line it governs; the reasons that span the whole command live here.

## The order

The host pass runs first. The host crates hold the pure logic and fail in milliseconds with no
emulator, so a red host test costs nothing to find (DECISIONS §7 (testing: a QEMU harness and host-testable crates, from commit one)). Only then does the command spend
TCG boots: aarch64, riscv64, then x86_64. After the three legs come the boot tour on a `ramfb`
screen for the two boards, and last the host-side image checks.

## The flags

Each flag defaults to a full run, so a plain `script/test` always means the whole suite.

- `--arch` takes one architecture or a comma-separated list, and runs those legs in the suite's own
  order. CI's suite runs as two jobs, and one of them runs two legs (`--arch aarch64,riscv64`).
  `ArchLegs` is therefore a set of three flags rather than an enum: a set has no default arm for a
  future architecture to fall into. A misspelled or empty entry is refused, so a typo cannot drop a
  leg.
- `--no-host` skips the host pass. CI's `test-x86_64` row passes it because the
  `test-aarch64-riscv64` row ran that pass on the same commit. Running it twice cost about two
  minutes of the shorter job.
- `--cpu <model>` sets `NIFE_CPU`, which both QEMU runners read. Unset means `cortex-a72` on aarch64
  and `rv64` on riscv64. The model reaches the runners through the environment because cargo owns
  the QEMU command line. `script/cpu-matrix` drives `--arch` and `--cpu` (notes/cpu-models.md).
- `--hvf` runs the aarch64 leg on the physical Apple Silicon core. Hypervisor.framework runs the
  host's own ISA, so the flag narrows the run to aarch64 and refuses `--cpu` and the other legs.
  notes/hvf-leg.md has the mechanism. `script/ci-build` passes `--hvf`, the `--arch` list and
  `--no-host`.
- `--test <substring>` runs only the kernel tests whose full path contains the substring. It has
  the shape `cargo test <name>` has (milestone 210 (no kernel test can be run by name, so one
  falsification costs a whole suite)). `kernel/build.rs` bakes it into the test binary as a
  `rustc-env`, because a booted kernel has no command line. A change costs a kernel relink of about
  2.3 seconds and saves the roughly 53 seconds the aarch64 suite spends running 312 tests.
- `--repeat <N>` runs the `--test` selection N times in one boot, so a rare failure gets a frequency
  from one bench visit (milestone 321 (a corpse that did not park, on four real cores)).

`--test` selects tests, not architectures. Every leg still runs, because a filter that quietly ran
one ISA would be the parity hole DECISIONS §19 (architectural parity is a tenet) distrusts; add `--arch` to narrow. Under a filter the
command also skips the host pass (about 72 seconds here, and the host crates already have
`cargo test <name>`), the root-port and AMD-Vi boots, and the image checks. It drops the scanout and
inbound referees' verdicts too. Each of those asserts what unselected tests would have written. A
filter that matches nothing in either test image fails the leg rather than reporting a green zero.

## The host pass

The host pass is `cargo test --workspace` less the host-excluded crates. It adds every crate kept
in a workspace of its own: the vendored RedoxFS host tool and `no_std` engine, the glyph atlas generator,
the FS server's sans-IO core, and the TLS graph's two crates. A hand-kept list of `-p` flags drifts,
and the crates it misses stay green because nothing runs them.

The host-excluded set is derived by `helpers/host-excluded-crates.sh`. Every workspace member that
reaches `crates/user_mode_runtime` cannot compile for the host, because `svc` and `ecall` there are
EL0 syscalls. The whole set must be excluded: `--exclude` removes a package from the test selection
and not from the dependency graph, so cargo still builds an excluded crate that another member
needs. On an aarch64 host the bare-metal crates can compile by accident, which hides the gap on CI's
aarch64 runners. A derivation takes in the next crate to gain such a dependency without anyone
remembering it. `kernel` and `system_tests` are excluded by judgment as well, because they are bare
metal through `kernel` and the dependency walk does not see that.

## Each leg describes its own machine

Every leg regenerates the disk images it attaches before it boots. The legs share image paths and
their tests write those images, so reuse would make a leg's result depend on which leg ran before
it. Each leg gets the same known-good fixtures instead, and any one leg can be run alone. The
cross-boot write failure this separates out is a tracked item in notes/fs-server.md.

The same rule covers the environment. `host::cargo` exports `NIFE_INITIAL_RAMDISK` and `NIFE_DISK`
for aarch64, and a leg that runs later inherits whatever an earlier leg set. The riscv64 and x86_64
legs therefore set the archive, the disk and `NIFE_NET` themselves. An x86_64 kernel handed an
aarch64 archive refuses every program in it with a `machine` error that names neither.

The suite's devices (a virtio-gpu, a virtio keyboard, two virtio-rngs and an NVMe controller) are
set by `test` and not by `cargo()`. The benchmark boot uses the same runners, and a device its
instrument did not measure before is drift. Each device's test asserts the device is present
rather than skipping, so a leg that lost one fails loudly.

## The x86_64 leg

The x86_64 leg runs the same suite on a 4-level map, its scheduler and ring 3, exiting through
`isa-debug-exit`. Its archive carries every program in `user/`, the FS server and `mkfs`, but no
`std` farm. `q35` has no virtio-mmio bus, so the runner derives five sibling images from
`NIFE_DISK` (`-pci.img`, `-redoxfs.img`, `-redoxfs-crash.img`, `-gpt.img`, `-blank.img`) and
attaches each as a virtio-blk-pci function. The image `NIFE_DISK` itself names is attached nowhere.
`NIFE_X86_FIXTURES_REQUIRED` tells the system suite the runner owes all five, so a runner that lost
a soft-attached disk fails by name instead of skipping green. The NVMe image is the x86_64 and
VT-d data point DECISIONS §86 (whether an NVMe driver can leave the kernel) asked for.

After the suite the leg boots the same kernel on three other machines:

- **Behind a PCIe root port.** `q35` is a flat root complex where every device hangs off bus 0, so a
  kernel that enumerates only bus 0 passes there. `NIFE_PCIE_ROOT_PORT` puts the NVMe controller
  behind a `pcie-root-port`, the topology xenon's M.2 slot has. It runs one test, because the claim
  is one claim (the walk follows a bridge), and costs about three seconds.
- **On AMD-Vi.** `-device amd-iommu` writes an IVRS and no DMAR, so the kernel brings up
  `arch/x86_64/amd_vi.rs` in place of VT-d. The whole suite runs again, because DECISIONS §19 has
  the same suite prove a capability on every machine it ships on. `NIFE_IOMMU=amd` set by the
  caller replaces this boot, for one test on that machine.
- **Under OVMF.** The suite rides QEMU's PVH loader, which no real machine speaks. `uefi_boot`
  boots the tour from `\EFI\BOOT\BOOTX64.EFI`, as the Dell OptiPlex does; the tour is ten seconds and
  covers the whole boot path. `uefi_test` then boots the test binary, proving the tests pass on a
  memory map, an ACPI root and a PCIe window that came from firmware.

## The image checks

After the legs, the host tool reopens the images the last leg wrote: x86_64 in a full run, riscv64
when x86_64 was not asked for, aarch64 alone. It checks that the FS server's write persisted and the
filesystem parses. It checks that the crash disk holds one payload whole (milestone 37 (prove
RedoxFS's crash consistency)). It checks that the blank disk carries the partition table and
filesystem the guest wrote (milestone 57 (partitioning and formatting a real drive, and extended
attributes)). No crash test runs on x86_64, since its twins are cfg-gated to the other two ISAs.
After an x86_64 leg the crash check is skipped, because it would open a pristine disk and pass
without witnessing anything.
