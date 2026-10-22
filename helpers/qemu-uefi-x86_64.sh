#!/bin/sh
#
# Name: ratified 2026-08-30 (calef, in session, on milestone 87's lane report). Hyphens because
# shell commands are hyphenated everywhere, per AGENTS.md's per-domain naming table, and the ISA
# suffix matches its sibling helpers/qemu-runner-x86_64.sh. NOTE: script/names scans script/ and
# not helpers/, so nothing reads this block today; it is written where a reader meets the thing.
#
# Boot the x86_64 kernel under REAL FIRMWARE (milestone 87): OVMF, the open-source UEFI
# implementation that ships with QEMU, loading `uefi_loader` from a FAT filesystem exactly the way
# the Dell OptiPlex 7050's firmware loads it from a USB stick.
#
# WHY THIS IS NOT helpers/qemu-runner-x86_64.sh WITH A FLAG. That runner is a cargo `runner`: cargo
# appends a kernel ELF and QEMU's PVH loader reads it with `-kernel`. This path has no `-kernel` at
# all. The firmware finds `\EFI\BOOT\BOOTX64.EFI` on a disk, starts it, and the loader places the
# kernel itself. Nothing is shared but the machine model, and merging them would mean a runner that
# ignores the one argument cargo exists to pass it.
#
#   helpers/qemu-uefi-x86_64.sh <esp-directory> [extra qemu args...]
#
# `cargo xtask uefi-image` populates the directory; see notes/x86-uefi-boot.md for the whole picture
# and for the bench procedure on the real machine.
#
# THE FAT FILESYSTEM IS QEMU'S OWN, and that is what makes this need no new host tooling. `file=fat:`
# is QEMU's vvfat block driver: it synthesises a FAT filesystem out of a host DIRECTORY, live, so
# there is no image to build and no `mtools`/`mformat` to install (neither is available on this
# machine, and `grub-mkrescue` is not installable at all here; see notes/x86-uefi-boot.md's fork).
# `fat:rw:` rather than read-only because OVMF writes its own boot-order bookkeeping.
#
# BOUNDED, ALWAYS. The kernel halts with `hlt` rather than exiting, so QEMU never exits on its own.
# `timeout(1)` does not exist on macOS and `perl -e 'alarm N'` does not work on QEMU (it installs its
# own SIGALRM handler and swallows it, which once leaked eleven emulators). NIFE_UEFI_TIMEOUT moves
# the bound; it is a bound rather than a choice.

set -e
cd "$(dirname "$0")/.."

ESP="$1"
if [ -z "$ESP" ] || [ ! -d "$ESP" ]; then
    echo "qemu-uefi-x86_64: usage: $0 <esp-directory> [qemu args...]" >&2
    echo "                 run \`cargo xtask uefi-image\` to populate one" >&2
    exit 1
fi
shift

# OVMF, wherever this machine keeps it. On macOS it ships WITH QEMU rather than beside it, which is
# the fact that decided milestone 87's fork: nothing had to be installed for this to work.
# **That is a Homebrew fact and not a universal one**, learned when this gate first ran in CI:
# Debian packages the firmware separately, so `script/bootstrap` installs `ovmf` on Linux, and
# Ubuntu 24.04 names the file `OVMF_CODE_4M.fd` rather than `OVMF_CODE.fd`, which is why both
# spellings are searched. NIFE_OVMF_CODE names it explicitly on a machine that puts it elsewhere.
#
# **The variable store is `edk2-i386-vars.fd` even for x86_64**, which is QEMU's own naming and not
# a mistake: the vars template is architecture-shared across the i386/x86_64 pair. Same prefix-first
# search as the code image below, for the same reason.
#
# **Ask QEMU where it lives, first.** The header above says the firmware ships WITH QEMU, and that is
# the fact to act on rather than a list of places QEMU has been seen. CI builds QEMU from source into
# a cached prefix (`script/ci-qemu`, `$HOME/.cache/nife-qemu`), so no absolute path in a list can
# ever name it, and the hardcoded list is exactly why this gate failed on its first CI run while
# passing on every developer machine. `<prefix>/share/qemu/edk2-x86_64-code.fd` is where QEMU's own
# build puts it, which is also what makes the Homebrew entry below work.
qemu_bin="$(command -v qemu-system-x86_64 2>/dev/null || true)"
qemu_prefix=""
[ -n "$qemu_bin" ] && qemu_prefix="$(dirname "$(dirname "$qemu_bin")")"
if [ -z "$NIFE_OVMF_CODE" ]; then
    if [ -n "$qemu_prefix" ]; then
        for candidate in \
            "$qemu_prefix/share/qemu/edk2-x86_64-code.fd" \
            "$qemu_prefix/share/edk2-x86_64-code.fd"
        do
            [ -f "$candidate" ] && NIFE_OVMF_CODE="$candidate" && break
        done
    fi
