//! Fuzz the capability registers and the extended capability walk with any register file a
//! controller might present.
//!
//! The first 24 bytes are the six capability dwords (`CAPLENGTH`, `HCSPARAMS1`, `HCSPARAMS2`,
//! `HCCPARAMS1`, `DBOFF`, `RTSOFF`), little-endian, and the next three are an extended capability
//! id and a start offset of the target's own. The rest is the register file the walk reads, one
//! dword at a byte offset, with its length as the limit. The target holds every decoded field to
//! the bits it comes from, every offset `Capabilities` computes to its stride, and the walk to the
//! file: it reads only whole dwords inside the limit, at most 64 of them, and returns only an
//! offset whose id matched.

#![no_main]

use extensible_host_controller_interface::Capabilities;
use extensible_host_controller_interface::registers::{ERDP, IMAN, find_extended};
use libfuzzer_sys::fuzz_target;

fn dword(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Walk the extended capability list in `file` from `first`, and hold the walk to the file.
fn walk(file: &[u8], first: u64, id: u8) {
    let limit = file.len() as u64;
    let mut reads = 0u32;
    let mut last = None;
    let mut read = |at: u64| {
        reads += 1;
        assert!(reads <= 64, "the walk is bounded at 64 reads");
        assert_ne!(at, 0, "offset 0 ends the list and is never read");
        let end = at
            .checked_add(4)
            .expect("a read offset leaves room for its dword");
        assert!(end <= limit, "every read is a whole dword inside the limit");
        if let Some(previous) = last {
            assert!(at > previous, "the walk only moves forward");
        }
        last = Some(at);
        let start = at as usize;
        dword(&file[start..start + 4])
    };
    if let Some(found) = find_extended(first, limit, id, &mut read) {
        let start = found as usize;
        assert_eq!(dword(&file[start..start + 4]) as u8, id);
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((registers, rest)) = data.split_first_chunk::<24>() else {
        return;
    };
    let Some((walk_head, file)) = rest.split_first_chunk::<3>() else {
        return;
    };
    let words: [u32; 6] = core::array::from_fn(|i| dword(&registers[i * 4..i * 4 + 4]));
    let [
        capability_length,
        structural_parameters_1,
        structural_parameters_2,
        capability_parameters_1,
        doorbell_offset,
        runtime_offset,
    ] = words;
    let capabilities = Capabilities::decode(
        capability_length,
        structural_parameters_1,
        structural_parameters_2,
        capability_parameters_1,
        doorbell_offset,
        runtime_offset,
    );
    assert_eq!(
        capabilities,
        Capabilities::decode(
            capability_length,
            structural_parameters_1,
            structural_parameters_2,
            capability_parameters_1,
            doorbell_offset,
            runtime_offset,
        )
    );

    assert!(capabilities.operational <= 0xff, "CAPLENGTH is one byte");
    assert_eq!(capabilities.version, (capability_length >> 16) as u16);
    assert_eq!(capabilities.maximum_slots, structural_parameters_1 as u8);
    assert!(
        capabilities.maximum_interrupters < 1 << 11,
        "MaxIntrs is 11 bits"
    );
    assert_eq!(
        capabilities.maximum_ports,
        (structural_parameters_1 >> 24) as u8
    );
    assert!(
        capabilities.scratchpads < 1 << 10,
        "the scratchpad count is 10 bits"
    );
    assert!(matches!(capabilities.context_bytes, 32 | 64));
    assert_eq!(
        capabilities.extended % 4,
        0,
        "the extended pointer counts dwords"
    );
    assert!(capabilities.extended < 1 << 18);
    assert_eq!(capabilities.doorbells % 4, 0, "DBOFF is dword aligned");
    assert_eq!(capabilities.runtime % 32, 0, "RTSOFF is 32-byte aligned");

    // Every port's register follows the last at the 16-byte stride, from the operational base.
    let mut previous = None;
    for port in 1..=capabilities.maximum_ports {
        let offset = capabilities.portsc(port);
        assert!(offset >= capabilities.operational);
        if let Some(before) = previous {
            assert_eq!(offset, before + 0x10);
        }
        previous = Some(offset);
    }
    assert_eq!(capabilities.portsc(0), capabilities.portsc(1));
    for slot in 0..=capabilities.maximum_slots {
        assert_eq!(
            capabilities.doorbell(slot),
            capabilities.doorbells + 4 * u64::from(slot)
        );
    }
    assert_eq!(
        capabilities.interrupter(ERDP) - capabilities.interrupter(IMAN),
        ERDP - IMAN
    );
    assert!(capabilities.interrupter(IMAN) >= capabilities.runtime);

    let id = walk_head[0];
    let first = u64::from(u16::from_le_bytes([walk_head[1], walk_head[2]]));
    walk(file, capabilities.extended, id);
    walk(file, first, id);
});
