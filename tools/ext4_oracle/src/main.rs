//! Print the manifest of an ext4 image as `rsext4` reads it: the side under test of the ext4 read
//! oracle.
//!
//! Usage: `ext4_oracle <image> <sector-size>`
//!
//! The image is opened read-only as a device of `<sector-size>`-byte logical sectors (512 or 4096)
//! and mounted read-only with journal replay off. The output is one line per directory entry in
//! the fields `debugfs_manifest.py` prints, sorted by path, so the two manifests compare line for
//! line; `compare.py` does that over the generated corpus. A mount the crate refuses prints one
//! `MOUNT-ERROR` line with the error and exits 2. An error on one entry is printed in that entry's
//! line rather than ending the walk.
//!
//! See nifeos/record:design/roadmap/0190-ext4-read-write/read-oracle.md.
//!
//! Name: provisional. calef names files.
use std::fs::File;
use std::io::Write;
use std::os::unix::fs::FileExt;

use rsext4::error::Ext4Result;
use rsext4::*;

struct Image {
    file: File,
    sector: u32,
    sectors: u64,
}

impl BlockIo for Image {
    fn read(&mut self, buffer: &mut [u8], sector: SectorId, count: u32) -> Ext4Result<()> {
        let n = self.sector as usize * count as usize;
        if buffer.len() < n {
            return Err(Ext4Error::buffer_too_small(buffer.len(), n));
        }
        self.file
            .read_exact_at(&mut buffer[..n], sector.raw() * u64::from(self.sector))
            .map_err(|_| Ext4Error::io())
    }
    fn write(&mut self, _: &[u8], _: SectorId, _: u32) -> Ext4Result<()> {
        Err(Ext4Error::io())
    }
    fn flush(&mut self) -> Ext4Result<()> {
        Ok(())
    }
    fn geometry(&self) -> DeviceGeometry {
        DeviceGeometry::new(self.sector, self.sectors)
    }
    fn capabilities(&self) -> DeviceCapabilities {
        DeviceCapabilities {
            read_only: true,
            ..DeviceCapabilities::default()
        }
    }
}

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Ext4Result<Ext4Timestamp> {
        Ok(Ext4Timestamp::new(1_600_000_000, 0))
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn namespace(attribute_namespace: XattrNamespace) -> String {
    match attribute_namespace {
        XattrNamespace::User => "user.".into(),
        XattrNamespace::Trusted => "trusted.".into(),
        XattrNamespace::Security => "security.".into(),
        XattrNamespace::System => "system.".into(),
        XattrNamespace::PosixAclAccess => "system.posix_acl_access".into(),
        XattrNamespace::PosixAclDefault => "system.posix_acl_default".into(),
        other => format!("{other:?}."),
    }
}

type Fs = Ext4<Image, MountedServices<(), NoopObserver, ()>>;

fn read_all<F>(
    fs: &mut F,
    read: impl Fn(&mut F, u64, &mut [u8]) -> Ext4Result<usize>,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let n = read(fs, out.len() as u64, &mut buf).map_err(|e| format!("{e:?}"))?;
        if n == 0 {
            return Ok(out);
        }
        out.extend_from_slice(&buf[..n]);
    }
}

fn main() {
    let mut fs = mount_from_arguments();
    let mut lines = walk(&mut fs);
    lines.sort();
    let mut out = std::io::stdout().lock();
    for l in lines {
        writeln!(out, "{l}").unwrap();
    }
}

/// Open the image the arguments name and mount it read-only, or exit: 64 on a usage error, 2 with
/// one `MOUNT-ERROR` line when the crate refuses the mount.
fn mount_from_arguments() -> Fs {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: ext4_oracle <image> <sector-size>");
        std::process::exit(64);
    }
    let path = &args[1];
    let sector: u32 = args[2]
        .parse()
        .expect("the sector size is a number of bytes");
    let file = File::open(path).expect("the image opens for reading");
    let sectors = file.metadata().expect("the image has a length").len() / u64::from(sector);
    let image = Image {
        file,
        sector,
        sectors,
    };
    let services = MountServices::new(FixedClock, (), NoopObserver);
    match Ext4::mount(image, services, MountOptions::read_only_no_journal_replay()) {
        Ok(fs) => fs,
        Err(e) => {
            println!("MOUNT-ERROR\t{e:?}");
            std::process::exit(2);
        }
    }
}

