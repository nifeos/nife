//! Tests of the manifest types.

use crate::*;

// ---- survivor triage of the 2026-10-03 census ---------------------------------------------

/// An option set is at most sixteen distinct letters or digits, and sixteen is allowed.
#[test]
fn an_option_set_may_hold_exactly_its_maximum_and_no_repeat() {
    let sixteen = b"abcdefghijklmnop";
    assert_eq!(sixteen.len(), MAXIMUM_DECLARED_SHORT_OPTIONS);
    assert!(ShortOptions::try_new(sixteen).is_some());
    assert!(ShortOptions::try_new(b"abcdefghijklmnopq").is_none());
    assert!(ShortOptions::try_new(b"aa").is_none());
    assert!(ShortOptions::try_new(b"aba").is_none());
    assert!(ShortOptions::try_new(b"ab").is_some());
    assert!(ShortOptions::try_new(b"a-").is_none());
}

fn a_manifest_with(directory: DirectoryGrantDeclaration, options: &[u8]) -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory,
        short_options: ShortOptions::new(options),
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
    }
}

/// **A manifest is well formed when its widening option is one it declares, wherever in the list
/// that letter sits.** The first, a middle and the last letter are all found; a letter that is
/// not declared, in a list of any length including none, is refused (and never reads past the
/// end of the list).
#[test]
fn a_widening_option_must_be_one_the_manifest_declares() {
    let widening = |letter| DirectoryGrantDeclaration::Required {
        subtree_option: Some(letter),
    };
    for letter in *b"rfi" {
        assert!(a_manifest_with(widening(letter), b"rfi").is_well_formed());
    }
    assert!(!a_manifest_with(widening(b'x'), b"rfi").is_well_formed());
    assert!(!a_manifest_with(widening(b'r'), b"").is_well_formed());
    assert!(a_manifest_with(widening(b'r'), b"r").is_well_formed());
    let no_widening = DirectoryGrantDeclaration::Required {
        subtree_option: None,
    };
    assert!(a_manifest_with(no_widening, b"").is_well_formed());
    assert!(a_manifest_with(DirectoryGrantDeclaration::Forbidden, b"").is_well_formed());
}
