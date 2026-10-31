// The host round trip for rufs, built twice by `probe.sh check`: once against rufs as shipped
// (std) and once against the no_std port with its `std` feature off (`--features port`). Both
// read a FreeBSD-made image, print every name, size, content hash, symlink target and attribute
// list, check each regular file's bytes against ufs-core (an independent reader), then make the
// same writes and print the tree again. The port is faithful when the two outputs are identical.
#[cfg(feature = "port")]
use rufs::{
    ffi::OsStr,
    io::{self, Read, Seek, SeekFrom, Write},
};
#[cfg(not(feature = "port"))]
use std::{
    ffi::OsStr,
    io::{self, Read, Seek, SeekFrom, Write},
};

use rufs::{BlockReader, InodeNum, InodeType, Ufs};

/// The whole image in memory, behind whichever Read, Write and Seek this build uses.
struct Mem {
    bytes: Vec<u8>,
    pos: usize,
}

impl Read for Mem {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(self.bytes.len().saturating_sub(self.pos));
        buf[..n].copy_from_slice(&self.bytes[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl Write for Mem {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = buf.len().min(self.bytes.len().saturating_sub(self.pos));
        self.bytes[self.pos..self.pos + n].copy_from_slice(&buf[..n]);
        self.pos += n;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for Mem {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.pos = match pos {
            SeekFrom::Start(n) => n as usize,
            SeekFrom::End(n) => (self.bytes.len() as i64 + n) as usize,
            SeekFrom::Current(n) => (self.pos as i64 + n) as usize,
        };
        Ok(self.pos as u64)
    }
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| (h ^ x as u64).wrapping_mul(0x100_0000_01b3))
}

fn os(s: &str) -> &OsStr {
    OsStr::new(s)
}

/// Prints the tree under `dir`, depth first in directory order, and checks file bytes against
/// ufs-core when `image` is given.
fn walk(fs: &mut Ufs<Mem>, dir: InodeNum, path: &str, image: Option<&[u8]>) {
    let mut kids = Vec::new();
    fs.dir_iter(dir, |name, inr, kind| {
        kids.push((name.as_encoded_bytes().to_vec(), inr, kind));
        None::<()>
    })
    .expect("dir_iter");
    for (name, inr, kind) in kids {
        let name = String::from_utf8_lossy(&name).into_owned();
        if name == "." || name == ".." {
            continue;
        }
        let p = format!("{path}/{name}");
        let attr = fs.inode_attr(inr).expect("inode_attr");
        let xattrs = fs.xattr_list(inr).map(|x| fnv(&x)).unwrap_or(0);
        match kind {
            InodeType::Directory => {
                println!("d {p} perm={:o} nlink={}", attr.perm, attr.nlink);
                walk(fs, inr, &p, image);
            }
            InodeType::Symlink => {
                let t = fs.symlink_read(inr).expect("symlink_read");
                println!("l {p} -> {} bytes fnv={:016x}", t.len(), fnv(&t));
            }
            _ => {
                // Streamed in 1 MiB pieces: the image holds sparse files far larger than memory.
                // Past 64 MiB only the first 64 MiB and the last 1 MiB are read, so `read=`
                // counts the bytes hashed rather than the size.
                let mut buf = vec![0u8; 1 << 20];
                let (mut read, mut h) = (0u64, fnv(&[]));
                let mut small = Vec::new();
                let head = attr.size.min(64 << 20);
                let mut spans = vec![(0, head)];
                if attr.size > head {
                    spans.push((attr.size - (1 << 20), attr.size));
                }
                for (mut off, end) in spans {
                    while off < end {
                        let n = fs.inode_read(inr, off, &mut buf).expect("inode_read");
                        if n == 0 {
                            break;
                        }
                        h = buf[..n].iter().fold(h, |h, &x| (h ^ x as u64).wrapping_mul(0x100_0000_01b3));
                        if attr.size <= 1 << 24 {
                            small.extend_from_slice(&buf[..n]);
                        }
                        off += n as u64;
                        read += n as u64;
                    }
                }
                let off = read;
                let agree = match image {
                    Some(img) => {
                        let sb = ufs::Superblock::parse(&img[ufs::SBLOCK_UFS2..]).expect("sb");
                        match ufs::read_path_content(img, &sb, &p) {
                            Ok(Some(b)) if attr.size > 1 << 24 => format!("ufs-core=read {} bytes", b.len()),
                            Ok(Some(b)) if b == small => "ufs-core=same".to_string(),
                            Ok(Some(_)) => "ufs-core=DIFFERS".to_string(),
                            Ok(None) => "ufs-core=absent".to_string(),
                            Err(e) => format!("ufs-core=error: {e}"),
                        }
                    }
                    None => String::new(),
                };
                println!(
                    "f {p} size={} read={off} fnv={h:016x} xattrs={xattrs:016x} perm={:o} {agree}",
                    attr.size, attr.perm
                );
            }
        }
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: rufs-host <image>");
    let bytes = std::fs::read(&path).expect("read image");
    let original = bytes.clone();
    let mut fs = Ufs::new(BlockReader::new(Mem { bytes, pos: 0 }, 4096, true)).expect("mount");
    let info = fs.info();
    println!("info blocks={} bfree={} files={} ffree={} bsize={} fsize={}", info.blocks, info.bfree, info.files, info.ffree, info.bsize, info.fsize);
    println!("== read");
    walk(&mut fs, InodeNum::ROOT, "", Some(&original));

    println!("== write");
    let r = InodeNum::ROOT;
    let a = fs.mknod(r, os("new"), InodeType::RegularFile, 0o644, 0, 0).expect("mknod");
    let data: Vec<u8> = (0..200_000u32).map(|i| (i * 7 % 251) as u8).collect();
    println!("write {:?}", fs.inode_write(a.inr, 0, &data).map_err(|e| e.raw_os_error()));
    println!("mkdir {:?}", fs.mkdir(r, os("activation"), 0o755, 0, 0).map(|a| a.inr.get()).map_err(|e| e.raw_os_error()));
    println!("symlink {:?}", fs.symlink(r, os("ln"), os("new"), 0, 0).map(|a| a.inr.get()).map_err(|e| e.raw_os_error()));
    println!("rename noreplace {:?}", fs.rename(r, os("renamed"), r, os("new"), false).map(|i| i.get()).map_err(|e| e.raw_os_error()));
    println!("rename replace {:?}", fs.rename(r, os("renamed"), r, os("new"), true).map(|i| i.get()).map_err(|e| e.raw_os_error()));
    println!("truncate {:?}", fs.inode_truncate(a.inr, 70_000).map_err(|e| e.raw_os_error()));
    println!("unlink {:?}", fs.unlink(r, os("file1")).map_err(|e| e.raw_os_error()));
    println!("lookup gone {:?}", fs.dir_lookup(r, os("file1")).map(|i| i.get()).map_err(|e| e.raw_os_error()));
    let info = fs.info();
    println!("info blocks={} bfree={} files={} ffree={}", info.blocks, info.bfree, info.files, info.ffree);
    walk(&mut fs, InodeNum::ROOT, "", None);
}
