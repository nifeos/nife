//! Tests of what an image's bytes are bound and endowed with.

use super::*;
use crate::*;

/// **The unvouched manifest is §219's ruling and nothing wider** (gate D2, calef 2026-09-26).
/// The two read-only pages are allowed; the process domain, entropy and the network are not;
/// nothing a line could designate is accepted either, which is the no-note manifest's shape.
#[test]
fn the_unvouched_manifest_allows_the_two_pages_and_nothing_of_the_progenitors() {
    let m = UNVOUCHED_MANIFEST;
    assert!(m.clock && m.config, "the ruling allows both pages");
    assert!(!m.domain, "never the process domain");
    assert!(!m.entropy, "never entropy");
    assert!(!m.network, "never the network");
    assert!(!m.reboot, "never the reboot object");
    assert!(!m.sync, "never a sync");
    let bound = NO_NOTE_MANIFEST;
    assert_eq!(
        (m.arg, m.mem, m.file, m.dir, m.input, m.output, m.startup),
        (
            bound.arg,
            bound.mem,
            bound.file,
            bound.dir,
            bound.input,
            bound.output,
            bound.startup
        ),
        "an unvouched child is endowed exactly what the shell bound the line against",
    );
}

/// **Exactly one program can restart the machine or ask for a sync alone, and no installed bytes
/// can** (milestone 805,
/// DECISIONS §251's mitigation for the foot gun in its method: a holder that skips the sync
/// loses writes, and the answer is that only one program is endowed). A second declarer, or an
/// image a note could endow, is that claim going false without anybody deciding it.
#[test]
fn only_reboot_declares_reboot_and_no_image_can() {
    for sync in [false, true] {
        let mut declarers = Program::ALL.iter().filter(|p| {
            let m = p.manifest();
            if sync { m.sync } else { m.reboot }
        });
        assert_eq!(declarers.next(), Some(&Program::Reboot));
        assert_eq!(
            declarers.next(),
            None,
            "a second program declares it (sync: {sync})"
        );
        let asks = Manifest {
            reboot: !sync,
            sync,
            ..NO_NOTE_MANIFEST
        };
        assert!(!image_can_carry(&asks));
        assert_eq!(
            image_manifest(Some(asks), true),
            Err(ImageRefusal::NotCarried)
        );
    }
}

/// **Vouched bytes get their note; unvouched bytes get the ruling, whatever the note says.**
/// The progenitor-endowed authorities an unvouched note asks for are simply not there (§219);
/// a line-designated one is a refusal, because the shell already bound it.
#[test]
fn an_image_is_endowed_from_its_note_only_when_vouched() {
    let asks_everything = Manifest {
        network: true,
        entropy: true,
        domain: true,
        clock: true,
        ..NO_NOTE_MANIFEST
    };
    assert_eq!(
        image_manifest(Some(asks_everything), true),
        Ok(asks_everything)
    );
    assert_eq!(
        image_manifest(Some(asks_everything), false),
        Ok(UNVOUCHED_MANIFEST),
        "an unvouched note grants nothing"
    );
    assert_eq!(
        image_manifest(None, true),
        Ok(NO_NOTE_MANIFEST),
        "no note, the default"
    );
    assert_eq!(image_manifest(None, false), Ok(UNVOUCHED_MANIFEST));

    let asks_an_argument = Program::LeastAuthorityDemo.manifest();
    assert_eq!(
        image_manifest(Some(asks_an_argument), true),
        Ok(asks_an_argument)
    );
    assert_eq!(
        image_manifest(Some(asks_an_argument), false),
        Err(ImageRefusal::ExceedsVouch),
        "a note that asks more than its vouch allows is refused"
    );
    let asks_memory = Manifest {
        mem: MemSpec::Required { min: 1, max: 4 },
        ..NO_NOTE_MANIFEST
    };
    assert_eq!(
        image_manifest(Some(asks_memory), false),
        Err(ImageRefusal::ExceedsVouch)
    );
}

