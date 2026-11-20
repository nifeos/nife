//! **Machine-checked proofs (Kani) of the manifest descriptor**: whatever `decode` accepts,
//! `encode` spells back byte for byte; whatever `encode` builds, `decode` reads back as the same
//! manifest; `decode` and `decode_subtree_grants` never panic; and `grants_for` finds each verb
//! `decode_verbs` accepts. Behind
//! `#[cfg(kani)]`, so only `cargo kani` compiles them.
//!
//! Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats,
//! after the `verification` module every other Kani crate here keeps out of line.

use super::*;

/// **Whatever `decode` accepts, `encode` spells back byte for byte**, over every descriptor the
/// solver can choose. This is the "one manifest, one encoding" claim: no two byte strings
/// decode to one manifest, so no two readers can disagree about which bytes meant what. The
/// unwind bound is the 56-byte comparison, plus one.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.a_decoded_manifest_encodes_back_to_its_bytes.patch`
#[kani::proof]
#[kani::unwind(57)]
fn a_decoded_manifest_encodes_back_to_its_bytes() {
    let d: [u8; DESCRIPTOR_BYTES] = kani::any();
    if let Ok(m) = decode(&d) {
        assert!(encode(&m) == d);
    }
}

/// **`decode` never panics, at any length up to one past the layout.** The bytes come from a
/// file anyone could have written.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.decode_never_panics.patch`
#[kani::proof]
#[kani::unwind(18)]
fn decode_never_panics() {
    let d: [u8; DESCRIPTOR_BYTES + 1] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= d.len());
    let _ = decode(&d[..len]);
}

/// **`decode` never panics through a version 2 or 3 descriptor, and neither do `zones` and
/// `asks_service` after it**, at every length from the 56-byte fixed part to 62 bytes, for six zone
/// layouts: no zone; one zone of one byte; one zone whose length byte runs past every length; the
/// root zone, then a zone of one byte; a zone of one byte, then one of two; and a zone count past
/// [`MAXIMUM_ZONES`]. Across those lengths the walk meets a length byte that fits and one that runs
/// past the end, a service byte and an end where the service byte should be, so it reaches both
/// zone checks (canonical, and sorted after the zone before), the service byte and the exact-length
/// check, which the 57-byte harness above stops short of.
///
/// The solver chooses the version word, the zones' letters and the service byte. The zone count,
/// the length bytes and the length are each case's own, and the rest of the fixed part is a plain
/// manifest's, because `decode_never_panics` and `a_decoded_manifest_encodes_back_to_its_bytes`
/// already choose every byte of it, and the zone walk reads none of it. Below 56 bytes there is
/// nothing more to see: `zones` and `asks_service` call `decode` first, which refuses a descriptor
/// shorter than its fixed part. With the count, a length byte or the length chosen by the solver
/// instead, every variant measured ran out of memory under a 12 GiB cap: the walk is then unrolled
/// for every count and every length at once.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.decode_never_panics_through_a_serviced_descriptor.patch`
#[kani::proof]
#[kani::unwind(18)]
fn decode_never_panics_through_a_serviced_descriptor() {
    zone_walk_never_panics(0, &[]);
    zone_walk_never_panics(1, &[1]);
    zone_walk_never_panics(1, &[5]);
    zone_walk_never_panics(2, &[0, 1]);
    zone_walk_never_panics(2, &[1, 2]);
    zone_walk_never_panics(MAXIMUM_ZONES as u8 + 1, &[]);
}

/// One case of `decode_never_panics_through_a_serviced_descriptor`: a zone count of `count` and
/// the zones' length bytes `lengths`, each placed where the walk will read it (one that falls past
/// the last byte is left out), at every length from the fixed part to [`SERVICED_BYTES`].
// Byte by byte on purpose: Kani keeps bytes written by index concrete, and a `copy_from_slice`
// or `fill` makes them all symbolic.
#[allow(clippy::needless_range_loop)]
fn zone_walk_never_panics(count: u8, lengths: &[u8]) {
    let mut d = PLAIN_SERVICED;
    d[0] = kani::any();
    d[1] = kani::any();
    d[2] = kani::any();
    d[3] = kani::any();
    for i in DESCRIPTOR_BYTES + 1..SERVICED_BYTES {
        d[i] = kani::any();
    }
    d[DESCRIPTOR_BYTES] = count;
    let mut at = DESCRIPTOR_BYTES + 1;
    for &length in lengths {
        if at < SERVICED_BYTES {
            d[at] = length;
        }
        at += 1 + length as usize;
    }
    for len in DESCRIPTOR_BYTES..=SERVICED_BYTES {
        let _ = zones(&d[..len]);
        let _ = asks_service(&d[..len]);
    }
}

