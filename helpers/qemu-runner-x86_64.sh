#!/bin/sh
#
# The x86_64 QEMU runner (milestone 161). Cargo invokes this for `cargo run` and `cargo test` on the
# x86_64 target, appending the path to the ELF it just built.
#
# The simplest of the three runners, and deliberately so while the port is partial. There is no
# flat-Image objcopy step (that is aarch64's arm64 boot protocol) and no `-bios` handoff (that is
# RISC-V's OpenSBI): QEMU's `q35` reads the PVH note in our ELF, loads the segments at their
# physical addresses and enters the 32-bit trampoline directly. See kernel/src/arch/x86_64/boot.s.
#
# WHAT IS HERE, device by device: all five disk fixtures the other two runners build, every one a
# `virtio-blk-pci` function, in the ordinal order `find_block_device_n` counts (nifefs=0,
# redoxfs=1, crash=2, gpt=3, blank=4); an NVMe controller (below, decisions §86's x86_64/VT-d data
# point, because the kernel-resident NVMe driver is arch-neutral); an `e1000e` NIC under `NIFE_NET`
# (milestone 494, the network card a PC actually has; there is no `virtio-net-pci`, because the
# virtio-net tests' driver is not this port's NIC); a `virtio-rng-pci` under `NIFE_RNG`; and, under
# `NIFE_GPU` and `NIFE_KEYBOARD`, a `virtio-gpu-pci` (with `-vga none`, below) and a
# `virtio-keyboard-pci`, plus a monitor socket under `NIFE_GPU_MON`. The monitor is what makes the
# two display tests run rather than fail: the keyboard test's keystroke is the host's `sendkey` and
# the scanout is graded from `screendump`, both of which come from cargo xtask's scanout referee.
# The system-suite legs run it (`cargo_test_with_scanout_only`, with no inbound prober, because the
# listeners ride a virtio-mmio NIC that `q35` lacks). Without a referee an attached keyboard fails
# its test, so those variables belong to the test legs and not to a bare run. The OVMF runner
# (helpers/qemu-uefi-x86_64.sh) attaches no GPU or keyboard. See
# design/roadmap/0420-the-rest-of-the-x86-64-fixture-set.md.
#
# The kernel halts with `hlt` (arch::halt), so QEMU does not exit on its own. Bound any interactive
# run with helpers/qemu-bounded.sh. The rule the bound enforces:
# notes/skills/qemu-hygiene/SKILL.md, "Never leave QEMU running".

set -e

ELF="$1"
shift

# Two by default since 2026-09-23, NOT four like the other two runners. SMP bring-up
# (INIT-SIPI-SIPI through the local APIC, the SMP item of milestone 161 (the x86_64 kernel port)) is
# built and NIFE_SMP moves this
# the same way it does on the other two runners. It sat at 1 from 2026-08-25, for reasons that were
# fixed one at a time; DECISIONS §153 (how a two-core x86_64 test earns its place) is the rule that
# moved it, and milestone 315 (a port revoke that reaches every core) is the last of them.
#
# The crash that first held it at 1 (a fault partway through ordinary thread reaping, at RIP 0 or
# at `stack::PAINT`) is FIXED: it was a missing cross-core TLB shootdown, since `invlpg` is local
# to one CPU and this port had no remote half. See `arch::x86_64::mmu::shoot_down_others`.
#
# The boot-core-identity bug that made `smp::tests::every_secondary_runs_scheduled_work` fail about
# half the time at two is also FIXED (milestone 316, ap_boot's BUGS #3): `boot_cpu_id` recomputed
# "which core am I" from CPUID on every call, and now reads a record `boot.s` stamps once on the
# boot processor, the shape riscv64's BOOT_HARTID already had.
#
# The AP-bring-up flake (ap_boot's BUGS #1, a core reported "did not start" that had started) is
# FIXED too (milestone 161, 2026-09-19): `cpu_start` re-read the online count after the STARTUP IPIs
# and waited for it to move again, so a core that checked in during the settle delay was counted as
# absent. It reached two cores as well as three, which is the UEFI leg's one-in-three.
#
# The last reason it stayed at 1 was not an SMP bug at all:
# `user::x86_port_tests::a_revoked_holder_faults_on_its_next_port_write` went red intermittently at
# two cores, because `PortRange::REVOKE` reset the TSS I/O bitmap on the revoker's core only, so a
# holder on the other core kept the ports for up to one tick. Milestone 315 closed that by
# broadcasting the reset over the TLB shootdown's NMI (`segments::revoke_port_grant_everywhere`),
# and DECISIONS §153's rule is why the flip is here rather than in a later lane: the two-core suite
# going green IS the verification that the broadcast worked, so leaving it to be remembered is how
# it would not happen. What is still open at three cores and above is a silicon question, carried
# by `arch::x86_64::ap_boot`'s module doc (unmeasured above two on real hardware; xenon has not
# been asked to bring all four online); see also design/roadmap/0316-x86-smp-two-cores.md and
# design/roadmap/0161-x86-64-kernel-port.md item 5.
SMP="${NIFE_SMP:-2}"