/// Every shape the image wire cannot carry is refused before the verdict matters.
#[test]
fn an_image_manifest_the_request_cannot_carry_is_refused_vouched_or_not() {
    for p in [Program::Wc, Program::Rm, Program::Uuid, Program::Date] {
        let m = p.manifest();
        if image_can_carry(&m) {
            continue;
        }
        assert_eq!(
            image_manifest(Some(m), true),
            Err(ImageRefusal::NotCarried),
            "{}",
            p.name()
        );
        assert_eq!(
            image_manifest(Some(m), false),
            Err(ImageRefusal::NotCarried)
        );
    }
    assert!(
        !image_can_carry(&Program::Wc.manifest()),
        "wc reads an input"
    );
    assert!(
        !image_can_carry(&Program::Rm.manifest()),
        "rm takes a directory and options"
    );
    assert!(
        !image_can_carry(&Program::Uuid.manifest()),
        "uuid is a std program"
    );
    assert!(
        !image_can_carry(&Program::Date.manifest()),
        "date declares a second stream"
    );
    assert!(
        image_can_carry(&Program::LeastAuthorityDemo.manifest()),
        "an argument and a word in a register"
    );
}

#[test]
fn an_image_request_that_does_not_fit_its_manifest_is_refused() {
    let none = NO_NOTE_MANIFEST;
    assert!(image_request_fits(&none, 0, 0, false));
    assert!(
        !image_request_fits(&none, 5, 0, false),
        "an argument the manifest forbids"
    );
    assert!(
        !image_request_fits(&none, 0, 2, false),
        "memory the manifest forbids"
    );
    let mem = Manifest {
        mem: MemSpec::Required { min: 2, max: 4 },
        ..none
    };
    assert!(image_request_fits(&mem, 0, 2, false));
    assert!(!image_request_fits(&mem, 0, 5, false));
    assert!(
        !image_request_fits(&mem, 0, 0, false),
        "required memory not sent"
    );
    assert!(image_request_fits(
        &Program::LeastAuthorityDemo.manifest(),
        7,
        0,
        false
    ));
    // **The argv bit sized the region, so it must match the note** (milestone 205): a request
    // that carried words for a native manifest, or none for a `std` one, is refused.
    assert!(!image_request_fits(&none, 0, 0, true));
    let std = UNVOUCHED_STD_MANIFEST;
    assert!(image_request_fits(&std, 0, 0, true));
    assert!(!image_request_fits(&std, 0, 0, false));
}

/// **A `std` image is carried exactly when it hears words** (milestone 205), and unvouched it
/// gets the unvouched grants in the `std` layout whatever its note asks.
#[test]
fn a_std_image_is_carried_when_it_hears_words_and_unvouched_gets_two_pages() {
    let ex = Program::StdExerciser.manifest();
    assert!(image_can_carry(&ex));
    assert!(image_hears_words(&ex));
    assert_eq!(image_manifest(Some(ex), true), Ok(ex));
    // Unvouched, its note's entropy grants nothing (§219 (how the shell names an installed
    // program to the spawner)): the unvouched `std` manifest differs from the native one only
    // in the runtime and the words.
    assert_eq!(image_manifest(Some(ex), false), Ok(UNVOUCHED_STD_MANIFEST));
    assert_eq!(
        Manifest {
            arg: ArgSpec::Forbidden,
            startup: StartupContract::Native,
            ..UNVOUCHED_STD_MANIFEST
        },
        UNVOUCHED_MANIFEST
    );
    // Either half without the other is not carried.
    assert!(!image_can_carry(&Manifest {
        arg: ArgSpec::Forbidden,
        ..ex
    }));
    assert!(!image_can_carry(&Manifest {
        arg: ArgSpec::Words(WordGrant::ReadOnly),
        ..NO_NOTE_MANIFEST
    }));
}

// ---- survivor triage of the 2026-10-03 census ---------------------------------------------

/// What an image can carry in the `std` layout: bytes out, nothing a `std` layout cannot hold.
/// Each variant takes away one allowance and is refused for that alone.
#[test]
fn a_std_image_is_refused_for_each_thing_its_layout_cannot_hold() {
    let ok = Manifest {
        output: OutputSpec::Bytes,
        ..UNVOUCHED_STD_MANIFEST
    };
    assert!(image_can_carry(&ok), "the baseline must be carriable");
    assert!(!image_can_carry(&Manifest {
        output: OutputSpec::Words,
        ..ok
    }));
    assert!(!image_can_carry(&Manifest {
        mem: Program::MemoryGrantDepleter.manifest().mem,
        ..ok
    }));
    assert!(!image_can_carry(&Manifest { domain: true, ..ok }));
    assert!(!image_can_carry(&Manifest {
        network: true,
        ..ok
    }));
}
