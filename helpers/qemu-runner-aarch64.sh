#!/bin/sh
#
# The QEMU runner. Cargo invokes this for `cargo run` and `cargo test`, appending the
# path to the ELF it just built.
#
# Why this exists rather than a plain `qemu-system-aarch64 ... -kernel` line in
# .cargo/config.toml: QEMU only follows the **Linux arm64 boot protocol** (and
# therefore only hands us a device tree pointer in x0) for a flat `Image`. Given an
# ELF, it takes a bare-metal path instead and populates no registers at all.
#
# So we strip the ELF down to a flat binary. The arm64 Image header lives at byte 0
# of it (kernel/src/arch/aarch64/image_header.s), which is what makes QEMU recognize
# the blob as a kernel.
#
# Tests boot through exactly the same path as `cargo xtask run` does, deliberately.
# A test harness that exercises a different boot path than the real thing is testing
# a fiction.
#
# See notes/boot-protocol.md.

set -e

# CPU and accelerator.
#
# By default we run under TCG (QEMU translates every aarch64 instruction), with an emulated
# cortex-a72. That is deterministic and runs identically on any host, which is what the test
# harness wants.
#
# Set NIFE_ACCEL=hvf to run under Apple's Hypervisor.framework instead: HVF puts the kernel on
# the real Apple Silicon core at guest EL1, using the hardware virtualization the chip already
# has. The coincidence that makes this a flag and not a port is that the host and the guest are the
# same ISA (aarch64). Two consequences:
#
#   - HVF runs the PHYSICAL core, so `-cpu host` is mandatory; you cannot ask for an emulated a72.
#   - gic-version is STATED rather than left to QEMU, and under HVF it is **3**: QEMU 11.1.1
#     refuses `gic-version=2` with HVF outright ("HVF does not support GICv2 emulation", milestone
#     222). The kernel drives both since milestone 227 and reads which one it has from the device
#     tree, so the version is the runner's choice rather than a constraint on the kernel. See
#     NIFE_GIC below for the TCG default and why it stays 2.
#
# NIFE_CPU overrides the TCG model (milestone 59, the parity twin of the riscv runner's flag).
# Under HVF there is nothing to override: the guest runs on the physical Apple core, so `-cpu host`
# is the only answer and asking for anything else is a mistake worth failing on rather than
# silently ignoring.
# NIFE_EL2=1 builds the machine with an EL2 the kernel is started at, which is the closest
# rehearsal patagonia can give of a real board (milestone 127, the seL4 machine). QEMU's `virt`
# starts a kernel at EL1 by default; `virtualization=on` gives the machine a hypervisor level and
# enters the payload there, which is what U-Boot does on the Jetson TX1 with TF-A's BL31 below it.
# `boot.s` then reads `CurrentEL` and drops itself, and the whole suite runs unchanged.
#
# Two things change with it and both are the machine talking, not a workaround. `/psci` states
# `smc` rather than `hvc`, because an `hvc` from EL1 would arrive at the EL2 we just left; and
# PSCI starts every secondary at EL2 too, whichever level called `CPU_ON`, so `secondary_boot`
# takes the same drop.
#
# TCG only. HVF has no nested virtualization to offer, so asking for both is a mistake worth
# failing on rather than silently ignoring, the same posture as NIFE_CPU under HVF below.
VIRT_EL2=""
if [ -n "$NIFE_EL2" ]; then
    if [ "$NIFE_ACCEL" = "hvf" ]; then
        echo "qemu-runner-aarch64: NIFE_EL2 cannot apply under HVF (no nested virtualization; drop NIFE_ACCEL=hvf)" >&2
        exit 1
    fi
    VIRT_EL2=",virtualization=on"
fi

# **NIFE_GIC picks the interrupt controller: 2 (the TCG default) or 3** (milestone 317 added the
# flag to reproduce a failure; milestone 227 made the kernel drive what it asks for). The name is
# PROVISIONAL, a lane's to propose and an architect's to ratify.
#
# The kernel no longer cares which: `machine_discovery::gic` reads the device tree's `compatible`
# (`arm,cortex-a15-gic` for 2, `arm,gic-v3` for 3), and `arch::irq::init` confirms it against the
# hardware before driving it, panicking with both sides named if they disagree. Before 227 a
# GICv3 booted, printed `interrupts ON` and took no interrupts at all (`notes/interrupts.md`).
#
# **Why TCG stays at 2.** Every recorded TCG measurement (the icount tripwire's baselines, the
# fastpath footprint, the benchmark history) was taken on a GICv2, and argon (the Jetson TX1) is a
# GIC-400, so the default keeps the machine those numbers describe and the silicon this kernel
# ships on next. HVF has no such choice, since QEMU refuses it a GICv2; so under HVF the default
# is 3, and asking for 2 there reaches the probe below and its explanation.
#
# The machine differs in one other way worth knowing: `gic-version=3` gives QEMU's `virt` an ITS
# (`its@8080000`) where 2 gives a GICv2m frame, and under HVF QEMU gives a GICv2m frame either way.
# Nothing in the kernel drives either (milestone 317 wants the ITS for interrupt remapping).
if [ "$NIFE_ACCEL" = "hvf" ]; then
    GIC="${NIFE_GIC:-3}"