# **`NIFE_TCG_THREAD=multi` gives this port parallel cores instead of interleaved ones** (milestone
# 321). **Provisional name.** Empty by default, which changes nothing: QEMU keeps choosing, and on
# this host it chooses round-robin.
#
# The distinction is not pedantry, and milestone 321 is the case that needed it. TCG has two vCPU
# models. Round-robin (`thread=single`) runs every guest core on ONE host thread, timeslicing them;
# multi-threaded TCG (`thread=multi`) gives each guest core its own host thread, so two cores really
# do execute at the same instant. **On an aarch64 host running an x86_64 guest, QEMU picks
# round-robin**, so `NIFE_SMP=4` here has never meant four cores running at once, only four cores
# taking turns. Measured rather than assumed: at `-smp 4` this QEMU (11.1.1) creates 5 threads under
# `thread=single` and 8 under `thread=multi`.
#
# That matters because the failures worth finding on more than one core are the ones that need two
# cores inside the same instant: a wake that is never delivered, a revocation that reaches one core's
# state and not another's, a corpse that does not reach the queue the other core is reading. Under
# round-robin those windows are narrow or absent, which is why `design/fatal-risks/README.md` risk 2 keeps
# having to say "found on a bench, invisible in QEMU".
#
# # BUGS
#
# - **This is a hunting instrument, not a gate, and a red run under it needs confirming before it is
#   believed.** x86 has a stronger memory model (TSO) than an aarch64 host provides, so a faithful
#   MTTCG has to insert barriers the guest's own instructions do not carry. QEMU 11.1.1 accepts
#   `thread=multi` for this pair and issues no warning, and nobody here has audited whether its
#   barrier placement is complete. So a failure seen only under this knob might be the guest's bug or
#   might be the emulator's, and the honest next step for one is a bench, not a patch.
# - **Round-robin is not the safe reading either.** The `REAP` defect milestone 321 (a corpse that
#   did not park, on four real cores) fixed, a corpse still standing on its stack being refused,
#   reproduced under both settings and more often under round-robin, where a timeslice can land
#   inside a window a few hundred instructions long on silicon. Neither setting is the faithful
#   one, which is why the default is unchanged and this is a knob rather than a new posture.
TCG_THREAD=""
if [ -n "$NIFE_TCG_THREAD" ]; then
    TCG_THREAD="-accel tcg,thread=$NIFE_TCG_THREAD"
fi

# `q35` rather than the older `pc` because it is what the physical target looks like: a PCIe root
# complex with an ECAM window, an AHCI controller, and the legacy 16550 COM1 at port 0x3f8 that
# milestone 87's Dell C4PDJ module also presents. One machine model, both paths.
#
# `-cpu max` gives us every feature QEMU models. It is the permissive choice, and the same caveat
# applies as on the RISC-V side (notes/cpu-models.md): a kernel that has only ever run here has
# never been told no. NIFE_CPU narrows it; `qemu64` is the conservative baseline.
CPU="${NIFE_CPU:-max}"

# The userspace archive rides in as an initial ramdisk (milestone 161), the same knob both other
# runners take. The mechanism underneath differs and is worth one line: there is no device tree
# here, so QEMU's PVH loader puts the file in RAM and describes it in the `hvm_start_info` module
# list, which arch::x86_64::machine::initial_ramdisk reads. Unset, the kernel finds no module and
# says so.
#
# A SET NIFE_INITIAL_RAMDISK naming a missing file is an error rather than a silent no-op, matching
# the check both other runners make about NIFE_DISK: `cargo xtask test` exports this variable
# unconditionally, so a typo or a stale path would otherwise boot a kernel with no userspace and
# report thirty test failures that name the tests rather than the cause.
INITIAL_RAMDISK=""
if [ -n "$NIFE_INITIAL_RAMDISK" ]; then
    if [ ! -f "$NIFE_INITIAL_RAMDISK" ]; then
        echo "qemu-runner-x86_64: NIFE_INITIAL_RAMDISK=$NIFE_INITIAL_RAMDISK does not exist (run cargo xtask initial-ramdisk-x86)" >&2
        exit 1
    fi
    INITIAL_RAMDISK="-initrd $NIFE_INITIAL_RAMDISK"
