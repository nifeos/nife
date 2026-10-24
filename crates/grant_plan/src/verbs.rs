//! **Authorities by verb** (milestone 809 (the package client becomes a program), its option V2): a
//! program's manifest narrowed by the word a line gives after its name, so one program holds, per
//! line, only what that verb needs. `jig list` reads the catalog; `jig install` also holds the
//! installer and the network. [`crate::Program::verbs`] holds each program's table, and
//! `capability_manifest_note`'s verb record is the same list as bytes.
//!
//! Name: provisional (milestone 809), as are the module, the types and the bit positions.

use crate::{IndexStoreDeclaration, Manifest, each_word};

/// **The authorities a verb table may hand out**, as bits (milestone 809 (the package client
/// becomes a program), option V2). Each names one of [`Manifest`]'s fields the progenitor endows
/// from the manifest rather than from the line: nothing a person designates, so a verb is the only
/// thing on the line that can choose among them. A verb never widens: [`narrow_by_verb`] keeps a
/// field only when the manifest declares it **and** the verb names it.
///
/// Name: ratified 2026-10-18 (calef: "Yes"), for the type, its `authorities` field in
/// [`VerbGrant`] and its `CONFIGURATION` bit. The bit positions are provisional (milestone 809),
/// and `capability_manifest_note`'s verb record carries them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ManifestAuthorities(u16);

impl ManifestAuthorities {
    /// Nothing. What a word that is no verb in the table is granted.
    pub const NONE: ManifestAuthorities = ManifestAuthorities(0);
    /// [`Manifest::clock`].
    pub const CLOCK: ManifestAuthorities = ManifestAuthorities(1 << 0);
    /// [`Manifest::configuration`].
    pub const CONFIGURATION: ManifestAuthorities = ManifestAuthorities(1 << 1);
    /// [`Manifest::entropy`].
    pub const ENTROPY: ManifestAuthorities = ManifestAuthorities(1 << 2);
    /// [`Manifest::network`].
    pub const NETWORK: ManifestAuthorities = ManifestAuthorities(1 << 3);
    /// [`Manifest::domain`].
    pub const DOMAIN: ManifestAuthorities = ManifestAuthorities(1 << 4);
    /// [`Manifest::machine`].
    pub const MACHINE: ManifestAuthorities = ManifestAuthorities(1 << 5);
    /// [`Manifest::share`].
    pub const SHARE: ManifestAuthorities = ManifestAuthorities(1 << 6);
    /// [`Manifest::installer`].
    pub const INSTALLER: ManifestAuthorities = ManifestAuthorities(1 << 7);
    /// [`Manifest::catalog`].
    pub const CATALOG: ManifestAuthorities = ManifestAuthorities(1 << 8);
    /// [`Manifest::indexes`], read only (milestone 801 (packages over the internet)). Name:
    /// provisional 2026-10-12 (UTC), as is the bit.
    pub const INDEXES: ManifestAuthorities = ManifestAuthorities(1 << 9);
    /// [`Manifest::indexes`], read and written: `jig update`. Holds nothing without
    /// [`ManifestAuthorities::INDEXES`]. Name: provisional 2026-10-12 (UTC), as is the bit.
    pub const INDEXES_WRITE: ManifestAuthorities = ManifestAuthorities(1 << 10);
    /// [`Manifest::activation_directory`]: `activation/`, read only, in place of the index store
    /// (milestone 809, `jig list -u`).
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane `milestone/809-jig-list-u`, as is the bit.
    pub const ACTIVATION: ManifestAuthorities = ManifestAuthorities(1 << 11);
    /// Every bit above, and no other.
    pub const ALL: ManifestAuthorities = ManifestAuthorities((1 << 12) - 1);

    /// Both sets.
    pub const fn union(self, other: ManifestAuthorities) -> ManifestAuthorities {
        ManifestAuthorities(self.0 | other.0)
    }

    /// Whether every bit of `other` is in this set.
    pub const fn contains(self, other: ManifestAuthorities) -> bool {
        self.0 & other.0 == other.0
    }

    /// The bits, as a verb record carries them.
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// The set these bits name, or `None` if any bit names no authority.
    pub const fn from_bits(bits: u16) -> Option<ManifestAuthorities> {
        if bits & !ManifestAuthorities::ALL.0 == 0 {
            Some(ManifestAuthorities(bits))
        } else {
            None
        }
    }
}

