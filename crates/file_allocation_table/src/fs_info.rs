//! **The FAT32 `FSInfo` sector**: two hints a driver keeps about the table, parsed from hostile
//! bytes and written back without touching anything else in the sector.
//!
//! The sector holds a free-cluster count and the cluster to start looking for a free one at. The
//! specification says a driver must treat both as hints: they can be stale, unknown (all ones) or
//! plain wrong on a volume a stranger wrote. So the parse reports a value only when it could be
//! true of this volume, and nothing in the crate depends on one for correctness. The layout is
//! that of Microsoft's "FAT: General Overview of On-Disk Format", version 1.03, section 5, "FAT32
//! FSInfo Sector Structure and Backup Boot Sector".
//!
//! Name: provisional. The module for the sector `BootSector::fs_info_sector` points at.

/// `FSI_LeadSig`, "RRaA", at byte 0.
const LEAD_SIGNATURE: u32 = 0x4161_5252;
/// `FSI_StrucSig`, "rrAa", at byte 484.
const STRUCTURE_SIGNATURE: u32 = 0x6141_7272;
/// `FSI_TrailSig` at byte 508.
const TRAIL_SIGNATURE: u32 = 0xAA55_0000;
/// Where `FSI_Free_Count` starts.
const FREE_COUNT_AT: usize = 488;
/// Where `FSI_Nxt_Free` starts.
const NEXT_FREE_AT: usize = 492;
/// The value that means "not known" in both fields.
const UNKNOWN: u32 = 0xFFFF_FFFF;

/// **Why a sector is not an `FSInfo` sector.**
///
/// Name: provisional. Beside `ParseError`, which says the same of a boot sector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FsInfoError {
    /// The sector is shorter than the 512 bytes the structure spans.
    ///
    /// Name: provisional. The condition.
    TooShort,
    /// One of the three signatures is wrong.
    ///
    /// Name: provisional. The condition; the three signatures are checked together, since a sector
    /// with one wrong is not one a driver may write to.
    BadSignature,
}

/// **The two `FSInfo` hints**, each present only when it could be true of the volume.
///
/// Name: provisional. The specification's "FSInfo", spelled as the crate spells it
/// (`BootSector::fs_info_sector`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[doc(alias = "FSInfo")]
pub struct FsInfo {
    free_clusters: Option<u32>,
    next_free: Option<u32>,
}

impl FsInfo {
    /// **Parse an `FSInfo` sector of a volume with `clusters` data clusters.**
    ///
    /// Checks the three signatures. A free count above `clusters`, or all ones, is `None`, and so
    /// is a next-free cluster that is not a data cluster. Never panics, whatever `sector` holds.
    ///
    /// Name: provisional. The verb `BootSector::parse` uses for hostile bytes.
    pub fn parse(sector: &[u8], clusters: u32) -> Result<FsInfo, FsInfoError> {
        let word = |at: usize| match sector.get(at..at + 4) {
            Some(b) => Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            None => Err(FsInfoError::TooShort),
        };
        let lead = word(0)?;
        let structure = word(484)?;
        let trail = word(508)?;
        if lead != LEAD_SIGNATURE || structure != STRUCTURE_SIGNATURE || trail != TRAIL_SIGNATURE {
            return Err(FsInfoError::BadSignature);
        }
        let free = word(FREE_COUNT_AT)?;
        let next = word(NEXT_FREE_AT)?;
        Ok(FsInfo {
            free_clusters: (free != UNKNOWN && free <= clusters).then_some(free),
            next_free: (next >= 2 && next - 2 < clusters).then_some(next),
        })
    }

    /// The count of free clusters the sector claims, when it could be true.
    ///
    /// Name: provisional. `FSI_Free_Count`.
    pub const fn free_clusters(&self) -> Option<u32> {
        self.free_clusters
    }

    /// The cluster the sector says to start looking for a free one at, when it is a data cluster.
    ///
    /// Name: provisional. `FSI_Nxt_Free`.
    pub const fn next_free(&self) -> Option<u32> {
        self.next_free
    }

    /// **Write the two hints into `sector`**, which already holds a valid `FSInfo` sector, and
    /// leave every other byte as it is. `None` stores the "not known" value. Returns `false`
    /// and writes nothing when the sector's signatures are wrong, so a writer never turns a
    /// sector that is not `FSInfo` into one.
    pub(crate) fn store(
        sector: &mut [u8],
        free_clusters: Option<u32>,
        next_free: Option<u32>,
    ) -> bool {
        if FsInfo::parse(sector, u32::MAX - 1).is_err() {
            return false;
        }
        sector[FREE_COUNT_AT..FREE_COUNT_AT + 4]
            .copy_from_slice(&free_clusters.unwrap_or(UNKNOWN).to_le_bytes());
        sector[NEXT_FREE_AT..NEXT_FREE_AT + 4]
            .copy_from_slice(&next_free.unwrap_or(UNKNOWN).to_le_bytes());
        true
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
