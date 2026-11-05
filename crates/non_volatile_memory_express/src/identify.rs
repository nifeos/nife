//! **IDENTIFY namespace parsing**: the two facts a block driver needs from the controller's
//! answer, read defensively.
//!
//! Name: provisional. The module holding [`parse_identify_namespace`] and [`IdentifyNamespace`].

/// The two facts the driver needs from the 4096-byte IDENTIFY namespace structure (section 5.15.2,
/// figure 245): the namespace size in logical blocks (NSZE, bytes 0..8) and the
/// data shift of the LBA format in use (FLBAS names one of 16 formats; each format's LBADS is
/// `1 << shift` bytes per block). `None` if the structure is shorter than the fields it must
/// carry, or describes a block size outside what a driver of 4096-byte blocks can serve (LBADS below 9 is the spec's "unsupported" marker; above 12 a block would outgrow the
/// transfer buffer).
///
/// Name: ratified 2026-10-25 (#2720).
pub fn parse_identify_namespace(data: &[u8]) -> Option<IdentifyNamespace> {
    if data.len() < 384 {
        return None; // NSZE..the LBA format table must be present
    }
    let nsze = u64::from_le_bytes(data[0..8].try_into().ok()?);
    let flbas = data[26] & 0xf;
    // LBA format `flbas` is 4 bytes at 128 + 4*flbas; LBADS is its third byte.
    let lbads = data[128 + 4 * flbas as usize + 2];
    if !(9..=12).contains(&lbads) {
        return None;
    }
    Some(IdentifyNamespace {
        blocks: nsze,
        logical_block_address_shift: lbads as u32,
    })
}

/// **The LBA data shift the namespace is formatted with, whatever it is**: the same LBADS byte
/// [`parse_identify_namespace`] reads, before that function's bounds refuse it. `None` only when
/// the structure is too short to carry the format table.
///
/// It exists so a refusal can say what it refused. A drive must be formatted so that
/// `blocks_per_transfer` falls in `1..=8`, and a namespace this driver declines has to print its
/// LBA size rather than arrive as a server that never started and a test that reads "skipped".
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `lba_format_shift` (`lba` spelled out, as the
/// acronym test asks).
pub fn logical_block_address_format_shift(data: &[u8]) -> Option<u8> {
    if data.len() < 384 {
        return None;
    }
    let flbas = data[26] & 0xf;
    Some(data[128 + 4 * flbas as usize + 2])
}

/// See [`parse_identify_namespace`]. The struct is `#[non_exhaustive]`: a later release may read
/// more of the structure, so code outside the crate gets one from the parse, or from
/// [`Default`] as a placeholder before IDENTIFY has run.
///
/// Name: ratified 2026-10-25 (#2720).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct IdentifyNamespace {
    /// The namespace's size in logical blocks.
    pub blocks: u64,
    /// `1 << logical_block_address_shift` bytes per logical block; between 9 and 12 by construction.
    pub logical_block_address_shift: u32,
}

impl Default for IdentifyNamespace {
    /// An empty namespace of 512-byte blocks: the placeholder a driver holds before IDENTIFY has
    /// answered. It satisfies the same bounds as a parsed one (a shift between 9 and 12).
    fn default() -> Self {
        IdentifyNamespace {
            blocks: 0,
            logical_block_address_shift: 9,
        }
    }
}