else
    GIC="${NIFE_GIC:-2}"
fi
case "$GIC" in
    2|3) ;;
    *)
        echo "qemu-runner-aarch64: NIFE_GIC=$GIC is not a GIC version this kernel drives (2 or 3)" >&2
        exit 1
        ;;
esac

if [ "$NIFE_ACCEL" = "hvf" ]; then
    # iommu=smmuv3 is on BOTH paths since milestone 81, and this is a correction: it used to be
    # TCG-only, on the recorded belief that "smmuv3 emulation alongside HVF acceleration is the
    # fragile combination". Nobody had run it. The suite on the physical core says otherwise, and
    # the belief cost a real gap while it stood: without an SMMU the display test fails outright
    # ("a virtio-gpu is present but the IOMMU is not active") and the DMA confinement tests, which
    # assert the HARDWARE faults an escaping DMA, would have had no hardware to assert about.
    #
    # It is also the right place for it on principle. The accelerator chooses how CPU instructions
    # execute; the SMMU is in front of the PCIe root complex and translates DEVICE traffic, which
    # QEMU emulates in the host process either way. The two are orthogonal, and the suite proves it.
    MACHINE="virt,accel=hvf,gic-version=$GIC,iommu=smmuv3"
    if [ -n "$NIFE_CPU" ] && [ "$NIFE_CPU" != "host" ]; then
        echo "qemu-runner-aarch64: NIFE_CPU=$NIFE_CPU cannot apply under HVF (the guest runs the physical core; -cpu host is mandatory)" >&2
        exit 1
    fi
    CPU="host"
else
    # iommu=smmuv3 puts an SMMUv3 in front of the PCIe root complex (milestone 16b). The device tree
    # then carries an `smmuv3@...` node (memory::smmu_region finds it) and an identity iommu-map for
    # the bus. A plain boot without a PCI disk still gets the SMMU; it just has nothing to confine.
    # The HVF branch above takes the same flag, since milestone 81.
    MACHINE="virt,gic-version=$GIC,iommu=smmuv3$VIRT_EL2"
    CPU="${NIFE_CPU:-cortex-a72}"
fi

# **The probe** (milestone 222). HVF and a GIC version are not always compatible, and when they
# are not, QEMU refuses to start the machine at all:
#
#     qemu-system-aarch64: HVF does not support GICv2 emulation
#
# That is QEMU 11.1.1's answer to `virt,gic-version=2,accel=hvf`, reproduced with no nife kernel
# involved. It is a constraint of the accelerator, not a defect in anyone's change, and before this
# it reached a contributor as a bare failure of `script/test --hvf` with no way to tell the two
# apart. So this script answers the question directly, on the machine string it would actually use.
#
# NIFE_PROBE=1 starts the machine, does nothing with it, and quits: `-S` leaves the CPU paused so
# nothing executes, and `quit` on the monitor's stdin exits immediately, which matters because a
# nife kernel that reaches `halt()` never exits and neither would QEMU. Machine init still happens,
# which is where the refusal comes from, so the answer is QEMU's own rather than a version test we
# would have to keep current. It costs about fifty milliseconds.
#
# It lives HERE, rather than in a script of its own, so that `$MACHINE` has exactly one definition.
# A separate probe would have to restate the machine string, and the day the runner's changed the
# probe would answer about a machine nobody runs, which is the false-skip this milestone exists to
# avoid producing.
# `probe_machine` starts $MACHINE, does nothing with it, and quits: `-S` leaves the CPU paused so
# nothing executes, and `quit` on the monitor's stdin exits immediately, which matters because a
# nife kernel that reaches `halt()` never exits and neither would QEMU. Machine init still happens,
# which is where a refusal comes from, so the answer is QEMU's own rather than a version test we
# would have to keep current. It costs about fifty milliseconds.
#
# QEMU's words go to STDOUT on failure and the explanation goes to STDERR, and the split is what
# lets one probe serve two readers. `script/ci-build` captures stdout for its one-line skip message and
# discards stderr; a person, and `cargo xtask test --hvf`, get the paragraph. A refusal we
# paraphrased would go stale the first time QEMU reworded it, so the one-line half is never ours.
probe_machine() {
    set +e
    probe_err="$(echo quit | qemu-system-aarch64 -machine "$MACHINE" -cpu "$CPU" -m 32M \
        -display none -monitor stdio -S 2>&1 >/dev/null)"
    probe_status=$?
    set -e
    if [ "$probe_status" -ne 0 ]; then
        printf '%s\n' "$probe_err"
    fi
    return "$probe_status"
}