fi
if [ -z "$NIFE_OVMF_CODE" ]; then
    for candidate in \
        /opt/homebrew/share/qemu/edk2-x86_64-code.fd \
        /usr/local/share/qemu/edk2-x86_64-code.fd \
        /usr/share/OVMF/OVMF_CODE_4M.fd \
        /usr/share/OVMF/OVMF_CODE.fd \
        /usr/share/edk2/x64/OVMF_CODE.fd
    do
        [ -f "$candidate" ] && NIFE_OVMF_CODE="$candidate" && break
    done
fi
if [ ! -f "$NIFE_OVMF_CODE" ]; then
    echo "qemu-uefi-x86_64: no OVMF firmware found. Set NIFE_OVMF_CODE to an edk2 x86_64 CODE image." >&2
    exit 1
fi

# The variable store has to be WRITABLE (the firmware records its boot order in it), so the pristine
# one is copied rather than used in place. It is `edk2-i386-vars.fd` even for an x86_64 build,
# because upstream edk2 builds one variable store for both and Homebrew keeps that name.
# **Where this boot's firmware variables live**, and why it is a knob (milestone 198 (a package
# manager, and the trivial install that makes a second customer possible), rung 2a). The default is
# one file that persists across every run in this checkout, which is what a real machine's firmware
# does and is right for the interactive runners. It is wrong for the install gate: boot 1 writes a
# boot option that boot 2 would then rely on, so "the firmware found the installed disk on its own"
# would be a claim about our own leftovers. That gate points this at a file it deletes first.
VARS="${NIFE_OVMF_VARS_OUT:-target/ovmf-vars.fd}"
if [ ! -f "$VARS" ]; then
    for candidate in \
        "$NIFE_OVMF_VARS" \
        "$qemu_prefix/share/qemu/edk2-i386-vars.fd" \
        "$qemu_prefix/share/edk2-i386-vars.fd" \
        /opt/homebrew/share/qemu/edk2-i386-vars.fd \
        /usr/local/share/qemu/edk2-i386-vars.fd \
        /usr/share/OVMF/OVMF_VARS.fd \
        /usr/share/OVMF/OVMF_VARS_4M.fd \
        /usr/share/edk2/x64/OVMF_VARS.fd
    do
        [ -n "$candidate" ] && [ -f "$candidate" ] && cp "$candidate" "$VARS" && break
    done
fi
if [ ! -f "$VARS" ]; then
    echo "qemu-uefi-x86_64: no OVMF variable store found. Set NIFE_OVMF_VARS." >&2
    exit 1
fi

