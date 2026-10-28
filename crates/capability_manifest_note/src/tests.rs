//! **The tests of the note's grammar**: version 1's layout, the zones of version 2, the
//! service ask of version 3, the verb record, and the note the `elf` reader finds.
//!
//! Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.

use grant_plan::Program;

use super::*;

/// **The subtree-grants note round-trips, and one encoding is the only encoding** (milestone
/// 606, ruling T1): the note's own descriptor decodes to its scope, and a wrong version, scope
/// or length is refused rather than read as "eligible".
#[test]
fn a_subtree_grants_note_has_one_encoding() {
    let note = SubtreeGrantsNote::of(Scope::SubtreeScope);
    let desc = &note.0[HEADER_LEN + NAME_FIELD..];
    assert_eq!(decode_subtree_grants(desc), Ok(Scope::SubtreeScope));
    assert_eq!(
        u32::from_le_bytes(note.0[8..12].try_into().unwrap()),
        SUBTREE_GRANTS
    );
    let mut bad = [0u8; SUBTREE_GRANTS_LEN];
    bad.copy_from_slice(desc);
    bad[4] = 2;
    assert_eq!(decode_subtree_grants(&bad), Err(Error::BadField(4)));
    bad[4] = 1;
    bad[0] = 2;
    assert_eq!(decode_subtree_grants(&bad), Err(Error::UnknownVersion));
    assert_eq!(decode_subtree_grants(&desc[..7]), Err(Error::WrongLength));
    assert_eq!(decode_subtree_grants(&[1, 0]), Err(Error::NoVersion));
    assert_eq!(Scope::SubtreeScope.declared(), "subtree_scope");
}

/// Every manifest the tree compiles in has a spelling, and reads back as itself. A program
/// added to `Program` with a manifest this layout cannot carry fails here rather than in a build
/// that tries to carry it.
#[test]
fn every_compiled_in_manifest_round_trips() {
    for p in Program::ALL {
        let m = p.manifest();
        // The two manifests with no note, by design (milestones 805 and 809): see `encode`.
        if m.reboot
            || m.sync
            || m.installer
            || m.catalog
            || m.indexes != IndexStoreDeclaration::Forbidden
            || m.activation_directory
        {
            continue;
        }
        assert_eq!(decode(&encode(&m)), Ok(m), "{}", p.name());
    }
    let m = grant_plan::UNVOUCHED_MANIFEST;
    assert_eq!(decode(&encode(&m)), Ok(m));
    // Each word grant a program that hears words may declare (milestone 205).
    for g in [WordGrant::ReadOnly, WordGrant::ReadWrite, WordGrant::Create] {
        let m = Manifest {
            program_argument: ProgramArgumentDeclaration::Words(g),
            ..grant_plan::UNVOUCHED_STD_MANIFEST
        };
        assert_eq!(decode(&encode(&m)), Ok(m), "{g:?}");
    }
}

#[test]
fn a_later_version_is_refused_rather_than_guessed_at() {
    let mut d = encode(&Program::Uptime.manifest());
    d[0] = 9;
    assert_eq!(decode(&d), Err(Error::UnknownVersion));
    assert_eq!(decode(&d[..3]), Err(Error::NoVersion));
    // Version 3 is known now (milestone 911 (scripted installs grant only what the owner's
    // approval file allows)); a version 1 descriptor with 3 in its version word is a
    // WrongLength, since 3's layout is longer.
    let mut d = encode(&Program::Uptime.manifest());
    d[0] = 3;
    assert_eq!(decode(&d), Err(Error::WrongLength));
}

#[test]
fn a_descriptor_one_byte_long_or_short_is_refused() {
    let d = encode(&Program::Uptime.manifest());
    assert_eq!(decode(&d[..DESCRIPTOR_LEN - 1]), Err(Error::WrongLength));
    let mut long = [0u8; DESCRIPTOR_LEN + 1];
    long[..DESCRIPTOR_LEN].copy_from_slice(&d);
    assert_eq!(decode(&long), Err(Error::WrongLength));
}

#[test]
fn every_field_refuses_a_value_outside_its_range() {
    let base = encode(&Program::Uptime.manifest());
    for at in [
        ARG,
        MEM,
        FILE,
        DIR,
        OUTPUT,
        INPUT,
        REPORTS,
        INTERRUPTIBLE,
        CLOCK,
        DOMAIN,
        CONFIG,
        ENTROPY,
        NETWORK,
        RUNTIME,
    ] {
        let mut d = base;
        d[at] = 0xff;
        assert_eq!(decode(&d), Err(Error::BadField(at)), "offset {at}");
    }
}

