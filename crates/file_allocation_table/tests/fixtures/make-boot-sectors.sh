#!/bin/sh
# Rebuild the boot-sector fixtures: dosfstools' mkfs.fat formats each volume and the first 512
# bytes of each are kept. --invariant and -i fix every random and time-based field, so the output
# is byte-identical for the same mkfs.fat (4.2 made these).
#
# Each fixture keeps mkfs.fat's jump, OEM name and BPB, and the 0x55 0xAA signature, and has the
# boot-code region between them zeroed: bytes 62 to 509 on FAT12 and FAT16, 90 to 509 on FAT32.
# That region holds the boot stub and message mkfs.fat writes, which are part of dosfstools
# (GPL-3.0-or-later) and not of this crate. No parser reads it.
#
#   tests/fixtures/make-boot-sectors.sh            from the crate's directory
set -eu
cd "$(dirname "$0")"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
make() {
    name=$1 kib=$2 code=$3
    shift 3
    mkfs.fat --invariant -i 1234ABCD -n FATTEST "$@" -C "$work/$name.img" "$kib" >/dev/null 2>&1
    {
        head -c "$code" "$work/$name.img"
        head -c $((510 - code)) /dev/zero
        tail -c +511 "$work/$name.img" | head -c 2
    } >"$name.boot"
}
make fat12-floppy 1440 62 -F 12
make fat16-32m 32768 62 -F 16
make fat16-4k 65536 62 -F 16 -S 4096
make fat32-64m 65536 90 -F 32
make fat32-4k 524288 90 -F 32 -S 4096
# A FAT32 layout under the 65525-cluster floor, which mkfs.fat makes with a warning.
make fat32-under-floor 20480 90 -F 32 -s 1
