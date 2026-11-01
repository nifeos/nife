//! The seeds of the `acpi_firmware_tables` fuzz target still decode as the generator
//! (`fuzz/seeds/make-acpi-tables.py`) intends. A seed that stops decoding is not a seed, and a fuzz
//! run would not say so.

use std::fs;
use std::path::Path;

use machine_discovery::acpi::ivrs::IvrsUnits;
use machine_discovery::acpi::{parse_arm_boot, parse_fadt_reset, parse_gtdt, parse_spcr};

fn seed(name: &str) -> (u8, Vec<u8>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fuzz/seeds/acpi_firmware_tables")
        .join(format!("{name}.bin"));
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    (bytes[0], bytes[1..].to_vec())
}

#[test]
fn the_ivrs_seeds_decode_and_their_selector_picks_the_ivrs_reader() {
    for (name, units, ranges, unity) in [
        ("ivrs_q35_amd_iommu", 1, 9, 0),
        ("ivrs_ranges_and_aliases", 1, 4, 0),
        ("ivrs_acpi_hid_entry", 1, 1, 0),
        ("ivrs_memory_definitions", 1, 1, 2),
    ] {
        let (selector, body) = seed(name);
        assert_eq!(selector % 5, 0, "{name}");
        let u = IvrsUnits::parse(&body);
        assert_eq!(u.units().len(), units, "{name}: units");
        assert_eq!(u.ranges().len(), ranges, "{name}: ranges");
        assert_eq!(u.unity_regions().len(), unity, "{name}: unity regions");
    }
}

#[test]
fn the_other_seeds_decode() {
    let (selector, body) = seed("gtdt_qemu_virt");
    assert_eq!(selector % 5, 1);
    assert_eq!(parse_gtdt(&body).expect("a GTDT").virtual_el1.0, 27);

    let (selector, body) = seed("spcr_pl011");
    assert_eq!(selector % 5, 2);
    assert_eq!(parse_spcr(&body).expect("an SPCR").interrupt, Some(33));

    let (selector, body) = seed("fadt_arm_boot_psci_hvc");
    assert_eq!(selector % 5, 3);
    let boot = parse_arm_boot(&body).expect("a FADT");
    assert!(boot.psci && boot.hvc);

    let (selector, body) = seed("fadt_reset_register_q35");
    assert_eq!(selector % 5, 4);
    assert_eq!(
        parse_fadt_reset(&body).expect("a reset register").address,
        0xcf9
    );
}
