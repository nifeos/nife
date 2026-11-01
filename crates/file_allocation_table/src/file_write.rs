//! **Writing a file's bytes**: a cursor that turns a position into a sector, extends the chain when
//! the position reaches its end, and keeps the size the directory entry must state.
//!
//! The core is [`WriteState`], written against [`Medium`] (the table plus the volume's data
//! sectors), so `verification.rs` can write and read back over a model whose every size Kani can
//! unroll. [`crate::FileWriter`] wraps it with the directory entry: after a call that changed the
//! size or the first cluster, the entry is rewritten, so the order on the medium is clusters
//! linked, bytes written, then the size. A crash in between leaves clusters past the size, which
//! `fsck` reports and no reader follows.
//!
//! Name: provisional. The writer's module, beside `file`, which reads.

use crate::allocation::LinksMut;
use crate::boot_sector::BootSector;
use crate::reader::{ChainError, is_data};
use crate::table::TableEntry;

/// **Why a write, a create or a delete stopped.**
///
/// Name: provisional. The one error of every operation that changes a volume above the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WriteError<E> {
    /// The chain could not be followed, or a sector could not be read or written.
    ///
    /// Name: provisional. The chain's own error, unchanged.
    Chain(ChainError<E>),
    /// Every data cluster is in use.
    ///
    /// Name: provisional. Beside `AllocationError::VolumeFull`.
    VolumeFull,
    /// The write would pass 4 GiB less one byte, the most a FAT file holds.
    ///
    /// Name: provisional. The condition.
    FileTooLarge,
    /// The chain ends before the size the entry states: the volume is damaged.
    ///
    /// Name: provisional. As `FileReadError::ShorterThanItsSize`.
    ShorterThanItsSize,
    /// The slot the entry names no longer holds that entry.
    ///
    /// Name: provisional. The entry went stale, because the directory changed under it.
    EntryChanged,
    /// An earlier write failed, so this writer does nothing more rather than skip bytes.
    ///
    /// Name: provisional. As `FileReadError::AfterAFailure`.
    AfterAFailure,
    /// The directory has no free slot and cannot grow: the fixed root of FAT12 and FAT16, or a
    /// directory at 65,536 entries.
    ///
    /// Name: provisional. The condition.
    DirectoryFull,
    /// The name is not a legal short name.
    ///
    /// Name: provisional. The condition.
    InvalidName,
    /// The directory already holds an entry with that short name.
    ///
    /// Name: provisional. The condition.
    AlreadyExists,
    /// The entry is not a file: it is a directory or the volume label.
    ///
    /// Name: provisional. As `NotAFile`.
    NotAFile,
    /// The entry is not a directory a caller may remove: a file, `.` or `..`.
    ///
    /// Name: provisional. The condition.
    NotADirectory,
    /// The file carries the read-only attribute, so it cannot be written, resized or removed until
    /// [`crate::WritableVolume::set_attributes`] clears the bit.
    ///
    /// Name: provisional. The condition, as DOS's "access denied".
    ReadOnly,
    /// The directory holds entries other than `.` and `..`.
    ///
    /// Name: provisional. The condition.
    DirectoryNotEmpty,
    /// A directory cannot move into itself or below itself.
    ///
    /// Name: provisional. The condition; it would cut the tree off from the root.
    MovedIntoItself,
}

impl<E> From<ChainError<E>> for WriteError<E> {
    fn from(error: ChainError<E>) -> Self {
        WriteError::Chain(error)
    }
}

/// **The table and the data sectors a file write changes.**
pub(crate) trait Medium: LinksMut {
    /// Read volume-relative `sector`, which is one sector long.
    fn read_data(&mut self, sector: u32, out: &mut [u8]) -> Result<(), ChainError<Self::Error>>;
    /// Write volume-relative `sector` from `data`, which is one sector long.
    fn write_data(&mut self, sector: u32, data: &[u8]) -> Result<(), ChainError<Self::Error>>;

    /// Merge `bytes` into a sector buffer as [`merge_partial`] does. A medium keeps this default;
    /// the chain proof overrides it, because it proves the arithmetic apart from the bytes.
    fn merge(&mut self, buffer: &mut [u8], kept: usize, offset: usize, bytes: &[u8]) {
        merge_partial(buffer, kept, offset, bytes);
    }
}

/// **Put `bytes` at `offset` in a sector buffer** that holds `kept` bytes of the file, and zero the
/// rest. The head `0..kept` keeps what the sector held, the slack from `kept` on becomes zero so a
/// stale cluster's bytes never reach the file, and `bytes` lands over both. A writer never starts
/// past the end of the file, so `offset <= kept`.
pub(crate) fn merge_partial(buffer: &mut [u8], kept: usize, offset: usize, bytes: &[u8]) {
    let kept = kept.min(buffer.len());
    buffer[kept..].fill(0);
    buffer[offset..offset + bytes.len()].copy_from_slice(bytes);
}

/// **Where a file write stands**, apart from the volume.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WriteState {
    pub(crate) first_cluster: u32,
    pub(crate) size: u32,
    pub(crate) position: u32,
    /// The index in the chain and the number of the cluster the last call reached.
    reached: Option<(u32, u32)>,
}

