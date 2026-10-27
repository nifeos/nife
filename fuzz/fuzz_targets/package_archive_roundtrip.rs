//! Fuzz the package **round trip**: write a package, parse it back, verify every member.
//!
//! §197 (a package is one archive file) owed the reader *"a fuzz target and the Kani treatment
//! `crates/nifefs` and `crates/elf` already carry."* This is the target, in `nifefs_roundtrip`'s
//! round-trip shape. **The property:** for any set of (name, bytes) the writer accepts, the file
//! parses, holds exactly that many members, reads each name back byte-identical, and passes its own
//! digest check; failing the last would make a verifier refuse a package for no reason a user could
//! act on. The input is structured, for `nifefs_roundtrip`'s reason.

#![no_main]

use libfuzzer_sys::fuzz_target;
use package_archive::{Attributes, MAX_MEMBERS, NAME_LEN, Package, package_size, write_package};

fuzz_target!(|input: (String, String, String, Vec<(String, Vec<u8>)>)| {
    let (name, version, architecture, members) = input;

    // The writer's own refusals, applied as filters so the target explores the region the writer
    // accepts. Each is pinned by example in the crate's host tests, so re-deriving them here would
    // spend the budget on settled questions. What is left is every legal input, including the
    // awkward legal ones: an empty name, a zero-byte member, a name of exactly NAME_LEN.
    let attributes = [&name, &version, &architecture];
    if attributes
        .iter()
        .any(|field| field.len() > NAME_LEN || field.as_bytes().contains(&0))
    {
        return;
    }
    if members.len() > MAX_MEMBERS {
        return;
    }
    if members
        .iter()
        .any(|(name, _)| name.len() > NAME_LEN || name.as_bytes().contains(&0))
    {
        return;
    }
    // Duplicates are the writer's refusal, not the reader's; filtered rather than asserted on.
    if (1..members.len()).any(|i| members[..i].iter().any(|(n, _)| *n == members[i].0)) {
        return;
    }
    // A cap on total bytes, so the fuzzer spends its time on structure rather than on SHA-256.
    if members.iter().map(|(_, bytes)| bytes.len()).sum::<usize>() > 1 << 20 {
        return;
    }

    let borrowed: Vec<(&str, &[u8])> = members
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    let attributes = Attributes {
        name: &name,
        version: &version,
        architecture: &architecture,
    };

    let mut file = vec![0u8; package_size(&borrowed)];
    write_package(&attributes, &borrowed, &mut file).expect("the filters above are the writer's");

    let package = Package::parse(&file).expect("a package this crate just wrote must parse");
    assert_eq!(package.name(), name, "the package lost its own name");
    assert_eq!(package.version(), version);
    assert_eq!(package.architecture(), architecture);
    assert_eq!(
        package.len(),
        members.len(),
        "a member was lost or invented"
    );

    for (index, (name, bytes)) in members.iter().enumerate() {
        assert_eq!(package.member_name(index), Some(name.as_str()));
        assert_eq!(
            package.member(index),
            Some(bytes.as_slice()),
            "{name:?} did not read back what was written"
        );
        assert_eq!(package.read(name), Some(bytes.as_slice()));
    }

    // The digests the writer wrote are the digests the reader computes. If this ever fails, every
    // recipe that vouched for a package vouched for nothing.
    package
        .verify()
        .expect("a freshly written package verifies");
});
