//! **What the server's host tests share**: an in-memory [`BlockIo`], and images built with a file
//! or a tree in them.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// A [`BlockIo`] over an in-memory image, so the exact chunking the EL0 binary uses is exercised
/// on the host. Byte for byte it must behave like `DiskMemory`.
pub(crate) struct VecIo(pub(crate) Vec<u8>);

impl BlockIo for VecIo {
    fn read_block(&mut self, block: u64, buf: &mut [u8; BLOCK]) -> Result<()> {
        let off = block as usize * BLOCK;
        buf.copy_from_slice(&self.0[off..off + BLOCK]);
        Ok(())
    }
    fn write_block(&mut self, block: u64, buf: &[u8; BLOCK]) -> Result<()> {
        let off = block as usize * BLOCK;
        self.0[off..off + BLOCK].copy_from_slice(buf);
        Ok(())
    }
    fn size_bytes(&mut self) -> Result<u64> {
        Ok(self.0.len() as u64)
    }
}

/// The disk behind [`server_with`], as a standalone [`DiskMemory`], so a test can open it, drop
/// the server, and reopen the same disk to prove persistence across a close.
pub(super) fn image_with(files: &[(&str, &[u8])]) -> DiskMemory {
    let disk = DiskMemory::new(16 * 1024 * 1024);
    let mut fs = FileSystem::create(disk, None, 0, 0).expect("create");
    fs.tx(|transaction| {
        for (name, data) in files {
            let ptr = transaction
                .create_node(TreePtr::root(), name, Node::MODE_FILE | 0o644, 0, 0)?
                .ptr();
            transaction.write_node(ptr, 0, data, 0, 0)?;
        }
        Ok(())
    })
    .expect("populate");
    fs.disk
}

/// Build an image with a subtree: files in the root, then `sub/` holding `inner` and `deeper/`,
/// and a sibling `other/` holding `secret`. The same shape `filesystem_fixture::tree` describes
/// and the device fixture carries, so the host tests and the guest tests attack one geometry.
pub(super) fn image_with_tree() -> DiskMemory {
    let disk = DiskMemory::new(16 * 1024 * 1024);
    let mut fs = FileSystem::create(disk, None, 0, 0).expect("create");
    fs.tx(|transaction| {
        let file = |transaction: &mut redoxfs::Transaction<DiskMemory>,
                    parent: TreePtr<Node>,
                    name: &str,
                    body: &[u8]| {
            let ptr = transaction
                .create_node(parent, name, Node::MODE_FILE | 0o644, 0, 0)?
                .ptr();
            transaction.write_node(ptr, 0, body, 0, 0)?;
            Ok::<(), Error>(())
        };
        file(transaction, TreePtr::root(), "motd", b"root motd")?;
        let sub = transaction
            .create_node(TreePtr::root(), "sub", Node::MODE_DIR | 0o755, 0, 0)?
            .ptr();
        file(transaction, sub, "inner", b"inside the grant")?;
        let deeper = transaction
            .create_node(sub, "deeper", Node::MODE_DIR | 0o755, 0, 0)?
            .ptr();
        file(transaction, deeper, "leaf", b"two levels down")?;
        let other = transaction
            .create_node(TreePtr::root(), "other", Node::MODE_DIR | 0o755, 0, 0)?
            .ptr();
        file(transaction, other, "secret", b"in the sibling")?;
        Ok(())
    })
    .expect("populate the tree");
    fs.disk
}

/// Collect a whole directory listing through `read_dir`, driving the cursor the way a client
/// does, so the cursor and the "a record is never split" rule are exercised rather than assumed.
pub(super) fn list(
    srv: &mut Server<DiskMemory>,
    handle: u32,
    buf_len: usize,
) -> Result<Vec<(String, bool)>> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; buf_len];
    let mut cursor = 0u32;
    // Bounded so a cursor that fails to advance fails the test instead of hanging it.
    for _ in 0..64 {
        let n = srv.read_dir(handle, cursor, &mut buf)?;
        if n == 0 {
            return Ok(out);
        }
        for (name, is_dir) in filesystem_contract::dirent::iter(&buf[..n]) {
            out.push((String::from_utf8(name.to_vec()).unwrap(), is_dir));
            cursor += 1;
        }
    }
    panic!("read_dir never reported the end of the directory");
}