/// **One verb and what it is granted** (milestone 809, option V2): `jig list` reads the catalog and
/// nothing else; `jig install` also holds the installer and the network. A list of these pairs is
/// the whole of a verb table, in a program's row ([`Program::verbs`](crate::Program::verbs)) or in its note (`capability_manifest_note`'s
/// verb record, which is the same list as bytes). Name: provisional.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VerbGrant {
    /// The word, as typed after the program's name, or that word, one space and the word after
    /// it, for a verb whose option changes what it holds (`list -u`). At most [`MAX_VERB`] bytes.
    /// [`narrow_by_verb`] tries the two words first and then the first alone.
    pub verb: &'static str,
    /// What a line with this verb is endowed with, of what the manifest declares.
    pub authorities: ManifestAuthorities,
}

/// The longest verb a verb table may hold, in bytes: `capability_manifest_note`'s record gives each sixteen.
pub const MAX_VERB: usize = 16;

/// **`jig`'s verbs** (milestone 809; the spellings are apt's, ruled 2026-10-10 by calef), sorted
/// by their bytes, which `capability_manifest_note`'s record requires. Each holds the least it
/// needs: `update` and `install` reach the index over TLS, so they take the network, the clock and
/// entropy; `install` also takes the installer, the catalog (to resolve a name and check what
/// arrives) and reads the index store, and `update` writes it. `remove` and `rollback` hold the
/// installer alone. `list` reads the catalog and the store, and `list -u` (calef, 2026-10-17 UTC)
/// only the activation directory, comparing what is installed with the index copy beneath it.
pub(crate) const JIG_VERBS: &[VerbGrant] = &[
    VerbGrant {
        verb: "install",
        authorities: ManifestAuthorities::INSTALLER
            .union(ManifestAuthorities::CATALOG)
            .union(ManifestAuthorities::NETWORK)
            .union(ManifestAuthorities::INDEXES)
            .union(ManifestAuthorities::CLOCK)
            .union(ManifestAuthorities::ENTROPY),
    },
    VerbGrant {
        verb: "list",
        authorities: ManifestAuthorities::CATALOG.union(ManifestAuthorities::INDEXES),
    },
    VerbGrant {
        verb: "list -u",
        authorities: ManifestAuthorities::ACTIVATION,
    },
    VerbGrant {
        verb: "remove",
        authorities: ManifestAuthorities::INSTALLER,
    },
    VerbGrant {
        verb: "rollback",
        authorities: ManifestAuthorities::INSTALLER,
    },
    VerbGrant {
        verb: "update",
        authorities: ManifestAuthorities::NETWORK
            .union(ManifestAuthorities::INDEXES)
            .union(ManifestAuthorities::INDEXES_WRITE)
            .union(ManifestAuthorities::CLOCK)
            .union(ManifestAuthorities::ENTROPY),
    },
];

/// **A manifest narrowed by the verb a line used** (milestone 809 (the package client becomes a
/// program), option V2, which calef's "One program." ruling of 2026-10-06 kept): with an empty
/// `verbs` the manifest is returned as it is, and otherwise each field [`ManifestAuthorities`] names is kept only
/// when the manifest declares it and the verb's entry names it. A `verb` not in the table, or none,
/// keeps none of them: the program runs holding only what no verb chooses, and says what it takes.
///
/// So `jig list` cannot reach the installer or the network, which is what 281 (`watch` holds
/// exactly what `ps` holds)'s test asks of two programs. The shell calls this to plan and preview,
/// and the progenitor to endow, each with the word it read from the same argv, so they agree.
pub fn narrow_by_verb(m: &Manifest, verbs: &[VerbGrant], verb: Option<&[u8]>) -> Manifest {
    if verbs.is_empty() {
        return *m;
    }
    let first = verb.map(|v| v.split(|&b| b == b' ').next().unwrap_or(v));
    let find = |key: Option<&[u8]>| verbs.iter().find(|v| Some(v.verb.as_bytes()) == key);
    let g = find(verb)
        .or_else(|| find(first))
        .map_or(ManifestAuthorities::NONE, |v| v.authorities);
    Manifest {
        clock: m.clock && g.contains(ManifestAuthorities::CLOCK),
        configuration: m.configuration && g.contains(ManifestAuthorities::CONFIGURATION),
        entropy: m.entropy && g.contains(ManifestAuthorities::ENTROPY),
        network: m.network && g.contains(ManifestAuthorities::NETWORK),
        domain: m.domain && g.contains(ManifestAuthorities::DOMAIN),
        machine: m.machine && g.contains(ManifestAuthorities::MACHINE),
        share: m.share && g.contains(ManifestAuthorities::SHARE),
        installer: m.installer && g.contains(ManifestAuthorities::INSTALLER),
        catalog: m.catalog && g.contains(ManifestAuthorities::CATALOG),
        activation_directory: m.activation_directory && g.contains(ManifestAuthorities::ACTIVATION),
        // **Standard input rides with the installer** (milestone 892 (a std program at the
        // prompt holds the network and a resolver)): the owner's yes at install is the one thing
        // read through it, so a verb that holds no installer reads nothing typed.
        console_input: m.console_input && g.contains(ManifestAuthorities::INSTALLER),
        indexes: match m.indexes {
            IndexStoreDeclaration::ReadWrite
                if g.contains(
                    ManifestAuthorities::INDEXES.union(ManifestAuthorities::INDEXES_WRITE),
                ) =>
            {
                IndexStoreDeclaration::ReadWrite
            }
            IndexStoreDeclaration::ReadWrite | IndexStoreDeclaration::ReadOnly
                if g.contains(ManifestAuthorities::INDEXES) =>
            {
                IndexStoreDeclaration::ReadOnly
            }
            _ => IndexStoreDeclaration::Forbidden,
        },
        ..*m
    }
}