fi

# isa-debug-exit is x86's answer to the semihosting exit the other two use: a write to this port
# terminates QEMU with status (value << 1) | 1, so the guest can report pass/fail to the harness.
# Always present, because a test build that cannot exit leaves an emulator running forever.
# See kernel/src/arch/x86_64/semihosting.rs.
DEBUG_EXIT="-device isa-debug-exit,iobase=0xf4,iosize=0x04"

# VT-d (milestone 161, roadmap item 6), unconditional, the same posture the other two runners take
# for their own IOMMU (`iommu=smmuv3` on aarch64, `-device riscv-iommu-pci` on riscv): idle when
# nothing attaches. `intel-iommu` is a q35-only device (it attaches to the host bridge, not to a PCI
# slot), which is one more reason this runner and the aarch64/riscv ones cannot share a code path.
# What proves the driver against real (emulated) hardware rather than only its own host-side unit
# tests is `arch::x86_64::machine::read_acpi` finding a DMAR with one DRHD, so
# `arch::x86_64::discovery::bring_up_iommus` brings VT-d up and prints so; the NVMe attachment below
# (§86's data point) is the first PCI device this runner confines behind it.
#
# **NIFE_INTREMAP passes `intremap=` through to the unit** (milestone 317; the name is PROVISIONAL,
# a lane's to propose and an architect's to ratify). `on` and `off` are the only values; unset
# leaves
# QEMU's own default, which is what every boot before this milestone got.
#
# **The useful value is `off`, and that is the opposite of what this milestone set out to build.**
# DECISIONS §86 (whether an NVMe driver can leave the kernel) and notes/confinement-claims.md both
# state that this runner "attaches `-device intel-iommu` with no `intremap=on`, so interrupt
# remapping is off in every x86_64 boot this tree runs." The first half is a true reading of this
# file. **The second half is false**, and it was reached by reading this file rather than by booting
# it. Measured by printing `ECAP` from inside the guest, QEMU 11.1.1, `q35` under TCG on patagonia:
#
#     -device intel-iommu                 ECAP = 0xf00f4a   IR set    (QEMU's default)
#     -device intel-iommu,intremap=on     ECAP = 0xf00f4a   IR set    (a no-op here)
#     -device intel-iommu,intremap=off    ECAP = 0xf42      IR clear
#
# QEMU's `intremap` property is tri-state and defaults to `auto`, which resolves to ON when there
# is no in-kernel irqchip to conflict with. patagonia has no KVM, so it has always resolved ON.
# **Interrupt remapping has been offered to this kernel in every x86_64 boot it has ever run**, and
# nothing read the bit, so nobody noticed. `intremap=off` is therefore the only way to reach a
# machine WITHOUT the capability, which is the comparison this knob exists to make possible.
#
# The default does not move: unset is QEMU's default is what the tree already ran, so this adds a
# knob and changes no existing boot. See
# design/roadmap/0317-interrupt-remapping-flags/x86-64.md.
#
# **`kernel-irqchip=split` is NOT required here, and that is measured rather than inherited.** The
# advice that pairs the two is real, and it is a KVM constraint: QEMU refuses `intremap=on` with an
# in-kernel irqchip. patagonia has no KVM, so `q35` under TCG (and under HVF, which does not apply
# to this runner) emulates the whole irqchip in the QEMU process and the check never fires. Against
# QEMU 11.1.1, `-machine q35 -device intel-iommu,intremap=on` starts with no diagnostic, and so
# does the same line with `kernel-irqchip=on`. A Linux host running these tests under KVM WOULD
# need the split irqchip; this runner does not add it because adding a flag that is a no-op here
# would be asserting a machine fact nobody on this machine can check.
#
# A value that is neither `on` nor `off` is an error rather than a silent pass-through, the same
# posture NIFE_INITIAL_RAMDISK and NIFE_DISK take about a missing file: QEMU would reject it anyway,
# and failing here names the variable instead of burying it in a device-option diagnostic.
if [ -n "$NIFE_INTREMAP" ] && [ "$NIFE_INTREMAP" != "on" ] && [ "$NIFE_INTREMAP" != "off" ]; then
    echo "qemu-runner-x86_64: NIFE_INTREMAP=$NIFE_INTREMAP is neither 'on' nor 'off'" >&2
    exit 1