explain_probe_failure() {
    echo "qemu-runner-aarch64: QEMU refused $MACHINE (its own message is above)." >&2
    echo "qemu-runner-aarch64: THIS IS NOT YOUR CHANGE. This QEMU will not start that machine under" >&2
    echo "  HVF. The kernel drives GICv2 and GICv3 (milestone 227), and HVF on QEMU 11.1.1 accepts only" >&2
    echo "  GICv3, which is this runner's HVF default; if NIFE_GIC=2 is set, unset it. Otherwise the" >&2
    echo "  refusal is QEMU's own, quoted above. script/ci-build skips this leg out loud rather than" >&2
    echo "  failing; notes/hvf-leg.md has the history." >&2
}

# NIFE_PROBE asks the question and answers nothing else: exit 0 if this machine starts, non-zero
# otherwise. `script/ci-build` uses it to decide whether to run the HVF leg or to skip it out loud, and
# `xtask`'s `--hvf` leg uses it to fail before it stands up a scanout referee and two network
# probers, each of which would otherwise report its own failure about a QEMU that never started.
# That cascade is the ambiguity milestone 222 exists to remove, one level up.
if [ -n "$NIFE_PROBE" ]; then
    if probe_machine; then
        exit 0
    fi
    explain_probe_failure
    exit 1
fi

# And the same question asked on the way in, so that a person who typed `script/test --hvf` is told
# what is wrong rather than left to read a bare QEMU line. Before milestone 222 that line was all
# they got, and it does not say whether the breakage is theirs.
if [ "$NIFE_ACCEL" = "hvf" ] && ! probe_machine >&2; then
    explain_probe_failure
    exit 1
fi


ELF="$1"
shift

# llvm-objcopy ships with the `llvm-tools` rustup component, which rust-toolchain.toml
# pins. We resolve it out of the sysroot rather than expecting it on PATH, because
# `rust-objcopy` needs a separate `cargo install cargo-binutils` and we'd rather not
# add a setup step that fails confusingly six months from now.
SYSROOT="$(rustc --print sysroot)"
HOST="$(rustc -vV | awk '/^host:/{print $2}')"
OBJCOPY="$SYSROOT/lib/rustlib/$HOST/bin/llvm-objcopy"

if [ ! -x "$OBJCOPY" ]; then
    echo "qemu-runner-aarch64: cannot find llvm-objcopy at $OBJCOPY" >&2
    echo "qemu-runner-aarch64: is the llvm-tools component installed? (rust-toolchain.toml pins it)" >&2
    exit 1
fi

IMG="$ELF.img"
# --remove-section for .eh_frame/.eh_frame_hdr: milestone (CFI in hand-written asm) stopped the
# link scripts discarding call-frame information, so the ELF `gdb $ELF` reads now carries it. This
# is the one place that has to NOT carry it along: the CFI is real ALLOC content the linker folds
# into a PT_LOAD segment (measured; a non-alloc placement either dropped the bytes or broke
# relocations elsewhere, see the comment in kernel/link-aarch64.ld), and `-O binary` on an
# unfiltered ELF would grow this flat Image by the CFI's own size for no boot-time benefit. See
# notes/cfi-unwind.md.
"$OBJCOPY" -O binary --remove-section=.eh_frame --remove-section=.eh_frame_hdr "$ELF" "$IMG"

# The userspace program rides in as an initrd, exactly the way Linux gets its initramfs: QEMU
# loads the file into RAM and writes the address into /chosen/linux,initrd-start in the device
# tree it generates. The kernel finds it there. Nothing about the binary is compiled into the
# kernel. See notes/elf.md and kernel/src/memory.rs.
INITRD=""
if [ -n "$NIFE_INITRD" ] && [ -f "$NIFE_INITRD" ]; then
    INITRD="-initrd $NIFE_INITRD"
fi

# Attach the nifefs image as a virtio-blk device. `if=none` + `-device virtio-blk-device`
# gives us a virtio-mmio block device on the `virt` machine, which is what the userspace driver
# probes for and reads. Without a disk, the kernel simply finds no block device and says so.
#
# A SET NIFE_DISK naming a missing file is an error, not a silent no-op. The old behaviour
# (quietly booting diskless) had the kernel truthfully reporting "no block device", which reads
# like a machine fact when it is actually a build-order mistake; it very likely produced the
# false "riscv virt has no mmio disk" record in notes/riscv-parity-scope.md.
DISK=""
if [ -n "$NIFE_DISK" ] && [ ! -f "$NIFE_DISK" ]; then
    echo "qemu-runner-aarch64: NIFE_DISK=$NIFE_DISK does not exist (run mkdisk first)" >&2
    exit 1
