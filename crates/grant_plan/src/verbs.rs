//! **Grants by verb** (milestone 809 (the package client becomes a program), its option V2): a
//! program's manifest narrowed by the word a line gives after its name, so one program holds, per
//! line, only what that verb needs. `jig list` reads the catalog; `jig install` also holds the
//! installer and the network. [`crate::Program::verbs`] holds each program's table, and
//! `capability_manifest_note`'s verb record is the same list as bytes.
//!
//! Name: provisional (milestone 809), as are the module, the types and the bit positions.

use crate::{IndexStore, Manifest, each_word};

/// **The authorities a verb table may hand out**, as bits (milestone 809 (the package client
/// becomes a program), option V2). Each names one of [`Manifest`]'s fields the progenitor endows
/// from the manifest rather than from the line: nothing a person designates, so a verb is the only
/// thing on the line that can choose among them. A verb never widens: [`narrow_by_verb`] keeps a
/// field only when the manifest declares it **and** the verb names it.
///
/// Name: provisional (milestone 809), as are the bit positions, which `capability_manifest_note`'s verb
/// record carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Grants(u16);

impl Grants {
    /// Nothing. What a word that is no verb in the table is granted.
    pub const NONE: Grants = Grants(0);
    /// [`Manifest::clock`].
    pub const CLOCK: Grants = Grants(1 << 0);
    /// [`Manifest::config`].
    pub const CONFIG: Grants = Grants(1 << 1);
    /// [`Manifest::entropy`].
    pub const ENTROPY: Grants = Grants(1 << 2);
    /// [`Manifest::network`].
    pub const NETWORK: Grants = Grants(1 << 3);
    /// [`Manifest::domain`].
    pub const DOMAIN: Grants = Grants(1 << 4);
    /// [`Manifest::machine`].
    pub const MACHINE: Grants = Grants(1 << 5);
    /// [`Manifest::share`].
    pub const SHARE: Grants = Grants(1 << 6);
    /// [`Manifest::installer`].
    pub const INSTALLER: Grants = Grants(1 << 7);
    /// [`Manifest::catalog`].
    pub const CATALOG: Grants = Grants(1 << 8);
    /// [`Manifest::indexes`], read only (milestone 801 (packages over the internet)). Name:
    /// provisional 2026-10-12 (UTC), as is the bit.
    pub const INDEXES: Grants = Grants(1 << 9);
    /// [`Manifest::indexes`], read and written: `jig update`. Holds nothing without
    /// [`Grants::INDEXES`]. Name: provisional 2026-10-12 (UTC), as is the bit.
    pub const INDEXES_WRITE: Grants = Grants(1 << 10);
    /// Every bit above, and no other.
    pub const ALL: Grants = Grants((1 << 11) - 1);

    /// Both sets.
    pub const fn union(self, other: Grants) -> Grants {
        Grants(self.0 | other.0)
    }

    /// Whether every bit of `other` is in this set.
    pub const fn contains(self, other: Grants) -> bool {
        self.0 & other.0 == other.0
    }