fi
IOMMU="-device intel-iommu${NIFE_INTREMAP:+,intremap=$NIFE_INTREMAP}"

# **NIFE_IOMMU=amd puts AMD-Vi on the machine instead of VT-d** (lane `amd-vi`; the name and its
# values are PROVISIONAL). Unset, or `intel`, is the machine every boot before this got. With
# `amd`, QEMU builds an IVRS rather than a DMAR, so the kernel finds no VT-d unit and brings up
# kernel/src/arch/x86_64/amd_vi.rs instead, which is the only way this tree can run that driver
# before an AMD board is on the bench.
#
# **`dma-remap=on` is not optional, and nothing inside the guest can tell it is missing.** Without
# it QEMU's model routes every device's DMA around the unit whatever the device table says
# (`amdvi_switch_address_space` in hw/i386/amd_iommu.c, QEMU 11.1.1): the driver would come up,
# every test that reads the unit's own registers would pass, and no DMA would be translated at
# all. The DMA-escape tests are what notice, which is why they run on this machine too.
#
# `intremap` is passed through exactly as for VT-d. The kernel never enables AMD-Vi's interrupt
# remapping either (every device table entry leaves IV clear), so either value boots the same.
#
# **The AMD-Vi machine also carries an empty conventional `pci-bridge`** (provisional milestone 767
# (AMD-Vi hardening before the first AMD boot)). A PCIe-to-PCI bridge is where AMD-Vi aliasing
# comes from: devices behind it reach the IOMMU under the bridge's requester id, not their own.
# QEMU's IVRS describes this with an "alias start of range" entry (type 43h) covering every id on
# the bridge's secondary bus, aliased to the bridge itself (`insert_ivhd` in
# hw/i386/acpi-build.c, QEMU 11.1.1). That is the only alias entry this tree can get before an
# AMD board, and `amd_vi::tests` drive the alias rules through it. Nothing sits behind the bridge,
# so no DMA ever arrives under the alias; the tests read the device table, not a device.
#
# `none` builds the machine with neither, which is every PC whose firmware publishes no DMAR and no
# IVRS: the kernel must keep booting there and say loudly that nothing is confined. Nothing in the
# suite runs on it; it is for looking at that boot.
case "$NIFE_IOMMU" in
    ""|intel) ;;
    amd) IOMMU="-device amd-iommu,dma-remap=on${NIFE_INTREMAP:+,intremap=$NIFE_INTREMAP} -device pci-bridge,id=amdvi-alias-bridge,chassis_nr=1" ;;
    none) IOMMU="" ;;
    *)
        echo "qemu-runner-x86_64: NIFE_IOMMU=$NIFE_IOMMU is not 'intel', 'amd' or 'none'" >&2
        exit 1
        ;;
esac