fi
if [ -n "$NIFE_DISK" ]; then
    # force-legacy=false selects MODERN virtio-mmio (version 2), whose split register interface
    # (separate physical addresses for the descriptor table and the two rings) is the current
    # design and the one worth learning. Without it QEMU gives legacy (version 1), a different
    # and older queue layout.
    #
    # Both transports are attached WRITABLE (milestone 32: the write-capable block path), which
    # is why there are two image files rather than one attached twice: QEMU's image locking
    # refuses to open one file for two devices once either can write. mkdisk writes the sibling
    # alongside the main image with identical contents; missing sibling = stale build, fail loud
    # (the readonly-era silent-degradation lesson, see the NIFE_DISK check above).
    #
    # iommu_platform=on is what puts the PCI disk BEHIND the SMMU (milestone 16b): the device then
    # emits IOVAs the SMMU translates through the domain the kernel built, and offers
    # VIRTIO_F_ACCESS_PLATFORM so the driver knows it. WITHOUT this flag QEMU's virtio device
    # bypasses the SMMU silently, and the confinement test (which asserts the hardware faults an
    # out-of-region DMA) fails loudly rather than passing on a fiction. The mmio disk (hd0) has no
    # IOMMU in front of it on this machine, so it takes no such flag.
    PCI_DISK="${NIFE_DISK%.img}-pci.img"
    if [ ! -f "$PCI_DISK" ]; then
        echo "qemu-runner-aarch64: $PCI_DISK does not exist (run mkdisk first; it writes both images)" >&2
        exit 1
    fi
    # The RedoxFS image (milestone 32 phase 2), the SECOND mmio block device. It is placed on the
    # command line BEFORE the nifefs disk on purpose: QEMU's `virt` assigns virtio-mmio devices
    # to slots in REVERSE command-line order (the last -device gets the lowest-address slot), and
    # the kernel finds block devices by ascending slot. So the nifefs disk must be the LAST mmio
    # device to keep slot 0 (find_block_device -> nifefs, the phase-1 driver tests), which leaves
    # the RedoxFS disk at slot 1 (find_block_device_n(1) -> RedoxFS, the FS server's block server).
    # Getting this backwards silently hands the phase-1 tests the wrong disk; that is exactly the
    # bug this ordering fixes. Soft: present only when the test flow built it (cargo xtask test),
    # absent for a plain interactive boot, which just skips the FS-server test. Created host-side by
    # tools/redoxfs_host; the server only ever opens it.
    REDOXFS_DISK="${NIFE_DISK%.img}-redoxfs.img"
    REDOXFS_MMIO=""
    if [ -f "$REDOXFS_DISK" ]; then
        REDOXFS_MMIO="-drive file=$REDOXFS_DISK,if=none,format=raw,id=hd2 -device virtio-blk-device,drive=hd2"
    fi
    # The crash test's OWN RedoxFS image (milestone 37), the THIRD mmio block device, at slot 2. It
    # goes FIRST on the command line because the slot assignment is reverse of the command-line
    # order, so the three land at nifefs=0, redoxfs=1, crash=2, which is what
    # `find_block_device_n` counts. A dedicated disk is the point: the crash test deliberately leaves
    # a filesystem half-written, and doing that to the shared image would couple every other FS
    # test's result to whether this one ran first (DECISIONS §27's order-coupled gate). Soft, like
    # the others: present only when the test flow built it.
    CRASH_DISK="${NIFE_DISK%.img}-redoxfs-crash.img"
    CRASH_MMIO=""
    if [ -f "$CRASH_DISK" ]; then
        CRASH_MMIO="-drive file=$CRASH_DISK,if=none,format=raw,id=hd3 -device virtio-blk-device,drive=hd3"
    fi
    # The GPT-partitioned image (milestone 57), the FOURTH mmio block device, at slot 3. It goes
    # FIRST on the command line because the slot assignment is the reverse of command-line order, so
    # the four land at nifefs=0, redoxfs=1, crash=2, gpt=3, which is what `find_block_device_n`
    # counts and what `disk_service::GPT_DISK` asks for. This one carries no filesystem at all: the
    # bytes are the `sgdisk` fixture from
    # crates/globally_unique_identifier_partition_table/tests/fixtures, so the guest reads a
    # partition table written by gptfdisk rather than by us. Soft, like the others: present only
    # when the test flow built it (cargo xtask test).
    GPT_DISK_IMG="${NIFE_DISK%.img}-gpt.img"
    GPT_MMIO=""
    if [ -f "$GPT_DISK_IMG" ]; then
        GPT_MMIO="-drive file=$GPT_DISK_IMG,if=none,format=raw,id=hd4 -device virtio-blk-device,drive=hd4"
    fi
    # The blank image (milestone 57's write half), the FIFTH mmio block device, at slot 4. It goes
    # FIRST on the command line for the same reason as the others: slot assignment is the reverse of
    # command-line order, so the five land at nifefs=0, redoxfs=1, crash=2, gpt=3, blank=4, which
    # is what `find_block_device_n` counts and what `disk_service::BLANK_DISK` asks for. This one
    # arrives as 64 MiB of ZEROS: the guest writes the partition table and then the filesystem
    # inside it, and the post-run host check reads both back. Its own disk, regenerated every run,
    # because a test that partitions a disk must not touch an image another test reads. Soft, like
    # the others: present only when the test flow built it.
    BLANK_DISK_IMG="${NIFE_DISK%.img}-blank.img"
    BLANK_MMIO=""
    if [ -f "$BLANK_DISK_IMG" ]; then
        BLANK_MMIO="-drive file=$BLANK_DISK_IMG,if=none,format=raw,id=hd5 -device virtio-blk-device,drive=hd5"
    fi
    DISK="$BLANK_MMIO $GPT_MMIO $CRASH_MMIO $REDOXFS_MMIO -drive file=$NIFE_DISK,if=none,format=raw,id=hd0 -device virtio-blk-device,drive=hd0 -drive file=$PCI_DISK,if=none,format=raw,id=hd1 -device virtio-blk-pci,drive=hd1,disable-legacy=on,iommu_platform=on"
