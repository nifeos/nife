---
status: BUILT
raised: 2026-09-16
built: 2026-10-19
promoted_from: the-rest-of-the-x86-64-fixture-set
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 420. x86_64 has one fixture of five and no NIC, GPU, keyboard or RNG, so its skip count is a device list

The title is a statement about the past, as milestone 303 (x86_64's FS service has a server and
no disk it can find)'s block says of its own. The runner attaches all five disk fixtures, the NIC,
an RNG, a GPU and a keyboard, and the system suite's x86_64 legs run the scanout referee the display
tests need, the OVMF leg included. Promoted from the proposal `the-rest-of-the-x86-64-fixture-set`, filed 2026-09-16 by
milestone 303's lane (x86_64's FS disk). That lane attached the RedoxFS image and deliberately
stopped there, so that one milestone proved one thing. Milestone 215 (a PCI function's interrupt
reaches nothing on x86_64, so no userspace driver) listed the same set first. It numbered none of
them. *(Number provisional until the merge queue lands it.)*

## What landed

`helpers/qemu-runner-x86_64.sh` attaches all five disk fixtures the other two runners build, each
one `virtio-blk-pci` function in command-line order. The ordinals `virtio::find_block_device_n`
counts stay nifefs=0, redoxfs=1, crash=2, gpt=3, blank=4, exactly as the mmio slots carry them on
the other two. A `virtio-rng-pci` rides behind the same `iommu_platform=on` the disks use, under
the `NIFE_RNG` every test leg already exports. `cargo xtask test`'s x86_64 leg and its AMD-Vi leg
build the three new fixtures. The leg exports `NIFE_NET` itself, so a single-architecture run
boots the machine a full run always leaked in. The device-by-device outcome, which is this
milestone's deliverable:

- GPT disk (ordinal 3): `the_disk_surveyor_reads_a_table_gptfdisk_wrote` compiled and skipped on
  every x86_64 boot. It now runs, asserting the `q35` shape (every block device a PCI function,
  none mmio) and the table's contents. Running it exposed the one kernel gap the fixture set hid.
  `blk::SIZE` over virtio-pci had always answered 0: the config read (mmio offset 0x100) had no
  pci arm. The transport now resolves the `DEVICE_CFG` vendor capability and answers it. The
  surveyor's size read is true on both buses, and the gpu's config becomes reachable the same way
  when the display half lands.
- Blank disk (ordinal 4):
  `the_write_half_needs_a_disk_and_an_entropy_rendezvous_and_holds_nothing_else` skipped twice
  over, no disk and no entropy. It now runs. Its entropy rendezvous rides the PCIe transport,
  which is the bus `q35` has.
- RNG: `a_client_obtains_unpredictable_bytes_from_a_virtual_io_rng_over_pcie` now runs on x86_64. It
  asserts the device sits behind the IOMMU (VT-d, or AMD-Vi on that leg). The mmio entropy arm
  still skips, and every wiring that asks for mmio by name (credentials, NTP, login) with it.
  `q35` has no virtio-mmio bus. That arm's skip line is a machine fact, not a missing device.
- NIC: the proposal named `virtio-net-pci`, but the virtio-net tests' driver is not this port's
  NIC. Milestone 494 (a driver for the network card a PC actually has) made the `e1000e` the card
  a PC has. The runner has attached it under `NIFE_NET` since that milestone. What was missing was
  the export. The four `net_confinement_tests` and four `e1000e_tests` skipped on every
  `--arch x86_64` run; only a full run leaked the variable in. They now run on every x86_64 leg.
- GPU and keyboard: `-device virtio-gpu-pci` and `virtio-keyboard-pci` ride behind the same
  `iommu_platform=on`, under the `NIFE_GPU` and `NIFE_KEYBOARD` every test leg exports, with
  `-vga none` and a monitor socket under `NIFE_GPU_MON`. `-vga none` is load-bearing on `q35`: the
  machine carries a standard VGA by default, `screendump` writes console 0, and the first run
  graded 729 dumps of the VGA's 720x400 text screen without ever seeing the GPU. The referee is
  the same one the other two legs run, minus the inbound prober (`cargo_test_with_scanout_only`),
  because the inbound listeners ride a virtio-mmio NIC that `q35` lacks. It presses the key the
  keyboard test waits for and grades the three pictures. The compositor, display-terminal and
  pattern checks reach the device's scanout pixel for pixel on the VT-d leg and the AMD-Vi leg,
  and `a_keystroke_from_a_virtual_io_keyboard_becomes_a_terminal_byte`,
  `a_confined_userspace_driver_puts_a_known_pattern_in_a_framebuffer`,
  `a_bitmap_font_and_a_vt_engine_put_readable_text_on_the_scanout`,
  `a_backing_outside_the_grant_is_refused_by_the_iommu` and the compositor test run instead of
  skipping. Both halves were seen red: with the VGA still attached all three scanout checks
  failed, and with no referee the keyboard test failed with its "never typed anything" sentence.
  Milestone 161 (the x86_64 kernel port)'s userspace console, input and `keyboard_driver` path needed no x86_64 work; the
  drivers are arch-neutral virtio-pci clients. A `--test` filter now runs the system image beside
  the referee on every ISA (verdict dropped), because a filter that selected the keyboard test
  failed it for want of a keystroke.
