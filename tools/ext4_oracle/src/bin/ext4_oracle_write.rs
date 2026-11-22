//! Apply a sequence of operations to an ext4 image through `rsext4`: the side under test of the
//! ext4 write oracle.
//!
//! Usage: `ext4_oracle_write <image> <sector-size> < operations`
//!
//! The image is opened read-write as a device of `<sector-size>`-byte logical sectors and mounted
//! read-write. Each line of standard input is one operation, its words separated by spaces, paths
//! absolute; `write_compare.py` documents the language and holds the model each sequence must
//! produce. After the last operation the volume is unmounted and the program prints `APPLIED` and
//! the number of operations. When an operation fails it stops there: it prints `APPLY-ERROR`, the
//! operation's index from 0 and the crate's error, still unmounts, and exits 3, so the oracle can
//! judge what a failed operation left behind. A mount the crate refuses prints `MOUNT-ERROR` and
//! exits 2; an unmount that fails prints `UNMOUNT-ERROR` and exits 4.
//!
//! The bytes a `write` puts down are `stream(seed)`, the xorshift64 generator below, which the
//! driver computes the same way, so the model never asks `rsext4` what it wrote.
//!
//! See design/roadmap/0190-ext4-read-write/write-oracle.md.
//!
//! Name: provisional. calef names files.
use std::fs::{File, OpenOptions};
use std::io::{BufRead, Write};
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
    fn write(&mut self, buffer: &[u8], sector: SectorId, count: u32) -> Ext4Result<()> {
        let n = self.sector as usize * count as usize;
        if buffer.len() < n {
            return Err(Ext4Error::buffer_too_small(buffer.len(), n));
        }
        self.file
            .write_all_at(&buffer[..n], sector.raw() * u64::from(self.sector))
            .map_err(|_| Ext4Error::io())
    }
    fn flush(&mut self) -> Ext4Result<()> {
        self.file.sync_data().map_err(|_| Ext4Error::io())
    }
    fn geometry(&self) -> DeviceGeometry {
        DeviceGeometry::new(self.sector, self.sectors)
    }
    fn capabilities(&self) -> DeviceCapabilities {
        DeviceCapabilities {
            flush: true,
            ..DeviceCapabilities::default()
        }
    }
}

/// The time every mutation records: the corpus's own `SOURCE_DATE_EPOCH`, which is also the
/// time `e2fsck` is told it is, so no inode reads as written in the future.
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Ext4Result<Ext4Timestamp> {
        Ok(Ext4Timestamp::new(1_600_000_000, 0))
    }
}

type Fs = Ext4<Image, MountedServices<(), NoopObserver, ()>>;

/// The xorshift64 stream a `write` or `setxattr` of `seed` puts down, `length` bytes of it:
/// eight little-endian bytes per step, from a state the seed sets and that is never zero.
fn stream(seed: u64, length: usize) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    let mut out = Vec::with_capacity(length + 8);
    while out.len() < length {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }
    out.truncate(length);
    out
}

const CONTEXT: MutationContext = MutationContext::new(0, 0, 0, 0);

fn permissions(bits: u16) -> Ext4Result<FilePermissions> {
    FilePermissions::new(bits)
}

/// The parent directory's inode and the last component of an absolute path.
fn split<'a>(fs: &mut Fs, path: &'a str) -> Ext4Result<(InodeNumber, FileName<'a>)> {
    let (parent, name) = path
        .rsplit_once('/')
        .ok_or_else(|| Ext4Error::invalid_input().with_operation("oracle:path"))?;
    Ok((resolve(fs, parent)?, FileName::new(name.as_bytes())?))
}

/// The inode an absolute path names, walked one `lookup_child` at a time from the root.
fn resolve(fs: &mut Fs, path: &str) -> Ext4Result<InodeNumber> {
    let mut at = fs.root_inode();
    for component in path.split('/').filter(|c| !c.is_empty()) {
        at = fs
            .lookup_child(at, FileName::new(component.as_bytes())?)?
            .ok_or_else(|| Ext4Error::not_found().with_operation("oracle:resolve"))?
            .number;
    }
    Ok(at)
}

fn attribute_name(full: &str) -> Ext4Result<(XattrNamespace, &str)> {
    for (prefix, namespace) in [
        ("user.", XattrNamespace::User),
        ("trusted.", XattrNamespace::Trusted),
        ("security.", XattrNamespace::Security),
    ] {
        if let Some(name) = full.strip_prefix(prefix) {
            return Ok((namespace, name));
        }
    }
    Err(Ext4Error::invalid_input().with_operation("oracle:attribute_namespace"))
}

fn number(word: Option<&&str>) -> Ext4Result<u64> {
    word.and_then(|w| w.parse().ok())
        .ok_or_else(|| Ext4Error::invalid_input().with_operation("oracle:number"))
}

fn word<'a>(words: &[&'a str], index: usize) -> Ext4Result<&'a str> {
    words
        .get(index)
        .copied()
        .ok_or_else(|| Ext4Error::invalid_input().with_operation("oracle:missing_word"))
}

/// Remove one name, and reclaim the inode when that was its last link, as a VFS does once it
/// holds no reference.
fn unlink(fs: &mut Fs, path: &str, directory: bool) -> Ext4Result<()> {
    let (parent, name) = split(fs, path)?;
    let outcome = if directory {
        fs.remove_empty_directory(parent, name)?
    } else {
        fs.unlink(parent, name)?
    };
    if outcome.requires_reap() {
        fs.reap_unlinked_inode(outcome.inode)?;
    }
    Ok(())
}

