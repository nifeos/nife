// The no_std probe for the rufs port: mount over an in-memory backend that implements the port's
// own Read, Write and Seek, then create, write, read, link, rename, list attributes, truncate and
// unlink, so the linker resolves the read and the write path against core and alloc alone.
#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;

use rufs::{
    ffi::OsStr,
    io::{self, Read, Seek, SeekFrom, Write},
    BlockReader, InodeNum, InodeType, Ufs,
};

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
        if let SeekFrom::Start(n) = pos {
            self.pos = n as usize;
        }
        Ok(self.pos as u64)
    }
}

fn exercise(bytes: Vec<u8>) -> io::Result<usize> {
    let mut fs = Ufs::new(BlockReader::new(Mem { bytes, pos: 0 }, 4096, true))?;
    let root = InodeNum::ROOT;
    let attr = fs.mknod(root, OsStr::new("a"), InodeType::RegularFile, 0o644, 0, 0)?;
    fs.inode_write(attr.inr, 0, b"nife")?;
    let mut buf = [0u8; 4];
    let n = fs.inode_read(attr.inr, 0, &mut buf)?;
    fs.symlink(root, OsStr::new("l"), OsStr::new("a"), 0, 0)?;
    fs.mkdir(root, OsStr::new("d"), 0o755, 0, 0)?;
    fs.rename(root, OsStr::new("b"), root, OsStr::new("a"), true)?;
    let x = fs.xattr_list(attr.inr)?;
    fs.inode_truncate(attr.inr, 0)?;
    fs.unlink(root, OsStr::new("b"))?;
    Ok(n + x.len() + fs.info().bsize as usize)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let image = core::hint::black_box(alloc::vec![0u8; 1 << 20]);
    core::hint::black_box(exercise(image).is_ok());
    halt()
}