- GPU and keyboard under OVMF: `helpers/qemu-uefi-x86_64.sh` attaches the same two devices under
  the same variables, with `-vga none`, and a second monitor under `NIFE_GPU_MON`. `cargo xtask
  uefi-test` turns them on for the system image's boot only and runs the referee beside it, as
  `x86_64-ovmf`. The display, compositor and keyboard tests run there and the three pictures grade
  pixel for pixel. The firmware question had a measured answer. OVMF's own virtio-gpu driver
  brings the device to DRIVER_OK and resets it to 0 at ExitBootServices (`-trace
  virtio_set_status`). Its GOP has no linear framebuffer, so the loader hands the kernel no screen,
  and the kernel's driver starts from a reset device, as under PVH. Seen red (2026-10-19): without
  `-vga none` the guest suite stayed green and all three scanout checks failed on the VGA's
  1280x800 screen. `uefi-boot`'s tour removes both variables, since its screen check reads the
  VGA's framebuffer.
- Crash disk (ordinal 2): attached, and its fixture built, but nothing witnesses it on this ISA.
  Both crash-consistency twins are `cfg`-gated to their own ISAs. No x86_64 test skips for want of
  this disk, and none runs with it. The end-of-run host check keeps the crash image out of the
  x86_64-only arm for that reason. A check whose subject did not happen is worse than no check.

## BUGS

- The crash disk is attached but unwitnessed on x86_64. The crash-consistency test has no x86_64
  arm. The fixture's only job on this machine is keeping the ordinals aligned with the other two
  runners'. A future x86_64 crash arm inherits both the attachment and this gap.
- The mmio skip lines (`no virtio-rng device on the mmio bus (NIFE_RNG not set?)`) mislead on this
  one machine. No value of `NIFE_RNG` can attach an mmio device to `q35`. The lines are true on
  the other two, where setting it works.
- The roster guard rides an environment variable. `cargo xtask test` sets `NIFE_X86_FIXTURES_REQUIRED`
  (provisional name) on the PVH leg, `NIFE_RNG` and `NIFE_NET` on every x86_64 leg, and
  `system_tests/build.rs` bakes them in. A bare `cargo test -p system_tests` sets none, so a
  runner that lost a `-device` line still skips there. The OVMF leg unsets the fixtures variable
  and grades no fixture count.

## Follow-on

- **Recorded.** The crash disk attached but unwitnessed on x86_64. This block's `BUGS` carries it,
  and `kernel/src/user/fs_service/crash.rs` is where an x86_64 arm starts (both twins are
  `cfg`-gated to their own ISAs).
- **Recorded.** The mmio skip lines that mislead on `q35`. This block's `BUGS` carries it, and
  `system_tests/src/user/entropy_tests.rs` is where the lines live.
- **Recorded.** The firmware path proves two of the new fixtures. `helpers/qemu-uefi-x86_64.sh`
  attaches the RNG under `NIFE_RNG`, so the PCIe entropy test runs under OVMF. The crash, GPT and
  blank disks stay off it: they ride behind the RedoxFS disk in ordinal order, that disk is opt-in
  there (its own `BUGS` records why), and `uefi_test` builds only the nifefs and NVMe images. Under
  OVMF the tests that want them skip on their own lines. The script's own `BUGS` carries the note.

## Index row

`helpers/qemu-runner-x86_64.sh` attaches all five disk fixtures the other two runners build, each
one `virtio-blk-pci` function in command-line order. The ordinals are nifefs=0 through blank=4,
the numbers `fs_service::crash` and `disk_service` name. It also carries the NVMe controller, an
`e1000e` NIC under `NIFE_NET` and a `virtio-rng-pci` under `NIFE_RNG`. The GPT surveyor, the
blank-disk write half, the PCIe entropy test and the `e1000e` tests therefore run on every x86_64
leg. They no longer skip for want of a device QEMU emulates on `q35`. The mmio entropy arm still
skips here, and that is a machine fact: `q35` has no virtio-mmio bus. The GPU and keyboard are
attached under `NIFE_GPU` and `NIFE_KEYBOARD` with a monitor socket, and the suite's x86_64 legs run
the scanout referee. The display, compositor and keyboard tests therefore run and grade the
device's scanout, under PVH and under OVMF. The crash, GPT and blank disks stay off the firmware path and
the crash disk is unwitnessed on x86_64; both are recorded limitations.