/// One manifest line per entry under the root, unsorted, walking every directory.
fn walk(fs: &mut Fs) -> Vec<String> {
    let mut lines = Vec::new();
    let mut stack = vec![(b"".to_vec(), fs.root_inode())];
    while let Some((prefix, dir)) = stack.pop() {
        let mut cursor = DirectoryCursor::Start;
        loop {
            let entries = match fs.read_directory(dir, cursor, 256) {
                Ok(e) => e,
                Err(e) => {
                    lines.push(format!(
                        "{}\tREADDIR-ERROR\t{e:?}",
                        String::from_utf8_lossy(&prefix)
                    ));
                    break;
                }
            };
            if entries.is_empty() {
                break;
            }
            for entry in &entries {
                if entry.name == b"." || entry.name == b".." {
                    continue;
                }
                let mut p = prefix.clone();
                p.push(b'/');
                p.extend_from_slice(&entry.name);
                let info = match fs.inode(entry.inode) {
                    Ok(i) => i,
                    Err(e) => {
                        lines.push(format!("{}\tINODE-ERROR\t{e:?}", hex_or_text(&p)));
                        continue;
                    }
                };
                if matches!(info.file_type(), DirectoryEntryType::Directory) {
                    stack.push((p.clone(), entry.inode));
                }
                lines.push(entry_line(fs, &p, entry.inode, &info));
            }
            cursor = entries.last().unwrap().next_cursor;
            if cursor == DirectoryCursor::End {
                break;
            }
        }
    }
    lines
}

/// The manifest line of one entry, in `debugfs_manifest.py`'s fields.
fn entry_line(fs: &mut Fs, path: &[u8], number: InodeNumber, info: &InodeInfo) -> String {
    let kind = format!("{:?}", info.file_type());
    let content = content_of(fs, number, info);
    let extents = if matches!(
        info.file_type(),
        DirectoryEntryType::RegularFile | DirectoryEntryType::Directory
    ) {
        extents_of(fs, number)
    } else {
        String::from("-")
    };
    let xattrs = attributes_of(fs, number);
    format!(
        "{}\t{}\t{kind}\t{:o}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{content}\t{xattrs}\t{extents}",
        hex_or_text(path),
        number.raw(),
        info.mode & 0o7777,
        info.uid,
        info.gid,
        info.links,
        info.size,
        time_of(info.atime),
        time_of(info.ctime),
        time_of(info.mtime),
        info.btime.map_or(String::from("-"), time_of),
    )
}

/// A time as `debugfs_manifest.py` prints it: signed seconds since 1970, with a nine-digit
/// nanosecond part when it is not zero.
fn time_of(time: Ext4Timestamp) -> String {
    if time.nsec == 0 {
        time.sec.to_string()
    } else {
        format!("{}.{:09}", time.sec, time.nsec)
    }
}

/// A file's length and FNV-1a-64, a symbolic link's target in hex, or `-` for anything else.
fn content_of(fs: &mut Fs, number: InodeNumber, info: &InodeInfo) -> String {
    let is_link = matches!(info.file_type(), DirectoryEntryType::Symlink);
    if !is_link && !matches!(info.file_type(), DirectoryEntryType::RegularFile) {
        return String::from("-");
    }
    match read_all(fs, |fs: &mut Fs, off, buf| fs.read_inode(number, off, buf)) {
        Ok(b) if is_link => hex(&b),
        Ok(b) => format!("{}:{:016x}", b.len(), fnv(&b)),
        Err(e) => format!("READ-ERROR {e}"),
    }
}

/// The leaf extents as `logical+length@physical`, in blocks, with `u` marking an unwritten one.
fn extents_of(fs: &mut Fs, number: InodeNumber) -> String {
    let map = match fs.inode_extents(number, 0, u64::MAX / 2, FileExtentTarget::Data, 100_000) {
        Ok(map) => map,
        Err(e) => return format!("EXTENT-ERROR {e:?}"),
    };
    let block = block_size(fs);
    let mut parts: Vec<String> = map
        .extents
        .iter()
        .map(|x| match x.state {
            FileExtentState::Inline => "inline".to_string(),
            s => format!(
                "{}+{}@{}{}",
                x.logical_start / block,
                x.length.div_ceil(block),
                x.physical_start / block,
                if s == FileExtentState::Unwritten {
                    "u"
                } else {
                    ""
                }
            ),
        })
        .collect();
    if !map.complete {
        parts.push("INCOMPLETE".into());
    }
    if parts.is_empty() {
        "none".into()
    } else {
        parts.join(",")
    }
}

/// Every extended attribute as `name=length:FNV-1a-64`, sorted, or `none`.
fn attributes_of(fs: &mut Fs, number: InodeNumber) -> String {
    let names = match fs.list_xattrs(number) {
        Ok(names) => names,
        Err(e) => return format!("XATTR-ERROR {e:?}"),
    };
    let mut v: Vec<String> = names
        .iter()
        .map(|n| {
            let full = format!(
                "{}{}",
                namespace(n.namespace),
                String::from_utf8_lossy(&n.name)
            );
            match fs.get_xattr(number, n.namespace, &n.name) {
                Ok(val) => format!("{full}={}:{:016x}", val.len(), fnv(&val)),
                Err(e) => format!("{full}=ERROR {e:?}"),
            }
        })
        .collect();
    v.sort();
    if v.is_empty() {
        "none".into()
    } else {
        v.join(",")
    }
}

fn block_size(fs: &Fs) -> u64 {
    fs.statfs().block_size
}

fn hex_or_text(p: &[u8]) -> String {
    String::from_utf8(p.to_vec()).unwrap_or_else(|_| hex(p))
}