# An NVMe controller when NIFE_NVME names an image (milestone 53's storage half; decisions §86's
# x86_64/VT-d data point), the twin of the aarch64 and riscv64 runners' blocks. No
# `iommu_platform` flag, same reason as the other two: that knob is virtio's opt-in, and a real
# PCI device model's DMA always goes through the PCI address space, so with `-device intel-iommu`
# on the machine the controller sits behind VT-d with no flag to forget, and the kernel must
# confine its requester id before the controller can fetch a single command
# (kernel/src/non_volatile_memory_express.rs). serial= is mandatory (QEMU refuses the device without one). A set
# NIFE_NVME naming a missing file is an error, the same NIFE_INITIAL_RAMDISK lesson above: a
# silently absent controller would read as a machine fact when it is a build-order mistake.
# The PCIe transport's disk (milestone 215) and the RedoxFS disk (milestone 303). `q35` has no
# virtio-mmio bus at all (`arch::x86_64::mmu::VIRTIO_SLOTS` is 0), so unlike the other two runners
# every block device here is a PCI function, and `NIFE_DISK` names the fixture set the same way it
# does there: the siblings `-pci.img` (mkdisk writes it) and `-redoxfs.img` (mkredoxfs does). A
# separate file rather than the main image because both are attached writable elsewhere and QEMU's
# image locking refuses one file to two writers.
#
# **The ORDER of these two -device arguments is a contract**, not a formatting choice.
# `virtio::find_block_device_n` orders block devices mmio-first-then-PCI-in-bdf-order, QEMU assigns
# `pcie.0` slots in command-line order, and the wirings ask for ordinals: the nifefs image is
# ordinal 0 (the PCIe transport tests' disk) and the RedoxFS image is ordinal 1 (what
# `fs_service::wire_servers` mounts). Swapping these two lines hands the FS server the nifefs image,
# which is not a RedoxFS filesystem, and the failure arrives as a mount error rather than as
# anything naming this file. The other two runners' mmio blocks carry the same coupling.
#
# disable-legacy=on makes the function MODERN (device id 0x1042); the transitional 0x1001 device's
# register layout is one this tree deliberately does not drive.
#
# iommu_platform=on is what puts the disk BEHIND VT-d, and on this machine it is the difference
# between a confinement claim and a decoration: QEMU's virtio-pci device uses the *system* address
# space unless the flag is set, so without it the device would bypass `-device intel-iommu`
# silently and every DMA would land wherever the driver asked. With it the device emits IOVAs the
# unit translates through the domain `virtio::register` builds, and a stray address faults at the
# IOMMU instead of reaching RAM. The same flag, for the same reason, is on the riscv64 runner's PCI
# disk. NVMe below needs no such flag: a real PCI device model always goes through the PCI address
# space.
#
# A missing sibling is a stale build, so it fails loud, the same rule the main image gets.
DISK=""
if [ -n "$NIFE_DISK" ]; then
    if [ ! -f "$NIFE_DISK" ]; then
        echo "qemu-runner-x86_64: NIFE_DISK=$NIFE_DISK does not exist (run mkdisk first)" >&2
        exit 1
    fi
    PCI_DISK="${NIFE_DISK%.img}-pci.img"
    if [ ! -f "$PCI_DISK" ]; then
        echo "qemu-runner-x86_64: $PCI_DISK does not exist (run mkdisk first; it writes both images)" >&2
        exit 1
    fi
    DISK="-drive file=$PCI_DISK,if=none,format=raw,id=hd1 -device virtio-blk-pci,drive=hd1,disable-legacy=on,iommu_platform=on"
    # The RedoxFS fixture as the SECOND function, when a leg built one (milestone 303). Absent is a
    # fact about the run rather than an error, unlike the two images above: `cargo xtask bench` and
    # `boot-check` set NIFE_DISK without ever calling mkredoxfs, and the FS tests' own
    # "no RedoxFS disk attached" arm is the honest answer for those boots.
    REDOXFS_DISK="${NIFE_DISK%.img}-redoxfs.img"
    if [ -f "$REDOXFS_DISK" ]; then
        DISK="$DISK -drive file=$REDOXFS_DISK,if=none,format=raw,id=hd2 -device virtio-blk-pci,drive=hd2,disable-legacy=on,iommu_platform=on"
    fi
    # The crash disk (milestone 37 (prove RedoxFS's crash consistency)), the GPT disk and the
    # blank disk (milestone 57 (partitioning and formatting a real drive and extended attributes)),
    # the THIRD through FIFTH functions, when a leg built them. Soft, like the RedoxFS sibling above: `cargo
    # xtask bench` and `boot-check` set NIFE_DISK without building these, and a fixture's absence
    # is the test's own skip line rather than a broken boot. The order extends the contract the
    # two functions above ride on: one function per fixture, in command-line order, so the
    # ordinals stay nifefs=0, redoxfs=1, crash=2, gpt=3, blank=4. Those are the numbers
    # `fs_service::crash` (CRASH_DISK_INDEX) and `disk_service` (GPT_DISK, BLANK_DISK) name, the
    # same ordinals the mmio slots carry on the other two runners.
    CRASH_DISK="${NIFE_DISK%.img}-redoxfs-crash.img"
    if [ -f "$CRASH_DISK" ]; then
        DISK="$DISK -drive file=$CRASH_DISK,if=none,format=raw,id=hd3 -device virtio-blk-pci,drive=hd3,disable-legacy=on,iommu_platform=on"
    fi
    GPT_DISK_IMG="${NIFE_DISK%.img}-gpt.img"
    if [ -f "$GPT_DISK_IMG" ]; then
        DISK="$DISK -drive file=$GPT_DISK_IMG,if=none,format=raw,id=hd4 -device virtio-blk-pci,drive=hd4,disable-legacy=on,iommu_platform=on"
    fi
    BLANK_DISK_IMG="${NIFE_DISK%.img}-blank.img"
    if [ -f "$BLANK_DISK_IMG" ]; then
        DISK="$DISK -drive file=$BLANK_DISK_IMG,if=none,format=raw,id=hd5 -device virtio-blk-pci,drive=hd5,disable-legacy=on,iommu_platform=on"
    fi
