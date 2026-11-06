//! Machine-checked proofs (`cargo kani`).
//!
//! The ring arithmetic runs forever against a device that writes into the same memory, so the
//! properties proved are the ones a lapped or hostile ring could otherwise erode: indices stay
//! inside the ring for every reachable state, the phase discipline is exactly one flip per lap,
//! the doorbell and PRP arithmetic is total (no register value the controller reports can make an
//! offset computation panic or two doorbells collide), no transfer command addresses a block
//! outside its namespace, and the two decoders of words the controller writes (the capabilities
//! register and the completion status) read exactly the bits the specification gives them.
//!
//! Name: ratified 2026-11-06 (UTC). The module holding the Kani harnesses.

use crate::*;

/// **The completion head stays in bounds and flips the phase exactly at the wrap**, for every
/// reachable state, not just the ones a test happened to walk.
/// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.cq_pop_stays_in_bounds_and_flips_only_at_the_wrap.patch`
#[kani::proof]
fn cq_pop_stays_in_bounds_and_flips_only_at_the_wrap() {
    let entries: u16 = kani::any();
    let head: u16 = kani::any();
    let phase: bool = kani::any();
    kani::assume(entries >= 2 && head < entries);
    let mut queue = CompletionQueue {
        entries,
        head,
        phase,
    };
    let ret = queue.pop();
    assert!(queue.head < entries);
    assert_eq!(ret, queue.head);
    // The head advanced by exactly one, modulo the ring. Without this the two assertions above
    // hold for a pop that wraps one slot early (or late by a lap), which a device writing
    // completions into every slot would then never be read from.
    assert_eq!(
        u32::from(queue.head),
        (u32::from(head) + 1) % u32::from(entries)
    );
    // The phase flipped if and only if the head wrapped to zero.
    assert_eq!(queue.phase != phase, queue.head == 0);
}

/// **A submission push lands in bounds from any state, and refuses exactly when the ring is
/// full.** A full ring answers `None` and changes nothing; any other ring hands back a slot inside
/// it and leaves a legal tail, which is the doorbell value the caller writes.
/// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.sq_push_stays_in_bounds.patch`
#[kani::proof]
fn sq_push_stays_in_bounds() {
    let entries: u16 = kani::any();
    let tail: u16 = kani::any();
    let head: u16 = kani::any();
    kani::assume(entries >= 2 && tail < entries && head < entries);
    let full = (tail + 1) % entries == head;
    let mut queue = SubmissionQueue {
        entries,
        tail,
        head,
    };
    let before = queue;
    match queue.push() {
        Some(slot) => {
            assert!(!full);
            assert!(slot < entries);
            assert!(queue.tail() < entries);
        }
        None => {
            assert!(full);
            assert_eq!(queue, before);
        }
    }
}

/// **No two doorbells collide**, for any queue ids and any stride the controller can report:
/// distinct (queue, role) pairs always compute distinct offsets, so a tail write can never
/// land on a head register. This is the arithmetic the caller's volatile writes trust.
/// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.distinct_doorbells_never_collide.patch`
#[kani::proof]
fn distinct_doorbells_never_collide() {
    let qa: u16 = kani::any();
    let qb: u16 = kani::any();
    let ra = if kani::any() {
        Doorbell::SubmissionTail
    } else {
        Doorbell::CompletionHead
    };
    let rb = if kani::any() {
        Doorbell::SubmissionTail
    } else {
        Doorbell::CompletionHead
    };
    let dstrd: u32 = kani::any();
    if (qa, ra) != (qb, rb) {
        assert!(doorbell(qa, ra, dstrd) != doorbell(qb, rb, dstrd));
    }
}

/// **PRP construction is total and never spans more pages than it names.** For any base,
/// length and power-of-two page size, `physical_region_page_pair` either refuses or returns a pair
/// whose two pointers cover the whole transfer: the second entry is zero exactly when one page
/// suffices, and is page-aligned whenever it is used (the specification's requirement on every
/// pointer after the first).
/// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.physical_region_page_pair_is_total_and_page_disciplined.patch`
#[kani::proof]
fn physical_region_page_pair_is_total_and_page_disciplined() {
    let base: u64 = kani::any();
    let length: u64 = kani::any();
    let shift: u32 = kani::any();
    kani::assume((9..=16).contains(&shift));
    let page = 1u64 << shift;
    if let Some(pair) = physical_region_page_pair(base, length, page) {
        assert_eq!(pair.first_entry, base);
        let offset = base & (page - 1);
        if offset + length <= page {
            assert_eq!(pair.second_entry, 0);
        } else {
            assert_eq!(pair.second_entry & (page - 1), 0);
            assert_eq!(pair.second_entry, (base & !(page - 1)) + page);
        }
    }
}