    /// The bits, as a verb record carries them.
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// The set these bits name, or `None` if any bit names no authority.
    pub const fn from_bits(bits: u16) -> Option<Grants> {
        if bits & !Grants::ALL.0 == 0 {
            Some(Grants(bits))
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
    /// The word, as typed after the program's name. At most [`MAX_VERB`] bytes.
    pub verb: &'static str,
    /// What a line with this verb is endowed with, of what the manifest declares.
    pub grants: Grants,
}

/// The longest verb a verb table may hold, in bytes: `capability_manifest_note`'s record gives each sixteen.
pub const MAX_VERB: usize = 16;

/// **`jig`'s verbs** (milestone 809; the spellings are apt's, ruled 2026-10-10 by calef), sorted
/// by their bytes, which `capability_manifest_note`'s record requires. The
/// least each needs: `list` the catalog, `remove` and `rollback` the installer, `install` all three
/// of the installer, the catalog (to resolve a name and check what arrives) and the network.
///
/// Since milestone 801 (packages over the internet): `update` holds the network, the clock and
/// entropy (TLS to the index) and writes the index store; `install` reads the store too, and
/// takes the clock and entropy, since a name the index copy lists is fetched over TLS; `list`
/// reads the store beside the catalog.
pub(crate) const JIG_VERBS: &[VerbGrant] = &[
    VerbGrant {
        verb: "install",
        grants: Grants::INSTALLER
            .union(Grants::CATALOG)
            .union(Grants::NETWORK)
            .union(Grants::INDEXES)
            .union(Grants::CLOCK)
            .union(Grants::ENTROPY),
    },
    VerbGrant {
        verb: "list",
        grants: Grants::CATALOG.union(Grants::INDEXES),
    },
    VerbGrant {
        verb: "remove",
        grants: Grants::INSTALLER,
    },
    VerbGrant {
        verb: "rollback",
        grants: Grants::INSTALLER,
    },
    VerbGrant {
        verb: "update",
        grants: Grants::NETWORK
            .union(Grants::INDEXES)
            .union(Grants::INDEXES_WRITE)
            .union(Grants::CLOCK)
            .union(Grants::ENTROPY),
    },
];

/// **A manifest narrowed by the verb a line used** (milestone 809 (the package client becomes a
/// program), option V2, which calef's "One program." ruling of 2026-10-06 kept): with an empty
/// `verbs` the manifest is returned as it is, and otherwise each field [`Grants`] names is kept only
/// when the manifest declares it and the verb's entry names it. A `verb` that is not in the table,
/// or none, keeps none of them: the program runs, holding only what no verb chooses, and says what
/// it takes.
///
/// So one program holds, per line, what 281 (`watch` holds exactly what `ps` holds)'s test asks of
/// two: `jig list` cannot reach the installer or the network, because that line was not granted
/// them. The shell calls this to plan and preview, and the progenitor to endow, each with the word
/// it read; they read the same argv, so they agree.
pub fn narrow_by_verb(m: &Manifest, verbs: &[VerbGrant], verb: Option<&[u8]>) -> Manifest {
    if verbs.is_empty() {
        return *m;
    }
    let g = verbs
        .iter()
        .find(|v| Some(v.verb.as_bytes()) == verb)
        .map_or(Grants::NONE, |v| v.grants);
    Manifest {
        clock: m.clock && g.contains(Grants::CLOCK),
        config: m.config && g.contains(Grants::CONFIG),
        entropy: m.entropy && g.contains(Grants::ENTROPY),
        network: m.network && g.contains(Grants::NETWORK),
        domain: m.domain && g.contains(Grants::DOMAIN),
        machine: m.machine && g.contains(Grants::MACHINE),
        share: m.share && g.contains(Grants::SHARE),
        installer: m.installer && g.contains(Grants::INSTALLER),
        catalog: m.catalog && g.contains(Grants::CATALOG),
        indexes: match m.indexes {
            IndexStore::ReadWrite if g.contains(Grants::INDEXES.union(Grants::INDEXES_WRITE)) => {
                IndexStore::ReadWrite
            }
            IndexStore::ReadWrite | IndexStore::ReadOnly if g.contains(Grants::INDEXES) => {
                IndexStore::ReadOnly
            }
            _ => IndexStore::Forbidden,
        },
        ..*m
    }
}

/// **The line's verb**: its second word, quotes off, as the program will hear it as `argv[1]`
/// (`each_word`'s reading, which [`argv`](crate::argv) assembles the page from). `None` past [`MAX_VERB`] bytes
/// or when there is no second word. Copied into `out`, because a quoted word's text is a temporary.
pub fn verb_of<'o>(line: &[u8], out: &'o mut [u8; MAX_VERB]) -> Option<&'o [u8]> {
    let mut seen = 0usize;
    let mut len = None;
    let _ = each_word(line, &mut |w| {
        seen += 1;
        if seen == 2 && w.len() <= MAX_VERB {
            out[..w.len()].copy_from_slice(w);
            len = Some(w.len());
        }
        Ok(())
    });
    len.map(|n| &out[..n])
}