fi

# **NIFE_PCIE_ROOT_PORT puts the NVMe controller behind a PCIe root port** instead of directly on
# the root complex (milestone 320 (every PCI bus the machine has, not just bus zero); the name is
# PROVISIONAL, a lane's to propose and an
# architect's to ratify). Unset, nothing changes and every existing boot gets the flat `q35` it
# always had.
#
# It exists because `q35` has no bridge in its default configuration and xenon does. The kernel
# mapped one megabyte of configuration space and enumerated bus 0 for a year, which is exactly
# right on a flat machine and finds nothing at all on a machine whose M.2 slot is behind a root
# port. Without this knob the bridge walk milestone 320 built has no topology to walk under QEMU
# and its only witness would be host tests over a fixture.
#
# **What it does NOT give you is a working disk, and that is a finding rather than a limitation of
# the knob.** A PCI-to-PCI bridge forwards memory transactions only inside the window its own
# memory base/limit registers describe, and those are written by firmware during ITS enumeration.
# This machine boots PVH with no firmware at all, so the root port comes up with a zero window and
# forwards nothing; the controller behind it enumerates, answers configuration reads, and its BAR
# does not decode. On xenon real firmware programmed both the bus numbers and the windows, and the
# kernel adopts what it finds (milestone 256), so the same code reaches a real disk there.
# Programming a bridge's memory window is its own piece of work; see
# design/roadmap/proposals/a-bridge-window-the-kernel-programs-itself.md.
#
# So the claim this knob supports is the enumeration one: the kernel finds a controller on a bus it
# could not previously see. `user::pci_topology_tests` is the witness.
if [ -n "$NIFE_PCIE_ROOT_PORT" ] && [ "$NIFE_PCIE_ROOT_PORT" != "1" ]; then
    echo "qemu-runner-x86_64: NIFE_PCIE_ROOT_PORT=$NIFE_PCIE_ROOT_PORT is not '1'" >&2
    exit 1
fi

NVME=""
if [ -n "$NIFE_NVME" ]; then
    if [ ! -f "$NIFE_NVME" ]; then
        echo "qemu-runner-x86_64: NIFE_NVME=$NIFE_NVME does not exist (xtask's mknvmedisk writes it)" >&2
        exit 1
    fi
    NVME="-drive file=$NIFE_NVME,if=none,format=raw,id=nvme0 -device nvme,serial=nife-nvme,drive=nvme0"
    if [ -n "$NIFE_PCIE_ROOT_PORT" ]; then
        NVME="-device pcie-root-port,id=nife-rp0,chassis=1,slot=0 $NVME,bus=nife-rp0"
    fi
fi

