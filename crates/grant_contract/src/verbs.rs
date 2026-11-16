//! **Authorities by verb** (milestone 809 (the package client becomes a program), its option V2): a
//! program's manifest narrowed by the word a line gives after its name, so one program holds, per
//! line, only what that verb needs. `jig list` reads the catalog; `jig install` also holds the
//! installer and the network. The spawner holds each program's table, and
//! `capability_manifest_note`'s verb record is the same list as bytes.
//!
//! Name: provisional (milestone 809), as are the module, the types and the bit positions.

use crate::{IndexStoreDeclaration, Manifest};

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
/// the whole of a verb table, in the spawner's row for a program or in its note (`capability_manifest_note`'s
/// verb record, which is the same list as bytes). Name: provisional.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VerbGrant {
    /// The word, as typed after the program's name, or that word, one space and the word after
    /// it, for a verb whose option changes what it holds (`list -u`). At most [`MAXIMUM_VERB`]
    /// bytes. [`narrow_by_verb`] tries the two words first and then the first alone.
    pub verb: &'static str,
    /// What a line with this verb is endowed with, of what the manifest declares.
    pub authorities: ManifestAuthorities,
}

/// The longest verb a verb table may hold, in bytes: `capability_manifest_note`'s record gives each sixteen.
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_VERB: usize = 16;

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

/// **The key [`narrow_by_verb`] looks up for a verb and the word after it**, copied into `out`: the
/// verb, then a space and the word after it when both fit in [`MAXIMUM_VERB`] bytes, which is the
/// key a verb with an option has (`list -u`). `None` for an empty verb, one longer than
/// [`MAXIMUM_VERB`], or one that holds a space itself. [`verb_of_page`] reads its two words off an
/// argument page, and a planner that reads them off a command line calls this with the same two.
///
/// Name: provisional; public since lane `milestone/1094-grant-plan-split`, so that a planner in
/// another crate builds the same key.
pub fn verb_key<'o>(
    verb: &[u8],
    option: Option<&[u8]>,
    out: &'o mut [u8; MAXIMUM_VERB],
) -> Option<&'o [u8]> {
    if verb.is_empty() || verb.len() > MAXIMUM_VERB || verb.contains(&b' ') {
        return None;
    }
    out[..verb.len()].copy_from_slice(verb);
    let mut len = verb.len();
    if let Some(option) = option.filter(|o| !o.is_empty() && len + 1 + o.len() <= MAXIMUM_VERB) {
        out[len] = b' ';
        out[len + 1..len + 1 + option.len()].copy_from_slice(option);
        len += 1 + option.len();
    }
    Some(&out[..len])
}

/// **The approval a delegated line asks to act within** (milestone 911 (scripted installs grant
/// only what the owner's approval file allows)): the word after `--within` on the argument page,
/// copied into `out`, its length returned. `None` when the line says no `--within`, when the
/// name is empty, or when it is longer than thirty-two bytes, which is the store's component
/// bound. The progenitor reads this when it places the installer endpoint, and the badge
/// carries the approval the name is stored under; `jig` reads its own copy of the same words to
/// know that its check is the progenitor's.
///
/// ```
/// use grant_contract::delegated_within;
/// let mut page = [0u8; program_argument_contract::PAGE_BYTES];
/// let mut builder = program_argument_contract::PageBuilder::new(&mut page);
/// for word in ["jig", "install", "--within", "lab-runner", "greeting"] {
///     builder.push(word.as_bytes()).unwrap();
/// }
/// builder.finish();
/// let mut out = [0u8; 32];
/// let n = delegated_within(&page, &mut out).unwrap();
/// assert_eq!(&out[..n], b"lab-runner");
/// ```
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub fn delegated_within(page: &[u8], out: &mut [u8; 32]) -> Option<usize> {
    let args = program_argument_contract::ArgPage::parse(page);
    let mut words = args.iter().skip(1);
    while let Some(word) = words.next() {
        if word == b"--within" {
            let name = words.next()?;
            if name.is_empty() || name.len() > 32 || name.contains(&b' ') {
                return None;
            }
            out[..name.len()].copy_from_slice(name);
            return Some(name.len());
        }
    }
    None
}

