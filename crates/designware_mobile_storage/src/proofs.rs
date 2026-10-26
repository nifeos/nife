use crate::host::fifo_step;
use crate::idmac::{DESCRIPTOR_BYTES, MAX_DESCRIPTORS, PAGE, descriptor, descriptors_for};
use crate::partition::{Entry, Mbr};
use crate::sd::in_range;

/// No partition table makes the write test's range touch a partition or sector 0.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.the_scratch_range_never_touches_a_partition_or_sector_zero.patch`
#[kani::proof]
#[kani::unwind(5)]
fn the_scratch_range_never_touches_a_partition_or_sector_zero() {
    let entry = || Entry {
        kind: kani::any(),
        start: kani::any(),
        sectors: kani::any(),
    };
    let mbr = Mbr {
        entries: [entry(), entry(), entry(), entry()],
    };
    let want: u32 = kani::any();
    if let Some((first, count)) = mbr.scratch(want) {
        assert!(first >= 1);
        assert!((1..=want).contains(&count));
        let k: u32 = kani::any();
        kani::assume(k < count);
        let sector = u64::from(first) + u64::from(k);
        for e in &mbr.entries {
            assert!(!e.contains(sector));
        }
    }
}

/// A FIFO step never moves more than is owed, and a write step never more than the FIFO has
/// room for, whatever `STATUS` says.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_fifo_step_never_overruns_the_fifo_or_the_buffer.patch`
#[kani::proof]
fn a_fifo_step_never_overruns_the_fifo_or_the_buffer() {
    let status: u32 = kani::any();
    let depth: u32 = kani::any();
    kani::assume((1..=4096).contains(&depth));
    let write: bool = kani::any();
    let remaining: u32 = kani::any();
    let n = fifo_step(status, depth, write, remaining);
    assert!(n <= remaining);
    assert!(n <= depth * 4);
    let filled = crate::regs::fifo_count(status);
    if write && filled <= depth {
        assert!(n <= (depth - filled) * 4);
    }
}

/// A range the driver admits is inside the card and inside what the command's 32-bit address
/// can name.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.an_admitted_range_is_inside_the_card_and_the_address.patch`
#[kani::proof]
fn an_admitted_range_is_inside_the_card_and_the_address() {
    let blocks: u64 = kani::any();
    let block_addressed: bool = kani::any();
    let lba: u64 = kani::any();
    let count: u64 = kani::any();
    if in_range(blocks, block_addressed, lba, count) {
        assert!(count >= 1);
        assert!(lba + count <= blocks);
        let last = lba + count - 1;
        if block_addressed {
            assert!(last <= u64::from(u32::MAX));
        } else {
            assert!((last + 1) * 512 <= 1 << 32);
        }
    }
}

/// A chain for any transfer a region admits describes exactly its bytes, each descriptor a page
/// or less inside the region's data pages, and points only at descriptors in the chain page.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_chain_describes_its_bytes_inside_the_region.patch`
#[kani::proof]
fn a_chain_describes_its_bytes_inside_the_region() {
    let pages: u32 = kani::any();
    kani::assume((1..=MAX_DESCRIPTORS).contains(&pages));
    let bus: u32 = kani::any();
    kani::assume(bus.is_multiple_of(PAGE));
    kani::assume(u64::from(bus) + u64::from((pages + 1) * PAGE) <= 1 << 32);
    let bytes: u32 = kani::any();
    kani::assume((1..=pages * PAGE).contains(&bytes));
    let count = descriptors_for(bytes);
    assert!((1..=pages).contains(&count));
    let i: u32 = kani::any();
    kani::assume(i < count);
    let [des0, size, buffer, next] = descriptor(bus, i, count, bytes);
    assert!((1..=PAGE).contains(&size));
    assert!(buffer >= bus + PAGE && buffer - bus + size <= (pages + 1) * PAGE);
    if i + 1 == count {
        assert!(i * PAGE + size == bytes);
        assert!(des0 & crate::idmac::DES0_LD != 0);
    } else {
        assert!(size == PAGE && next == bus + (i + 1) * DESCRIPTOR_BYTES);
        assert!(des0 & crate::idmac::DES0_LD == 0);
    }
}
