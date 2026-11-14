//! Tests of what this crate holds beside the rows: the region budgets, the unvouched manifests,
//! who is sent a share view, and `jig`'s verbs narrowed the way the shell and the progenitor narrow
//! them. Moved here from `grant_plan` with the items they test (milestone 1094 (an outside review of
//! `grant_plan`)).
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane milestone/1094-grant-plan-split.

use grant_plan::{argv, verb_of};

use crate::*;

/// **The region sizes the shell splits and the progenitor counts are exactly these pages**
/// (milestone 933 (every job is built the way a supervised job is)): a native job, a
/// directory-granted one, a `std` one, and the pool of six native regions and one `std` region. A
/// size that drifted on one side would make the progenitor refuse a region the shell split, or the
/// shell split one the progenitor wasted.
#[test]
fn the_region_a_job_is_held_in_has_exactly_the_pages_both_sides_count() {
    assert_eq!(JOB_REGION_PAGES, 60);
    assert_eq!(DIRECTORY_JOB_REGION_PAGES, 98);
    assert_eq!(STD_REGION_PAGES, 384);
    assert_eq!(JOB_POOL_PAGES, 6 * 60 + 384);
    let native = Program::LeastAuthorityDemo.manifest();
    assert_eq!(job_region_pages(&native, false), 60);
    assert_eq!(job_region_pages(&native, true), 98);
    let std = Program::StdExerciser.manifest();
    assert_eq!(job_region_pages(&std, false), 384);
    assert_eq!(job_region_pages(&std, true), 384);
}

/// **A held job is sent the shell's budget as its share view only when its program declares one**:
/// `free` and `slabtop` do, `date` and `ps` do not. Both sides ask this one function, so a wrong
/// answer in either direction sends a capability nobody reads or reads one nobody sent.
#[test]
fn only_a_program_that_declares_a_share_view_is_sent_one() {
    assert!(spawnproto::held_shares(Program::Free));
    assert!(spawnproto::held_shares(Program::Slabtop));
    assert!(!spawnproto::held_shares(Program::Date));
    assert!(!spawnproto::held_shares(Program::Ps));
    for p in Program::ALL {
        assert_eq!(spawnproto::held_shares(*p), p.manifest().share, "{p:?}");
    }
}

/// The `std` unvouched manifest is in the `std` layout, which is what makes it that manifest.
#[test]
fn the_std_unvouched_manifest_is_a_std_manifest() {
    assert_eq!(UNVOUCHED_STD_MANIFEST.startup, StartupContract::Std);
    assert_eq!(UNVOUCHED_MANIFEST.startup, StartupContract::Native);
}

/// A file's region is the image's own pages and the runtime's region, added.
#[test]
fn an_image_region_is_its_pages_plus_its_runtimes() {
    assert_eq!(STD_REGION_PAGES, 384);
    assert_eq!(image_region_pages(10, StartupContract::Std, 99), 10 + 384);
    assert_eq!(image_region_pages(10, StartupContract::Native, 99), 10 + 99);
    assert_eq!(image_region_pages(0, StartupContract::Native, 0), 0);
}

/// **A package manager's region carries one package more**: a named `std` program's region is
/// `STD_REGION_PAGES`, and one declaring the installer adds the largest image it may read.
#[test]
fn an_installer_region_holds_one_more_package() {
    let jig = Program::Jig.manifest();
    assert!(jig.installer);
    assert_eq!(
        named_std_region_pages(&jig),
        STD_REGION_PAGES + spawnproto::IMAGE_MAXIMUM_PAGES
    );
    let plain = Manifest {
        installer: false,
        ..jig
    };
    assert_eq!(named_std_region_pages(&plain), STD_REGION_PAGES);
}