/// **The verb on an argument page**: its `argv[1]`, and `argv[2]` after a space when both fit,
/// as the program will read them. `grant_plan::verb_of`'s reading of the page `grant_plan::argv`
/// assembled from the line, which is what the progenitor holds; `None` for a page that does not
/// parse, has one word, or whose second is longer than [`MAXIMUM_VERB`] or holds a space.
pub fn verb_of_page<'o>(page: &[u8], out: &'o mut [u8; MAXIMUM_VERB]) -> Option<&'o [u8]> {
    let args = program_argument_contract::ArgPage::parse(page);
    let mut words = args.iter().skip(1);
    let verb = words.next()?;
    verb_key(verb, words.next(), out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DirectoryGrantDeclaration, FileGrantDeclaration, InputDeclaration, MemoryGrantDeclaration,
        NO_SHORT_OPTIONS, OutputDeclaration, ProgramArgumentDeclaration, StartupContract,
    };

    /// A manifest that declares nothing, for a test to set the fields it is about.
    const NOTHING: Manifest = Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
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
        startup: StartupContract::Std,
    };

    /// An argument page carrying `words`, as a spawner receives one.
    fn page(words: &[&[u8]]) -> [u8; program_argument_contract::PAGE_BYTES] {
        let mut page = [0u8; program_argument_contract::PAGE_BYTES];
        let mut builder = program_argument_contract::PageBuilder::new(&mut page);
        for word in words {
            builder.push(word).unwrap();
        }
        builder.finish();
        page
    }

    /// **A page's verb fits the key buffer, and an empty word is no verb.** The shell's reading
    /// caps a word at [`MAXIMUM_VERB`] before it reaches [`verb_key`], but the progenitor reads the
    /// page, where an argument may be any length: one byte too long would overrun `out`.
    #[test]
    fn a_page_verb_is_bounded_and_never_empty() {
        let read = |words: &[&[u8]]| {
            let page = page(words);
            let mut out = [0u8; MAXIMUM_VERB];
            verb_of_page(&page, &mut out).map(<[u8]>::to_vec)
        };
        assert_eq!(
            read(&[b"jig", b"sixteen-bytes-ab"]),
            Some(b"sixteen-bytes-ab".to_vec())
        );
        assert_eq!(read(&[b"jig", b"seventeen-bytes-a"]), None);
        assert_eq!(read(&[b"jig", b"", b"-u"]), None);
    }

    /// **Each authority a verb can choose is narrowed on its own.** A manifest declaring all twelve,
    /// and a table with one verb per authority: each verb keeps its one, and only that one.
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
            ..NOTHING
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

    /// **The approval a line acts within is a store name**: the word after `--within`, at most the
    /// store's thirty-two byte component, so thirty-two is a name and thirty-three is refused
    /// whole; a line that says no `--within` names none.
    #[test]
    fn a_delegated_line_names_an_approval_of_at_most_thirty_two_bytes() {
        let named = |words: &[&[u8]], out: &mut [u8; 32]| delegated_within(&page(words), out);
        let mut out = [0u8; 32];
        assert_eq!(
            named(
                &[b"jig", b"install", b"--within", b"lab-runner", b"greeting"],
                &mut out
            ),
            Some(10)
        );
        assert_eq!(&out[..10], b"lab-runner");
        let at_the_bound = [b'n'; 32];
        assert_eq!(
            named(&[b"jig", b"install", b"--within", &at_the_bound], &mut out),
            Some(32)
        );
        let past_it = [b'n'; 33];
        assert_eq!(
            named(&[b"jig", b"install", b"--within", &past_it], &mut out),
            None
        );
        assert_eq!(named(&[b"jig", b"install", b"greeting"], &mut out), None);
    }
}