# **The network card a PC actually has** (milestone 494 (a driver for the network card a PC actually
# has)), when `NIFE_NET` is set: QEMU's `e1000e`, an Intel 82574L, the family xenon's I219 belongs
# to. Until this the x86_64 leg attached no NIC at all, because the only driver this tree had was
# virtio-net and the kernel finds that on virtio-mmio, which `q35` does not have. The slirp network
# is the other two runners' in every detail that a gate reads: DHCP on 10.0.2.0/24, the TCP echo
# peer at 10.0.2.9:7777, the package peer at 10.0.2.9:8080, the TLS peer at 10.0.2.9:8443, the name
# server at 10.0.2.9:53 and the TFTP root. See the aarch64 runner for why each exists. A real PCI
# device model, so its DMA goes through `-device intel-iommu` with no `iommu_platform` knob (the
# paragraph above `$DISK` explains that knob).
# `mac=` is the address `e1000e_tests` asserts reached `net_stack` through the kernel.
NET=""
if [ -n "$NIFE_NET" ]; then
    PACKAGE_PEER="$(cd "$(dirname "$0")" && pwd)/package-http-peer"
    # The TLS peer at 10.0.2.9:8443 (milestone 501 (a TLS client that speaks to one pinned peer)),
    # the aarch64 runner's twin; that runner says what it serves.
    TLS_PEER="$(cd "$(dirname "$0")" && pwd)/tls-peer"
    # The name server (milestone 384 (in a capability system the resolver is a grant)), on the other
    # two runners' terms: a fresh `helpers/name-server-peer` per TCP connection to 10.0.2.9:53.
    NAME_SERVER_PEER="$(cd "$(dirname "$0")" && pwd)/name-server-peer"
    GUESTFWD="guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat,guestfwd=tcp:10.0.2.9:8080-cmd:$PACKAGE_PEER,guestfwd=tcp:10.0.2.9:8443-cmd:$TLS_PEER,guestfwd=tcp:10.0.2.9:53-cmd:$NAME_SERVER_PEER"
    TFTPDIR="$(dirname "$0")/../target/tftp"
    # **The boot's tag** (milestone 871 (a sixth outsider pass attacks the confinement claim)):
    # a fresh value per emulator start, exported so the per-connection guestfwd peers (which slirp
    # spawns through a shell, with no arguments and no stable parent to key on) can tell one boot
    # from the next. helpers/name-server-peer's rebinding answer is the user: a boot's first query
    # for rebind.basalt.test must say public where every later one says the private peer.
    NIFE_BOOT_TAG="$$-$(date +%s)"
    export NIFE_BOOT_TAG

    mkdir -p "$TFTPDIR"
    printf 'nife-tftp!' > "$TFTPDIR/nife"
    NET="-netdev user,id=net0,$GUESTFWD,tftp=$TFTPDIR -device e1000e,netdev=net0,mac=52:54:00:e1:00:0e,romfile="
fi

# A `virtio-rng-pci` when NIFE_RNG is set, the PCIe transport of milestone 56 (secrets,
# credentials, and the entropy to make them safe)'s entropy half (milestone 420 (x86_64 has one
# fixture of five and no NIC, GPU, keyboard or RNG) attached it here). NIFE_RNG=1 is a test-leg
# device exported for every leg's
# runner, so this changes no bench or boot-check boot. `iommu_platform=on` is load-bearing for
# the same reason it is on the disks above, and the entropy tests hold the line: the pcie test
# ASSERTS the device is confined behind the IOMMU rather than skipping past it, so a line that
# lost the flag fails loudly instead of quietly shipping an unconfined entropy source. There is
# no image file to fail loud on; the manufactured-fact hazard (NIFE_RNG set but no device
# enumerated) is caught by the kernel test, which asserts a device rather than skipping when the
# bus names one. The mmio RNG the other two runners also attach has no analogue here: `q35` has
# no virtio-mmio bus at all, so that arm's skip line is a machine fact on this port, not a
# missing device.
RNG=""
if [ -n "$NIFE_RNG" ]; then
    RNG="-device virtio-rng-pci,disable-legacy=on,iommu_platform=on"
fi

# `-no-reboot` turns a triple fault into an exit instead of a silent reset loop, which is the
# difference between seeing that early boot died and watching a blank terminal. Every failure in
# this port's bring-up so far has been a triple fault; add `-d int,cpu_reset` to see the state.
#
# `NIFE_ALLOW_REBOOT=1` drops it, for the one run that wants a reset to reset: milestone 249 (the boot lottery is sampled by a person walking to the board)'s
# `script/soak-test --reboot --arch x86_64`, whose proof is a second boot. That run bounds itself
# and fails on a panic line, so a triple-fault loop costs it a timeout rather than a blank terminal.
# A virtio-gpu when NIFE_GPU is set (milestone 29 (a display terminal)), the twin of the other two runners' block: modern
# (disable-legacy=on) and behind the IOMMU (iommu_platform=on), because a GPU's backing addresses
# ride in a device-level command payload that the transport's shadow-ring validator never sees, so
# the IOMMU is the only thing that bounds them.
GPU=""
if [ -n "$NIFE_GPU" ]; then
    GPU="-device virtio-gpu-pci,disable-legacy=on,iommu_platform=on"
    # **`-vga none` is not optional on q35.** The machine carries a standard VGA by default, and
    # `screendump` with no device argument writes console 0, which is whichever display QEMU
    # created first. Measured 2026-10-17 (UTC), QEMU 11.1.2: with both, the referee's 729 dumps were
    # all the VGA's 720x400 text screen and never the virtio-gpu's scanout. The aarch64 and riscv64
    # `virt` machines have no default display, so their runners need no such line.
    GPU="-vga none $GPU"