fi

# **Modern virtio-mmio is a property of the BUS, not of the disks, so the switch that selects it
# has to outlive the disk block** (found 2026-09-10, timing the `hw entropy` step).
#
# `virtio-mmio.force-legacy` defaults to TRUE in QEMU, and a legacy slot reports VERSION 1 where
# this kernel requires 2 (`virtio.rs`'s `find_by_device_id`). The `-global` that turns it off used
# to live inside `$DISK`, so **every mmio device on this command line was modern only when a disk
# happened to be attached**. That held for the whole test suite, which always builds disks, and it
# is why nothing caught it: the two other mmio devices here (`virtio-rng-device` under `NIFE_RNG`,
# `virtio-net-device` under `NIFE_NET`) are only ever attached by the same flow.
#
# `NIFE_RNG=1` with no `NIFE_DISK` is what found it: the riscv64 boot tour scanned the mmio bus,
# met a legacy RNG, and the kernel panicked on the version assertion at a point in the boot where
# nothing named virtio at all. Hoisting the global fixes that case and the identical latent one on
# the NIC.
#
# Set only when an mmio device will actually be attached, because QEMU warns about a `-global`
# that matches nothing instantiated, and a warning on every device-free boot is noise nobody would
# read twice.
MMIO_MODERN=""
if [ -n "$NIFE_DISK" ] || [ -n "$NIFE_RNG" ] || [ -n "$NIFE_NET" ]; then
    MMIO_MODERN="-global virtio-mmio.force-legacy=false"
fi

# Attach a virtio-net NIC on QEMU user-mode (slirp) networking when NIFE_NET is set (milestone
# 30). slirp NATs the guest with a built-in DHCP server (10.0.2.0/24, gateway 10.0.2.2) and DNS
# resolver (10.0.2.3), and needs no host setup, so the net tests run with zero privilege. Two NICs
# mirror the two disks: the mmio NIC (net0) has no IOMMU in front of it, the PCI NIC (net1) sits
# behind the SMMU (iommu_platform=on), the same hardware confinement the PCI disk gets. There is no
# image file to fail loud on here; the manufactured-fact hazard (NIFE_NET set but no NIC
# enumerated) is caught by the net test, which asserts a NIC is present rather than skipping.
#
# guestfwd adds a deterministic TCP echo peer at 10.0.2.9:7777 inside slirp: a connection to it is
# piped to a fresh `/bin/cat`, so the TCP round-trip gate (connect, send, receive the echo, close) runs
# with zero host setup and nothing outlives QEMU. Verified against QEMU 11.0.2. Each slirp instance
# is its own network, so both NICs can use the same virtual address without conflict.
GUESTFWD="guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat"
# **The package source** (milestone 198 (a package manager) rung 3a), on the echo peer's terms: 10.0.2.9:8080 is a
# fresh `helpers/package-http-peer` per connection, speaking HTTP/1.0 on the connection and serving
# `target/packages/`, which the archive build fills. A real HTTP exchange with a host process, and
# still nothing that binds a host port or outlives QEMU. The path is absolute because slirp runs the
# command from QEMU's working directory, and it must contain no space or comma, because `$NET` is
# word-split below and a comma ends a `-netdev` option.
PACKAGE_PEER="$(cd "$(dirname "$0")" && pwd)/package-http-peer"
GUESTFWD="$GUESTFWD,guestfwd=tcp:10.0.2.9:8080-cmd:$PACKAGE_PEER"

# **The name server** (milestone 384 (in a capability system the resolver is a grant)), on the
# package source's terms: 10.0.2.9:53 is a fresh `helpers/name-server-peer` per connection, answering
# one DNS query over TCP from a fixed zone that includes the lies a resolver must refuse. TCP because
# guestfwd forwards nothing else, and a UDP name server would have to bind a host port.
NAME_SERVER_PEER="$(cd "$(dirname "$0")" && pwd)/name-server-peer"
GUESTFWD="$GUESTFWD,guestfwd=tcp:10.0.2.9:53-cmd:$NAME_SERVER_PEER"

