#!/bin/sh
# Probes behind fat-crates-measured.md, beside this script. Name provisional.
#
#   fatfs-probe.sh build     # no_std builds and section sizes on the three bare-metal targets
#   fatfs-probe.sh deny      # this tree's deny.toml against the two ways fatfs could arrive
#   fatfs-probe.sh image     # write the installer's ESP through fatfs; macOS mounts it, fsck_msdos checks it
#   fatfs-probe.sh firmware <BOOTRISCV64.EFI>   # the same ESP holding a real loader, booted by EDK2 on riscv64
#
# Every probe is generated OUTSIDE this repository, under $WORK, as probe.sh's are, so this tree's
# Cargo.lock and rust-toolchain.toml stay untouched and fatfs is a dependency of nothing here.
# fatfs is rust-fatfs's main line at the commit below, cloned into $WORK; `deny` also tries
# lamfat 0.4.3, its crates.io republish.
#
# BUGS: needs the network. `image` needs macOS (`hdiutil`, `fsck_msdos`). `firmware` needs a built
# riscv64 loader (`cargo xtask stick` stages one at target/stick/EFI/BOOT/BOOTRISCV64.EFI) and runs
# helpers/qemu-stick.sh, which bounds QEMU.
set -eu
HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH='' cd -- "$HERE/../../.." && pwd)
WORK="${WORK:-${TMPDIR:-/tmp}/nife-784-fatfs}"
MODE=${1:?usage: fatfs-probe.sh build|deny|image|firmware <efi>}
REV=38b3a7950bf0e765e34286d1dfe76c519ee347e2
PIN=$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$ROOT/rust-toolchain.toml")
TARGETS="riscv64imac-unknown-none-elf aarch64-unknown-none-softfloat x86_64-unknown-none"
# A generic agent string: crates.io asks for one, and nothing personal goes with it.
export CARGO_HTTP_USER_AGENT=nife-784-probe
mkdir -p "$WORK"
FATFS="$WORK/rust-fatfs"
if [ ! -d "$FATFS" ]; then
    git clone -q https://github.com/rafalh/rust-fatfs "$FATFS"
fi
git -C "$FATFS" checkout -q "$REV"

manifest() { # manifest <dir> <dependency line>
    mkdir -p "$WORK/$1/src"
    printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n[dependencies]\n%s\n' \
        "$1" "$2" > "$WORK/$1/Cargo.toml"
    # The installer's profile: the workspace release profile, overflow checks on, no LTO.
    printf '[profile.release]\noverflow-checks = true\npanic = "abort"\n' >> "$WORK/$1/Cargo.toml"
}

# What both bare-metal probes share: a sector disk, a boot-file source and a report port, each a
# volatile access, so the optimizer keeps every path that reaches them.
env_rs() {
    cat > "$WORK/$1/src/env.rs" <<'RUST'
pub const LBA: usize = 512;
pub const ESP_SECTORS: u64 = 512 * 1024 * 2;
pub const BOOT_FILE_LEN: u64 = 3_000_000;
#[inline(never)]
pub fn disk_write(lba: u64, buf: &[u8; LBA]) {
    unsafe {
        core::ptr::write_volatile(0x1000_0000 as *mut u64, lba);
        for b in buf { core::ptr::write_volatile(0x1000_0008 as *mut u8, *b); }
    }
}
#[allow(dead_code)]
#[inline(never)]
pub fn disk_read(lba: u64, buf: &mut [u8; LBA]) {
    unsafe {
        core::ptr::write_volatile(0x1000_0010 as *mut u64, lba);
        for b in buf.iter_mut() { *b = core::ptr::read_volatile(0x1000_0018 as *const u8); }
    }
}
#[inline(never)]
pub fn boot_file(offset: u64, out: &mut [u8]) {
    unsafe {
        core::ptr::write_volatile(0x1000_0020 as *mut u64, offset);
        for b in out.iter_mut() { *b = core::ptr::read_volatile(0x1000_0028 as *const u8); }
    }
}
#[inline(never)]
pub fn report(code: u64) -> ! {
    unsafe { core::ptr::write_volatile(0x1000_0030 as *mut u64, code); }
    loop {}
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { report(0xdead) }
RUST
}

case "$MODE" in
build)
    manifest esp-ours "file_allocation_table = { path = \"$ROOT/crates/file_allocation_table\" }"
    env_rs esp-ours
    cat > "$WORK/esp-ours/src/main.rs" <<'RUST'