/// **A verb narrows and never widens** (milestone 809, option V2). `jig list` holds the catalog
/// alone, `jig remove` the installer alone, `jig install` all three, and a word that is no verb
/// nothing; and no verb names an authority the manifest does not declare. A program with a verb
/// table declares no `machine`, the one such field the shell delivers rather than the
/// progenitor, so a shell that read the unnarrowed manifest could not send it a page.
#[test]
fn a_verb_narrows_and_never_widens() {
    let jig = Program::Jig.manifest();
    let as_typed = |line: &[u8]| {
        let mut buf = [0u8; MAXIMUM_VERB];
        narrow_by_verb(&jig, Program::Jig.verbs(), verb_of(line, &mut buf))
    };
    let list = as_typed(b"jig list");
    assert!(list.catalog && !list.installer && !list.network);
    let install = as_typed(b"jig install greeting@0.1.0");
    assert!(install.catalog && install.installer && install.network);
    let remove = as_typed(b"jig 'remove' noteless");
    assert!(remove.installer && !remove.catalog && !remove.network);
    // Milestone 801: only `update` writes the index store; `list` and `install` read it, and
    // `remove` holds none of it.
    assert_eq!(
        as_typed(b"jig update").indexes,
        IndexStoreDeclaration::ReadWrite
    );
    assert!(!as_typed(b"jig update").installer);
    assert_eq!(list.indexes, IndexStoreDeclaration::ReadOnly);
    assert_eq!(install.indexes, IndexStoreDeclaration::ReadOnly);
    assert_eq!(remove.indexes, IndexStoreDeclaration::Forbidden);
    // The progenitor's reading of the page the shell assembles agrees with the shell's.
    let mut page = [0u8; program_argument_contract::PAGE_BYTES];
    argv(b"jig 'install' greeting", &mut page).unwrap();
    let mut read = [0u8; MAXIMUM_VERB];
    assert_eq!(
        verb_of_page(&page, &mut read),
        Some(&b"install greeting"[..])
    );
    // **`list -u` holds the activation directory and nothing else** (milestone 809), and
    // only typed as two words: quoted as one, it is no verb at all.
    let outdated = as_typed(b"jig list -u");
    assert!(outdated.activation_directory && !outdated.catalog);
    assert_eq!(outdated.indexes, IndexStoreDeclaration::Forbidden);
    assert!(!list.activation_directory && !install.activation_directory);
    assert!(!as_typed(b"jig list --upgradable").activation_directory);
    assert_eq!(as_typed(b"jig 'list -u'"), as_typed(b"jig upgrade"));
    let mut page = [0u8; program_argument_contract::PAGE_BYTES];
    argv(b"jig list -u", &mut page).unwrap();
    let mut read = [0u8; MAXIMUM_VERB];
    assert_eq!(verb_of_page(&page, &mut read), Some(&b"list -u"[..]));
    let nothing = as_typed(b"jig upgrade");
    assert!(!nothing.installer && !nothing.catalog && !nothing.network);
    assert_eq!(nothing.indexes, IndexStoreDeclaration::Forbidden);
    assert_eq!(as_typed(b"jig"), nothing);
    // Everything else about the manifest is untouched.
    assert_eq!(list.program_argument, jig.program_argument);
    assert_eq!(list.startup, jig.startup);
    for &p in Program::ALL {
        let m = p.manifest();
        for v in p.verbs() {
            assert!(v.verb.len() <= MAXIMUM_VERB, "{}", v.verb);
            // Everything the verb names survives the narrowing: it names only what the
            // manifest declares.
            let narrowed = narrow_by_verb(&m, p.verbs(), Some(v.verb.as_bytes()));
            let kept = [
                (narrowed.clock, ManifestAuthorities::CLOCK),
                (narrowed.configuration, ManifestAuthorities::CONFIGURATION),
                (narrowed.entropy, ManifestAuthorities::ENTROPY),
                (narrowed.network, ManifestAuthorities::NETWORK),
                (narrowed.domain, ManifestAuthorities::DOMAIN),
                (narrowed.machine, ManifestAuthorities::MACHINE),
                (narrowed.share, ManifestAuthorities::SHARE),
                (narrowed.installer, ManifestAuthorities::INSTALLER),
                (narrowed.catalog, ManifestAuthorities::CATALOG),
                (
                    narrowed.indexes != IndexStoreDeclaration::Forbidden,
                    ManifestAuthorities::INDEXES,
                ),
                (
                    narrowed.indexes == IndexStoreDeclaration::ReadWrite,
                    ManifestAuthorities::INDEXES_WRITE,
                ),
                (
                    narrowed.activation_directory,
                    ManifestAuthorities::ACTIVATION,
                ),
            ]
            .into_iter()
            .filter(|(held, _)| *held)
            .fold(ManifestAuthorities::NONE, |g, (_, bit)| g.union(bit));
            assert_eq!(kept, v.authorities, "{} {}", p.name(), v.verb);
            assert!(!narrowed.machine, "{}", p.name());
            // The `std` layout has one directory slot: a verb holds the activation directory
            // or the index store, never both.
            assert!(
                !(narrowed.activation_directory
                    && narrowed.indexes != IndexStoreDeclaration::Forbidden),
                "{} {}",
                p.name(),
                v.verb
            );
        }
        if p.verbs().is_empty() {
            assert_eq!(narrow_by_verb(&m, p.verbs(), Some(b"x")), m);
        }
    }
    assert_eq!(
        ManifestAuthorities::from_bits(ManifestAuthorities::ALL.bits()),
        Some(ManifestAuthorities::ALL)
    );
    assert_eq!(ManifestAuthorities::from_bits(1 << 15), None);
}