/// **The line's verb**: its second word, quotes off, as the program will hear it as `argv[1]`
/// (`each_word`'s reading, which [`argv`](crate::argv) assembles the page from), followed by a
/// space and the third word when both fit in [`MAX_VERB`] bytes, which is the key a verb with an
/// option has (`list -u`). `None` when there is no second word, when it is longer than
/// [`MAX_VERB`], or when it holds a space itself, so a quoted `'list -u'` is no verb. Copied into
/// `out`, because a quoted word's text is a temporary.
pub fn verb_of<'o>(line: &[u8], out: &'o mut [u8; MAX_VERB]) -> Option<&'o [u8]> {
    let mut seen = 0usize;
    let mut held = [[0u8; MAX_VERB]; 2];
    let mut lens = [None; 2];
    let _ = each_word(line, &mut |w| {
        seen += 1;
        if (2..=3).contains(&seen) && w.len() <= MAX_VERB {
            held[seen - 2][..w.len()].copy_from_slice(w);
            lens[seen - 2] = Some(w.len());
        }
        Ok(())
    });
    let verb = &held[0][..lens[0]?];
    let option = lens[1].map(|n| &held[1][..n]);
    verb_key(verb, option, out)
}

/// The key [`narrow_by_verb`] looks up for a verb and the word after it: see [`verb_of`].
fn verb_key<'o>(
    verb: &[u8],
    option: Option<&[u8]>,
    out: &'o mut [u8; MAX_VERB],
) -> Option<&'o [u8]> {
    if verb.is_empty() || verb.len() > MAX_VERB || verb.contains(&b' ') {
        return None;
    }
    out[..verb.len()].copy_from_slice(verb);
    let mut len = verb.len();
    if let Some(option) = option.filter(|o| !o.is_empty() && len + 1 + o.len() <= MAX_VERB) {
        out[len] = b' ';
        out[len + 1..len + 1 + option.len()].copy_from_slice(option);
        len += 1 + option.len();
    }
    Some(&out[..len])
}