#[test]
fn a_byte_the_layout_says_is_zero_is_refused() {
    let base = encode(&Program::Uptime.manifest());
    // Uptime declares no memory, no input, no directory and no options, so all of these are
    // absent fields, and the three trailing bytes are always zero.
    for at in [
        MEM_MINIMUM,
        MEM_MAXIMUM + 7,
        WRITES_WHILE_READING,
        SUBTREE,
        FLAG_LETTERS,
        TAIL,
        DESCRIPTOR_LEN - 1,
    ] {
        let mut d = base;
        d[at] = 1;
        assert_eq!(decode(&d), Err(Error::BadField(at)), "offset {at}");
    }
}

#[test]
fn options_are_distinct_letters_and_the_subtree_option_is_one_of_them() {
    let rm = encode(&Program::Rm.manifest());
    assert_eq!(&rm[FLAG_LETTERS..FLAG_LETTERS + 3], b"rfv");
    assert_eq!(rm[SUBTREE], b'r');

    let mut twice = rm;
    twice[FLAG_LETTERS + 1] = b'r';
    assert_eq!(decode(&twice), Err(Error::BadField(FLAG_LETTERS)));

    let mut undeclared = rm;
    undeclared[SUBTREE] = b'q';
    assert_eq!(decode(&undeclared), Err(Error::BadField(SUBTREE)));

    let mut too_many = rm;
    too_many[FLAG_COUNT] = MAXIMUM_DECLARED_SHORT_OPTIONS as u8 + 1;
    assert_eq!(decode(&too_many), Err(Error::BadField(FLAG_COUNT)));
}

#[test]
fn a_memory_range_upside_down_is_refused() {
    let m = Manifest {
        memory_grant: MemoryGrantDeclaration::Required {
            minimum: 4,
            maximum: 8,
        },
        ..Program::Uptime.manifest()
    };
    let mut d = encode(&m);
    assert_eq!(decode(&d), Ok(m));
    d[MEM_MINIMUM] = 9;
    assert_eq!(decode(&d), Err(Error::BadField(MEM_MINIMUM)));
}

#[test]
fn the_note_is_what_elf_finds() {
    let n = Note::of(&Program::Uptime.manifest());
    assert_eq!(n.0.len(), NOTE_LEN);
    assert_eq!(&n.0[0..4], &5u32.to_le_bytes());
    assert_eq!(&n.0[4..8], &(DESCRIPTOR_LEN as u32).to_le_bytes());
    assert_eq!(&n.0[8..12], &MANIFEST.to_le_bytes());
    assert_eq!(&n.0[12..20], b"nife\0\0\0\0");
    assert_eq!(decode(&n.0[20..]), Ok(Program::Uptime.manifest()));
}

/// The manifest every field of which is at a non-default value except those a test sets, so a
/// field moved to the wrong byte or dropped shows as a different manifest.
fn base() -> Manifest {
    grant_plan::NO_NOTE_MANIFEST
}

/// **Every value of every enumerated field encodes and decodes to itself**, one field at a
/// time. A `decode` arm for a value no round trip names is an arm nobody would notice losing:
/// the descriptor would then be refused as malformed, and only for programs that use it.
#[test]
fn every_value_of_every_field_round_trips() {
    let args = [
        ProgramArgumentDeclaration::Forbidden,
        ProgramArgumentDeclaration::Required,
        ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
        ProgramArgumentDeclaration::Words(WordGrant::ReadWrite),
        ProgramArgumentDeclaration::Words(WordGrant::Create),
    ];
    for arg in args {
        let m = Manifest {
            program_argument: arg,
            ..base()
        };
        assert_eq!(decode(&encode(&m)), Ok(m), "{arg:?}");
    }
    for file in [
        FileGrantDeclaration::Forbidden,
        FileGrantDeclaration::Required { writable: false },
        FileGrantDeclaration::Required { writable: true },
    ] {
        let m = Manifest { file, ..base() };
        assert_eq!(decode(&encode(&m)), Ok(m), "{file:?}");
    }
    for dir in [
        DirectoryGrantDeclaration::Forbidden,
        DirectoryGrantDeclaration::Required {
            subtree_option: None,
        },
    ] {
        let m = Manifest {
            directory: dir,
            ..base()
        };
        assert_eq!(decode(&encode(&m)), Ok(m), "{dir:?}");
    }
    for output in [
        OutputDeclaration::Silent,
        OutputDeclaration::Words,
        OutputDeclaration::Bytes,
        OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
    ] {
        let m = Manifest { output, ..base() };
        assert_eq!(decode(&encode(&m)), Ok(m), "{output:?}");
    }
}

