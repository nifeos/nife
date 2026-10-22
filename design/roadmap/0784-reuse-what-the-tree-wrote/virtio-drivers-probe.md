# `virtio-drivers` behind the IOMMU, probed for item (c0)

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md). calef's ruling
on (c0), 2026-10-22 21:58 (UTC), on nifeos/nife#2586, carries `virtio-drivers` 0.13.0 plus
rcore-os/virtio-drivers#232 as a vendored fork "once a probe shows #232 works for the input device
behind the IOMMU". This is that probe.

The question it answers: driven by 0.13.0 plus #232 with `VIRTIO_F_ACCESS_PLATFORM` negotiated, does
a key event from the virtio keyboard arrive correctly while the device's DMA is translated by the
IOMMU?

Measured 2026-10-22 (UTC) at base `79941d883`, on an Apple M3 running macOS 26.6.2, with the pinned
nightly of 2026-10-22 and QEMU 11.1.2 for all three systems. The crate is exactly
`vendor/virtio-drivers`. [virtio-drivers-probe.sh](virtio-drivers-probe.sh) builds the probe in a
detached worktree and runs it; nothing in this tree depends on the crate.

## Result

It passes on all three architectures.

| | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| IOMMU in front of the keyboard | SMMUv3 (`iommu=smmuv3`) | RISC-V IOMMU (`riscv-iommu-pci`) | VT-d (`intel-iommu`) |
| `iommu::scope_of` confining | yes | yes | yes |
| status after `VirtIOInput::new` | `0x0f` | `0x0f` | `0x0f` |
| every `Hal` call saw `access_platform` | true | true | true |
| first key event, domain covers every buffer | `EV_KEY` 30 (`KEY_A`) value 1 | the same | the same |
| IOMMU faults then | none | none | none |
| control: bounce pool outside the domain | no key event; fault at the pool's first byte, SMMU event `0x10` (`F_TRANSLATION`) | no key event; fault at the pool's first byte, cause 15 (store page fault) | no key event; fault at the pool's first byte, reason 5 (write not permitted) |

x86_64 also ran the probe under OVMF, the system image booted by `uefi_loader`, with the same
result: the key press, no fault, and in the control reason 5 at the pool's first byte.

Status `0x0f` is `ACKNOWLEDGE | DRIVER | FEATURES_OK | DRIVER_OK`. An `iommu_platform=on` device
leaves `FEATURES_OK` clear for a driver that does not accept bit 33 (`kernel/src/virtio.rs`,
`reset_to_features_ok`), so `0x0f` is the negotiation succeeding. The control is what shows the DMA is translated rather than merely permitted. With the
event buffers' pages left out of the domain, the device's event writes fault in the IOMMU at the
address the driver handed it, and nothing arrives. With the pages in, the same writes land.

## What the probe is

A system test drives the keyboard from the kernel, through the crate's own `PciTransport` and
`VirtIOInput`, and `script/test`'s referee presses `a` over the QEMU monitor, as it does for the
keyboard test.

- `Hal` serves one 16-page region the kernel allocates. `dma_alloc` takes the first 8 pages for the
  virtqueues. `share` copies each buffer into a bounce slot in the other 8 and returns the slot's
  physical address, and `unshare` copies it back. The domain is an identity map (IOVA equals
  physical address), as `iommu::confine` builds for every device.
- The positive run confines the keyboard's requester id to all 16 pages. The control confines it to
  the queue pages only and fills the bounce pool with `0xff` first, so a buffer the device could not
  write reads back as poison rather than as the previous run's event.
- Config space goes through the kernel's ECAM accessors; the BARs are where `pci::bring_up` placed
  them.

## What it does not show

- It does not drive the device through nife's confined transport. The keyboard driver is a
  userspace program behind the kernel's `virtio_service`, which places the queues itself and shadows
  the descriptor table and available ring. The crate's `VirtQueue` allocates its own layout and
  hands the addresses to `Transport::queue_set`. Joining the two is (c1)'s design.
- `Hal`'s methods are static, so the probe holds its DMA region in globals. A driver built on the
  crate holds its DMA capability the same way; (c1) decides where that global lives.
