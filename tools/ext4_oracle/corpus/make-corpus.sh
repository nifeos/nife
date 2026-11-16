#!/bin/sh
# Build the ext4 read-oracle corpus: one image per profile, each made by e2fsprogs from the tree
# populate.py writes. The images are generated, never committed.
#
# Usage: tools/ext4_oracle/corpus/make-corpus.sh <output-directory>
#
# Each profile names its feature set in full rather than reading the host's /etc/mke2fs.conf,
# because distributions ship different defaults and the corpus must mean the same thing on every
# machine. `default` is the ext4 stanza e2fsprogs 1.47 ships (`orphan_file` included); the others
# change one thing each. MKE2FS_CONFIG points at a config that defines the type and nothing else, so
# no host stanza adds to it.
# UUID, hash seed and time are fixed (SOURCE_DATE_EPOCH also fixes the change time mke2fs -d
# copies, which no tool can set on the source file), so two runs give byte-identical images.
#
# After `mke2fs -d`, `e2fsck -fyD` indexes every directory large enough for an htree (mke2fs -d
# writes linear directories), and `e2fsck -fn` must then exit 0, so a profile that e2fsprogs itself
# calls damaged never reaches the reader.
#
# Needs: mke2fs, e2fsck and dumpe2fs from e2fsprogs 1.47 or later, and python3. Linux only: the
# tree carries user extended attributes, which macOS cannot set on a file for mke2fs to copy.
#
# See design/roadmap/0190-ext4-read-write.md.
#
# Name: provisional. calef names files.
set -eu

out=${1:?usage: make-corpus.sh <output-directory>}
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out"
config="$out/mke2fs.conf"
printf '[defaults]\n\tbase_features = ^has_journal\n[fs_types]\n\text4 = {\n\t\tinode_size = 256\n\t}\n\tsmall = {\n\t\tinode_size = 256\n\t}\n' >"$config"

default_features=has_journal,extent,huge_file,flex_bg,metadata_csum,metadata_csum_seed,64bit,dir_nlink,extra_isize,orphan_file,sparse_super,large_file,filetype,resize_inode,dir_index,ext_attr
no_checksum_features=has_journal,extent,huge_file,flex_bg,^metadata_csum,uninit_bg,64bit,dir_nlink,extra_isize,orphan_file,sparse_super,large_file,filetype,resize_inode,dir_index,ext_attr

# name, block size, feature list
profiles="
default-4k 4096 $default_features
default-2k 2048 $default_features
default-1k 1024 $default_features
no-metadata-csum-4k 4096 $no_checksum_features
no-metadata-csum-1k 1024 $no_checksum_features
inline-data-4k 4096 $default_features,inline_data
no-orphan-file-4k 4096 $(echo "$default_features" | sed 's/,orphan_file//')
"

echo "$profiles" | while read -r name block features; do
    [ -n "$name" ] || continue
    tree="$out/$name.tree"
    image="$out/$name.img"
    rm -rf "$tree" "$image"
    mkdir "$tree"
    python3 -I "$here/populate.py" "$tree" "$block"
    MKE2FS_CONFIG=$config SOURCE_DATE_EPOCH=1600000000 E2FSPROGS_FAKE_TIME=1600000000 mke2fs -q -F -t ext4 \
        -b "$block" -I 256 -N 8192 -O "$features" \
        -U 0b5f4a3e-1900-4e1a-8f00-000000000190 -E hash_seed=0b5f4a3e-1900-4e1a-8f00-0000000001a5 \
        -d "$tree" "$image" 64M
    # Times mke2fs -d cannot write: a nanosecond part, a time before 1970 (negative seconds) and a
    # time past 2106 (an epoch bit in the extra field). Each extra field is nanoseconds << 2 | epoch,
    # and Linux reads the seconds field as signed, so 0xffffff9c with epoch 0 is 100 seconds before
    # 1970. debugfs sets the epoch bits from the seconds it is given, so the extra fields are set after.
    debugfs -w -f - "$image" >/dev/null 2>&1 <<EOF
sif /timestamps/nanoseconds mtime 0x5f5e1000
sif /timestamps/nanoseconds mtime_extra 0x1d6f3454
sif /timestamps/nanoseconds atime_extra 0xee6b27fc
sif /timestamps/nanoseconds crtime_extra 0x00000004
sif /timestamps/before-1970 mtime 0xffffff9c
sif /timestamps/before-1970 mtime_extra 0
sif /timestamps/before-1970 atime 0x80000000
sif /timestamps/before-1970 atime_extra 0
sif /timestamps/after-2106 mtime 0x00000010
sif /timestamps/after-2106 mtime_extra 0x00000001
sif /timestamps/after-2106 crtime 0x00000020
sif /timestamps/after-2106 crtime_extra 0x00000002
EOF
    E2FSCK_TIME=1600000000 E2FSPROGS_FAKE_TIME=1600000000 e2fsck -fyD "$image" >/dev/null 2>&1 || [ $? -le 1 ]
    E2FSCK_TIME=1600000000 E2FSPROGS_FAKE_TIME=1600000000 e2fsck -fn "$image" >/dev/null
    dumpe2fs -h "$image" 2>/dev/null >"$out/$name.dumpe2fs-h"
    echo "$name: $(grep '^Filesystem features:' "$out/$name.dumpe2fs-h" | cut -d: -f2- | xargs)"
done
