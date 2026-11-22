/// Bytes one record takes. Also the value [`super::file_system::STATFS`] answers with on success; see
/// the module header on why the length is the version.
///
/// Name: provisional, minted 2026-11-18 (UTC) by the lane of milestone 1090 (an outside review of `file_system_contract`).
pub const RECORD_BYTES: usize = 24;

/// Write a record at the start of `out`, returning its length, or `None` if it does not fit.
pub fn encode(
    out: &mut [u8],
    block_size: u64,
    total_blocks: u64,
    free_blocks: u64,
) -> Option<usize> {
    if out.len() < RECORD_BYTES {
        return None;
    }
    out[0..8].copy_from_slice(&block_size.to_le_bytes());
    out[8..16].copy_from_slice(&total_blocks.to_le_bytes());
    out[16..24].copy_from_slice(&free_blocks.to_le_bytes());
    Some(RECORD_BYTES)
}

/// Read a record from the start of `buf` (the `r0` bytes a `file_system::STATFS` reply filled), as
/// `(block_size, total_blocks, free_blocks)`. `None` if the reply is shorter than this version
/// of the record, which is what a truncated reply looks like and must not be read as zeroes.
pub fn decode(buf: &[u8]) -> Option<(u64, u64, u64)> {
    if buf.len() < RECORD_BYTES {
        return None;
    }
    let w = |at: usize| u64::from_le_bytes(buf[at..at + 8].try_into().unwrap());
    Some((w(0), w(8), w(16)))
}

/// The volume's size in bytes, saturating rather than wrapping: the numbers come off a disk and
/// a corrupt pair must not become a small plausible answer.
pub const fn total_bytes(block_size: u64, total_blocks: u64) -> u64 {
    block_size.saturating_mul(total_blocks)
}

/// The free bytes, saturating for [`total_bytes`]'s reason.
pub const fn free_bytes(block_size: u64, free_blocks: u64) -> u64 {
    block_size.saturating_mul(free_blocks)
}

#[cfg(test)]
mod tests;
