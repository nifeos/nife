# Images a macOS host formatted and wrote

`fat32.img.zst` and `fat32-mbr.img.zst` are the two FAT32 images that
`notes/filesystem-crates-2026-10-07/make-images.sh` builds, compressed with `zstd -19`. Each
expands to 64 MiB.

- `fat32.img`: an unpartitioned volume, formatted by `newfs_msdos -F 32` and written through
  macOS's `msdos` driver. The volume label is `NIFEPROBE`.
- `fat32-mbr.img`: the same files inside an MBR partition table made by `diskutil eraseDisk FAT32
  NIFEMBR MBRFormat`, the way a USB stick usually arrives.

Both hold `hello.txt`, `sub/data.bin` (4096 lines of 16 bytes) and `Long File Name.txt`.

Made on 2026-10-27 (UTC) on a macOS host running macOS 26.6.2, by the script above at the commit
this directory landed in. The script needs macOS (`hdiutil`, `newfs_msdos`, `diskutil`), so CI does
not regenerate these; the compressed files are the fixtures.

| file | sha256 |
|---|---|
| `fat32.img.zst` | `090b8e6563310b95d4ff3754125a670c077d6ad516eb3d0508f441dc3d28f787` |
| `fat32-mbr.img.zst` | `f6383fd441a5dd64b8499923b161b0c8394c7bc2dee538ee0c0d104695fdc7af` |
| `fat32.img` | `4a14232f2b74848396b7857eef50865a8361d27e3c4b47f1ee66c5444345f613` |
| `fat32-mbr.img` | `75d76b76d7653c1196e7007efb87dcdb1dc7310b8d3889659bad1bfc3b987865` |

The hash of each file inside is in `notes/filesystem-crates-2026-10-07/expected.sha256`;
`tests/the_macos_images_read.rs` checks every file against it.