/// **Exactly one program holds the installer endpoint, and it is a `std` program** (milestone
/// 809 (the package client becomes a program), calef's 2026-10-06 ruling: "by default only
/// `jig` holds the installer endpoint"). The progenitor places the endpoint, its reply
/// endpoint and the catalog page in the `std` layout only, so a native program declaring
/// either would be planned and not delivered; and no image may declare them.
#[test]
fn only_jig_holds_the_installer_and_the_catalog() {
    for &p in Program::ALL {
        let m = p.manifest();
        assert_eq!(m.installer, p == Program::Jig, "{}", p.name());
        assert_eq!(
            m.indexes != IndexStoreDeclaration::Forbidden,
            p == Program::Jig,
            "{}",
            p.name()
        );
        assert_eq!(m.activation_directory, p == Program::Jig, "{}", p.name());
        if m.installer || m.catalog || m.indexes != IndexStoreDeclaration::Forbidden {
            assert_eq!(m.startup, StartupContract::Std, "{}", p.name());
        }
    }
    let jig = Program::Jig.manifest();
    assert!(!image_can_carry(&jig));
    assert!(!image_can_carry(&Manifest {
        installer: false,
        ..jig
    }));
    // Milestone 801: the index store alone is enough to refuse an image.
    assert!(!image_can_carry(&Manifest {
        installer: false,
        catalog: false,
        network: false,
        ..jig
    }));
    // Milestone 809: so is the activation directory.
    assert!(!image_can_carry(&Manifest {
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        network: false,
        ..jig
    }));
    assert!(image_can_carry(&Manifest {
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        activation_directory: false,
        console_input: false,
        network: false,
        ..jig
    }));
}

/// **A typed name resolves to its row, and the row comes back beside the endowment** planned against
/// that row's manifest. The two answers that come before a name come first: a reserved isolation
/// form is refused as unbuilt, and a name with no row is no such program.
#[test]
fn a_typed_name_is_planned_against_its_row() {
    let run = |line: &'static [u8]| {
        let grant_plan::Command::Run(r) = grant_plan::parse(line) else {
            panic!("not an invocation")
        };
        r
    };
    let (program, endowment) = plan(
        &run(b"least_authority_demo 9"),
        Holdings::default(),
        expand::Expansion::none(),
    )
    .unwrap();
    assert_eq!(program, Program::LeastAuthorityDemo);
    assert_eq!(endowment.manifest, Program::LeastAuthorityDemo.manifest());
    assert_eq!(endowment.program_argument, 9);
    assert_eq!(
        plan(
            &run(b"swish -c date"),
            Holdings::default(),
            expand::Expansion::none()
        ),
        Err(Refusal::IsolationNotBuilt)
    );
    assert_eq!(
        plan(
            &run(b"no_such_thing"),
            Holdings::default(),
            expand::Expansion::none()
        ),
        Err(Refusal::NoSuchProgram)
    );
}
