#!/bin/sh
# Rebuild the boot-sector fixtures: dosfstools' mkfs.fat formats each volume and the first 512
# bytes of each are kept. --invariant and -i fix every random and time-based field, so the output
# is byte-identical for the same mkfs.fat (4.2 made these).
#
#   tests/fixtures/make-boot-sectors.sh            from the crate's directory
set -eu
cd "$(dirname "$0")"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
make() {
    name=$1 kib=$2
    shift 2
    mkfs.fat --invariant -i 1234ABCD -n NIFETEST "$@" -C "$work/$name.img" "$kib" >/dev/null 2>&1
    head -c 512 "$work/$name.img" >"$name.boot"
}
make fat12-floppy 1440 -F 12
make fat16-32m 32768 -F 16
make fat16-4k 65536 -F 16 -S 4096
make fat32-64m 65536 -F 32
make fat32-4k 524288 -F 32 -S 4096
# A FAT32 layout under the 65525-cluster floor, which mkfs.fat makes with a warning.
make fat32-under-floor 20480 -F 32 -s 1
