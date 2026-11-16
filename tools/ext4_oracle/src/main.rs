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
//! See design/roadmap/0190-ext4-read-write/read-oracle.md.
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
    let mut fs: Fs = match Ext4::mount(image, services, MountOptions::read_only_no_journal_replay())
    {
        Ok(fs) => fs,
        Err(e) => {
            println!("MOUNT-ERROR\t{e:?}");
            std::process::exit(2);
        }
    };
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
                let shown = hex_or_text(&p);
                let info = match fs.inode(entry.inode) {
                    Ok(i) => i,
                    Err(e) => {
                        lines.push(format!("{shown}\tINODE-ERROR\t{e:?}"));
                        continue;
                    }
                };
                let kind = format!("{:?}", info.file_type());
                let mut content = String::from("-");
                let mut extents = String::from("-");
                match info.file_type() {
                    DirectoryEntryType::RegularFile | DirectoryEntryType::Symlink => {
                        let number = entry.inode;
                        content = match read_all(&mut fs, |fs: &mut Fs, off, buf| {
                            fs.read_inode(number, off, buf)
                        }) {
                            Ok(b) if matches!(info.file_type(), DirectoryEntryType::Symlink) => {
                                hex(&b)
                            }
                            Ok(b) => format!("{}:{:016x}", b.len(), fnv(&b)),
                            Err(e) => format!("READ-ERROR {e}"),
                        };
                    }
                    DirectoryEntryType::Directory => stack.push((p.clone(), entry.inode)),
                    _ => {}
                }
                if matches!(
                    info.file_type(),
                    DirectoryEntryType::RegularFile | DirectoryEntryType::Directory
                ) {
                    extents = match fs.inode_extents(
                        entry.inode,
                        0,
                        u64::MAX / 2,
                        FileExtentTarget::Data,
                        100_000,
                    ) {
                        Ok(map) => {
                            let mut parts: Vec<String> = map
                                .extents
                                .iter()
                                .map(|x| match x.state {
                                    FileExtentState::Inline => "inline".to_string(),
                                    s => format!(
                                        "{}+{}@{}{}",
                                        x.logical_start / block_size(&fs),
                                        x.length.div_ceil(block_size(&fs)),
                                        x.physical_start / block_size(&fs),
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
                        Err(e) => format!("EXTENT-ERROR {e:?}"),
                    };
                }
                let xattrs = match fs.list_xattrs(entry.inode) {
                    Ok(names) => {
                        let mut v: Vec<String> = names
                            .iter()
                            .map(|n| {
                                let full = format!(
                                    "{}{}",
                                    namespace(n.namespace),
                                    String::from_utf8_lossy(&n.name)
                                );
                                match fs.get_xattr(entry.inode, n.namespace, &n.name) {
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
                    Err(e) => format!("XATTR-ERROR {e:?}"),
                };
                lines.push(format!(
                    "{shown}\t{}\t{kind}\t{:o}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{content}\t{xattrs}\t{extents}",
                    entry.inode.raw(),
                    info.mode & 0o7777,
                    info.uid,
                    info.gid,
                    info.links,
                    info.size,
                    info.atime,
                    info.ctime,
                    info.mtime,
                    info.btime,
                ));
            }
            cursor = entries.last().unwrap().next_cursor;
            if cursor == DirectoryCursor::End {
                break;
            }
        }
    }
    lines.sort();
    let mut out = std::io::stdout().lock();
    for l in lines {
        writeln!(out, "{l}").unwrap();
    }
}

fn block_size(fs: &Fs) -> u64 {
    fs.statfs().block_size
}

fn hex_or_text(p: &[u8]) -> String {
    String::from_utf8(p.to_vec()).unwrap_or_else(|_| hex(p))
}
