//! **Machine-checked proofs (Kani) of the write path**, in two parts that compose into "written
//! bytes read back". Behind `#[cfg(kani)]`, so only `cargo kani` compiles them.
//!
//! One proves the bytes of a partial sector write (the head kept, the slack zeroed, the new bytes in
//! place) over every sector. The other runs the real cursor, chain extension and size arithmetic
//! over a model of three one-sector clusters behind the same [`Medium`] the device-backed volume
//! implements. A single harness over both did not close in 20 minutes: symbolic 512-byte copies
//! under symbolic offsets are what Kani cannot afford. What neither shows is the device's sector
//! reads and writes, which the oracle tests check on real volumes of every FAT type.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;
use crate::allocation::verification::{Model, free_model};
use crate::reader::Links;

/// A volume of three one-sector clusters. It keeps no bytes, only which sectors were touched, as a
/// bit for each, so the chain and size arithmetic can be proved apart from the bytes.
struct Volume3 {
    table: Model,
    touched: u8,
    first_data: u32,
}

impl Links for Volume3 {
    type Error = ();

    fn clusters(&self) -> u32 {
        self.table.clusters()
    }

    fn table_entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<()>> {
        self.table.table_entry(cluster)
    }
}

impl LinksMut for Volume3 {
    fn set_table_entry(&mut self, cluster: u32, entry: TableEntry) -> Result<(), ChainError<()>> {
        self.table.set_table_entry(cluster, entry)
    }

    fn append(&mut self, last: Option<u32>) -> Result<Option<u32>, ChainError<()>> {
        self.table.append(last)
    }
}

impl Medium for Volume3 {
    fn read_data(&mut self, sector: u32, _out: &mut [u8]) -> Result<(), ChainError<()>> {
        assert!(sector >= self.first_data && sector < self.first_data + 3);
        Ok(())
    }

    fn write_data(&mut self, sector: u32, _data: &[u8]) -> Result<(), ChainError<()>> {
        assert!(sector >= self.first_data && sector < self.first_data + 3);
        self.touched |= 1 << (sector - self.first_data);
        Ok(())
    }

    // The byte proof covers the merge; this one covers clusters and sizes.
    fn merge(&mut self, _buffer: &mut [u8], _kept: usize, _offset: usize, _bytes: &[u8]) {}
}

/// The sector size the byte proof uses. `merge_partial` takes any slice, so a 16-byte sector shows
/// its logic for every length; Kani cannot afford symbolic copies inside 512 bytes.
const SMALL: usize = 16;

/// **A partial sector write keeps the head, zeroes the slack and lands the bytes.** For any contents
/// of a 16-byte sector, any amount of the file already in the sector, any offset up to that amount and up to
/// four bytes: the byte at any probe position is the new byte where it landed, the old byte
/// before the end of the file, and zero everywhere else.
/// Falsification: replayable `crates/file_allocation_table/falsifications/write_cursor.verification.a_partial_sector_write_keeps_the_head_and_zeroes_the_slack.patch`
#[kani::proof]
#[kani::unwind(6)]
fn a_partial_sector_write_keeps_the_head_and_zeroes_the_slack() {
    let old: [u8; SMALL] = kani::any();
    let mut buffer = old;
    let kept: usize = kani::any();
    kani::assume(kept <= SMALL);
    let offset: usize = kani::any();
    kani::assume(offset <= kept);
    let length: usize = kani::any();
    kani::assume((1..=4).contains(&length) && offset + length <= SMALL);
    let bytes: [u8; 4] = kani::any();
    merge_partial(&mut buffer, kept, offset, &bytes[..length]);
    let probe: usize = kani::any();
    kani::assume(probe < SMALL);
    if probe >= offset && probe < offset + length {
        assert_eq!(buffer[probe], bytes[probe - offset]);
    } else if probe < kept {
        assert_eq!(buffer[probe], old[probe]);
    } else {
        assert_eq!(buffer[probe], 0);
    }
}

/// **A write extends the chain to the size and stays inside it.** From an empty file on a volume
/// of three one-sector clusters: a write of up to 700 bytes, then a second of up to 700 bytes at
/// any position inside the file. The size is the larger of the old size and the end of the second
/// write, the position is that end, every sector the write touched is a sector of the file's own
/// chain, the chain has one cluster per 512 bytes of size and ends there, and no other cluster is
/// in use. The bytes themselves are `a_partial_sector_write_keeps_the_head_and_zeroes_the_slack`'s
/// and the oracle tests'.
/// Falsification: replayable `crates/file_allocation_table/falsifications/write_cursor.verification.a_write_extends_the_chain_to_the_size_and_stays_inside_it.patch`
#[kani::proof]
#[kani::unwind(8)]
fn a_write_extends_the_chain_to_the_size_and_stays_inside_it() {
    let boot = crate::directory::verification::fat12(3, 1);
    let mut volume = Volume3 {
        table: free_model(3),
        touched: 0,
        first_data: boot.first_data_sector(),
    };
    let mut state = WriteState::new(0, 0);
    let mut scratch = [0u8; 512];
    let zeros = [0u8; 700];

    let first: usize = kani::any();
    kani::assume(first <= 700);
    assert_eq!(
        state.write(&mut volume, &boot, &zeros[..first], &mut scratch),
        Ok(first)
    );
    assert_eq!(state.size as usize, first);

    let at: usize = kani::any();
    kani::assume(at <= first);
    state.seek(at as u32);
    let second: usize = kani::any();
    kani::assume(second <= 700);
    assert_eq!(
        state.write(&mut volume, &boot, &zeros[..second], &mut scratch),
        Ok(second)
    );
    let size = first.max(at + second);
    assert_eq!(state.size as usize, size);
    assert_eq!(state.position as usize, at + second);

    let needed = size.div_ceil(512) as u32;
    let mut walk = crate::reader::Walk::new(state.first_cluster);
    let mut in_chain = 0;
    let mut chain_sectors = 0u8;
    while let Some(Ok(cluster)) = walk.step(&mut volume) {
        in_chain += 1;
        chain_sectors |= 1 << (cluster - 2);
    }
    assert_eq!(in_chain, needed);
    assert_eq!(volume.touched & !chain_sectors, 0);
    let mut in_use = 0;
    for cluster in 2..5 {
        if volume.table.table_entry(cluster).unwrap() != TableEntry::Free {
            in_use += 1;
        }
    }
    assert_eq!(in_use, needed);
}