impl IdentifyNamespace {
    /// The namespace's capacity in bytes.
    ///
    /// Name: ratified 2026-10-25 (#2720). Refused `bytes` (it is the namespace's capacity in bytes,
    /// not its bytes).
    pub fn capacity_bytes(&self) -> u64 {
        self.blocks << self.logical_block_address_shift
    }
    /// How many logical blocks one `unit`-byte transfer covers, or `None` if `unit` is not a
    /// whole number of logical blocks (a namespace this driver cannot serve 4096-byte blocks
    /// on; `parse_identify_namespace`'s shift bounds make it impossible for unit 4096).
    ///
    /// Name: ratified 2026-10-25 (#2720). Refused `blocks_per` (it did not say per what).
    pub fn blocks_per_transfer(&self, unit: u64) -> Option<u16> {
        if unit == 0 || unit & ((1 << self.logical_block_address_shift) - 1) != 0 {
            return None;
        }
        Some((unit >> self.logical_block_address_shift) as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_namespace_yields_size_and_shift() {
        let mut data = [0u8; 4096];
        data[0..8].copy_from_slice(&0x4000u64.to_le_bytes()); // NSZE: 16384 blocks
        data[26] = 0; // FLBAS: format 0
        data[130] = 9; // format 0's LBADS: 512-byte blocks
        let id = parse_identify_namespace(&data).unwrap();
        assert_eq!(id.blocks, 0x4000);
        assert_eq!(id.logical_block_address_shift, 9);
        assert_eq!(id.capacity_bytes(), 0x4000 * 512);
        assert_eq!(id.blocks_per_transfer(4096), Some(8));

        data[130] = 13; // an LBA format this driver cannot serve
        assert!(parse_identify_namespace(&data).is_none());
        assert!(parse_identify_namespace(&[0u8; 100]).is_none(), "truncated");
    }

    /// A refused format still says what it was, which is what lets a caller print "8192-byte
    /// LBAs" instead of "skipped". Read through FLBAS, so a namespace formatted with its second
    /// format reports that one rather than format 0.
    #[test]
    fn a_refused_lba_format_still_reports_its_shift() {
        let mut data = [0u8; 4096];
        data[26] = 1; // FLBAS: format 1
        data[130] = 9; // format 0: 512 bytes, not the one in use
        data[134] = 13; // format 1: 8192 bytes
        assert!(parse_identify_namespace(&data).is_none());
        assert_eq!(logical_block_address_format_shift(&data), Some(13));
        assert_eq!(logical_block_address_format_shift(&[0u8; 383]), None);
    }

    /// The shift is read at `128 + 4 * FLBAS + 2` for every format the nibble can name, from a
    /// structure exactly 384 bytes long, with a different shift planted in every slot so a wrong
    /// stride lands on another format's value.
    #[test]
    fn the_reported_shift_is_the_one_in_the_flbas_slot_for_every_format() {
        for flbas in 0..16u8 {
            let mut data = [0u8; 384];
            for format in 0..16usize {
                data[128 + 4 * format + 2] = 20 + format as u8;
            }
            data[26] = flbas | 0x70; // the bits above the format number are not part of it
            assert_eq!(
                logical_block_address_format_shift(&data),
                Some(20 + flbas),
                "flbas {flbas}"
            );
        }
        assert_eq!(
            logical_block_address_format_shift(&[0u8; 384]),
            Some(0),
            "384 bytes is enough"
        );
    }

    #[test]
    fn identify_namespace_accepts_the_minimum_length_and_refuses_one_byte_less() {
        // The `[0u8; 100]` case above is far short of the boundary and cannot tell `< 384` from
        // `<= 384`. Sit exactly on it instead.
        let mut data = [0u8; 384];
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        data[26] = 0;
        data[130] = 9;
        assert!(
            parse_identify_namespace(&data).is_some(),
            "384 bytes is documented as enough"
        );
        assert!(
            parse_identify_namespace(&data[..383]).is_none(),
            "one byte short of the documented minimum must be refused"
        );
    }

    #[test]
    fn identify_namespace_reads_the_lba_format_table_at_flbas_not_format_zero() {
        // The test above uses FLBAS=0, where the table offset `128 + 4*flbas` happens to equal
        // `128 - 4*flbas`: both are 128. Use FLBAS=1 so the two diverge, and plant a different,
        // still-valid LBADS at format 0's slot so a lane reading the wrong slot gets caught by
        // value rather than by accidentally landing on unset (and therefore refused) bytes.
        let mut data = [0u8; 384];
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        data[26] = 1; // FLBAS: format 1
        data[128 + 2] = 10; // format 0's LBADS: a decoy
        data[128 + 4 + 2] = 11; // format 1's LBADS: the one that must be read
        let id = parse_identify_namespace(&data).unwrap();
        assert_eq!(
            id.logical_block_address_shift, 11,
            "format 1's slot, not format 0's"
        );
    }

    #[test]
    fn blocks_per_transfer_masks_by_the_actual_block_size() {
        // `identify_namespace_yields_size_and_shift`'s only `blocks_per_transfer` call is 4096, which
        // is a multiple of every mask this function could plausibly compute (511, 512, or 513: none
        // of their low bits reach as high as bit 12), so it cannot tell them apart, and it never
        // calls `blocks_per_transfer(0)` either.
        let id = IdentifyNamespace {
            blocks: 1,
            logical_block_address_shift: 9, // 512-byte blocks; mask is 511
        };
        assert_eq!(
            id.blocks_per_transfer(0),
            None,
            "a zero unit is refused even though 0 & anything is 0"
        );
        // 1536 = 3 * 512, so the correct mask (511) says it divides evenly. The `+1` mutant's
        // mask (513) and the `/1` mutant's mask (512) both share bit 9 with 1536 and would
        // wrongly refuse it.
        assert_eq!(id.blocks_per_transfer(1536), Some(3));
    }
}