/// A second stream is declared at one slot only, and a manifest naming another has no spelling:
/// the constant evaluation that would reject it is a compile error, and here a panic.
#[test]
#[should_panic(expected = "DIAGNOSTICS_SLOT")]
fn a_second_stream_at_any_other_slot_has_no_encoding() {
    let m = Manifest {
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT + 1,
        },
        ..base()
    };
    let _ = encode(&m);
}

/// The reboot object has no spelling in a note (milestone 805): a program carrying one that
/// asked for it fails to build, rather than shipping a request no reader may grant.
#[test]
#[should_panic(expected = "cannot carry `reboot`")]
fn the_reboot_object_has_no_encoding() {
    let _ = encode(&grant_plan::Program::Reboot.manifest());
}

/// A subtree option is accepted when it is one of the declared letters, including when it is
/// the only one, and has no encoding when it is not.
#[test]
fn a_subtree_option_must_be_a_declared_letter_even_the_only_one() {
    let with = |letters: &[u8], flag: u8| Manifest {
        short_options: ShortOptions::try_new(letters).unwrap(),
        directory: DirectoryGrantDeclaration::Required {
            subtree_option: Some(flag),
        },
        ..base()
    };
    let only = with(b"r", b'r');
    assert_eq!(decode(&encode(&only)), Ok(only));
    let second = with(b"fr", b'r');
    assert_eq!(decode(&encode(&second)), Ok(second));
    extern crate std;
    let undeclared = std::panic::catch_unwind(|| encode(&with(b"r", b'x')));
    assert!(undeclared.is_err());
}

/// The boundaries of the layout, each at the value that is last to be accepted: a descriptor of
/// four bytes has a version and nothing else (wrong length, not "no version"), the longest
/// option list and an empty memory range are accepted, and the last byte of the layout is
/// checked to be zero.
#[test]
fn the_layout_accepts_its_largest_values_and_checks_its_last_byte() {
    assert_eq!(decode(&[1, 2, 3]), Err(Error::NoVersion));
    assert_eq!(decode(&VERSION.to_le_bytes()), Err(Error::WrongLength));
    assert_eq!(decode(&9u32.to_le_bytes()), Err(Error::UnknownVersion));

    let letters: [u8; MAXIMUM_DECLARED_SHORT_OPTIONS] = core::array::from_fn(|i| b'a' + i as u8);
    let m = Manifest {
        short_options: ShortOptions::try_new(&letters).unwrap(),
        ..base()
    };
    assert_eq!(decode(&encode(&m)), Ok(m));

    let m = Manifest {
        memory_grant: MemoryGrantDeclaration::Required {
            minimum: 8,
            maximum: 8,
        },
        ..base()
    };
    assert_eq!(decode(&encode(&m)), Ok(m));

    let mut d = encode(&base());
    let last = d.len() - 1;
    d[last] = 1;
    assert_eq!(decode(&d), Err(Error::BadField(last)));
}

/// The whole note's header says what an ELF reader expects, for both note types: the owner's
/// length including its NUL, the descriptor's length, and the type.
#[test]
fn both_notes_state_their_owner_length_descriptor_length_and_type() {
    let n = SubtreeGrantsNote::of(Scope::SubtreeScope);
    assert_eq!(&n.0[0..4], &5u32.to_le_bytes());
    assert_eq!(&n.0[4..8], &(SUBTREE_GRANTS_LEN as u32).to_le_bytes());
    assert_eq!(&n.0[8..12], &SUBTREE_GRANTS.to_le_bytes());
    assert_eq!(&n.0[12..20], b"nife\0\0\0\0");
    assert_eq!(&n.0[20..24], &SUBTREE_GRANTS_VERSION.to_le_bytes());
    assert_eq!(&n.0[24..28], &1u32.to_le_bytes());
}