/// **The verb on an argument page**: its `argv[1]`, as the program will read it. [`verb_of`]'s
/// reading of the page [`argv`](crate::argv) assembled from the line, which is what the progenitor holds; `None`
/// for a page that does not parse, has one word, or whose second is longer than [`MAX_VERB`].
pub fn verb_of_page(page: &[u8]) -> Option<&[u8]> {
    program_argument_contract::ArgPage::parse(page)
        .iter()
        .nth(1)
        .filter(|w| w.len() <= MAX_VERB)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Program, Runtime, argv, image_can_carry};

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
        assert_eq!(as_typed(b"jig update").indexes, IndexStore::ReadWrite);
        assert!(!as_typed(b"jig update").installer);
        assert_eq!(list.indexes, IndexStore::ReadOnly);
        assert_eq!(install.indexes, IndexStore::ReadOnly);
        assert_eq!(remove.indexes, IndexStore::Forbidden);
        // The progenitor's reading of the page the shell assembles agrees with the shell's.
        let mut page = [0u8; program_argument_contract::PAGE_BYTES];
        argv(b"jig 'install' greeting", &mut page).unwrap();
        assert_eq!(verb_of_page(&page), Some(&b"install"[..]));
        let nothing = as_typed(b"jig upgrade");
        assert!(!nothing.installer && !nothing.catalog && !nothing.network);
        assert_eq!(nothing.indexes, IndexStore::Forbidden);
        assert_eq!(as_typed(b"jig"), nothing);
        // Everything else about the manifest is untouched.
        assert_eq!(list.arg, jig.arg);
        assert_eq!(list.runtime, jig.runtime);
        for &p in Program::ALL {
            let m = p.manifest();
            for v in p.verbs() {
                assert!(v.verb.len() <= MAX_VERB, "{}", v.verb);
                // Everything the verb names survives the narrowing: it names only what the
                // manifest declares.
                let narrowed = narrow_by_verb(&m, p.verbs(), Some(v.verb.as_bytes()));
                let kept = [
                    (narrowed.clock, Grants::CLOCK),
                    (narrowed.config, Grants::CONFIG),
                    (narrowed.entropy, Grants::ENTROPY),
                    (narrowed.network, Grants::NETWORK),
                    (narrowed.domain, Grants::DOMAIN),
                    (narrowed.machine, Grants::MACHINE),
                    (narrowed.share, Grants::SHARE),
                    (narrowed.installer, Grants::INSTALLER),
                    (narrowed.catalog, Grants::CATALOG),
                    (narrowed.indexes != IndexStore::Forbidden, Grants::INDEXES),
                    (
                        narrowed.indexes == IndexStore::ReadWrite,
                        Grants::INDEXES_WRITE,
                    ),
                ]
                .into_iter()
                .filter(|(held, _)| *held)
                .fold(Grants::NONE, |g, (_, bit)| g.union(bit));
                assert_eq!(kept, v.grants, "{} {}", p.name(), v.verb);
                assert!(!narrowed.machine, "{}", p.name());
            }
            if p.verbs().is_empty() {
                assert_eq!(narrow_by_verb(&m, p.verbs(), Some(b"x")), m);
            }
        }
        assert_eq!(Grants::from_bits(Grants::ALL.bits()), Some(Grants::ALL));
        assert_eq!(Grants::from_bits(1 << 15), None);
    }

    /// **Each authority a verb can choose is narrowed on its own.** A manifest declaring all eleven,
    /// and a table with one verb per authority: each verb keeps its one, and only that one.
    #[test]
    fn each_field_is_narrowed_by_its_own_bit() {
        let all = Manifest {
            clock: true,
            config: true,
            entropy: true,
            network: true,
            domain: true,
            machine: true,
            share: true,
            installer: true,
            catalog: true,
            indexes: IndexStore::ReadWrite,
            ..Program::Jig.manifest()
        };
        let one = |verb: &'static str, grants| VerbGrant { verb, grants };
        let table = [
            one("a", Grants::CLOCK),
            one("b", Grants::CONFIG),
            one("c", Grants::ENTROPY),
            one("d", Grants::NETWORK),
            one("e", Grants::DOMAIN),
            one("f", Grants::MACHINE),
            one("g", Grants::SHARE),
            one("h", Grants::INSTALLER),
            one("i", Grants::CATALOG),
            one("j", Grants::INDEXES),
            one("k", Grants::INDEXES.union(Grants::INDEXES_WRITE)),
        ];
        for v in &table {
            let n = narrow_by_verb(&all, &table, Some(v.verb.as_bytes()));
            let held = [
                n.clock,
                n.config,
                n.entropy,
                n.network,
                n.domain,
                n.machine,
                n.share,
                n.installer,
                n.catalog,
                n.indexes != IndexStore::Forbidden,
                n.indexes == IndexStore::ReadWrite,
            ];
            let bits = held
                .iter()
                .enumerate()
                .filter(|(_, h)| **h)
                .fold(0u16, |b, (i, _)| b | 1 << i);
            assert_eq!(bits, v.grants.bits(), "verb {}", v.verb);
        }
    }

    /// The set of every authority is the eleven bits and no twelfth, and a union keeps a bit both
    /// sides hold.
    #[test]
    fn all_is_eleven_bits_and_a_union_keeps_shared_bits() {
        assert_eq!(Grants::ALL.bits(), 0x7ff);
        assert_eq!(Grants::from_bits(1 << 11), None);
        assert_eq!(Grants::CLOCK.union(Grants::CLOCK), Grants::CLOCK);
        assert_eq!(
            Grants::CLOCK
                .union(Grants::CLOCK.union(Grants::SHARE))
                .bits(),
            Grants::CLOCK.bits() | Grants::SHARE.bits()
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
                m.indexes != IndexStore::Forbidden,
                p == Program::Jig,
                "{}",
                p.name()
            );
            if m.installer || m.catalog || m.indexes != IndexStore::Forbidden {
                assert_eq!(m.runtime, Runtime::Std, "{}", p.name());
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
        assert!(image_can_carry(&Manifest {
            installer: false,
            catalog: false,
            indexes: IndexStore::Forbidden,
            network: false,
            ..jig
        }));
    }
}