# `q35`, `-cpu max` and `isa-debug-exit` are deliberately the same as
# helpers/qemu-runner-x86_64.sh's, so a difference between the two boots is the FIRMWARE and not the
# machine. One core by default (the PVH runner defaults to two), NOT because UEFI has anything to
# do with it: the two AP-bring-up defects that once made two cores flake are fixed, and the whole
# suite at two cores under OVMF is unmeasured since. `cargo xtask uefi-boot` is where the second
# core is gated (it sets NIFE_SMP=2 and asserts `smp: 2 core(s) online`); `uefi-test` keeps this
# default.
#
# THE MEMORY SIZE IS THE ONE THING THAT IS NOT THE PVH RUNNER'S, and it is a bound rather than a
# preference. Firmware places its ACPI tables just under the top of RAM, so the memory size decides
# what physical addresses the kernel is asked to read. At `-m 256M` they land at 0x0fb7e014 and any
# reach bug hides; at 2 GiB they land at 0x7fb7e014, which is where a real machine puts them.
# `arch::x86_64::machine`'s BOOT_DIRECT_MAP_LIMIT was wrong by 4x for exactly as long as this
# script matched its sibling, and that bug cost the APICs, the timer, PCI and VT-d on any machine
# with real RAM. NIFE_MEM sets it back to 256M for a like-for-like memory-map comparison against
# the PVH runner (notes/x86-uefi-boot.md's table was measured that way).
# A QEMU monitor on a unix socket, when NIFE_SCREEN_MON names one (milestone 243). This is what
# lets a GATE read a screen: `screendump` writes a PPM of the scanout and works with `-display none`
# (the aarch64 runner has relied on that since milestone 29), and `board_console::screen` turns that
# PPM back into the text nife's framebuffer console drew into it. Without the variable QEMU gets no
# monitor, exactly as before.
#
# **This machine has a display adapter whether or not anyone looks at it**, which is the fact the
# whole milestone rests on: `-display none` suppresses the host WINDOW, not the emulated VGA device,
# so OVMF finds an `EFI_GRAPHICS_OUTPUT_PROTOCOL` here for the same reason a real machine's firmware
# finds one. That is why this gate is worth anything at all.
#
# The path must stay under 104 bytes (the OS limit on a unix socket path), which is why xtask puts
# the socket in /tmp rather than in a worktree's target/.
MON=""
if [ -n "$NIFE_SCREEN_MON" ]; then
    MON="-monitor unix:$NIFE_SCREEN_MON,server,nowait"
fi

SMP="${NIFE_SMP:-1}"
CPU="${NIFE_CPU:-max}"
MEM="${NIFE_MEM:-2048}"
TIMEOUT="${NIFE_UEFI_TIMEOUT:-90}"

# `-no-reboot` turns a triple fault into an exit rather than a silent loop back through OVMF, the
# same reason helpers/qemu-runner-x86_64.sh gives. `NIFE_ALLOW_REBOOT=1` drops it for the run whose
# proof is a reset: `swish-check`'s first boot (milestone 805 (`reboot` at the prompt)), which ends by
# typing `reboot` and waits for OVMF and a second prompt. That run is bounded and reads every line,
# so a triple-fault loop costs it a timeout rather than a blank terminal.
NO_REBOOT="-no-reboot"
if [ -n "$NIFE_ALLOW_REBOOT" ]; then
    NO_REBOOT=""
fi

# THE DEVICES ARE THE PVH RUNNER'S, AND SINCE MILESTONE 195 THAT IS THE POINT. The tour needs none
# of them, but the kernel SUITE runs here now (`cargo xtask uefi-test`), and a suite that skipped
# every device test would be reporting on the firmware and nothing else. What each buys that the PVH
# runner cannot:
#
#   - the virtio-blk-pci function, because OVMF enumerates the bus and assigns its BARs before this
#     kernel ever sees it. Under `-kernel` the kernel places them itself, so "the driver can reach an
#     MSI-X table a PCI bus walk found" and "the driver can reach one IT put there" were the same
#     sentence. Here they are not. That is one of the three things milestone 215's BUGS listed as
#     answerable only on xenon.
#   - `intel-iommu`, so the confinement test has a unit to fault on, and so the two boots differ in
#     the firmware rather than in the machine.
#   - the NVMe controller, for the same reason it is on the PVH runner (decisions §86).
#   - the RedoxFS disk (a second virtio-blk-pci function), for `script/swish-check`'s x86_64 leg,
#     only when NIFE_UEFI_REDOXFS is set (see below).
#
# Each is attached only when its variable names an image, exactly as on the PVH runner, so a plain
# `helpers/qemu-uefi-x86_64.sh target/esp` is still the bare tour machine.
#
# BUGS
#
# - **The fixture set here is the two-disk one, and it stayed that way when the PVH runner got all
#   five** (milestone 420 (x86_64 has one fixture of five and no NIC, GPU, keyboard or RNG)). The
#   crash, GPT and blank disks are the PVH runner's alone, and they stay so for a reason that is
#   not a `-device` line: they ride behind the RedoxFS disk in the ordinal order
#   `find_block_device_n` counts, the RedoxFS disk is opt-in here (above, a recorded defect), and
#   `uefi_test` builds only the nifefs and NVMe images. Under OVMF the tests that want them skip on
#   their own lines, which is honest but means the firmware path proves none of them. (The NIC is
#   not in that set: the e1000e below is unconditional, so the net tests do run here. The RNG is
#   attached under `NIFE_RNG`, so the PCIe entropy test runs here.)
DISK=""
if [ -n "$NIFE_DISK" ]; then
    PCI_DISK="${NIFE_DISK%.img}-pci.img"
    if [ ! -f "$PCI_DISK" ]; then
        echo "qemu-uefi-x86_64: $PCI_DISK does not exist (run mkdisk first; it writes both images)" >&2
        exit 1
    fi
    DISK="-drive file=$PCI_DISK,if=none,format=raw,id=hd1 -device virtio-blk-pci,drive=hd1,disable-legacy=on,iommu_platform=on"
    # The RedoxFS fixture as the SECOND function, when a caller asks with NIFE_UEFI_REDOXFS=1
    # (name provisional). Added by milestone 182 (x86_64's own interactive-boot entry point), whose
    # `swish-check` leg boots this runner and
    # types `>`, `<` and `rm` at the prompt, all of which need a filesystem the progenitor mounts.
    #
    # **Opt-in rather than the PVH runner's attach-when-present, and that is a recorded defect, not
    # a design.** Attached whenever the image existed, `uefi-test` failed
    # `dir_capability_tests::a_full_directory_capability_does_everything_inside_and_nothing_outside`
    # (2026-09-19): the full directory capability "could not do what it was granted". The same test
    # passes on the PVH leg a minute earlier, which wrote to the SAME image, so the likeliest reading
    # is `mkredoxfs`'s own second-boot case (a mount of an image a previous boot wrote) rather than
    # anything about firmware. Not investigated; milestone 182's block records it.
    if [ -n "$NIFE_UEFI_REDOXFS" ]; then
        REDOXFS_DISK="${NIFE_DISK%.img}-redoxfs.img"
        if [ ! -f "$REDOXFS_DISK" ]; then
            echo "qemu-uefi-x86_64: $REDOXFS_DISK does not exist (run mkredoxfs first)" >&2
            exit 1
        fi
        DISK="$DISK -drive file=$REDOXFS_DISK,if=none,format=raw,id=hd2 -device virtio-blk-pci,drive=hd2,disable-legacy=on,iommu_platform=on"
    fi
