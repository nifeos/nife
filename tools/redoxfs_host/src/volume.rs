//! **A volume: the whole file, or one partition inside it**, and the windowed disk the engine is
//! handed for it, with the opens that resolve one.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Which partition to read, when the path names a device rather than an image.
///
/// Two forms, because they answer different questions and both get asked. **PROVISIONAL naming**
/// (CLAUDE.md: names are an architect's call).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartitionSelector {
    /// The partition number a person reads off a listing: 1-based, the `2` in `/dev/sda2`, in
    /// macOS's `disk0s2`, and in `sgdisk -i 2`. It is the array index plus one, and it survives a
    /// hole in the array, so it means the same thing here as it does in every other partition tool.
    Index(u32),
    /// The partition's **type GUID**, which is how nife finds its own data partition on the
    /// board (`redoxfs_server/src/bin/mkfs.rs`, DECISIONS §45). A slot number is a fact about one disk's
    /// current layout; a type GUID is a fact about what the partition *is*, so a script should ask
    /// this way and a person with a listing in front of them should not have to.
    ///
    /// **Not the partition name.** GPT names are cosmetic and frequently absent: macOS writes none
    /// at all (notes/globally-unique-identifier-partition-table.md), so a selector keyed on one
    /// would fail on exactly the disk the recovery story is about.
    Type(Uuid),
}

/// Where a filesystem is: the whole file, or one partition inside it.
///
/// `Volume::image` is what this tool always did, and it is still the right thing for an image file,
/// where offset zero is the filesystem. `Volume::partition` is milestone 110: a device has a
/// partition table at offset zero instead, and the filesystem starts wherever the table says.
#[derive(Clone, Copy)]
pub struct Volume<'a> {
    /// The image file or device to open.
    pub path: &'a Path,
    /// `None` means the file *is* the filesystem.
    pub partition: Option<PartitionSelector>,
}