# **The TLS peer** (milestone 501 (a TLS client that speaks to one pinned peer)), on the package
# source's terms: 10.0.2.9:8443 is a fresh `helpers/tls-peer` per connection, a TLS 1.3 server (Python's
# `ssl`, so OpenSSL) presenting test certificates chosen by the name the client asks for, so one port
# gives the pinned client its peer and both of the refusals it must make.
TLS_PEER="$(cd "$(dirname "$0")" && pwd)/tls-peer"
GUESTFWD="$GUESTFWD,guestfwd=tcp:10.0.2.9:8443-cmd:$TLS_PEER"

# `tftp=` turns on slirp's OWN TFTP server, at the gateway (10.0.2.2:69), and that is what makes the
# gating UDP test deterministic and offline. The UDP test used to query 10.0.2.3:53, which is NOT a
# resolver: libslirp NATs anything sent there to the HOST's nameserver, so that test silently
# depended on the developer's DNS working at that instant and flaked whenever a query was dropped
# (measured ~2.5% per query against a real resolver). TFTP is served inside libslirp, so the request
# and its reply never leave the emulator. This is the UDP twin of the guestfwd echo peer above.
# The fixture's name and contents are fixed and must match components/src/socket_test_client.rs (TFTP_NAME/TFTP_BODY);
# `printf` writes it with no trailing newline so the client can assert the bytes exactly.
TFTPDIR="$(dirname "$0")/../target/tftp"
# **The boot's tag** (milestone 868 (a sixth outsider pass attacks the confinement claim)): a fresh
# value per emulator start, exported so the per-connection guestfwd peers (which slirp spawns
# through a shell, with no arguments and no stable parent to key on) can tell one boot from the
# next. helpers/name-server-peer's rebinding answer is the user: a boot's first query for
# rebind.basalt.test must say public where every later one says the private peer.
export NIFE_BOOT_TAG="$$-$(date +%s)"

mkdir -p "$TFTPDIR"
printf 'nife-tftp!' > "$TFTPDIR/nife"

# hostfwd is guestfwd's mirror and the inbound gate's whole mechanism (milestone 107): QEMU listens
# on a HOST port and forwards connections to the guest's 10.0.2.15:7778, so a host process can
# connect INTO the guest. Everything this project had proved over the network was the guest as a
# client; this is the other direction.
#
# Only on the mmio NIC, and only when NIFE_HOSTFWD_PORT names a port. Both restrictions are
# deliberate. This is the one QEMU flag here that **binds a port on the developer's machine**, so it
# does not belong on a plain `cargo xtask run` or on the benchmark boot, both of which share this
# runner; and the port is chosen by xtask (a free one, asked of the OS) rather than fixed here,
# because two lanes running the suite at once on one machine would otherwise collide and the loser
# would fail to start QEMU at all. The guest address is spelled out rather than defaulted so the
# line says which guest it means.
HOSTFWD=""
if [ -n "$NIFE_HOSTFWD_PORT" ]; then
    HOSTFWD=",hostfwd=tcp:127.0.0.1:$NIFE_HOSTFWD_PORT-10.0.2.15:7778"
fi

NET=""
if [ -n "$NIFE_NET" ]; then
    NET="-netdev user,id=net0,$GUESTFWD,tftp=$TFTPDIR$HOSTFWD -device virtio-net-device,netdev=net0 -netdev user,id=net1,$GUESTFWD,tftp=$TFTPDIR -device virtio-net-pci,netdev=net1,disable-legacy=on,iommu_platform=on"
fi

# **An `e1000e` NIC beside the two virtio ones** (milestone 494 (a driver for the network card a PC
# actually has)): QEMU's 82574L, the family xenon's I219 belongs to, on its own slirp network with
# the same echo peer, package peer and TFTP root, so the same gates run over it. A real PCI device
# model, so its DMA goes through the IOMMU with no `iommu_platform` knob to forget (see the x86_64
# runner's note on that flag). Attached on every `NIFE_NET` boot because the kernel touches it only
# when a test asks `e1000e_service` for it. `mac=` is the address `e1000e_tests` asserts reached
# `net_stack` through the kernel; `romfile=` skips an option ROM nothing here boots.
if [ -n "$NIFE_NET" ]; then
    NET="$NET -netdev user,id=net2,$GUESTFWD,tftp=$TFTPDIR -device e1000e,netdev=net2,mac=52:54:00:e1:00:0e,romfile="
fi

