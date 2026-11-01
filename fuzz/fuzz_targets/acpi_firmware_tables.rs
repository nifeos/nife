//! Fuzz the ACPI table-body readers that no Kani harness reaches, with arbitrary bytes.
//!
//! Firmware writes these bytes. On x86_64 the kernel reads the IVRS (AMD's IOMMU description)
//! at boot before any process exists, and what it decodes becomes IOMMU identity mappings, so it
//! shapes the DMA confinement. On aarch64 the loader reads the GTDT, SPCR and FADT. Kani covers the
//! RSDP, the root table, the SDT header, the MCFG, the MADT walk and the DMAR walk
//! (`crates/machine_discovery/src/acpi/`); it does not cover these five, and `IvrsUnits::parse` is a
//! 738-line variable-length walk over an IVHD and IVMD stream whose lengths the table chooses.
//!
//! The first input byte picks the reader and the rest is the table body (the bytes after the SDT
//! header), so one corpus serves all five and a seed is a body with its selector in front.
//! Properties beyond "no panic and no hang": every IVRS unity region (an IOMMU identity mapping)
//! is page aligned, non-empty and does not wrap the address space; the recorded counts stay
//! inside their arrays; every recorded device range is ordered. notes/fuzzing.md has the rest.

#![no_main]

use libfuzzer_sys::fuzz_target;
use machine_discovery::acpi::ivrs::{
    IvrsUnits, MAXIMUM_RESERVED_REGIONS, MAXIMUM_TRANSLATION_UNIT_DEVICE_RANGES,
    MAXIMUM_TRANSLATION_UNITS,
};
use machine_discovery::acpi::{parse_arm_boot, parse_fadt_reset, parse_gtdt, parse_spcr};

const PAGE: u64 = 4096;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, body)) = data.split_first() else {
        return;
    };
    match selector % 5 {
        0 => ivrs(body),
        1 => {
            let _ = parse_gtdt(body);
        }
        2 => {
            let _ = parse_spcr(body);
        }
        3 => {
            let _ = parse_arm_boot(body);
        }
        _ => {
            let _ = parse_fadt_reset(body);
        }
    }
});

fn ivrs(body: &[u8]) {
    let u = IvrsUnits::parse(body);

    assert!(u.ivhd_count <= MAXIMUM_TRANSLATION_UNITS);
    assert!(u.range_count <= MAXIMUM_TRANSLATION_UNIT_DEVICE_RANGES);
    assert!(u.unity_count <= MAXIMUM_RESERVED_REGIONS);
    assert_eq!(u.units().len(), u.ivhd_count);
    assert_eq!(u.ranges().len(), u.range_count);
    assert_eq!(u.unity_regions().len(), u.unity_count);

    for r in u.unity_regions() {
        assert_eq!(
            r.base % PAGE,
            0,
            "an identity mapping starts on a page: {r:?}"
        );
        assert_eq!(
            r.size % PAGE,
            0,
            "an identity mapping is whole pages: {r:?}"
        );
        assert!(
            r.size != 0,
            "an empty identity mapping is no mapping: {r:?}"
        );
        assert!(
            r.base.checked_add(r.size).is_some(),
            "an identity mapping must not wrap the address space: {r:?}"
        );
        assert!(
            r.first <= r.last,
            "a mapping names an inverted device range: {r:?}"
        );
        let _ = r.device_may_write();
    }
    for range in u.ranges() {
        assert!(
            range.first <= range.last,
            "an inverted device range: {range:?}"
        );
        assert!(usize::from(range.unit) < u.ivhd_count);
    }

    // The queries the kernel makes of a decoded table, over ids the table chose and ones it did not.
    let mut ids = [0x0000u16, 0x0008, 0x00a0, 0x0100, 0x4008, 0xfffe, 0xffff];
    for r in u.unity_regions() {
        ids[0] = r.first;
        ids[1] = r.last;
    }
    for id in ids {
        let _ = u.owner_index(id);
        let _ = u.source_id(id);
        u.unity_for(id, &mut |r| {
            assert!(
                r.first <= id && id <= r.last,
                "unity_for answered a region that excludes {id:#x}"
            );
        });
    }
    for unit in 0..u.ivhd_count {
        let _ = u.last_device_id(unit);
    }
}