fn apply(fs: &mut Fs, line: &str) -> Ext4Result<()> {
    let words: Vec<&str> = line.split(' ').collect();
    match word(&words, 0)? {
        "mkdir" => {
            let (parent, name) = split(fs, word(&words, 1)?)?;
            fs.create_directory(CONTEXT, parent, name, permissions(0o755)?)?;
        }
        "create" => {
            let (parent, name) = split(fs, word(&words, 1)?)?;
            fs.create_regular_file(CONTEXT, parent, name, permissions(0o644)?)?;
        }
        "symlink" => {
            let (parent, name) = split(fs, word(&words, 1)?)?;
            fs.create_symlink(CONTEXT, parent, name, word(&words, 2)?.as_bytes())?;
        }
        "link" => {
            let target = resolve(fs, word(&words, 1)?)?;
            let (parent, name) = split(fs, word(&words, 2)?)?;
            fs.hard_link(target, parent, name)?;
        }
        "write" => {
            let inode = resolve(fs, word(&words, 1)?)?;
            let offset = number(words.get(2))?;
            let bytes = stream(number(words.get(4))?, number(words.get(3))? as usize);
            fs.write_inode(inode, offset, &bytes)?;
        }
        "stripes" => {
            let inode = resolve(fs, word(&words, 1)?)?;
            let count = number(words.get(2))?;
            let stride = number(words.get(3))?;
            let length = number(words.get(4))? as usize;
            let seed = number(words.get(5))?;
            for i in 0..count {
                fs.write_inode(inode, i * stride, &stream(seed + i, length))?;
            }
        }
        "truncate" => {
            let inode = resolve(fs, word(&words, 1)?)?;
            fs.truncate_inode(inode, number(words.get(2))?)?;
        }
        "rename" => {
            let (old_parent, old_name) = split(fs, word(&words, 1)?)?;
            let (new_parent, new_name) = split(fs, word(&words, 2)?)?;
            let outcome = fs.rename(
                old_parent,
                old_name,
                new_parent,
                new_name,
                RenameOptions::REPLACE,
            )?;
            if let Some(replaced) = outcome.replaced
                && replaced.requires_reap()
            {
                fs.reap_unlinked_inode(replaced.inode)?;
            }
        }
        "unlink" => unlink(fs, word(&words, 1)?, false)?,
        "rmdir" => unlink(fs, word(&words, 1)?, true)?,
        "fill" => {
            let directory = resolve(fs, word(&words, 1)?)?;
            let prefix = word(&words, 3)?;
            let data = match (words.get(4), words.get(5)) {
                (None, None) => None,
                (length, seed) => Some((number(length)? as usize, number(seed)?)),
            };
            for i in 0..number(words.get(2))? {
                let name = format!("{prefix}{i:05}");
                let file = fs.create_regular_file(
                    CONTEXT,
                    directory,
                    FileName::new(name.as_bytes())?,
                    permissions(0o644)?,
                )?;
                if let Some((length, seed)) = data {
                    fs.write_inode(file.number, 0, &stream(seed + i, length))?;
                }
            }
        }
        "setxattr" => {
            let inode = resolve(fs, word(&words, 1)?)?;
            let (namespace, name) = attribute_name(word(&words, 2)?)?;
            let value = stream(number(words.get(4))?, number(words.get(3))? as usize);
            fs.set_xattr(
                inode,
                namespace,
                name.as_bytes(),
                &value,
                XattrSetMode::Upsert,
            )?;
        }
        "removexattr" => {
            let inode = resolve(fs, word(&words, 1)?)?;
            let (namespace, name) = attribute_name(word(&words, 2)?)?;
            fs.remove_xattr(inode, namespace, name.as_bytes())?;
        }
        "sync" => fs.sync()?,
        _ => return Err(Ext4Error::invalid_input().with_operation("oracle:unknown_operation")),
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: ext4_oracle_write <image> <sector-size> < operations");
        std::process::exit(64);
    }
    let sector: u32 = args[2]
        .parse()
        .expect("the sector size is a number of bytes");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&args[1])
        .expect("the image opens for reading and writing");
    let sectors = file.metadata().expect("the image has a length").len() / u64::from(sector);
    let image = Image {
        file,
        sector,
        sectors,
    };
    let services = MountServices::new(FixedClock, (), NoopObserver);
    let mut fs = match Ext4::mount(image, services, MountOptions::read_write()) {
        Ok(fs) => fs,
        Err(e) => {
            println!("MOUNT-ERROR\t{e:?}");
            std::process::exit(2);
        }
    };
    let operations: Vec<String> = std::io::stdin()
        .lock()
        .lines()
        .map(|l| l.expect("the operations are text"))
        .filter(|l| !l.is_empty())
        .collect();
    let mut out = std::io::stdout().lock();
    let mut status = 0;
    let mut applied = operations.len();
    for (index, line) in operations.iter().enumerate() {
        if let Err(e) = apply(&mut fs, line) {
            writeln!(out, "APPLY-ERROR\t{index}\t{e:?}").unwrap();
            status = 3;
            applied = index;
            break;
        }
    }
    if let Err(e) = fs.unmount() {
        writeln!(out, "UNMOUNT-ERROR\t{e:?}").unwrap();
        std::process::exit(4);
    }
    if status == 0 {
        writeln!(out, "APPLIED\t{applied}").unwrap();
    }
    std::process::exit(status);
}