/// **`grants_for` finds every verb `decode_verbs` accepts in a table of one or two verbs**, with
/// the grants the iterator gives. The search is correct only on a table sorted strictly, which is
/// what `decode_verbs` refuses to let through otherwise.
///
/// The solver chooses each record's grant bits, padding and first two word bytes, and the rest of
/// each word is zero, so a verb is one or two bytes long. Two verbs of one or two bytes reach every
/// order the sort check and the search compare: the first byte decides, the second does, or one
/// word is a prefix of the other. The table is whole, with a version word of [`VERBS_VERSION`] and
/// a count that matches its length, since `decode_verbs` refuses anything else before it reads a
/// record; an empty table has no verb to find. The search runs on the view `decode_verbs` returns
/// when it accepts, built here over the same records, because through `decode_verbs`'s own result
/// Kani cannot see that the table's length is fixed, and the run did not finish in 15 minutes.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.a_decoded_verb_table_finds_each_verb.patch`
#[kani::proof]
#[kani::unwind(18)]
// The version bytes are copied by index for the same reason as in `zone_walk_never_panics`.
#[allow(clippy::manual_memcpy, clippy::needless_range_loop)]
fn a_decoded_verb_table_finds_each_verb() {
    let mut d = [0u8; verbs_descriptor_bytes(2)];
    let version = VERBS_VERSION.to_le_bytes();
    for i in 0..4 {
        d[i] = version[i];
    }
    d[4] = 2;
    for record in 0..2 {
        let at = 8 + record * VERB_RECORD_BYTES;
        for i in [
            0,
            1,
            grant_contract::MAXIMUM_VERB_BYTES,
            grant_contract::MAXIMUM_VERB_BYTES + 1,
            grant_contract::MAXIMUM_VERB_BYTES + 2,
            grant_contract::MAXIMUM_VERB_BYTES + 3,
        ] {
            d[at + i] = kani::any();
        }
    }
    finds_each_verb(&d);
    let mut one = d;
    one[4] = 1;
    finds_each_verb(&one[..verbs_descriptor_bytes(1)]);
}

/// One table of `a_decoded_verb_table_finds_each_verb`: if `decode_verbs` accepts `d`, the search
/// finds each verb of it with the iterator's grants.
fn finds_each_verb(d: &[u8]) {
    if decode_verbs(d).is_ok() {
        let verbs = Verbs { records: &d[8..] };
        for (word, grants) in verbs.iter() {
            assert!(verbs.grants_for(word) == Some(grants));
        }
    }
}

/// **`decode_subtree_grants` never panics**, at any length up to one past its 8 bytes.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.decode_subtree_grants_never_panics.patch`
#[kani::proof]
#[kani::unwind(10)]
fn decode_subtree_grants_never_panics() {
    let d: [u8; SUBTREE_GRANTS_BYTES + 1] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= d.len());
    let _ = decode_subtree_grants(&d[..len]);
}

