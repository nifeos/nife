---
status: NOT-STARTED
raised: 2026-09-13
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 396. An apt QEMU that is installed and then shadowed

`script/bootstrap` was re-read on 2026-09-19 and the premise is intact. Its Linux branch still runs
`sudo apt-get install -y qemu-system-arm qemu-system-misc qemu-system-x86 ipxe-qemu ovmf` (line 78).
It still sources `helpers/qemu-path.sh` at line 18 before any `command -v` probe and again at line
198 after `script/ci-qemu` builds the pinned QEMU. The three emulator packages are still shadowed
for the life of the checkout.

Nothing here needs calef. It needs four gate runs on a Linux box with the packages
absent, listed below, and a lane can do all four.

## In brief

On a cold Linux clone, `script/bootstrap` installs five apt packages:

```
sudo apt-get install -y qemu-system-arm qemu-system-misc qemu-system-x86 ipxe-qemu ovmf
```

and then, minutes later on the same run, builds QEMU 11.0.2 from source because no Ubuntu release
ships one with `riscv-iommu-pci`. From that moment `helpers/qemu-path.sh` puts the built prefix ahead
of `/usr/bin` on PATH, so **the three emulator packages are shadowed for the life of the checkout**.
They are a few hundred megabytes downloaded to be overridden.

The two firmware packages were the reason to be careful, and they may be redundant too. `ipxe-qemu`
supplies `efi-virtio.rom`, the option ROM `-device virtio-blk-device` loads; `ovmf` supplies
milestone 87's UEFI firmware, and Ubuntu 24.04 spells it `OVMF_CODE_4M.fd`. Measured on the prefix
QEMU's own `make install` output (`~/.cache/nife-qemu/share/qemu`, 71 files):

```
efi-virtio.rom          pxe-e1000.rom     pxe-virtio.rom     edk2-x86_64-code.fd
edk2-aarch64-code.fd    edk2-i386-vars.fd edk2-riscv-code.fd (and nine more edk2-*)
```

So QEMU ships both of them itself, into the prefix, and `helpers/qemu-uefi-x86_64.sh` already
searches `<prefix>/share/qemu/edk2-x86_64-code.fd` first, ahead of the `/usr/share/OVMF` entries,
for a reason its own header records. CI builds QEMU into a cached prefix, so no absolute path in a
list can ever name the firmware.

## Why milestone 287 did not do it

The prefix *containing* the files is not the same claim as the gates *passing* without the packages.
The x86_64 UEFI boot and `script/netboot-rehearsal` are the two paths that consume this firmware, and
neither was run on a box with `ipxe-qemu` and `ovmf` absent. Removing a package on that evidence
would be the shape milestone 287 exists to correct: a change that looks right and was never run
against the configuration it changes.

There is a second-order effect worth stating, because milestone 287 introduced it. `script/bootstrap`
now sources `helpers/qemu-path.sh` before its `command -v` probes, so on a machine where the prefix
already holds the pinned QEMU the whole apt branch is skipped, firmware packages included. A box
that ran `script/ci-qemu` before its first `script/setup` therefore never gets `ovmf` or `ipxe-qemu`
at all. That is believed fine, on the measurement above, and it is currently believed rather than
shown.

## What would close it

1. On a Linux box, `apt-get remove --purge ipxe-qemu ovmf` and confirm nothing else on the machine
   wanted them.
2. `cargo xtask uefi-image && cargo xtask uefi-boot` green, which exercises
   `helpers/qemu-uefi-x86_64.sh`'s firmware search against the prefix alone.
3. `script/netboot-rehearsal` green, which is the other consumer of the option ROMs.
4. `script/test` green on all three ISAs, which is where `-device virtio-blk-device` loads
   `efi-virtio.rom`.

If all four pass, drop the five packages from `script/bootstrap`'s Linux branch. Let
`script/ci-qemu` be the only emulator source on Linux, with the bootstrap comment rewritten to say
why (it currently explains at length why the firmware packages are needed on Debian, and that
paragraph is the thing being falsified).

If any fail, the finding is the opposite and worth just as much: name in that comment which file
comes from apt and which from the prefix, so the next person does not have to re-derive it.

## What it is worth

A few hundred megabytes and one less confusing step on a first clone. **Not correctness**: the
current arrangement works, it is merely wasteful and it reads as though apt's QEMU matters when it
does not. Priced accordingly.

## Index row

On a cold Linux clone `script/bootstrap` installs five apt packages and then builds QEMU 11.0.2 from
source, because no Ubuntu release ships one with `riscv-iommu-pci`. `helpers/qemu-path.sh` then puts
the built prefix ahead of `/usr/bin`, so the three emulator packages are a few hundred megabytes
downloaded to be overridden. The two firmware packages are probably redundant too: the prefix's
`make install` lays down `efi-virtio.rom` and thirteen `edk2-*` images, and
`helpers/qemu-uefi-x86_64.sh` searches the prefix ahead of `/usr/share/OVMF`. Milestone 287
(`script/bootstrap` installs a working QEMU on Linux, instead of printing how to) did not act on
that, because files in the prefix do not prove the gates pass without the packages. The work is four
runs on a Linux box with `ipxe-qemu` and `ovmf` purged: `cargo xtask uefi-image` and `uefi-boot`,
`script/netboot-rehearsal`, and `script/test` on all three ISAs. If all pass, the five packages leave
the Linux branch. If any fail, name in the comment which file comes from apt and which from the
prefix. It saves a few hundred megabytes and one confusing step, and it is not correctness.