/// `put4` lays four bytes forward from `at`, in order, touching nothing else. The notes' own
/// words are all under 256, so only a word with four different bytes tells `at + i` from
/// `at - i`. Mutation survivor triage, milestone 326 (turn a mutation score upward), batch 3.
#[test]
fn put4_writes_four_bytes_forward_and_nothing_else() {
    let mut out = [0u8; 12];
    put4(&mut out, 4, [1, 2, 3, 4]);
    assert_eq!(out, [0, 0, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0]);
}

/// **A version 2 descriptor has one reading** (calef's 2026-10-19 (UTC) ruling on #2405): it
/// decodes to the manifest its first 56 bytes spell, its zones come back in order, and each way
/// a second spelling of the same ask could arise is refused: no zones (that is version 1),
/// unsorted or repeated zones, uppercase, an end or doubled dot, more than eight, or a byte past
/// the last zone.
#[test]
fn a_zoned_descriptor_has_one_encoding() {
    extern crate std;
    use std::vec::Vec;
    const ZONES: &[&str] = &["", "basalt.nifeos.org", "example.net"];
    const M: Manifest = Manifest {
        network: true,
        ..grant_plan::NO_NOTE_MANIFEST
    };
    const D: [u8; zoned_len(ZONES)] = encode_zoned(&M, ZONES);
    assert_eq!(decode(&D), Ok(M));
    let z = zones(&D).unwrap();
    assert!(z.has_root());
    assert_eq!(
        z.iter().collect::<Vec<_>>(),
        [b"".as_slice(), b"basalt.nifeos.org", b"example.net"]
    );
    assert!(!z.is_empty());
    assert!(
        zones(&encode(&M)).unwrap().is_empty(),
        "version 1 asks none"
    );
    const NO_ROOT: &[&str] = &["nife.test"];
    const D2: [u8; zoned_len(NO_ROOT)] = encode_zoned(&M, NO_ROOT);
    assert!(!zones(&D2).unwrap().has_root());
    const EIGHT: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "h"];
    const D8: [u8; zoned_len(EIGHT)] = encode_zoned(&M, EIGHT);
    assert_eq!(
        zones(&D8).unwrap().iter().count(),
        MAXIMUM_ZONES,
        "eight is the most, and allowed"
    );
    let unsorted =
        std::panic::catch_unwind(|| encode_zoned::<{ zoned_len(&["b", "a"]) }>(&M, &["b", "a"]));
    assert!(
        unsorted.is_err(),
        "encode refuses unsorted zones, as decode does"
    );

    let respell = |zones: &[&[u8]]| {
        let mut d: Vec<u8> = D[..DESCRIPTOR_LEN].to_vec();
        d.push(zones.len() as u8);
        for z in zones {
            d.push(z.len() as u8);
            d.extend_from_slice(z);
        }
        decode(&d)
    };
    assert_eq!(respell(&[b"", b"basalt.nifeos.org", b"example.net"]), Ok(M));
    for bad in [
        &[][..],
        &[b"example.net".as_slice(), b"basalt.nifeos.org"],
        &[b"example.net".as_slice(), b"example.net"],
        &[b"Example.net".as_slice()],
        &[b"example.net.".as_slice()],
        &[b".example.net".as_slice()],
        &[b"example..net".as_slice()],
        &[b"a", b"b", b"c", b"d", b"e", b"f", b"g", b"h", b"i"],
    ] {
        assert!(
            matches!(respell(bad), Err(Error::BadField(_))),
            "{bad:?} must be refused"
        );
    }
    let mut long = D.to_vec();
    long.push(0);
    assert_eq!(decode(&long), Err(Error::WrongLength));
    assert_eq!(decode(&D[..D.len() - 1]), Err(Error::WrongLength));
}

