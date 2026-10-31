// The std probe for rufs: mount over an in-memory backend, then create, write, read, link, list
// attributes and unlink, so the linker has to resolve both the read and the write path. The image
// is zeroes; this probe measures the build, not a mount.
use std::{ffi::OsStr, hint::black_box, io::Cursor};

use rufs::{BlockReader, InodeNum, InodeType, Ufs};

fn exercise(image: Vec<u8>) -> std::io::Result<usize> {
    let mut fs = Ufs::new(BlockReader::new(Cursor::new(image), 4096, true))?;
    let root = InodeNum::ROOT;
    let attr = fs.mknod(root, OsStr::new("a"), InodeType::RegularFile, 0o644, 0, 0)?;
    fs.inode_write(attr.inr, 0, b"nife")?;
    let mut buf = [0u8; 4];
    let n = fs.inode_read(attr.inr, 0, &mut buf)?;
    fs.symlink(root, OsStr::new("l"), OsStr::new("a"), 0, 0)?;
    fs.mkdir(root, OsStr::new("d"), 0o755, 0, 0)?;
    fs.rename(root, OsStr::new("b"), root, OsStr::new("a"), false)?;
    let x = fs.xattr_list(attr.inr)?;
    fs.inode_truncate(attr.inr, 0)?;
    fs.unlink(root, OsStr::new("b"))?;
    Ok(n + x.len() + fs.info().bsize as usize)
}

fn main() {
    println!("{:?}", exercise(black_box(vec![0u8; 1 << 20])));
}