impl WriteState {
    /// A writer at the start of a file of `size` bytes whose chain starts at `first_cluster`.
    pub(crate) const fn new(first_cluster: u32, size: u32) -> WriteState {
        WriteState {
            first_cluster,
            size,
            position: 0,
            reached: None,
        }
    }

    /// Move to `position`, or to the end when it is past it.
    pub(crate) fn seek(&mut self, position: u32) {
        self.position = position.min(self.size);
    }

    /// Forget the cursor: the chain behind it changed.
    pub(crate) fn forget_cursor(&mut self) {
        self.reached = None;
    }

    /// **The cluster at chain index `index`**, extending the chain by whole clusters when the
    /// index is the one past its end and the size allows it. `Ok(None)` means the volume is full.
    fn reach<M: Medium>(
        &mut self,
        medium: &mut M,
        boot: &BootSector,
        index: u32,
    ) -> Result<Option<u32>, WriteError<M::Error>> {
        if let Some((at, cluster)) = self.reached
            && at == index
        {
            return Ok(Some(cluster));
        }
        let clusters = medium.clusters();
        let per_cluster = u64::from(boot.bytes_per_cluster());
        let (mut at, mut cluster) = match self.reached {
            Some((at, cluster)) if at < index => (at, cluster),
            _ if self.first_cluster != 0 => {
                if !is_data(clusters, self.first_cluster) {
                    return Err(ChainError::NotADataCluster(self.first_cluster).into());
                }
                (0, self.first_cluster)
            }
            _ => {
                // An empty file: only a size of 0 agrees with having no chain.
                if self.size != 0 {
                    return Err(WriteError::ShorterThanItsSize);
                }
                let Some(first) = medium.append(None)? else {
                    return Ok(None);
                };
                self.first_cluster = first;
                (0, first)
            }
        };
        let mut steps = 0;
        while at < index {
            steps += 1;
            if steps > clusters {
                return Err(ChainError::TooLong.into());
            }
            match medium.table_entry(cluster)? {
                TableEntry::Next(next) if is_data(clusters, next) => cluster = next,
                TableEntry::EndOfChain => {
                    // The chain may grow only past the clusters the size already needs.
                    if u64::from(self.size) > (u64::from(at) + 1) * per_cluster {
                        return Err(WriteError::ShorterThanItsSize);
                    }
                    let Some(next) = medium.append(Some(cluster))? else {
                        self.reached = Some((at, cluster));
                        return Ok(None);
                    };
                    cluster = next;
                }
                entry => return Err(ChainError::Broken { cluster, entry }.into()),
            }
            at += 1;
        }
        self.reached = Some((at, cluster));
        Ok(Some(cluster))
    }

    /// **Write `data` at the position**, extending the file and its chain as needed, and return
    /// how many bytes were written. A volume that fills part way returns the bytes that fit; the
    /// next call returns `VolumeFull`. `scratch` is one sector long.
    pub(crate) fn write<M: Medium>(
        &mut self,
        medium: &mut M,
        boot: &BootSector,
        data: &[u8],
        scratch: &mut [u8],
    ) -> Result<usize, WriteError<M::Error>> {
        let sector_bytes = boot.sector_size().bytes();
        let per_cluster = boot.bytes_per_cluster();
        let room = u64::from(u32::MAX - self.position);
        let want = usize::try_from(room).map_or(data.len(), |room| data.len().min(room));
        if want == 0 && !data.is_empty() {
            return Err(WriteError::FileTooLarge);
        }
        let mut done = 0;
        let mut passes = 0usize;
        while done < want {
            // Every pass writes at least one byte, so a pass count past `want` is a stuck loop.
            passes += 1;
            debug_assert!(
                passes <= want && done < want,
                "a write pass made no progress"
            );
            let Some(cluster) = self.reach(medium, boot, self.position / per_cluster)? else {
                if done > 0 {
                    break;
                }
                return Err(WriteError::VolumeFull);
            };
            let within = self.position % per_cluster;
            let sector = boot
                .cluster_first_sector(cluster)
                .and_then(|first| first.checked_add(within / sector_bytes as u32))
                .ok_or(ChainError::NotADataCluster(cluster))?;
            let offset = (within % sector_bytes as u32) as usize;
            let len = (sector_bytes - offset).min(want - done);
            if offset == 0 && len == sector_bytes {
                medium.write_data(sector, &data[done..done + len])?;
            } else {
                let buffer = &mut scratch[..sector_bytes];
                let sector_start = self.position - offset as u32;
                let kept = if sector_start >= self.size {
                    0
                } else {
                    medium.read_data(sector, buffer)?;
                    ((self.size - sector_start) as usize).min(sector_bytes)
                };
                medium.merge(buffer, kept, offset, &data[done..done + len]);
                medium.write_data(sector, buffer)?;
            }
            self.position += len as u32;
            done += len;
            self.size = self.size.max(self.position);
        }
        Ok(done)
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