/// **The verb on an argument page**: its `argv[1]`, and `argv[2]` after a space when both fit,
/// as the program will read them. [`verb_of`]'s reading of the page [`argv`](crate::argv)
/// assembled from the line, which is what the progenitor holds; `None` for a page that does not
/// parse, has one word, or whose second is longer than [`MAX_VERB`] or holds a space.
pub fn verb_of_page<'o>(page: &[u8], out: &'o mut [u8; MAX_VERB]) -> Option<&'o [u8]> {
    let args = program_argument_contract::ArgPage::parse(page);
    let mut words = args.iter().skip(1);
    let verb = words.next()?;
    verb_key(verb, words.next(), out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Program, StartupContract, argv, image_can_carry};

    /// **A verb narrows and never widens** (milestone 809, option V2). `jig list` holds the catalog
    /// alone, `jig remove` the installer alone, `jig install` all three, and a word that is no verb
    /// nothing; and no verb names an authority the manifest does not declare. A program with a verb
    /// table declares no `machine`, the one such field the shell delivers rather than the
    /// progenitor, so a shell that read the unnarrowed manifest could not send it a page.
    #[test]
    fn a_verb_narrows_and_never_widens() {
        let jig = Program::Jig.manifest();
        let as_typed = |line: &[u8]| {
            let mut buf = [0u8; MAX_VERB];
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
        let mut read = [0u8; MAX_VERB];
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
        let mut read = [0u8; MAX_VERB];
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
                assert!(v.verb.len() <= MAX_VERB, "{}", v.verb);
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

    /// **Each authority a verb can choose is narrowed on its own.** A manifest declaring all twelve,
    /// and a table with one verb per authority: each verb keeps its one, and only that one.
    /// **A page's verb fits the key buffer, and an empty word is no verb.** The shell's reading
    /// caps a word at [`MAX_VERB`] before it reaches [`verb_key`], but the progenitor reads the
    /// page, where an argument may be any length: one byte too long would overrun `out`.
    #[test]
    fn a_page_verb_is_bounded_and_never_empty() {
        let read = |line: &[u8]| {
            let mut page = [0u8; program_argument_contract::PAGE_BYTES];
            argv(line, &mut page).unwrap();
            let mut out = [0u8; MAX_VERB];
            verb_of_page(&page, &mut out).map(<[u8]>::to_vec)
        };
        assert_eq!(
            read(b"jig sixteen-bytes-ab"),
            Some(b"sixteen-bytes-ab".to_vec())
        );
        assert_eq!(read(b"jig seventeen-bytes-a"), None);
        assert_eq!(read(b"jig '' -u"), None);
    }

    #[test]
    fn each_field_is_narrowed_by_its_own_bit() {
        let all = Manifest {
            clock: true,
            configuration: true,
            entropy: true,
            network: true,
            domain: true,
            machine: true,
            share: true,
            installer: true,
            catalog: true,
            indexes: IndexStoreDeclaration::ReadWrite,
            activation_directory: true,
            console_input: true,
            ..Program::Jig.manifest()
        };
        let one = |verb: &'static str, authorities| VerbGrant { verb, authorities };
        let table = [
            one("a", ManifestAuthorities::CLOCK),
            one("b", ManifestAuthorities::CONFIGURATION),
            one("c", ManifestAuthorities::ENTROPY),
            one("d", ManifestAuthorities::NETWORK),
            one("e", ManifestAuthorities::DOMAIN),
            one("f", ManifestAuthorities::MACHINE),
            one("g", ManifestAuthorities::SHARE),
            one("h", ManifestAuthorities::INSTALLER),
            one("i", ManifestAuthorities::CATALOG),
            one("j", ManifestAuthorities::INDEXES),
            one(
                "k",
                ManifestAuthorities::INDEXES.union(ManifestAuthorities::INDEXES_WRITE),
            ),
            one("l", ManifestAuthorities::ACTIVATION),
        ];
        for v in &table {
            let n = narrow_by_verb(&all, &table, Some(v.verb.as_bytes()));
            let held = [
                n.clock,
                n.configuration,
                n.entropy,
                n.network,
                n.domain,
                n.machine,
                n.share,
                n.installer,
                n.catalog,
                n.indexes != IndexStoreDeclaration::Forbidden,
                n.indexes == IndexStoreDeclaration::ReadWrite,
                n.activation_directory,
            ];
            let bits = held
                .iter()
                .enumerate()
                .filter(|(_, h)| **h)
                .fold(0u16, |b, (i, _)| b | 1 << i);
            assert_eq!(bits, v.authorities.bits(), "verb {}", v.verb);
            assert_eq!(
                n.console_input, n.installer,
                "stdin rides with the installer"
            );
        }
    }

    /// The set of every authority is the twelve bits and no thirteenth, and a union keeps a bit both
    /// sides hold.
    #[test]
    fn all_is_twelve_bits_and_a_union_keeps_shared_bits() {
        assert_eq!(ManifestAuthorities::ALL.bits(), 0xfff);
        assert_eq!(ManifestAuthorities::from_bits(1 << 12), None);
        assert_eq!(
            ManifestAuthorities::CLOCK.union(ManifestAuthorities::CLOCK),
            ManifestAuthorities::CLOCK
        );
        assert_eq!(
            ManifestAuthorities::CLOCK
                .union(ManifestAuthorities::CLOCK.union(ManifestAuthorities::SHARE))
                .bits(),
            ManifestAuthorities::CLOCK.bits() | ManifestAuthorities::SHARE.bits()
        );
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
}
