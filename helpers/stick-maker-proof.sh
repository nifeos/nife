#!/bin/sh
# helpers/stick-maker-proof.sh: prove stick_maker end to end on macOS, on file-backed disks only.
#
#   cargo xtask stick                  # first: the payloads and the program
#   helpers/stick-maker-proof.sh       # then this
#
# Name: provisional. Minted 2026-09-19 by milestone/the-program-that-makes-the-stick.
#
# What it proves, in the order a stranger meets it:
#
#   1. The ERASE path. A blank 64 MiB file is attached as a disk; stick_maker finds it, erases it as
#      one FAT32 volume and writes the three boot files.
#   2. The COPY path. A second file is formatted FAT32 first (the state most sticks are sold in);
#      stick_maker finds it and copies without erasing, and a file already on it survives.
#   3. Each written image is then booted under all three UEFI firmwares as a USB stick
#      (helpers/qemu-stick.sh), and each has to reach the progenitor.
#
# **It never touches a real disk.** Every disk it names is one hdiutil just attached from a file in a
# fresh temporary directory, and before anything is written the script checks that diskutil calls it
# a `Disk Image`. stick_maker itself refuses a disk image unless `--include-disk-images` is given,
# and refuses a disk without the removable-media bit regardless, so a wrong identifier here would be
# refused twice before anything was written. macOS only: hdiutil is the file-backed disk this host has.

set -eu
cd "$(dirname "$0")/.."

# The universal binary when both macOS builds were made, else this Mac's own.
PROGRAM="target/stick-maker/stick_maker-macos"
[ -x "$PROGRAM" ] || PROGRAM="target/stick-maker/stick_maker-macos-$(uname -m)"
if [ ! -x "$PROGRAM" ]; then
    echo "stick-maker-proof: $PROGRAM is not built; run \`cargo xtask stick\` first" >&2
    exit 1
fi
command -v hdiutil > /dev/null || { echo "stick-maker-proof: macOS only (needs hdiutil)" >&2; exit 1; }

WORK="$(mktemp -d -t nife-stick-proof)"
ATTACHED=""
cleanup() {
    for d in $ATTACHED; do hdiutil detach -force "/dev/$d" > /dev/null 2>&1 || true; done
    rm -rf "$WORK"
}
trap cleanup EXIT

# Attach a raw file as a disk and print its identifier, having checked it is a disk image.
attach() {
    dev="$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$1" | awk 'NR==1{print $1}' | sed 's#/dev/##')"
    ATTACHED="$ATTACHED $dev"
    if ! diskutil info "$dev" | grep -q "Protocol: *Disk Image"; then
        echo "stick-maker-proof: $dev is not a disk image; stopping before anything is written" >&2
        exit 1
    fi
    echo "$dev"
}

detach() {
    hdiutil detach "/dev/$1" > /dev/null 2>&1 || true
    ATTACHED="$(echo "$ATTACHED" | sed "s/ $1//")"
}

failed=0

echo "== the erase path: a blank file =="
mkfile -n 64m "$WORK/blank.img"
dev="$(attach "$WORK/blank.img")"
"$PROGRAM" --include-disk-images --disk "$dev" --erase
detach "$dev"

echo "== the copy path: a file already FAT32, holding somebody's file =="
mkfile -n 64m "$WORK/fat.img"
dev="$(attach "$WORK/fat.img")"
diskutil eraseDisk FAT32 SOLDASIS MBR "$dev" > /dev/null
mount_point="$(diskutil info "${dev}s1" | sed -n 's/^ *Mount Point: *//p')"
echo "a photo" > "$mount_point/holiday.txt"
"$PROGRAM" --include-disk-images --disk "$dev" --yes --keep-mounted
if [ "$(cat "$mount_point/holiday.txt")" != "a photo" ]; then
    echo "stick-maker-proof: the copy path lost a file that was already on the stick" >&2
    failed=1
fi
echo "holiday.txt survived the copy"
detach "$dev"

# A green self-test verdict: a `nife self-test: ` line with no `FAILED: ` on it. What follows the
# count depends on the machine (a check whose device it lacks is `absent: `), so the line is not
# matched whole.
all_passed() {
    awk '/nife self-test: / { seen = 1; if (index($0, "FAILED: ")) red = 1 } END { exit !(seen && !red) }' "$1"
}

for image in blank fat; do
    for arch in x86_64 aarch64 riscv64; do
        log="$WORK/$image-$arch.log"
        NIFE_STICK_TIMEOUT="${NIFE_STICK_TIMEOUT:-120}" helpers/qemu-stick.sh "$arch" "$WORK/$image.img" < /dev/null > "$log" 2>&1 || true
        if grep -q "nife: handing the system to the userspace progenitor." "$log" \
            && all_passed "$log" \
            && ! grep -q "\[PANIC\]" "$log"; then
            echo "$image.img on $arch: booted to the progenitor"
        else
            echo "$image.img on $arch: FAILED; the end of its transcript:" >&2
            tail -20 "$log" >&2
            failed=1
        fi
    done
done
exit "$failed"