// The installer's ESP path through crates/file_allocation_table: metadata sectors, then the file.
#![no_std]
#![no_main]
mod env;
use env::*;
use file_allocation_table as fat;
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let Ok(v) = fat::Volume::new(ESP_SECTORS, 0, BOOT_FILE_LEN, 0x1234_5678, "BOOTX64.EFI") else { report(1) };
    let mut s = [0u8; LBA];
    for i in 0..v.metadata_sectors() {
        if !v.sector(i, &mut s) { s.fill(0); }
        disk_write(i, &s);
    }
    let first = v.file_first_sector();
    let mut off = 0u64;
    while off < BOOT_FILE_LEN {
        let n = core::cmp::min(LBA as u64, BOOT_FILE_LEN - off) as usize;
        s.fill(0);
        boot_file(off, &mut s[..n]);
        disk_write(first + off / LBA as u64, &s);
        off += LBA as u64;
    }
    report(0)
}
RUST
    manifest esp-fatfs "fatfs = { path = \"$FATFS\", default-features = false, features = [\"lfn\"] }"
    env_rs esp-fatfs
    cat > "$WORK/esp-fatfs/src/main.rs" <<'RUST'
// The same ESP through fatfs, no_std, no allocator, long names on. fatfs addresses bytes, so the
// installer would need this one-sector read-modify-write adapter over its block capability.
#![no_std]
#![no_main]
mod env;
use env::*;
use fatfs::Write as _;
struct Disk { pos: u64, len: u64, lba: u64, buf: [u8; LBA], loaded: bool, dirty: bool }
impl Disk {
    fn load(&mut self, lba: u64) {
        if self.loaded && self.lba == lba { return; }
        self.flush_sector();
        disk_read(lba, &mut self.buf);
        self.lba = lba;
        self.loaded = true;
    }
    fn flush_sector(&mut self) {
        if self.dirty { disk_write(self.lba, &self.buf); self.dirty = false; }
    }
}
#[derive(Debug)]
struct E;
impl fatfs::IoError for E {
    fn is_interrupted(&self) -> bool { false }
    fn new_unexpected_eof_error() -> Self { E }
    fn new_write_zero_error() -> Self { E }
}
impl fatfs::IoBase for Disk { type Error = E; }
impl fatfs::Read for Disk {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, E> {
        if self.pos >= self.len { return Ok(0); }
        let at = (self.pos % LBA as u64) as usize;
        let n = core::cmp::min(out.len(), LBA - at);
        self.load(self.pos / LBA as u64);
        out[..n].copy_from_slice(&self.buf[at..at + n]);
        self.pos += n as u64;
        Ok(n)
    }
}
impl fatfs::Write for Disk {
    fn write(&mut self, src: &[u8]) -> Result<usize, E> {
        let at = (self.pos % LBA as u64) as usize;
        let n = core::cmp::min(src.len(), LBA - at);
        self.load(self.pos / LBA as u64);
        self.buf[at..at + n].copy_from_slice(&src[..n]);
        self.dirty = true;
        self.pos += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> Result<(), E> { self.flush_sector(); Ok(()) }
}
impl fatfs::Seek for Disk {
    fn seek(&mut self, p: fatfs::SeekFrom) -> Result<u64, E> {
        self.pos = match p {
            fatfs::SeekFrom::Start(o) => o,
            fatfs::SeekFrom::End(d) => (self.len as i64 + d) as u64,
            fatfs::SeekFrom::Current(d) => (self.pos as i64 + d) as u64,
        };
        Ok(self.pos)
    }
}
fn install() -> Result<(), fatfs::Error<E>> {
    let mut disk = Disk { pos: 0, len: ESP_SECTORS * LBA as u64, lba: 0, buf: [0; LBA], loaded: false, dirty: false };
    fatfs::format_volume(&mut disk, fatfs::FormatVolumeOptions::new()
        .fat_type(fatfs::FatType::Fat32).bytes_per_cluster(4096).volume_id(0x1234_5678))?;
    let fs = fatfs::FileSystem::new(disk, fatfs::FsOptions::new())?;
    let boot = fs.root_dir().create_dir("EFI")?.create_dir("BOOT")?;
    let mut f = boot.create_file("BOOTRISCV64.EFI")?;
    let mut chunk = [0u8; 4096];
    let mut off = 0u64;
    while off < BOOT_FILE_LEN {
        let n = core::cmp::min(chunk.len() as u64, BOOT_FILE_LEN - off) as usize;
        boot_file(off, &mut chunk[..n]);
        f.write_all(&chunk[..n])?;
        off += n as u64;
    }
    f.flush()?;
    drop(f);
    drop(boot);
    fs.unmount()
}
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    match install() { Ok(()) => report(0), Err(_) => report(1) }
}
RUST
    manifest esp-fatfs-036 'fatfs = { version = "=0.3.6", default-features = false, features = ["core_io"] }'
    env_rs esp-fatfs-036
    printf '#![no_std]\n#![no_main]\nmod env;\n#[unsafe(no_mangle)]\npub extern "C" fn _start() -> ! { let _ = fatfs::FatType::Fat32; env::report(0) }\n' \
        > "$WORK/esp-fatfs-036/src/main.rs"
    SIZE="$(rustc +"$PIN" --print sysroot)/lib/rustlib/$(rustc +"$PIN" -vV | sed -n 's/^host: //p')/bin/llvm-size"
    for t in $TARGETS; do
        for c in esp-ours esp-fatfs; do
            (cd "$WORK/$c" && cargo +"$PIN" build -q --release --target "$t")
            printf '%-32s %-10s ' "$t" "$c"
            "$SIZE" -A "$WORK/$c/target/$t/release/$c" |
                awk '/^\.text|^\.rodata/{a[$1]=$2} END{printf ".text %d .rodata %d\n", a[".text"], a[".rodata"]}'
        done
    done
    echo "== fatfs 0.3.6, no_std through core_io (expected to fail):"
    (cd "$WORK/esp-fatfs-036" && cargo +"$PIN" build -q --release --target riscv64imac-unknown-none-elf 2>&1 |
        grep -E 'Unknown compiler|error' | head -3) || true
    ;;
