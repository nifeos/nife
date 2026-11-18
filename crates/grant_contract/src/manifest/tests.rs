//! Tests of the manifest types.

use crate::*;

/// **The rights a word grant asks for are the filesystem contract's bits** (milestone 205).
/// `WordGrant::rights` spells them as numbers because this crate does not link the contract.
#[test]
fn word_rights_are_the_filesystem_contracts() {
    use filesystem_contract::dir;
    let read = dir::ENUMERATE | dir::READ | dir::DESCEND;
    assert_eq!(WordGrant::ReadOnly.rights(), read);
    assert_eq!(WordGrant::ReadWrite.rights(), read | dir::WRITE);
    assert_eq!(WordGrant::Create.rights(), read | dir::WRITE | dir::CREATE);
}

/// **The index store opens its directory with exactly a word grant's rights**: none when
/// forbidden, read-only's for `jig list` and `jig install`, and create's for `jig update`, whose
/// first run makes the copy. Pinned against the filesystem contract's bits, so a reading verb
/// that could write, or a writing one that could not create, fails here.
#[test]
fn the_index_store_opens_with_a_word_grants_rights() {
    use filesystem_contract::dir;
    let read = dir::ENUMERATE | dir::READ | dir::DESCEND;
    assert_eq!(IndexStoreDeclaration::Forbidden.rights(), None);
    assert_eq!(IndexStoreDeclaration::ReadOnly.rights(), Some(read));
    assert_eq!(
        IndexStoreDeclaration::ReadWrite.rights(),
        Some(read | dir::WRITE | dir::CREATE)
    );
}

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