# Attach a virtio-gpu when NIFE_GPU is set (milestone 29, the display ladder's rung one).
#
# PCIe only, and that is not a shortcut: there is no virtio-gpu on this machine's virtio-mmio bus in
# any configuration, so unlike the disk and the NIC there is no mmio twin to attach. The parity that
# matters is aarch64 virt and riscv virt, and both carry virtio-gpu-pci over the §18 transport.
#
# disable-legacy=on makes the function MODERN (device id 0x1050); iommu_platform=on puts it BEHIND
# the SMMU, so the pixel reads its RESOURCE_ATTACH_BACKING asks for are translated through the domain
# the kernel built. That flag is load-bearing here in a way it is not for the disk: a virtio-gpu's
# backing addresses ride in a device-level command payload, not in a descriptor, so the transport's
# shadow-ring validator never sees them and the IOMMU is the only thing that bounds them. Drop the
# flag and the GPU could name any physical address (see notes/framebuffer-contract.md).
#
# There is no image file to fail loud on, as with the NIC. The manufactured-fact hazard (NIFE_GPU
# set but no GPU enumerated) is caught in the kernel test, which ASSERTS a GPU is present rather than
# skipping, and asserts the IOMMU is active while one is.
GPU=""
if [ -n "$NIFE_GPU" ]; then
    GPU="-device virtio-gpu-pci,disable-legacy=on,iommu_platform=on"
fi

# Attach a virtio keyboard when NIFE_KEYBOARD is set (milestone 29's input).
#
# PCIe here is a CHOICE, not a constraint: unlike the GPU, this machine does offer a
# virtio-keyboard-device on the virtio-mmio bus. The keyboard rides PCIe anyway so it lands in the
# same IOMMU domain the GPU does, and iommu_platform=on is what puts it there. A keyboard is the one
# device whose DMA you would least like unconfined: its buffers are where every keystroke lands.
#
# The keys themselves come from the HOST, over the QEMU monitor below (`sendkey`), because nothing in
# the guest can press one. QEMU drops key events until a driver sets DRIVER_OK, so xtask can send
# them from the start of the run with nothing to synchronize.
KBD=""
if [ -n "$NIFE_KEYBOARD" ]; then
    KBD="-device virtio-keyboard-pci,disable-legacy=on,iommu_platform=on"
fi

# Attach a USB keyboard on an xHCI controller when NIFE_USB_KEYBOARD is set (milestone 242 (USB host
# and HID)). No iommu_platform flag, NVMe's reason: that knob is virtio's opt-in, and a real PCI
# device model's DMA always goes through the PCI address space, so the controller sits behind
# this machine's IOMMU with nothing to forget. The keys come from the host over the monitor
# (`sendkey`), which delivers to the most recently activated keyboard; with no virtio keyboard
# attached, that is this one. script/swish-check's USB keyboard boot is the one user.
# NIFE_USB_KEYBOARD_OPTS is appended to the usb-kbd device (`,usb_version=1` makes it full speed),
# NIFE_USB_CONTROLLER_OPTS to the controller (`,msix=off,msi=on` leaves it MSI only, as an Intel PCH is).
USBKBD=""
if [ -n "$NIFE_USB_KEYBOARD" ]; then
    USBKBD="-device qemu-xhci,id=xhci${NIFE_USB_CONTROLLER_OPTS:-} -device usb-kbd,bus=xhci.0${NIFE_USB_KEYBOARD_OPTS:-}"
fi

# Attach two virtio-rng devices when NIFE_RNG is set (milestone 56, the entropy half).
#
# BOTH transports, because the entropy service must be the same binary on either bus (DECISIONS §18)
# and a random source that works on one is not a random source. The mmio one goes on the mmio bus
# alongside the disks; it is a slot the block-device scan skips, since that scan matches on DeviceID
# and an RNG reports 4, so the nifefs=0 / redoxfs=1 / crash=2 ordering above is unaffected.
#
# The PCI one is behind the SMMU (iommu_platform=on) on the same terms as the GPU and the keyboard:
# the buffer this device writes into is where the system's key material comes from, so it is the last
# device you would want writing wherever it liked.
#
# QEMU backs virtio-rng with the host's /dev/urandom by default (`rng-random`, filename
# /dev/urandom), which is what makes these bytes real and not an emulated counter. That is a fact
# about the emulator, recorded rather than assumed: on hardware the source is the board's TRNG and
# notes/entropy.md carries the caveat.
#
# There is no image file to fail loud on, as with the NIC and the GPU. The manufactured-fact hazard
# (NIFE_RNG set but no device enumerated) is caught in the kernel test, which ASSERTS a device is
# present on each bus rather than skipping.
RNG=""
if [ -n "$NIFE_RNG" ]; then
    RNG="-device virtio-rng-device -device virtio-rng-pci,disable-legacy=on,iommu_platform=on"
fi

