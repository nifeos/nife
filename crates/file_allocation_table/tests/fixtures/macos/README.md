# Images a macOS host formatted and wrote

`fat32.img.zst` and `fat32-mbr.img.zst` are the two FAT32 images that `make-images.sh` builds,
compressed with `zstd -19`. Each expands to 64 MiB.

- `fat32.img`: an unpartitioned volume, formatted by `newfs_msdos -F 32` and written through
  macOS's `msdos` driver. The volume label is `FATPROBE`.
- `fat32-mbr.img`: the same files inside an MBR partition table made by `diskutil eraseDisk FAT32
  FATMBR MBRFormat`, the way a USB stick usually arrives.

Both hold `hello.txt`, `sub/data.bin` (4096 lines of 16 bytes) and `Long File Name.txt`.

macOS 26.6.2 made both images with this script under other volume labels. The label fields were
then set to the script's labels, in all three places a FAT32 volume keeps one: the boot sector, the
backup boot sector and the root directory's volume-label entry. No other byte differs from what
macOS wrote. The script needs macOS (`hdiutil`, `newfs_msdos`, `diskutil`), so CI does not
regenerate these; the compressed files are the fixtures.

| file | sha256 |
|---|---|
| `fat32.img.zst` | `6a4a2bbadc03607018c44267551eb72679c8ade170586467ae32ce6d6d68fee7` |
| `fat32-mbr.img.zst` | `665774c7ee9ddc7e5fc0d8214cb77e12ec0554e0934e105dbab864a1cf8f5223` |
| `fat32.img` | `ddc980a96032fafdf34d028563255e24e5ae623c1831ee3bea32d0170fdd9d19` |
| `fat32-mbr.img` | `97c3ad63f550da3cc8d8126680ad52d0aa2f35c48a583bac04ae76ed6679f440` |

`tests/the_macos_images_read.rs` checks each file against the SHA-256 of the bytes the script
writes into it.