fi

# A virtio keyboard when NIFE_KEYBOARD is set (milestone 29's input), on the same terms. The keys
# come from the host over the monitor below (`sendkey`), because nothing in the guest can press one.
KBD=""
if [ -n "$NIFE_KEYBOARD" ]; then
    KBD="-device virtio-keyboard-pci,disable-legacy=on,iommu_platform=on"
fi

# A QEMU monitor on a unix socket when NIFE_GPU_MON names one (milestone 29): `screendump` over it
# writes a PPM of the scanout even with -display none, and `sendkey` is the keystroke the keyboard
# test waits for. cargo xtask's scanout referee owns that connection. The path must stay under
# 104 bytes, which is why xtask puts it in /tmp.
MON=""
if [ -n "$NIFE_GPU_MON" ]; then
    MON="-monitor unix:$NIFE_GPU_MON,server,nowait"
fi

NO_REBOOT="-no-reboot"
if [ -n "$NIFE_ALLOW_REBOOT" ]; then
    NO_REBOOT=""
fi
#
# NOT `exec`, and that is the one thing in this file that is not like the other two runners. See the
# status translation below.
#
# **Because it is not `exec`, this script has to forward signals itself**, and until 2026-09-21 it
# did not. `helpers/qemu-bounded.sh` bounds a run by sending SIGTERM to *the child it started*,
# which on aarch64 and riscv64 is QEMU (both those runners `exec`) and here was this shell. The
# shell died, QEMU was reparented to pid 1, and the bound did nothing: the `calib` lane's
# calibration sweep orphaned an emulator on every single boot before anyone looked, and the
# wrapper's own BUGS section ("a SIGKILL to the killer defeats all of it") named a different cause
# than the one operating. So QEMU runs in the background, this shell waits for it, and a TERM or
# HUP is passed along before this shell exits.
#
# `wait` returns >128 when it is interrupted by a trapped signal rather than by the child
# finishing, so the status has to be read a second time after the child is actually reaped.
#
# The `exec 3<&0` / `<&3 3<&-` dance is not decoration either, and it is the same one
# `helpers/qemu-bounded.sh` documents at length: POSIX gives a backgrounded command's stdin
# /dev/null *before* its own redirections, so under dash (every CI runner's /bin/sh) a plain `<&0`
# duplicates /dev/null onto itself and nothing piped in ever reaches the serial port. The
# descriptor is saved before the job is backgrounded, and closed in the child so QEMU inherits no
# stray fd.
set +e
exec 3<&0
qemu-system-x86_64 \
    -machine q35 \
    $TCG_THREAD \
    -cpu "$CPU" \
    -smp "$SMP" \
    -m 256M \
    -display none \
    -serial stdio \
    $NO_REBOOT \
    $DEBUG_EXIT \
    $IOMMU \
    $DISK \
    $NVME \
    $NET \
    $RNG \
    $GPU \
    $KBD \
    $MON \
    -kernel "$ELF" \
    $INITIAL_RAMDISK \
    "$@" <&3 3<&- &
QEMU=$!
exec 3<&-
trap 'kill -TERM "$QEMU" 2>/dev/null' TERM HUP
wait "$QEMU"
STATUS=$?
# Interrupted by the trap rather than by QEMU exiting: reap it for real, then take that status.
if [ "$STATUS" -gt 128 ]; then
    wait "$QEMU" 2>/dev/null
    STATUS=$?
fi
trap - TERM HUP

# **isa-debug-exit cannot produce exit status 0.** It terminates QEMU with `(value << 1) | 1`, so
# every status it can report is odd and "the suite passed" has to be some other agreed number. The
# guest writes 1, which lands here as 3; this turns that back into the 0 the harness and every other
# architecture's runner mean by success. The matching half is EXIT_SUCCESS in
# kernel/src/arch/x86_64/semihosting.rs, and the two files name the same number on purpose: getting
# this backwards produces a suite that passes when it fails.
if [ "$STATUS" -eq 3 ]; then
    exit 0
fi
exit "$STATUS"
