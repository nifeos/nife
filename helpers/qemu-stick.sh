#!/bin/sh
# helpers/qemu-stick.sh: boot a nife stick under one architecture's UEFI firmware, as a USB stick.
#
#   helpers/qemu-stick.sh x86_64  target/stick          # a directory laid out like the stick
#   helpers/qemu-stick.sh aarch64 stick.img             # or a raw disk image the program wrote
#   helpers/qemu-stick.sh riscv64 target/stick
#
# Name: provisional. Minted 2026-09-19 by milestone/the-program-that-makes-the-stick, in the family of
# `qemu-uefi-x86_64.sh` (which boots one architecture's ESP directory) and named for what it boots.
#
# The same stick boots on all three because UEFI fixes a removable-media file name per architecture
# (`\EFI\BOOT\BOOTX64.EFI`, `BOOTAA64.EFI`, `BOOTRISCV64.EFI`) and each firmware looks only for its
# own. That is the whole of the universal stick, and this script is how it is proved: point all three
# at the same directory or image. See nifeos/record:notes/boot-stick.md.
#
# The stick is attached through an emulated xHCI controller as USB mass storage, which is what a real
# stick is, rather than as a virtio disk. That also keeps it out of the kernel's way: nife drives
# virtio-mmio and would otherwise find the stick as a legacy virtio disk and refuse it.
#
# The firmware images are the EDK2 builds Homebrew's QEMU ships in share/qemu. Each run copies the
# variable store fresh, so one boot's NVRAM (a boot entry, a changed setting) cannot leak into the
# next and make a result depend on history. Bounded by helpers/qemu-bounded.sh, never by `alarm`.
#
# Environment: NIFE_STICK_TIMEOUT (seconds, default 90), NIFE_SMP (default 1), NIFE_MEM (default
# 256M, 2048M on x86_64 where OVMF keeps its tables high).

set -e
cd "$(dirname "$0")/.."
. helpers/qemu-path.sh

ARCH="$1"
STICK="$2"
if [ -z "$ARCH" ] || [ -z "$STICK" ] || [ ! -e "$STICK" ]; then
    echo "usage: $0 x86_64|aarch64|riscv64 <stick directory or raw image> [qemu args...]" >&2
    exit 2
fi
shift 2

if [ -d "$STICK" ]; then
    # QEMU's vvfat driver synthesises a FAT filesystem from a directory: no image, no mtools.
    DRIVE="file=fat:rw:$STICK,format=raw,if=none,id=stick"
else
    DRIVE="file=$STICK,format=raw,if=none,id=stick"
fi

TIMEOUT="${NIFE_STICK_TIMEOUT:-90}"
SMP="${NIFE_SMP:-1}"

firmware_dir() {
    bin="$(command -v "$1" 2>/dev/null || true)"
    [ -n "$bin" ] && echo "$(dirname "$(dirname "$bin")")/share/qemu"
}

VARS="$(mktemp "${TMPDIR:-/tmp}/nife-stick-vars.XXXXXX")"
trap 'rm -f "$VARS"' EXIT
trap 'rm -f "$VARS"; exit 143' HUP INT TERM

case "$ARCH" in
x86_64)
    SHARE="$(firmware_dir qemu-system-x86_64)"
    cp "$SHARE/edk2-i386-vars.fd" "$VARS"
    set -- -machine q35 -cpu max -m "${NIFE_MEM:-2048M}" \
        -drive "if=pflash,format=raw,readonly=on,file=$SHARE/edk2-x86_64-code.fd" \
        -drive "if=pflash,format=raw,file=$VARS" "$@"
    QEMU=qemu-system-x86_64
    ;;
aarch64)
    SHARE="$(firmware_dir qemu-system-aarch64)"
    # There is no edk2-aarch64-vars.fd; the 32-bit Arm variable store is the same 64 MiB flash
    # layout, and is what QEMU's own documentation pairs with the aarch64 code image.
    cp "$SHARE/edk2-arm-vars.fd" "$VARS"
    # acpi=off by default, because EDK2 withholds the device tree when it presents ACPI and the
    # device tree is the richer of the two descriptions this kernel can read (measured 2026-09-19;
    # nifeos/record:notes/boot-stick.md). NIFE_ACPI=on is how the other description is exercised: it is the state
    # every aarch64 cloud machine is in, where SBBR forbids a device tree, and it exists so that
    # path is bootable here for free rather than only on somebody's rented instance. NIFE_GIC=3 is
    # the GIC a cloud machine has, and under TCG the only `virt` with an ITS; unset keeps QEMU's own
    # default, as the kernel runner's NIFE_GIC does.
    set -- -machine "virt,acpi=${NIFE_ACPI:-off}${NIFE_GIC:+,gic-version=$NIFE_GIC}" -cpu cortex-a72 -m "${NIFE_MEM:-256M}" \
        -drive "if=pflash,format=raw,readonly=on,file=$SHARE/edk2-aarch64-code.fd" \
        -drive "if=pflash,format=raw,file=$VARS" "$@"
    QEMU=qemu-system-aarch64
    ;;
riscv64)
    SHARE="$(firmware_dir qemu-system-riscv64)"
    cp "$SHARE/edk2-riscv-vars.fd" "$VARS"
    # QEMU's default OpenSBI runs in M-mode and jumps to the EDK2 image in the first flash bank.
    set -- -machine virt,pflash0=code,pflash1=vars,acpi=off -m "${NIFE_MEM:-256M}" \
        -blockdev "node-name=code,driver=file,read-only=on,filename=$SHARE/edk2-riscv-code.fd" \
        -blockdev "node-name=vars,driver=file,filename=$VARS" "$@"
    QEMU=qemu-system-riscv64
    ;;
*)
    echo "qemu-stick: unknown architecture $ARCH (x86_64, aarch64, riscv64)" >&2
    exit 2
    ;;
esac

# Not `exec`: the trap above has to run after QEMU exits to remove the copied variable store.
helpers/qemu-bounded.sh "$TIMEOUT" "$QEMU" -smp "$SMP" -display none -serial stdio \
    -device qemu-xhci -drive "$DRIVE" -device usb-storage,drive=stick,removable=on "$@"