fi

# **The installation medium, detached** (milestone 198 (a package manager, and the trivial install
# that makes a second customer possible), rung 2a). An install that only boots while the stick is
# still in the machine has not installed anything, so the second half of `cargo xtask install-boot`
# runs this script with no vvfat drive at all and the firmware has to find
# \EFI\BOOT\BOOTX64.EFI on the disk it was just written to. The ESP directory argument is still
# required and still has to exist, because everything else about the invocation is the same and a
# second code path here would be a second machine.
STICK="-drive format=raw,file=fat:rw:$ESP"
if [ -n "$NIFE_UEFI_NO_STICK" ]; then
    STICK=""
fi
# **The stick, chosen first**, which is what a person does from the firmware's one-time boot menu.
# Without it OVMF prefers an NVMe disk that carries \EFI\BOOT\BOOTX64.EFI, so a gate meaning to boot
# the stick on an installed machine would quietly boot the disk instead (measured: boot 3 of
# `cargo xtask install-boot`, 2026-10-04). Same AHCI bus the plain `-drive` lands on; only the
# boot index is new. Name provisional.
if [ -n "$NIFE_UEFI_STICK_FIRST" ] && [ -z "$NIFE_UEFI_NO_STICK" ]; then
    STICK="-drive if=none,id=stick,format=raw,file=fat:rw:$ESP -device ide-hd,drive=stick,bus=ide.0,bootindex=0"
fi

NVME=""
if [ -n "$NIFE_NVME" ]; then
    if [ ! -f "$NIFE_NVME" ]; then
        echo "qemu-uefi-x86_64: NIFE_NVME=$NIFE_NVME does not exist (xtask's mknvmedisk writes it)" >&2
        exit 1
    fi
    # **Two knobs for fatal risk 6's rehearsal** (milestone 261 (the NVMe driver leaves the kernel); `cargo xtask disk-throughput`,
    # notes/risk-6-bench-evening.md). Names provisional. NIFE_NVME_ROOT_PORT=1 puts the controller
    # behind a PCIe root port, which is where xenon's Micron is (bus 1, not bus 0), so the bridge
    # walk and the DMAR scope resolution both run against the shape the bench has.
    # NIFE_NVME_DEVICE_OPTS is appended to `-device nvme` verbatim, for the namespace's block size
    # (`,logical_block_size=8192,physical_block_size=8192` is the one this driver must refuse).
    NVME_BUS=""
    if [ -n "$NIFE_NVME_ROOT_PORT" ]; then
        NVME="-device pcie-root-port,id=nvmeport,chassis=1,slot=1"
        NVME_BUS=",bus=nvmeport"
    fi
    NVME="$NVME -drive file=$NIFE_NVME,if=none,format=raw,id=nvme0 -device nvme,serial=nife-nvme,drive=nvme0$NVME_BUS${NIFE_NVME_DEVICE_OPTS:-}"