# Attach an NVMe controller when NIFE_NVME names an image (milestone 53's storage half): the
# first NON-virtio DMA device this project drives, which is the whole point of it. No
# iommu_platform flag, because that knob is virtio's opt-in; a real PCI device model's DMA always
# goes through the PCI address space, so with iommu=smmuv3 on the machine the controller sits
# behind the SMMU with no flag to forget, and the kernel must confine its requester id before the
# controller can fetch a single command (kernel/src/non_volatile_memory_express.rs). serial= is mandatory (QEMU refuses
# the device without one); the value is arbitrary identity, not configuration.
#
# A set NIFE_NVME naming a missing file is an error, the NIFE_DISK lesson above: a silently
# absent controller would read as a machine fact when it is a build-order mistake. The kernel test
# ASSERTS the controller is present rather than skipping, like the GPU's.
NVME=""
if [ -n "$NIFE_NVME" ]; then
    if [ ! -f "$NIFE_NVME" ]; then
        echo "qemu-runner-aarch64: NIFE_NVME=$NIFE_NVME does not exist (xtask's mknvmedisk writes it)" >&2
        exit 1
    fi
    NVME="-drive file=$NIFE_NVME,if=none,format=raw,id=nvme0 -device nvme,serial=nife-nvme,drive=nvme0"
fi

# **A `ramfb` when NIFE_SCREEN is set**, milestone 243 (a machine with no serial port). The one
# thing this `virt` board can present that looks like a screen the firmware left running: the guest
# allocates the pixels and tells QEMU where they are over `fw_cfg`, and QEMU scans them out.
# `kernel/src/screen.rs` is the guest half.
#
# Off by default, and a test-leg/gate device only, for the two reasons the GPU line above gives and
# one of its own: `ramfb` adds a QEMU **console**, and `screendump` with no device argument writes
# console 0, so a boot carrying both a virtio-gpu and a ramfb is a boot whose screendump means
# whichever QEMU ordered first. `cargo xtask screen-boot` therefore attaches this one alone.
SCREEN=""
if [ -n "$NIFE_SCREEN" ]; then
    SCREEN="-device ramfb"
fi

# A QEMU monitor on a unix socket, when NIFE_GPU_MON names one (milestone 29). This is how the
# **scanout** gets proven rather than only the framebuffer: `screendump` writes a PPM of the scanout
# and it works with -display none (verified against QEMU 11.0.2), so the host can see the pixels the
# guest cannot read back. xtask drives it while the suite runs (see gpu_shot); nothing else uses it,
# and without the variable QEMU gets no monitor at all, exactly as before.
#
# The path must stay under 104 bytes: that is the OS limit on a unix socket path, and a worktree
# checkout plus target/ gets close, which is why xtask puts the socket in /tmp and not in target/.

# NIFE_SCREEN_MON is the same socket for the `ramfb` gate (milestone 243), named apart so that the
# two gates cannot both think they own console 0. Exactly one of the two is ever set; if both were,
# the GPU's wins, because two `-monitor` options is a QEMU error and a silent preference is easier
# to diagnose than a machine that will not start.
MON=""
if [ -n "$NIFE_GPU_MON" ]; then
    MON="-monitor unix:$NIFE_GPU_MON,server,nowait"
elif [ -n "$NIFE_SCREEN_MON" ]; then
    MON="-monitor unix:$NIFE_SCREEN_MON,server,nowait"
fi

# Number of cores. Four by default, the SMP tests' shape (§11); NIFE_SMP moves it, and the
# ceiling is cpu::MAX_CPUS (the per-CPU statics), not this default: the suite asserts loudly if
# -smp exceeds what the kernel can seat. QEMU brings up core 0 running; the kernel starts the rest
# itself via PSCI CPU_ON (see smp.rs).
SMP="${NIFE_SMP:-4}"

# 256 MiB, explicit where this file used to take QEMU's 128 MiB default. Raised 2026-08-15 with
# the 24 KiB thread stacks (kernel/src/thread.rs): their bigger kmem carve (kmem.rs) takes one
# more megabyte from the general pool, and the aarch64 suite at 128 MiB had no megabyte to give.
# It failed twice in one evening, in two different pools (first kmem's carve, then the shell
# budget's 128-page contiguous run), each surfacing as an unrelated test's spawn failing late in
# the suite, which is the "unrelated test failing to get memory" shape disk_service.rs and
# fs_service.rs already narrate from 128 MiB days. The kernel asserts this size in memory.rs, so
# a drift between the two files fails loudly rather than silently changing what the suite means.
# shellcheck disable=SC2086  # $INITRD, $DISK, $NET, $GPU, $SCREEN, $KBD, $RNG and $NVME are deliberately word-split or empty
exec qemu-system-aarch64 \
    -machine "$MACHINE" \
    -cpu "$CPU" \
    -smp "$SMP" \
    -m 256M \
    -display none \
    -serial stdio \
    -semihosting \
    -kernel "$IMG" \
    $INITRD \
    $MMIO_MODERN \
    $DISK \
    $NET \
    $GPU \
    $SCREEN \
    $KBD \
    $USBKBD \
    $RNG \
    $NVME \
    $MON \
    "$@"