impl<'a> Volume<'a> {
    /// A whole-file image: the bytes at offset zero are the filesystem.
    pub fn image(path: &'a Path) -> Volume<'a> {
        Volume {
            path,
            partition: None,
        }
    }

    /// One partition of a device (or of a whole-device image, which is the same bytes in a file).
    pub fn partition(device: &'a Path, selector: PartitionSelector) -> Volume<'a> {
        Volume {
            path: device,
            partition: Some(selector),
        }
    }
}

/// So `ls(&img, "/")` still says what it always said for an image file.
impl<'a> From<&'a Path> for Volume<'a> {
    fn from(path: &'a Path) -> Volume<'a> {
        Volume::image(path)
    }
}

/// A [`Disk`] that is a window onto a file: block zero is `first_block` of the file, and the disk
/// is `blocks` long.
///
/// **The window is what keeps the engine inside the partition**, by construction rather than by the
/// engine's good behaviour: every address it computes is relative to block zero, and `size` reports
/// the partition's length, which is what its allocator sizes itself from. An address past the end is
/// `EIO` rather than a read of the next partition. This is `redoxfs_server/src/bin/mkfs.rs`'s
/// `PartitionDisk` with a file underneath instead of the block-service IPC; the two are deliberately
/// separate, because that one is `no_std` inside a `[[bin]]` on the board and this one wraps
/// `DiskFile`.
///
/// A whole-file volume is the same type with `first_block: 0` and no length, which delegates `size`
/// to the file. **That case must not be turned into a block count**: a raw device on macOS reports a
/// length of zero (see `Volume::window`), and computing a size from it would hand the engine a
/// zero-block disk.
pub struct PartitionDisk {
    file: DiskFile,
    /// Where the window starts, in RedoxFS blocks from the start of the file.
    first_block: u64,
    /// How long the window is, or `None` for "the whole file", which is the image case.
    blocks: Option<u64>,
}

impl PartitionDisk {
    /// The file block a filesystem block lands on, given a read or write of `len` bytes, or `EIO` if
    /// any part of it would fall outside the window.
    fn file_block(&self, block: u64, len: usize) -> Result<u64, Error> {
        if let Some(blocks) = self.blocks {
            let span = (len as u64).div_ceil(BLOCK_SIZE);
            let end = block.checked_add(span).ok_or(Error::new(EIO))?;
            if end > blocks {
                return Err(Error::new(EIO));
            }
        }
        block.checked_add(self.first_block).ok_or(Error::new(EIO))
    }
}

impl Disk for PartitionDisk {
    unsafe fn read_at(&mut self, block: u64, buffer: &mut [u8]) -> syscall::error::Result<usize> {
        let at = self.file_block(block, buffer.len())?;
        // SAFETY: the contract is `DiskFile`'s, which this forwards to unchanged.
        unsafe { self.file.read_at(at, buffer) }
    }

    unsafe fn write_at(&mut self, block: u64, buffer: &[u8]) -> syscall::error::Result<usize> {
        let at = self.file_block(block, buffer.len())?;
        // SAFETY: as above.
        unsafe { self.file.write_at(at, buffer) }
    }

    fn size(&mut self) -> syscall::error::Result<u64> {
        match self.blocks {
            Some(blocks) => Ok(blocks * BLOCK_SIZE),
            None => self.file.size(),
        }
    }
}

impl Volume<'_> {
    /// Resolve this volume to a window on the open file: `(first block, length in blocks)` in
    /// RedoxFS blocks, with `None` for "the whole file".
    ///
    /// Three refusals here are worth naming where a reader meets them:
    ///
    /// - **A partition whose first byte is not on a 4096-byte boundary** is refused rather than
    ///   rounded into. That filesystem would read back correctly only through a disk offset by the
    ///   same fraction, which is the worst failure available; the board's `mkfs` refuses to *create*
    ///   one for the same reason.
    /// - **A partition that runs past the end of a regular file** is refused, because that is a
    ///   truncated dump and half-reading one produces a corrupt recovery that looks like a real one.
    ///   Only for a regular file: **a raw device reports a length of zero on macOS**, so the bound
    ///   does not exist to be checked against and applying it would refuse every real disk.
    /// - **A partition shorter than one filesystem block** is refused; there is nothing to open.
    ///
    /// The length is *floored* to whole blocks rather than refused, unlike creation: a partition
    /// whose length is not a multiple of 4096 holds a filesystem that reads perfectly, plus a tail
    /// the engine never addresses.
    fn window(&self, file: &File, path: &Path) -> Result<(u64, Option<u64>), String> {
        let Some(selector) = self.partition else {
            return Ok((0, None));
        };
        let head = read_head(file, path)?;
        let (table, block_size) = parse_table(&head, path)?;
        let lba = block_size as u64;

        let entry = match selector {
            PartitionSelector::Index(n) => {
                let index = n.checked_sub(1).ok_or_else(|| {
                    format!(
                        "{}: partition numbers start at 1, the way every partition tool prints them",
                        path.display()
                    )
                })?;
                match table.entry(index as usize) {
                    Some(e) if e.is_used() => e,
                    Some(_) => {
                        return Err(format!(
                            "{}: partition {n} is an empty slot in the table",
                            path.display()
                        ));
                    }
                    None => {
                        return Err(format!(
                            "{}: this table has {} entries, so there is no partition {n}",
                            path.display(),
                            table.entry_count(),
                        ));
                    }
                }
            }
            PartitionSelector::Type(want) => table
                .partitions()
                .map(|(_, e)| e)
                .find(|e| e.type_guid == want)
                .ok_or_else(|| {
                    format!(
                        "{}: no partition of type {want:X} on this disk",
                        path.display(),
                    )
                })?,
        };

        let start = entry.first_lba * lba;
        // `GloballyUniqueIdentifierPartitionTable::parse` has already rejected an entry whose last
        // LBA is below its first, so this cannot be `None`; the arm exists so that a change there
        // cannot turn into an offset of zero.
        let len = entry
            .blocks()
            .ok_or_else(|| format!("{}: that partition has no length", path.display()))?
            * lba;

        if !start.is_multiple_of(BLOCK_SIZE) {
            return Err(format!(
                "{}: that partition starts at byte {start}, which is not a multiple of the {BLOCK_SIZE}-byte \
                 filesystem block. A filesystem there is readable only through a disk offset by the same \
                 fraction, so this refuses rather than guessing.",
                path.display(),
            ));
        }
        let blocks = len / BLOCK_SIZE;
        if blocks == 0 {
            return Err(format!(
                "{}: that partition is {len} bytes, less than one {BLOCK_SIZE}-byte filesystem block",
                path.display(),
            ));
        }

        // Only for a regular file: a raw device's metadata length is zero on macOS, and a bound of
        // zero would refuse every disk this feature exists for.
        if let Ok(meta) = file.metadata()
            && meta.is_file()
            && start + blocks * BLOCK_SIZE > meta.len()
        {
            return Err(format!(
                "{}: that partition ends at byte {}, past the end of a {}-byte file. This dump is \
                 truncated; recovering half of a filesystem would look like recovering all of it.",
                path.display(),
                start + blocks * BLOCK_SIZE,
                meta.len(),
            ));
        }

        Ok((start / BLOCK_SIZE, Some(blocks)))
    }
}

/// Open an existing volume **read-only**, for the recovery paths. No `cleanup`, so nothing is
/// replayed or tidied, and the file is opened without write permission in the first place: a disk
/// that is failing, or an image on read-only media, still reads.
///
/// Skipping `cleanup` does not weaken what is read. `FileSystem::open` picks the newest *valid*
/// header out of the ring regardless, which is the crash-consistency property itself; `cleanup`
/// only releases nodes an unclean unmount left open. Upstream's own `redoxfs-clone` reads its
/// source disk exactly this way.
pub(crate) fn open_ro(volume: Volume) -> Result<FileSystem<PartitionDisk>, String> {
    let path = volume.path;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(false)
        .open(path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    if volume.partition.is_none() {
        warn_if_partitioned(&file, path);
    }
    let (first_block, blocks) = volume.window(&file, path)?;
    let disk = PartitionDisk {
        file: DiskFile::from(file),
        first_block,
        blocks,
    };
    FileSystem::open(disk, None, None, false)
        .map_err(|e| open_failed(path, first_block * BLOCK_SIZE, e))
}

/// Explain a failed open, and in particular explain the one failure the operational rule exists to
/// prevent: a reader that does not match the format it is reading.
///
/// `Header::valid` checks the version before anything else, so an image written by a *different*
/// RedoxFS presents as no valid header anywhere in the ring, and the engine's own error for that is
/// ENOENT. "No such file or directory" for a file that is plainly there is the wrong thing to be
/// told at 2am, so when the signature is on the disk we read the version field beside it and say so.
/// See notes/host-recovery.md: this is the message that tells you to go and find the pin.
///
/// `at` is where the filesystem starts in the file, which is zero for an image and the partition's
/// first byte for a device. Reading the version from the wrong place would answer "not a RedoxFS
/// image" for every partitioned disk, which is the message this function exists to avoid giving.
pub(crate) fn open_failed(image: &Path, at: u64, err: impl std::fmt::Display) -> String {
    match on_disk_version(image, at) {
        Some(v) if v != redoxfs::VERSION => format!(
            "{}: on-disk format version {v}, but this build of redoxfs_host reads version {}. \
             A reader must match the format version it reads; recover with the RedoxFS pin the \
             backup was written by (see notes/host-recovery.md).",
            image.display(),
            redoxfs::VERSION,
        ),
        _ => format!("{} is not a RedoxFS image: {err}", image.display()),
    }
}

/// The format version recorded on the disk, if a RedoxFS signature is anywhere in the header ring's
/// range. Best effort and deliberately independent of the engine: it reads 16 bytes at a block
/// boundary, so it still works when nothing in the image parses.
pub(crate) fn on_disk_version(image: &Path, at: u64) -> Option<u64> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(image).ok()?;
    let mut buf = [0u8; 16];
    for block in 0..redoxfs::HEADER_RING {
        file.seek(SeekFrom::Start(at + block * redoxfs::BLOCK_SIZE))
            .ok()?;
        if file.read_exact(&mut buf).is_err() {
            return None;
        }
        if &buf[..8] == redoxfs::SIGNATURE {
            return Some(u64::from_le_bytes(buf[8..16].try_into().ok()?));
        }
    }
    None
}