fi

# **NIFE_ACCEL=kvm runs the guest on the host's own cores** (milestone 628 (provisional), which
# measured what it buys; notes/benchmarks/swish-check-x86-leg.md). Only on an x86_64 Linux host with
# a usable /dev/kvm, which `swish-check` checks before setting it; anything else is QEMU's default,
# TCG, exactly as before. Unlike the aarch64 runner's `hvf` there is nothing to probe: KVM runs the
# guest's own ISA, and the machine below needs no change for it. Two facts that change underneath:
# `-cpu max` becomes the host's own CPU model (so RDSEED is the host's, which every x86 runner GitHub
# offers has), and `intel-iommu`'s `intremap` resolves OFF rather than ON, because KVM's in-kernel
# irqchip is what QEMU's `auto` defers to (helpers/qemu-runner-x86_64.sh's NIFE_INTREMAP block). The
# kernel never acts on interrupt remapping (arch::x86_64::iommu's BUGS), so neither is a boot this
# tree behaves differently on. Any other value, `hvf` included (the aarch64 runner's, which a shell
# may still have exported), leaves this runner on TCG, as it always did, rather than refusing.
# Attach a USB keyboard on an xHCI controller when NIFE_USB_KEYBOARD is set (milestone 242 (USB host
# and HID)). No iommu_platform flag, NVMe's reason: that knob is virtio's opt-in, and a real PCI
# device model's DMA always goes through the PCI address space, so the controller sits behind
# the intel-iommu below with nothing to forget. The keys come from the host over the monitor
# (`sendkey`), which delivers to the most recently activated keyboard; q35's PS/2 keyboard
# registers first and never activates, so that is this one. script/swish-check's USB keyboard boot is the one user.
# NIFE_USB_KEYBOARD_OPTS is appended to the usb-kbd device (`,usb_version=1` makes it full speed),
# NIFE_USB_CONTROLLER_OPTS to the controller (`,msix=off,msi=on` leaves it MSI only, as an Intel PCH is).
USBKBD=""
if [ -n "$NIFE_USB_KEYBOARD" ]; then
    USBKBD="-device qemu-xhci,id=xhci${NIFE_USB_CONTROLLER_OPTS:-} -device usb-kbd,bus=xhci.0${NIFE_USB_KEYBOARD_OPTS:-}"
fi

ACCEL=""
if [ "${NIFE_ACCEL:-}" = "kvm" ]; then
    ACCEL="-accel kvm"
elif [ -n "$NIFE_TCG_THREAD" ]; then
    # Parallel or interleaved TCG cores, the PVH runner's knob with the same meaning and the same
    # BUGS (helpers/qemu-runner-x86_64.sh): a hunting instrument, not a gate. Provisional name.
    ACCEL="-accel tcg,thread=$NIFE_TCG_THREAD"
fi