- The crate's `begin_init` writes `FEATURES_OK` and never reads it back, in 0.13.0 and on upstream's
  main line at `c76e09d7`. A failed negotiation then proceeds silently, so (c1)'s driver checks the
  status after init.
- In the control, QEMU still returned every buffer as used (286 to 292 of them) after the write
  faulted. A used entry does not prove the device wrote the buffer.
- x86_64 was probed on VT-d only. The AMD-Vi leg (`NIFE_IOMMU=amd`) did not run.
- No control ran unpatched 0.13.0. Its `Hal` lacks the `access_platform` argument, so the probe does
  not build against it.

## QEMU command lines

As `helpers/qemu-runner-*.sh` built them for the probe's system-test image, with `<tree>` for the
probe's worktree. The kernel image's own runs, which select no test, are left out. x86_64 has two: the PVH boot, then
the same system image booted by OVMF and `uefi_loader`.

```sh
qemu-system-aarch64 \
    -machine virt,gic-version=2,iommu=smmuv3 \
    -cpu cortex-a72 \
    -smp 4 \
    -m 256M \
    -display none \
    -serial stdio \
    -semihosting \
    -kernel <tree>/target/aarch64-unknown-none-softfloat/debug/build/system_tests/78a17a4b4bcfa343/out/system_tests-78a17a4b4bcfa343.img \
    -initrd <tree>/target/initial-ramdisk.img \
    -global virtio-mmio.force-legacy=false \
    -drive file=<tree>/target/nifefs-blank.img,if=none,format=raw,id=hd5 \
    -device virtio-blk-device,drive=hd5 \
    -drive file=<tree>/target/nifefs-gpt.img,if=none,format=raw,id=hd4 \
    -device virtio-blk-device,drive=hd4 \
    -drive file=<tree>/target/nifefs-redoxfs-crash.img,if=none,format=raw,id=hd3 \
    -device virtio-blk-device,drive=hd3 \
    -drive file=<tree>/target/nifefs-redoxfs.img,if=none,format=raw,id=hd2 \
    -device virtio-blk-device,drive=hd2 \
    -drive file=<tree>/target/nifefs.img,if=none,format=raw,id=hd0 \
    -device virtio-blk-device,drive=hd0 \
    -drive file=<tree>/target/nifefs-pci.img,if=none,format=raw,id=hd1 \
    -device virtio-blk-pci,drive=hd1,disable-legacy=on,iommu_platform=on \
    -netdev user,id=net0,guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat,guestfwd=tcp:10.0.2.9:8080-cmd:<tree>/helpers/package-http-peer,guestfwd=tcp:10.0.2.9:53-cmd:<tree>/helpers/name-server-peer,guestfwd=tcp:10.0.2.9:8443-cmd:<tree>/helpers/tls-peer,tftp=<tree>/helpers/../target/tftp \
    -device virtio-net-device,netdev=net0 \
    -netdev user,id=net1,guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat,guestfwd=tcp:10.0.2.9:8080-cmd:<tree>/helpers/package-http-peer,guestfwd=tcp:10.0.2.9:53-cmd:<tree>/helpers/name-server-peer,guestfwd=tcp:10.0.2.9:8443-cmd:<tree>/helpers/tls-peer,tftp=<tree>/helpers/../target/tftp \
    -device virtio-net-pci,netdev=net1,disable-legacy=on,iommu_platform=on \
    -netdev user,id=net2,guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat,guestfwd=tcp:10.0.2.9:8080-cmd:<tree>/helpers/package-http-peer,guestfwd=tcp:10.0.2.9:53-cmd:<tree>/helpers/name-server-peer,guestfwd=tcp:10.0.2.9:8443-cmd:<tree>/helpers/tls-peer,tftp=<tree>/helpers/../target/tftp \
    -device e1000e,netdev=net2,mac=52:54:00:e1:00:0e,romfile= \
    -device virtio-gpu-pci,disable-legacy=on,iommu_platform=on \
    -device virtio-keyboard-pci,disable-legacy=on,iommu_platform=on \
    -device virtio-rng-device \
    -device virtio-rng-pci,disable-legacy=on,iommu_platform=on \
    -drive file=<tree>/target/nife-nvme.img,if=none,format=raw,id=nvme0 \
    -device nvme,serial=nife-nvme,drive=nvme0 \
    -monitor unix:/tmp/nife-gpu-aarch64-7426.sock,server,nowait

qemu-system-riscv64 \
    -machine virt \
    -cpu rv64 \
    -smp 4 \
    -m 256M \
    -bios default \
    -display none \
    -serial stdio \
    -kernel <tree>/target/riscv64imac-unknown-none-elf/debug/build/system_tests/ce6dd73b3ab48254/out/system_tests-ce6dd73b3ab48254 \
    -device riscv-iommu-pci \
    -initrd <tree>/target/initial-ramdisk-riscv.img \
    -global virtio-mmio.force-legacy=false \
    -drive file=<tree>/target/nifefs-blank.img,if=none,format=raw,id=hd5 \
    -device virtio-blk-device,drive=hd5 \
    -drive file=<tree>/target/nifefs-gpt.img,if=none,format=raw,id=hd4 \
    -device virtio-blk-device,drive=hd4 \
    -drive file=<tree>/target/nifefs-redoxfs-crash.img,if=none,format=raw,id=hd3 \
    -device virtio-blk-device,drive=hd3 \
    -drive file=<tree>/target/nifefs-redoxfs.img,if=none,format=raw,id=hd2 \
    -device virtio-blk-device,drive=hd2 \
    -drive file=<tree>/target/nifefs.img,if=none,format=raw,id=hd0 \
    -device virtio-blk-device,drive=hd0 \
    -drive file=<tree>/target/nifefs-pci.img,if=none,format=raw,id=hd1 \
    -device virtio-blk-pci,drive=hd1,disable-legacy=on,iommu_platform=on \
    -netdev user,id=net0,guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat,guestfwd=tcp:10.0.2.9:8080-cmd:<tree>/helpers/package-http-peer,guestfwd=tcp:10.0.2.9:53-cmd:<tree>/helpers/name-server-peer,guestfwd=tcp:10.0.2.9:8443-cmd:<tree>/helpers/tls-peer,tftp=<tree>/helpers/../target/tftp \
    -device virtio-net-device,netdev=net0 \
    -netdev user,id=net1,guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat,guestfwd=tcp:10.0.2.9:8080-cmd:<tree>/helpers/package-http-peer,guestfwd=tcp:10.0.2.9:53-cmd:<tree>/helpers/name-server-peer,guestfwd=tcp:10.0.2.9:8443-cmd:<tree>/helpers/tls-peer,tftp=<tree>/helpers/../target/tftp \
    -device virtio-net-pci,netdev=net1,disable-legacy=on,iommu_platform=on,addr=0x3.0,multifunction=on \
    -netdev user,id=net2,guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat,guestfwd=tcp:10.0.2.9:8080-cmd:<tree>/helpers/package-http-peer,guestfwd=tcp:10.0.2.9:53-cmd:<tree>/helpers/name-server-peer,guestfwd=tcp:10.0.2.9:8443-cmd:<tree>/helpers/tls-peer,tftp=<tree>/helpers/../target/tftp \
    -device e1000e,netdev=net2,mac=52:54:00:e1:00:0e,romfile=,addr=0x3.1 \
    -device virtio-gpu-pci,disable-legacy=on,iommu_platform=on \
    -device virtio-keyboard-pci,disable-legacy=on,iommu_platform=on \
    -device virtio-rng-device \
    -device virtio-rng-pci,disable-legacy=on,iommu_platform=on \
    -drive file=<tree>/target/nife-nvme.img,if=none,format=raw,id=nvme0 \
    -device nvme,serial=nife-nvme,drive=nvme0 \
    -monitor unix:/tmp/nife-gpu-riscv64-20762.sock,server,nowait

qemu-system-x86_64 \
    -machine q35 \
    -cpu max \
    -smp 2 \
    -m 256M \
    -display none \
    -serial stdio \
    -no-reboot \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -device intel-iommu \
    -drive file=<tree>/target/nifefs-pci.img,if=none,format=raw,id=hd1 \
    -device virtio-blk-pci,drive=hd1,disable-legacy=on,iommu_platform=on \
    -drive file=<tree>/target/nifefs-redoxfs.img,if=none,format=raw,id=hd2 \
    -device virtio-blk-pci,drive=hd2,disable-legacy=on,iommu_platform=on \
    -drive file=<tree>/target/nifefs-redoxfs-crash.img,if=none,format=raw,id=hd3 \
    -device virtio-blk-pci,drive=hd3,disable-legacy=on,iommu_platform=on \
    -drive file=<tree>/target/nifefs-gpt.img,if=none,format=raw,id=hd4 \
    -device virtio-blk-pci,drive=hd4,disable-legacy=on,iommu_platform=on \
    -drive file=<tree>/target/nifefs-blank.img,if=none,format=raw,id=hd5 \
    -device virtio-blk-pci,drive=hd5,disable-legacy=on,iommu_platform=on \
    -drive file=<tree>/target/nife-nvme.img,if=none,format=raw,id=nvme0 \
    -device nvme,serial=nife-nvme,drive=nvme0 \
    -netdev user,id=net0,guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat,guestfwd=tcp:10.0.2.9:8080-cmd:<tree>/helpers/package-http-peer,guestfwd=tcp:10.0.2.9:8443-cmd:<tree>/helpers/tls-peer,guestfwd=tcp:10.0.2.9:53-cmd:<tree>/helpers/name-server-peer,tftp=<tree>/helpers/../target/tftp \
    -device e1000e,netdev=net0,mac=52:54:00:e1:00:0e,romfile= \
    -device virtio-rng-pci,disable-legacy=on,iommu_platform=on \
    -vga none \
    -device virtio-gpu-pci,disable-legacy=on,iommu_platform=on \
    -device virtio-keyboard-pci,disable-legacy=on,iommu_platform=on \
    -monitor unix:/tmp/nife-gpu-x86_64-34182.sock,server,nowait \
    -kernel <tree>/target/x86_64-unknown-none/debug/build/system_tests/179ee379f9b33b66/out/system_tests-179ee379f9b33b66 \
    -initrd <tree>/target/initial-ramdisk-x86_64.img

qemu-system-x86_64 \
    -machine q35 \
    -cpu max \
    -smp 1 \
    -m 2048 \
    -display none \
    -serial stdio \
    -no-reboot \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -device intel-iommu \
    -drive file=<tree>/target/nifefs-pci.img,if=none,format=raw,id=hd1 \
    -device virtio-blk-pci,drive=hd1,disable-legacy=on,iommu_platform=on \
    -drive file=<tree>/target/nife-nvme.img,if=none,format=raw,id=nvme0 \
    -device nvme,serial=nife-nvme,drive=nvme0 \
    -netdev user,id=net0,guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat,guestfwd=tcp:10.0.2.9:8080-cmd:<tree>/helpers/package-http-peer,guestfwd=tcp:10.0.2.9:53-cmd:<tree>/helpers/name-server-peer,guestfwd=tcp:10.0.2.9:9494-cmd:<tree>/helpers/network-bench-peer,guestfwd=tcp:10.0.2.9:8443-cmd:<tree>/helpers/tls-peer,tftp=helpers/../target/tftp \
    -device e1000e,netdev=net0,mac=52:54:00:e1:00:0e,romfile= \
    -device virtio-rng-pci,disable-legacy=on,iommu_platform=on \
    -vga none \
    -device virtio-gpu-pci,disable-legacy=on,iommu_platform=on \
    -device virtio-keyboard-pci,disable-legacy=on,iommu_platform=on \
    -monitor unix:/tmp/nife-gpu-x86_64-ovmf-34182.sock,server,nowait \
    -drive if=pflash,format=raw,unit=0,readonly=on,file=/opt/homebrew/share/qemu/edk2-x86_64-code.fd \
    -drive if=pflash,format=raw,unit=1,file=target/ovmf-vars.fd \
    -drive format=raw,file=fat:rw:<tree>/target/esp-test
```
