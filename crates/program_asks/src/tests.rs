//! **The tests of the one reading**: the asks a note yields, in the record's order, and the
//! refusals that leave a program asking nothing.
//!
//! Name: provisional, minted 2026-10-29 (UTC) by lane
//! milestone/909-item-3-composition-checks-the-approval.

extern crate std;

use std::vec;
use std::vec::Vec;

use installed_programs::Granted;

use super::*;

/// The manifest every fixture program carries, with the asks a test adds on top. Two consts,
/// because the note constructors are `const fn` and their bindings' types pin the notes' lengths.
const MANIFEST_NETWORK: grant_plan::Manifest = grant_plan::Manifest {
    network: true,
    ..grant_plan::Program::Uptime.manifest()
};
const MANIFEST_PLAIN: grant_plan::Manifest = grant_plan::Manifest {
    network: false,
    ..grant_plan::Program::Uptime.manifest()
};

/// A one-`PT_LOAD` ELF whose one `PT_NOTE` segment carries `whole_note`: the bytes
/// `capability_manifest_note`'s note constructors produce, which are the segment's content, an
/// ELF note record whole (header, owner, descriptor).
fn elf_image(whole_note: &[u8]) -> Vec<u8> {
    let segment = whole_note;
    let phnum = 2usize;
    let mut body_off = 64 + phnum * 56;
    let mut v = vec![0u8; 64];
    v[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    v[4] = 2;
    v[5] = 1;
    v[6] = 1;
    v[16..18].copy_from_slice(&2u16.to_le_bytes());
    v[18..20].copy_from_slice(&elf::NATIVE_MACHINE.to_le_bytes());
    v[24..32].copy_from_slice(&0x40_0000u64.to_le_bytes());
    v[32..40].copy_from_slice(&64u64.to_le_bytes());
    v[54..56].copy_from_slice(&56u16.to_le_bytes());
    v[56..58].copy_from_slice(&(phnum as u16).to_le_bytes());
    let code = [0xaau8; 16];
    let mut ph = vec![0u8; 56];
    ph[0..4].copy_from_slice(&1u32.to_le_bytes());
    ph[4..8].copy_from_slice(&(elf::PF_R | elf::PF_X).to_le_bytes());
    ph[8..16].copy_from_slice(&(body_off as u64).to_le_bytes());
    ph[16..24].copy_from_slice(&0x40_0000u64.to_le_bytes());
    ph[24..32].copy_from_slice(&0x40_0000u64.to_le_bytes());
    ph[32..40].copy_from_slice(&(code.len() as u64).to_le_bytes());
    ph[40..48].copy_from_slice(&(code.len() as u64).to_le_bytes());
    v.extend_from_slice(&ph);
    body_off += code.len();
    let mut ph = vec![0u8; 56];
    ph[0..4].copy_from_slice(&4u32.to_le_bytes());
    ph[8..16].copy_from_slice(&(body_off as u64).to_le_bytes());
    ph[32..40].copy_from_slice(&(segment.len() as u64).to_le_bytes());
    ph[40..48].copy_from_slice(&(segment.len() as u64).to_le_bytes());
    ph[48..56].copy_from_slice(&4u64.to_le_bytes());
    v.extend_from_slice(&ph);
    v.extend_from_slice(&code);
    v.extend_from_slice(segment);
    v
}

/// A packaged program carrying `desc`'s note. Leaked so the `Program`'s borrowed bytes outlive
/// the test frame, which is the one allocation a test makes and never frees.
fn program(desc: &[u8]) -> package_archive::Program<'static> {
    let bytes: &'static [u8] = std::boxed::Box::leak(elf_image(desc).into_boxed_slice());
    package_archive::Program {
        name: "seeker",
        bytes,
        digest: [7u8; 32],
        member: 0,
    }
}

#[test]
fn a_note_asking_the_network_and_one_zone_yields_both_in_record_order() {
    const ZONES: &[&str] = &["nife.test"];
    const DESC: capability_manifest_note::ServicedNote<
        { capability_manifest_note::serviced_note_len(ZONES) },
    > = capability_manifest_note::ServicedNote::of::<
        { capability_manifest_note::serviced_len(ZONES) },
    >(&MANIFEST_NETWORK, ZONES, false);
    let p = program(&DESC.0);
    let asks: Vec<Granted<'_>> = asked_grants_of(&p).collect();
    assert_eq!(
        asks,
        [Granted::Network, Granted::Zone("nife.test")],
        "the network, then the zones, as the record spells them"
    );
    // The keyed form carries the same asks on the program's digest, which is what an install
    // writes into the record.
    let grants: Vec<installed_programs::Grant<'_>> = asked_grants(&p).collect();
    assert_eq!(grants.len(), asks.len());
    assert!(grants.iter().all(|g| g.digest == [7u8; 32]));
}

#[test]
fn a_service_ask_yields_the_service_grant() {
    const DESC: capability_manifest_note::ServicedNote<
        { capability_manifest_note::serviced_note_len(&[]) },
    > = capability_manifest_note::ServicedNote::of::<{ capability_manifest_note::serviced_len(&[]) }>(
        &MANIFEST_PLAIN,
        &[],
        true,
    );
    let p = program(&DESC.0);
    let asks: Vec<Granted<'_>> = asked_grants_of(&p).collect();
    assert_eq!(asks, [Granted::Service]);
}

#[test]
fn a_serviced_note_yields_the_network_the_service_and_each_zone() {
    const ZONES: &[&str] = &["a.test", "b.c.test"];
    const DESC: capability_manifest_note::ServicedNote<
        { capability_manifest_note::serviced_note_len(ZONES) },
    > = capability_manifest_note::ServicedNote::of::<
        { capability_manifest_note::serviced_len(ZONES) },
    >(&MANIFEST_NETWORK, ZONES, true);
    let p = program(&DESC.0);
    let asks: Vec<Granted<'_>> = asked_grants_of(&p).collect();
    assert_eq!(
        asks,
        [
            Granted::Network,
            Granted::Service,
            Granted::Zone("a.test"),
            Granted::Zone("b.c.test"),
        ],
        "the network, then the service, then the zones, as the record walks them"
    );
}

#[test]
fn a_program_without_a_readable_note_asks_for_nothing() {
    // Not an ELF at all, and an ELF with no PT_NOTE: the loader's bytes, no manifest. Both run
    // with no grants, which is the same rule `endowed_image` applies at spawn.
    let mut bare_elf = [0u8; 64];
    bare_elf[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    bare_elf[4] = 2;
    bare_elf[5] = 1;
    for bytes in [&b"not an elf"[..], &bare_elf[..]] {
        let owned: &'static [u8] = std::boxed::Box::leak(bytes.to_vec().into_boxed_slice());
        let p = package_archive::Program {
            name: "noteless",
            bytes: owned,
            digest: [0u8; 32],
            member: 0,
        };
        assert_eq!(asked_grants_of(&p).count(), 0);
    }
}