# **The NIC, said out loud** (milestone 494 (a driver for the network card a PC actually has)).
# Until 2026-10-04 this runner named no network at all, and QEMU filled the gap with its default:
# on `q35` that is an `e1000e` on user networking, which every boot here carried without anybody
# having chosen it. Milestone 494's lane found it when its UDP gate reached slirp's TFTP server and
# was told the fixture did not exist: the implicit network had no `tftp=` and no echo peer. So the
# device is now the one the default already was, on the slirp network the other runners give their
# NICs: DHCP on 10.0.2.0/24, the echo peer at 10.0.2.9:7777, the package peer at 10.0.2.9:8080, the
# name server at 10.0.2.9:53 (milestone 384 (in a capability system the resolver is a grant)) and
# the TFTP root (helpers/qemu-runner-aarch64.sh has the reason for each), plus the bench boot's
# rehearsal peer at 10.0.2.9:9494 (helpers/network-bench-peer), and the TLS peer at 10.0.2.9:8443
# (helpers/tls-peer, milestone 501 (a TLS client that speaks to one pinned peer)). `romfile=` keeps
# OVMF from loading the card's iPXE option ROM, which this tree never boots from.
PACKAGE_PEER="$(cd "$(dirname "$0")" && pwd)/package-http-peer"
TFTPDIR="$(dirname "$0")/../target/tftp"
mkdir -p "$TFTPDIR"
printf 'nife-tftp!' > "$TFTPDIR/nife"
BENCH_PEER="$(cd "$(dirname "$0")" && pwd)/network-bench-peer"
TLS_PEER="$(cd "$(dirname "$0")" && pwd)/tls-peer"
NAME_SERVER_PEER="$(cd "$(dirname "$0")" && pwd)/name-server-peer"
# The boot's tag, as in qemu-runner-x86_64.sh: helpers/name-server-peer keys its rebinding answer
# for milestone 871 (a sixth outsider pass attacks the confinement claim) on it. Missing here at
# first, so every UEFI boot after the first shared one state file and the probe proved nothing.
NIFE_BOOT_TAG="$$-$(date +%s)"
export NIFE_BOOT_TAG
NET="-netdev user,id=net0,guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat,guestfwd=tcp:10.0.2.9:8080-cmd:$PACKAGE_PEER,guestfwd=tcp:10.0.2.9:53-cmd:$NAME_SERVER_PEER,guestfwd=tcp:10.0.2.9:9494-cmd:$BENCH_PEER,guestfwd=tcp:10.0.2.9:8443-cmd:$TLS_PEER,tftp=$TFTPDIR -device e1000e,netdev=net0,mac=52:54:00:e1:00:0e,romfile="

# A virtio-rng-pci under NIFE_RNG, as on the PVH runner (milestone 420), behind the same
# `iommu_platform=on` the disks use, so the PCIe entropy test runs on the firmware path too.
RNG=""
if [ -n "$NIFE_RNG" ]; then
    RNG="-device virtio-rng-pci,disable-legacy=on,iommu_platform=on"
fi

# A virtio-gpu under NIFE_GPU and a virtio keyboard under NIFE_KEYBOARD, as on the PVH runner
# (milestone 420), so the display, compositor and keyboard tests run under `cargo xtask uefi-test`.
# Same `-vga none` for the same reason: `screendump` writes console 0, and q35's default VGA would
# be it. That also means the firmware's GOP is the virtio-gpu's, so OVMF's VirtioGpuDxe drives the
# device before the kernel's driver does. It resets the device at ExitBootServices and reports
# `PixelBltOnly`, so the loader hands the kernel no screen and the driver starts from a reset
# device, as it does under PVH. `uefi_boot`'s tour removes both variables: its screen check reads
# the VGA's linear framebuffer through GOP.
GPU=""
if [ -n "$NIFE_GPU" ]; then
    GPU="-vga none -device virtio-gpu-pci,disable-legacy=on,iommu_platform=on"
fi
KBD=""
if [ -n "$NIFE_KEYBOARD" ]; then
    KBD="-device virtio-keyboard-pci,disable-legacy=on,iommu_platform=on"
fi
# The scanout referee's monitor (xtask's `ScanoutReferee`), beside the screen gate's above: it
# presses the key the keyboard test waits for and dumps the scanout. QEMU takes two monitors.
GPU_MON=""
if [ -n "$NIFE_GPU_MON" ]; then
    GPU_MON="-monitor unix:$NIFE_GPU_MON,server,nowait"
fi

exec helpers/qemu-bounded.sh "$TIMEOUT" qemu-system-x86_64 \
    $ACCEL \
    -machine q35 \
    -cpu "$CPU" \
    -smp "$SMP" \
    -m "$MEM" \
    -display none \
    -serial stdio \
    $NO_REBOOT \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -device intel-iommu \
    $MON \
    $DISK \
    $NVME \
    $NET \
    $RNG \
    $GPU \
    $KBD \
    $GPU_MON \
    $USBKBD \
    -drive "if=pflash,format=raw,unit=0,readonly=on,file=$NIFE_OVMF_CODE" \
    -drive "if=pflash,format=raw,unit=1,file=$VARS" \
    $STICK \
    "$@"