/// **The note around a version 2 descriptor is what `elf` finds**: its `descsz` is the
/// descriptor's exact length, and the padding after it is zero.
#[test]
fn a_zoned_note_names_its_exact_length_and_pads_to_four() {
    const ZONES: &[&str] = &["nife.test"];
    const NOTE: ZonedNote<{ zoned_note_len(ZONES) }> =
        ZonedNote::of::<{ zoned_len(ZONES) }>(&grant_plan::NO_NOTE_MANIFEST, ZONES);
    let n = &NOTE.0;
    // 56 + 1 + 1 + 9 = 67 bytes of descriptor after a 20-byte header and owner, rounded to 88.
    assert_eq!(n.len(), 88);
    assert_eq!(
        u32::from_le_bytes([n[0], n[1], n[2], n[3]]),
        5,
        "namesz: `nife` and its NUL"
    );
    let descsz = u32::from_le_bytes([n[4], n[5], n[6], n[7]]) as usize;
    assert_eq!(descsz, 67);
    assert_eq!(descsz, zoned_len(ZONES));
    let d = &n[HEADER_LEN + NAME_FIELD..HEADER_LEN + NAME_FIELD + descsz];
    assert_eq!(
        zones(d).unwrap().iter().next(),
        Some(b"nife.test".as_slice())
    );
    assert!(
        n[HEADER_LEN + NAME_FIELD + descsz..]
            .iter()
            .all(|&b| b == 0)
    );
}

/// **The service ask is version 3's and only its** (milestone 911 (scripted installs grant
/// only what the owner's approval file allows)): a manifest that asks to run as a service
/// encodes and reads back, with zones or without (which only version 3 can say), a version 1
/// or 2 descriptor reads as asking none, and a byte this reader does not know, a missing
/// service byte, or a trailing one is refused whole.
#[test]
fn the_service_ask_is_version_threes_and_only_its() {
    const ZONES: &[&str] = &["nife.test"];
    const D: [u8; serviced_len(ZONES)] =
        encode_serviced(&grant_plan::NO_NOTE_MANIFEST, ZONES, true);
    assert_eq!(decode(&D), Ok(grant_plan::NO_NOTE_MANIFEST));
    assert_eq!(zones(&D).unwrap().iter().next(), Some(&b"nife.test"[..]));
    assert_eq!(service(&D), Ok(true));

    const NONE: [u8; serviced_len(&[])] = encode_serviced(&grant_plan::NO_NOTE_MANIFEST, &[], true);
    assert_eq!(
        zones(&NONE).unwrap().iter().count(),
        0,
        "version 3 asks no zone"
    );
    assert_eq!(service(&NONE), Ok(true));
    const OFF: [u8; serviced_len(&[])] = encode_serviced(&grant_plan::NO_NOTE_MANIFEST, &[], false);
    assert_eq!(service(&OFF), Ok(false));

    let v1 = encode(&grant_plan::NO_NOTE_MANIFEST);
    assert_eq!(service(&v1), Ok(false));
    const ZONED: [u8; zoned_len(ZONES)] = encode_zoned(&grant_plan::NO_NOTE_MANIFEST, ZONES);
    assert_eq!(service(&ZONED), Ok(false));

    let mut unknown = D;
    *unknown.last_mut().unwrap() = 2;
    let mut trailing = [0u8; 69];
    trailing[..D.len()].copy_from_slice(&D);
    let cut = &D[..D.len() - 1];
    assert!(matches!(decode(&unknown), Err(Error::BadField(_))));
    assert_eq!(service(&unknown), Err(decode(&unknown).unwrap_err()));
    // The zones without the service byte: the byte at the cut is what faults.
    assert_eq!(decode(cut), Err(Error::BadField(67)));
    assert_eq!(service(cut), Err(decode(cut).unwrap_err()));
    assert_eq!(decode(&trailing), Err(Error::WrongLength));
    assert_eq!(service(&trailing), Err(decode(&trailing).unwrap_err()));
}

/// **The version 3 note names its exact length and pads to four**, as the version 2 note
/// does: the `elf` reader's `descsz` is the descriptor's own.
#[test]
fn a_serviced_note_names_its_exact_length_and_pads_to_four() {
    const NOTE: ServicedNote<{ serviced_note_len(&[]) }> =
        ServicedNote::of::<{ serviced_len(&[]) }>(&grant_plan::NO_NOTE_MANIFEST, &[], true);
    let n = &NOTE.0;
    // 20 bytes of header and owner, then 58 of descriptor, rounded to four: 80.
    assert_eq!(n.len(), 80);
    let descsz = u32::from_le_bytes([n[4], n[5], n[6], n[7]]) as usize;
    assert_eq!(descsz, serviced_len(&[]));
    assert_eq!(descsz, 58);
    assert_eq!(
        service(&n[HEADER_LEN + NAME_FIELD..HEADER_LEN + NAME_FIELD + descsz]),
        Ok(true)
    );
    assert!(
        n[HEADER_LEN + NAME_FIELD + descsz..]
            .iter()
            .all(|&b| b == 0)
    );
}