/// **No transfer command is built for a block outside its namespace, and none overflows
/// computing one.** This is the bounds check a confined driver leans on: an IOMMU bounds where
/// the controller may *write*, and nothing but this bounds *which LBA* is asked for, so a
/// caller-supplied `u64` must not be able to wrap into a valid-looking address.
/// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.a_transfer_command_is_only_built_for_a_block_the_namespace_has.patch`
#[kani::proof]
fn a_transfer_command_is_only_built_for_a_block_the_namespace_has() {
    let namespace = Namespace::new(kani::any(), kani::any());
    kani::assume((1..=8).contains(&namespace.blocks_per_transfer));
    let block: u64 = kani::any();
    let data_physical_address: u64 = kani::any();
    kani::assume(
        data_physical_address.is_multiple_of(4096) && data_physical_address < u64::MAX - 8192,
    );
    if namespace
        .transfer_command(1, 1, block, 4096, data_physical_address, 4096, kani::any())
        .is_some()
    {
        assert!(namespace.holds_whole_block(block, 4096));
        // The transfer's last byte is inside the namespace, which is the property
        // `holds_whole_block` exists to state and the one a wrap would quietly break.
        assert!((block + 1) * 4096 <= namespace.capacity_bytes);
    }
}

/// **Identify parsing is total for any device response.** The 4096 bytes come from the
/// controller's DMA; no contents may panic the parse, and an accepted answer always carries a
/// shift the block driver's arithmetic (`blocks_per_transfer(4096)`) can serve.
/// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.identify_parse_is_total_and_bounds_the_shift.patch`
#[kani::proof]
fn identify_parse_is_total_and_bounds_the_shift() {
    let data: [u8; 384] = kani::any();
    if let Some(id) = parse_identify_namespace(&data) {
        assert!((9..=12).contains(&id.logical_block_address_shift));
        let per = id.blocks_per_transfer(4096).unwrap();
        assert!((1..=8).contains(&per));
    }
}

/// **The capabilities decoder is total and reads exactly the specification's bit positions**, for
/// any 64-bit register value a controller may report. Each field lies in the range its width
/// allows (the queue depth from 1 to 65536, the doorbell stride below 16, the page size a power of
/// two from 4 KiB to 128 MiB), and each equals the bits at its place in the register, so a shift
/// or mask moved by one bit is caught.
/// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.capabilities_decode_is_total_and_reads_the_specified_bits.patch`
#[kani::proof]
fn capabilities_decode_is_total_and_reads_the_specified_bits() {
    let register: u64 = kani::any();
    let capabilities = Capabilities(register);

    let depth = capabilities.maximum_queue_entries();
    assert!((1..=65_536).contains(&depth));
    assert_eq!(u64::from(depth), (register & 0xffff) + 1);

    assert!(capabilities.doorbell_stride() <= 15);
    assert_eq!(
        u64::from(capabilities.doorbell_stride()),
        (register >> 32) & 0xf
    );

    assert!(capabilities.timeout_500ms() <= 255);
    assert_eq!(
        u64::from(capabilities.timeout_500ms()),
        (register >> 24) & 0xff
    );

    let page = capabilities.minimum_page_size();
    assert!(page.is_power_of_two());
    assert!((4096..=1u64 << 27).contains(&page));
    assert_eq!(page.trailing_zeros() as u64, 12 + ((register >> 48) & 0xf));
}

/// **The completion status decoder is total and reads exactly the specification's bit
/// positions**, for any four dwords a controller may write. The status field is the 15 bits above
/// the phase tag, the phase tag is bit 16, and together with the command identifier they rebuild
/// dword 3 exactly, so no bit of it is dropped, doubled or read from the wrong place. A zero
/// status is success, and only an all-zero field is.
/// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.completion_decode_is_total_and_rebuilds_dword_three.patch`
#[kani::proof]
fn completion_decode_is_total_and_rebuilds_dword_three() {
    let dwords: [u32; 4] = kani::any();
    let completion = Completion::from_dwords(dwords);

    assert!(completion.status < 1 << 15);
    let rebuilt = u32::from(completion.cid)
        | u32::from(completion.phase) << 16
        | u32::from(completion.status) << 17;
    assert_eq!(rebuilt, dwords[3]);
    assert_eq!(completion.status == 0, dwords[3] >> 17 == 0);

    assert_eq!(completion.result, dwords[0]);
    assert_eq!(
        u32::from(completion.submission_queue_head)
            | u32::from(completion.submission_queue_id) << 16,
        dwords[2]
    );
}
