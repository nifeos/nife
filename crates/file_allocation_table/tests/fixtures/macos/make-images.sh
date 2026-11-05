#!/bin/sh
# Build the two FAT32 images in this directory: a volume macOS formats and writes, bare and inside an
# MBR partition table. macOS only (hdiutil, newfs_msdos, diskutil, mkfile); then compress each with
# `zstd -19`. Run as a user who may attach disk images.
#
#   tests/fixtures/macos/make-images.sh <dir>
#
# No byte of either image comes from this crate, which is the point: a stick formatted and written
# by another operating system.
set -eu
D=${1:?usage: make-images.sh <dir>}
mkdir -p "$D"
cd "$D"

populate() { # mount point
    printf 'hello from macos\n' >"$1/hello.txt"
    mkdir "$1/sub"
    python3 -c "import sys; open(sys.argv[1],'wb').write(b''.join(b'%015d\n' % i for i in range(4096)))" "$1/sub/data.bin"
    printf 'long name\n' >"$1/Long File Name.txt"
}

# An unpartitioned ("superfloppy") FAT32, written by macOS's msdos driver.
rm -f fat32.img && mkfile -n 64m fat32.img
dev=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount fat32.img | awk 'NR == 1 { print $1 }')
newfs_msdos -F 32 -v FATPROBE "$dev" >/dev/null
mkdir -p mnt && mount -t msdos "$dev" mnt && populate mnt && umount mnt
hdiutil detach "$dev" >/dev/null

# The same, inside an MBR partition table, which is how a USB stick usually arrives.
rm -f fat32-mbr.img && mkfile -n 64m fat32-mbr.img
dev=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount fat32-mbr.img | awk 'NR == 1 { print $1 }')
diskutil eraseDisk FAT32 FATMBR MBRFormat "$dev" >/dev/null
populate /Volumes/FATMBR
diskutil eject "$dev" >/dev/null
fsck_msdos -n fat32.img >/dev/null