deny)
    manifest deny-lamfat 'lamfat = { version = "=0.4.3", default-features = false, features = ["lfn"] }'
    manifest deny-git "fatfs = { git = \"https://github.com/rafalh/rust-fatfs\", rev = \"$REV\", default-features = false, features = [\"lfn\"] }"
    for c in deny-lamfat deny-git; do
        printf '#![no_std]\n' > "$WORK/$c/src/lib.rs"
        sed -i.bak 's/^publish = false$/publish = false\nlicense = "MIT OR Apache-2.0"/' "$WORK/$c/Cargo.toml"
        (cd "$WORK/$c" && cargo +"$PIN" generate-lockfile -q)
        echo "== $c"
        cargo deny --manifest-path "$WORK/$c/Cargo.toml" --all-features --config "$ROOT/deny.toml" \
            check -A unmatched-skip -A license-exception-not-encountered 2>&1 | tail -1 || true
    done
    ;;
image|firmware)
    manifest esp-image "fatfs = { path = \"$FATFS\", default-features = false, features = [\"std\", \"lfn\", \"alloc\"] }"
    cat > "$WORK/esp-image/src/main.rs" <<'RUST'
// The installer's ESP through fatfs on the host: 512 MiB, FAT32, 4 KiB clusters, and
// \EFI\BOOT\BOOTRISCV64.EFI holding either the named file or 3,000,000 bytes in a prime-stride
// pattern, the shape crates/file_allocation_table/tests/a_third_party_driver_reads_it.rs writes.
use std::io::{Read, Write};
fn main() {
    let path = std::env::args().nth(1).expect("image path");
    let body: Vec<u8> = match std::env::args().nth(2) {
        Some(f) => std::fs::read(f).expect("the boot file"),
        None => (0..3_000_000).map(|i| (i % 251) as u8).collect(),
    };
    let f = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true)
        .open(&path).expect("create the image");
    f.set_len(512 * 1024 * 1024).expect("size the image");
    let mut disk = fatfs::StdIoWrapper::from(f);
    fatfs::format_volume(&mut disk, fatfs::FormatVolumeOptions::new()
        .fat_type(fatfs::FatType::Fat32).bytes_per_cluster(4096).volume_id(0x1234_5678)
        .volume_label(*b"NIFE       ")).expect("format");
    let fs = fatfs::FileSystem::new(disk, fatfs::FsOptions::new()).expect("mount");
    let boot = fs.root_dir().create_dir("EFI").unwrap().create_dir("BOOT").unwrap();
    let mut file = boot.create_file("BOOTRISCV64.EFI").unwrap();
    file.write_all(&body).unwrap();
    file.flush().unwrap();
    drop(file);
    for e in boot.iter() {
        let e = e.unwrap();
        println!("{} (short {}) {} bytes", e.file_name(), String::from_utf8_lossy(e.short_file_name_as_bytes()), e.len());
    }
    let mut back = Vec::new();
    boot.open_file("BOOTRISCV64.EFI").unwrap().read_to_end(&mut back).unwrap();
    assert!(back == body, "fatfs reads back what it wrote");
    drop(boot);
    fs.unmount().expect("unmount");
    std::fs::write(format!("{path}.expected"), &body).expect("write the expected bytes");
}
RUST
    IMG="$WORK/esp-fatfs.img"
    if [ "$MODE" = image ]; then
        (cd "$WORK/esp-image" && cargo +"$PIN" run -q --release -- "$IMG")
        DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | awk 'NR==1{print $1}')
        fsck_msdos -n "$DEV" && echo "fsck_msdos: exit 0"
        hdiutil detach -quiet "$DEV"
        OUT=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nobrowse -readonly "$IMG")
        DEV=$(echo "$OUT" | awk 'NR==1{print $1}')
        MNT=$(echo "$OUT" | awk -F'\t' '/Volumes/{print $NF}')
        diskutil info "$DEV" | grep Personality
        ls -l "$MNT/EFI/BOOT/"
        cmp "$MNT/EFI/BOOT/BOOTRISCV64.EFI" "$IMG.expected" && echo "byte-exact"
        hdiutil detach -quiet "$DEV"
    else
        EFI=${2:?usage: fatfs-probe.sh firmware <BOOTRISCV64.EFI>}
        (cd "$WORK/esp-image" && cargo +"$PIN" run -q --release -- "$IMG" "$EFI")
        (cd "$ROOT" && NIFE_STICK_TIMEOUT=150 helpers/qemu-stick.sh riscv64 "$IMG" > "$WORK/riscv64-edk2.log" 2>&1) || true
        grep -a -E 'nife uefi_loader: milestone|nife self-test:|handing the system' "$WORK/riscv64-edk2.log"
    fi
    ;;
*)
    echo "usage: fatfs-probe.sh build|deny|image|firmware <efi>" >&2
    exit 2
    ;;
esac