/// Any manifest a version 1 descriptor can spell, chosen field by field: letters `ShortOptions`
/// accepts, a subtree option that is one of them, the diagnostics slot, a memory range with its
/// minimum no greater than its maximum, and none of the grants only the owner's console gives.
fn any_manifest() -> Manifest {
    let letters: [u8; MAXIMUM_DECLARED_SHORT_OPTIONS] = kani::any();
    let count: usize = kani::any();
    kani::assume(count <= MAXIMUM_DECLARED_SHORT_OPTIONS);
    let short_options = ShortOptions::try_new(&letters[..count]);
    kani::assume(short_options.is_some());
    let short_options = short_options.unwrap();
    let program_argument = match kani::any::<u8>() % 5 {
        0 => ProgramArgumentDeclaration::Forbidden,
        1 => ProgramArgumentDeclaration::Required,
        2 => ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
        3 => ProgramArgumentDeclaration::Words(WordGrant::ReadWrite),
        _ => ProgramArgumentDeclaration::Words(WordGrant::Create),
    };
    let memory_grant = if kani::any() {
        let (minimum, maximum): (u64, u64) = (kani::any(), kani::any());
        kani::assume(minimum <= maximum);
        MemoryGrantDeclaration::Required { minimum, maximum }
    } else {
        MemoryGrantDeclaration::Forbidden
    };
    let file = if kani::any() {
        FileGrantDeclaration::Required {
            writable: kani::any(),
        }
    } else {
        FileGrantDeclaration::Forbidden
    };
    let directory = if kani::any() {
        let subtree_option = if kani::any() {
            let pick: usize = kani::any();
            kani::assume(pick < count);
            Some(letters[pick])
        } else {
            None
        };
        DirectoryGrantDeclaration::Required { subtree_option }
    } else {
        DirectoryGrantDeclaration::Forbidden
    };
    let output = match kani::any::<u8>() % 4 {
        0 => OutputDeclaration::Silent,
        1 => OutputDeclaration::Words,
        2 => OutputDeclaration::Bytes,
        _ => OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
    };
    let input = if kani::any() {
        InputDeclaration::Required {
            writes_while_reading: kani::any(),
        }
    } else {
        InputDeclaration::Forbidden
    };
    Manifest {
        program_argument,
        memory_grant,
        file,
        directory,
        short_options,
        output,
        input,
        reports: kani::any(),
        interruptible: kani::any(),
        clock: kani::any(),
        domain: kani::any(),
        configuration: kani::any(),
        entropy: kani::any(),
        network: kani::any(),
        machine: kani::any(),
        share: kani::any(),
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        activation_directory: false,
        console_input: false,
        startup: if kani::any() {
            StartupContract::Std
        } else {
            StartupContract::Native
        },
    }
}

/// **Whatever `encode` builds, `decode` reads back as the same manifest**, over every manifest a
/// descriptor can spell. The other direction of the harness above: together they are the claim
/// that one manifest has exactly one encoding, so a note the compiler put in a program is a note
/// the reader accepts.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.an_encoded_manifest_decodes_to_itself.patch`
#[kani::proof]
#[kani::unwind(18)]
fn an_encoded_manifest_decodes_to_itself() {
    let m = any_manifest();
    assert!(decode(&encode(&m)) == Ok(m));
}

/// **`encode` refuses a memory range whose minimum is past its maximum**, which `decode` would
/// refuse, so a program cannot build a note its reader rejects. A panic in `encode` is a compile
/// error in the constant `carry!` evaluates.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.an_inverted_memory_grant_does_not_encode.patch`
#[kani::proof]
#[kani::should_panic]
fn an_inverted_memory_grant_does_not_encode() {
    let (minimum, maximum): (u64, u64) = (kani::any(), kani::any());
    kani::assume(minimum > maximum);
    let m = Manifest {
        memory_grant: MemoryGrantDeclaration::Required { minimum, maximum },
        ..PLAIN
    };
    let _ = encode(&m);
}

/// How long `decode_never_panics_through_a_serviced_descriptor`'s descriptors run: the fixed
/// part, a zone count, one length byte and three letters, and a service byte.
const SERVICED_BYTES: usize = DESCRIPTOR_BYTES + 1 + 1 + 3 + 1;

/// [`PLAIN`]'s fixed part, then zeros to [`SERVICED_BYTES`].
const PLAIN_SERVICED: [u8; SERVICED_BYTES] = {
    let fixed = encode(&PLAIN);
    let mut out = [0; SERVICED_BYTES];
    let mut i = 0;
    while i < DESCRIPTOR_BYTES {
        out[i] = fixed[i];
        i += 1;
    }
    out
};

/// A manifest that asks for nothing, written out so the harness depends on no program's.
const PLAIN: Manifest = Manifest {
    program_argument: ProgramArgumentDeclaration::Forbidden,
    memory_grant: MemoryGrantDeclaration::Forbidden,
    file: FileGrantDeclaration::Forbidden,
    directory: DirectoryGrantDeclaration::Forbidden,
    short_options: ShortOptions::new(b""),
    output: OutputDeclaration::Silent,
    input: InputDeclaration::Forbidden,
    reports: false,
    interruptible: false,
    clock: false,
    domain: false,
    configuration: false,
    entropy: false,
    network: false,
    machine: false,
    share: false,
    reboot: false,
    sync: false,
    installer: false,
    catalog: false,
    indexes: IndexStoreDeclaration::Forbidden,
    activation_directory: false,
    console_input: false,
    startup: StartupContract::Native,
};
