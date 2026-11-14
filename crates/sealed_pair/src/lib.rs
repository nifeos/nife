//! **Does this kernel vouch for this archive?** The host-side half of measured boot: the kernel
//! compiles the digests of the archive entries it may enter into its `.rodata` (`kernel/build.rs`)
//! and refuses userspace whose bytes do not match (`kernel::trust::require`). [`inspect`] asks the
//! same question on the host by searching the kernel image for the digest of each entry it would
//! hand over, and checks the archive's measurement table against its own contents. See
//! notes/sealed-pair.md.

//! # BUGS
//!
//! - It duplicates the kernel's check; if they disagree, the kernel is right ([`Seal::explain`]).
//! - A refusal names the archive's digest, never the kernel's; `cargo xtask card-check` has it.
//! - Nothing checks a boot script or `extlinux.conf`, or forces a hand-written card through this.
//!   The loader's digest words are checked only when a caller passes them ([`inspect_with_words`]).
//! - `nifefs::write_image` does not refuse duplicate names, and an entry whose name is not UTF-8 is
//!   skipped: the first duplicate is hashed twice and the second never, and neither can be read by
//!   the kernel, so the verdict is right and the dead bytes go unreported.
//! - In the stick path the haystack contains the archive, so an archive entry that embedded another
//!   entry's raw digest would make a pair read as vouched with no digest in the kernel. No entry does.
//! - It asks whether a digest is present, not whether the kernel checks it; notes/sealed-pair.md.
//!
//! Name: provisional: the thing (a sealed pair), not the act. Refused `card_check` (three of the
//! checked places are not a card) and `seal` (a generic verb).

use std::fmt::Write as _;

use measured_boot::Digest;

/// The archive entries the kernel itself measures as a boot program, and therefore the ones its
/// trust root names: `progenitor` alone, which [`inspect`] requires of every archive. Every other
/// program the kernel enters, the six test child builders of milestone 19 (run a real workload)
/// included, is measured through the archive's table (`kernel::trust::require_program_in`). One
/// list for `xtask::measure`, `uefi_loader`'s build and [`inspect`]; see notes/sealed-pair.md, "One
/// list of boot programs".
pub const BOOT_PROGRAMS: [&str; 1] = [measured_boot::PROGENITOR];

/// Every archive entry a kernel image's trust root may name: the boot programs it can enter, plus
/// the measurement table it vouches for on the progenitor's behalf (milestone 104, init measures
/// what it loads).
///
/// The kernel never reads the table's contents. It hashes the entry and refuses to hand the archive
/// over when it is not the one this image was built against, which is what makes the progenitor's
/// own refusals worth as much as the kernel's.
pub fn measured_entries() -> [&'static str; BOOT_PROGRAMS.len() + 1] {
    let mut entries = [measured_boot::PROGRAM_MEASUREMENTS; BOOT_PROGRAMS.len() + 1];
    entries[..BOOT_PROGRAMS.len()].copy_from_slice(&BOOT_PROGRAMS);
    entries
}

/// Why a pair could not be judged at all, as distinct from being judged and refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreadable {
    /// The archive is not a `nifefs` image. A truncated copy onto a full card lands here, and so
    /// does a file that is not the archive.
    NotAnArchive(nifefs::Error),
    /// The archive parses but carries no `progenitor`, so no kernel can enter it. Not a mismatch:
    /// there is nothing for a kernel to vouch for.
    NoProgenitor,
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnArchive(e) => {
                write!(f, "not a nifefs archive ({})", archive_error_words(*e))
            }
            Self::NoProgenitor => {
                write!(f, "carries no `progenitor`, so no kernel can enter it")
            }
        }
    }
}

impl std::error::Error for Unreadable {}

/// One phrase for each way `nifefs` refuses an image, since `nifefs::Error` has no `Display`.
fn archive_error_words(e: nifefs::Error) -> &'static str {
    match e {
        nifefs::Error::BadMagic => "it does not start with the archive magic",
        nifefs::Error::TooManyFiles => "its directory claims more entries than an archive holds",
        nifefs::Error::NameTooLong => "an entry name is longer than the format allows",
        nifefs::Error::NameHasNul => "an entry name contains a NUL byte",
        nifefs::Error::OutOfBounds => "an entry's data runs past the end of the image",
        nifefs::Error::Truncated => "the image is shorter than the directory",
    }
}

/// What a kernel image says about one archive entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vouch {
    /// The digest of this entry's bytes, carried here, is in the kernel image. The kernel will
    /// enter it.
    Yes(Digest),
    /// The kernel image does not carry this digest of the archive's entry, carried here. The kernel
    /// halts at `MEASURED BOOT REFUSED` rather than enter it, which is the refusal this crate
    /// exists to move off the bench.
    No(Digest),
    /// The archive does not carry this entry. Not every archive carries every boot program, and a
    /// kernel refuses to enter a name it has no measurement for, so nothing is waved through.
    Absent,
}

/// One entry's verdict. The digest the archive's bytes actually have travels inside the verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySeal {
    /// The archive entry's name, as the trust root spells it.
    pub name: &'static str,
    /// Whether the kernel image carries the SHA-256 of the entry's bytes.
    pub vouch: Vouch,
}

/// A way the archive's own measurement table disagrees with the archive it is packed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableDisagreement {
    /// The table names this program with a digest that is not what the archive's copy hashes to.
    /// The progenitor treats it as absent, so the boot loses whatever that program provided.
    Changed(String),
    /// The archive carries this program and the table says nothing about it. Same outcome: the
    /// progenitor cannot vouch for it, so it is not loaded.
    Unlisted(String),
    /// The table names a program the archive does not carry. Harmless at boot, and a sign the
    /// table and the archive came from different builds.
    Missing(String),
    /// A line of the table that the one manifest parser rejects, kept whole. The progenitor skips
    /// the line, so only the program it named is refused, and the rest of the table still counts.
    Unparsable {
        /// The line as it stands in the table.
        line: String,
        /// The parser's reason, in its own words.
        reason: String,
    },
}

/// What the archive's own measurement table says about the archive (milestone 104's half of the
/// chain, which is the progenitor's refusal rather than the kernel's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableSeal {
    /// The archive carries no table. The kernel refuses to start a progenitor that can vouch for
    /// nothing, so this halts as surely as a digest mismatch does.
    Absent,
    /// The table is present but is not UTF-8. The progenitor reads an empty table, which vouches
    /// for nothing, and the console is refused. A line the parser rejects is not this: it is an
    /// [`TableDisagreement::Unparsable`] in a table that was read.
    Unreadable,
    /// The table was read. An empty `disagreements` is the healthy case.
    Checked {
        /// Every way the table and the archive fail to describe each other, sorted by name.
        disagreements: Vec<TableDisagreement>,
    },
}

/// What the loader's two digest words say about the archive.
///
/// Name: provisional, minted 2026-11-14 (UTC) by milestone/1101-1113-outside-review-fixes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordsSeal {
    /// No words were given to judge.
    NotGiven,
    /// The words do not parse. The kernel refuses them with the same reason.
    Refused(measured_boot::loader_digests::Refusal),
    /// The words parsed. The listed ones are not the digest of the archive's entry, which halts
    /// the boot at `MEASURED BOOT REFUSED`. Empty is the healthy case.
    Checked {
        /// Each word that disagrees with the archive.
        disagreeing: Vec<measured_boot::loader_digests::Word>,
    },
}

/// The whole verdict on one kernel-and-archive pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seal {
    /// One verdict per name in [`measured_entries`], in that order.
    pub entries: Vec<EntrySeal>,
    /// The progenitor's side of the chain.
    pub table: TableSeal,
    /// The loader's two words against the archive; [`WordsSeal::NotGiven`] when none were passed.
    pub words: WordsSeal,
}

impl Seal {
    /// **The kernel's decision**: would this kernel halt at `MEASURED BOOT REFUSED` on this archive?
    ///
    /// True when any entry the archive carries is one the kernel does not vouch for. An entry the
    /// archive does not carry is not a refusal here, because the kernel only measures what it is
    /// about to enter; a missing `progenitor` is refused earlier, by [`inspect`] itself.
    pub fn kernel_refuses(&self) -> bool {
        self.entries.iter().any(|e| matches!(e.vouch, Vouch::No(_)))
    }

    /// **The progenitor's decision**: would the first process fail to vouch for what it loads?
    ///
    /// A separate question from [`Seal::kernel_refuses`] on purpose. It fires after a boot the
    /// kernel allowed, and it costs the console rather than the whole boot.
    pub fn progenitor_refuses(&self) -> bool {
        match &self.table {
            TableSeal::Absent | TableSeal::Unreadable => true,
            TableSeal::Checked { disagreements } => disagreements
                .iter()
                .any(|d| !matches!(d, TableDisagreement::Missing(_))),
        }
    }

    /// **The kernel's other decision**: would it halt at `MEASURED BOOT REFUSED` on the loader's
    /// words? False when no words were given.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by milestone/1101-1113-outside-review-fixes.
    pub fn words_refuse(&self) -> bool {
        match &self.words {
            WordsSeal::NotGiven => false,
            WordsSeal::Refused(_) => true,
            WordsSeal::Checked { disagreeing } => !disagreeing.is_empty(),
        }
    }

    /// Whether this pair boots: neither side refuses, and the loader's words, if given, agree.
    pub fn is_sealed(&self) -> bool {
        !self.kernel_refuses() && !self.words_refuse() && !self.progenitor_refuses()
    }

    /// The verdict as a person at a bench wants it, given the two paths being judged.
    ///
    /// One line when the pair is sealed. When it is not, the entries that fail, what each one means
    /// at boot, and the command that fixes it. Written here rather than at each call site so the
    /// loader's build failure and the card check say the same thing in the same words.
    pub fn explain(&self, kernel: &str, archive: &str) -> String {
        if self.is_sealed() {
            let vouched = self
                .entries
                .iter()
                .filter(|e| matches!(e.vouch, Vouch::Yes(_)))
                .count();
            return format!(
                "SEALED: {kernel} vouches for {archive} ({vouched} measured entries, and the \
                 archive's own table agrees with its contents)."
            );
        }

        let mut out = String::new();
        if self.kernel_refuses() {
            let _ = writeln!(
                out,
                "NOT SEALED: {kernel} does not vouch for {archive}. The usual causes are \
                 a kernel and an archive from different builds, or a copy that changed one of \
                 them. Either way this pair halts at MEASURED BOOT REFUSED after the power cycle."
            );
            for e in &self.entries {
                if let Vouch::No(digest) = &e.vouch {
                    let _ = writeln!(
                        out,
                        "  the kernel image does not carry the digest of this archive's `{}`: it \
                         hashes to {}, which is not in the trust root",
                        e.name,
                        hex_string(digest)
                    );
                }
            }
        }
        match &self.words {
            WordsSeal::NotGiven => {}
            WordsSeal::Refused(why) => {
                let _ = writeln!(
                    out,
                    "NOT SEALED: the loader's digest words are refused ({why:?}), and the kernel \
                     halts at MEASURED BOOT REFUSED."
                );
            }
            WordsSeal::Checked { disagreeing } => {
                for word in disagreeing {
                    let _ = writeln!(
                        out,
                        "NOT SEALED: the loader's `{}` word is not the digest of {archive}'s \
                         entry, so the kernel halts at MEASURED BOOT REFUSED.",
                        word.key()
                    );
                }
            }
        }
        match &self.table {
            TableSeal::Absent => {
                let _ = writeln!(
                    out,
                    "NOT SEALED: {archive} carries no `{}` table, so the progenitor could vouch \
                     for nothing it loads and the kernel does not start it.",
                    measured_boot::PROGRAM_MEASUREMENTS
                );
            }
            TableSeal::Unreadable => {
                let _ = writeln!(
                    out,
                    "NOT SEALED: {archive}'s `{}` table cannot be read, so the progenitor measures \
                     against an empty table and refuses the console.",
                    measured_boot::PROGRAM_MEASUREMENTS
                );
            }
            TableSeal::Checked { disagreements } if !disagreements.is_empty() => {
                let _ = writeln!(
                    out,
                    "{archive}'s own measurement table disagrees with the archive it is packed in. \
                     The kernel starts, and then the progenitor treats what it cannot vouch for as \
                     what is not there:"
                );
                for d in disagreements {
                    let _ = match d {
                        TableDisagreement::Changed(n) => writeln!(
                            out,
                            "  `{n}` is in the archive with a different digest; it will not be loaded"
                        ),
                        TableDisagreement::Unlisted(n) => writeln!(
                            out,
                            "  `{n}` is in the archive and not in the table; it will not be loaded"
                        ),
                        TableDisagreement::Missing(n) => writeln!(
                            out,
                            "  `{n}` is in the table and not in the archive (harmless at boot)"
                        ),
                        TableDisagreement::Unparsable { line, reason } => writeln!(
                            out,
                            "  the table line `{line}` is rejected ({reason}); the program it \
                             names will not be loaded"
                        ),
                    };
                }
            }
            TableSeal::Checked { .. } => {}
        }
        out
    }
}

/// The sentence that has to travel with every refusal this crate produces, and milestone 223's own
/// `BUGS` entry in one line: this check duplicates the kernel's deliberately, as an early warning,
/// and the kernel is the authority. A caller prints it where a person reads the verdict.
pub const THE_BOARD_IS_THE_AUTHORITY: &str = "The kernel's own check is the authority and this one is the early warning; if they ever \
     disagree, the board is right.";

/// **Judge a kernel image against an archive.** The whole of this crate's question.
///
/// `kernel` is the image as it will be loaded: a flat `booti` image, an ELF, or a UEFI boot file
/// with the pair embedded. Nothing is parsed out of it; only its bytes are searched (see
/// notes/sealed-pair.md, "Searching bytes, not parsing them").
///
/// # Errors
///
/// [`Unreadable`] when the archive does not parse or carries no `progenitor`: those are not
/// mismatches, and calling them one would send somebody to rebuild the wrong half.
///
/// # Examples
///
/// A pair built together is sealed:
///
/// ```
/// # use sealed_pair::{inspect, Seal};
/// # let (kernel, archive) = sealed_pair::test_support::matched_pair();
/// let seal = inspect(&kernel, &archive).expect("the archive parses and carries a progenitor");
/// assert!(seal.is_sealed());
/// ```
///
/// A kernel beside another build's archive is refused, naming the entry:
///
/// ```
/// # use sealed_pair::inspect;
/// # let (kernel, _) = sealed_pair::test_support::matched_pair();
/// # let (_, archive) = sealed_pair::test_support::other_pair();
/// let seal = inspect(&kernel, &archive).expect("the archive parses and carries a progenitor");
/// assert!(!seal.is_sealed());
/// assert!(seal.explain("nife-vf2.img", "nife-initial-ramdisk.img").contains("progenitor"));
/// ```
pub fn inspect(kernel: &[u8], archive: &[u8]) -> Result<Seal, Unreadable> {
    inspect_with_words(kernel, archive, None)
}

/// **Judge a kernel image, an archive and the loader's two digest words.** [`inspect`] with the
/// third question the kernel asks: the words a loader hands over (`nife.progenitor`, `nife.base`)
/// must parse and must be the digests of the archive's `progenitor` and measurement table. On a
/// card they travel as the file `nife-initial-ramdisk.img.loader-digests`. `None` asks nothing of
/// them (a stick's loader checks its own words when it is built).
///
/// # Errors
///
/// As [`inspect`].
///
/// # Examples
///
/// ```
/// # use sealed_pair::{inspect_with_words, test_support};
/// let (kernel, archive) = test_support::matched_pair();
/// let words = test_support::words_for(&archive);
/// assert!(inspect_with_words(&kernel, &archive, Some(&words)).unwrap().is_sealed());
/// let (_, other) = test_support::other_pair();
/// let stale = test_support::words_for(&other);
/// assert!(!inspect_with_words(&kernel, &archive, Some(&stale)).unwrap().is_sealed());
/// ```
///
/// Name: provisional, minted 2026-11-14 (UTC) by milestone/1101-1113-outside-review-fixes.
pub fn inspect_with_words(
    kernel: &[u8],
    archive: &[u8],
    words: Option<&str>,
) -> Result<Seal, Unreadable> {
    let fs = nifefs::Fs::parse(archive).map_err(Unreadable::NotAnArchive)?;
    if fs.read(measured_boot::PROGENITOR).is_none() {
        return Err(Unreadable::NoProgenitor);
    }

    let entries = measured_entries()
        .into_iter()
        .map(|name| match fs.read(name) {
            None => EntrySeal {
                name,
                vouch: Vouch::Absent,
            },
            Some(bytes) => {
                let digest = measured_boot::sha256(bytes);
                let vouch = if contains(kernel, &digest) {
                    Vouch::Yes(digest)
                } else {
                    Vouch::No(digest)
                };
                EntrySeal { name, vouch }
            }
        })
        .collect();

    Ok(Seal {
        entries,
        table: inspect_table(&fs),
        words: words.map_or(WordsSeal::NotGiven, |w| inspect_words(w, &fs)),
    })
}

/// The kernel's third question: do the loader's words parse, and are they the archive's digests?
fn inspect_words(words: &str, fs: &nifefs::Fs<'_>) -> WordsSeal {
    use measured_boot::loader_digests::{Word, parse};
    let given = match parse(words.trim()) {
        Ok(given) => given,
        Err(why) => return WordsSeal::Refused(why),
    };
    let mut disagreeing = Vec::new();
    if fs
        .read(measured_boot::PROGENITOR)
        .is_some_and(|b| measured_boot::sha256(b) != given.progenitor)
    {
        disagreeing.push(Word::Progenitor);
    }
    // A missing table is reported by the table's own verdict; the word is judged when there is
    // something for it to disagree with.
    if fs
        .read(measured_boot::PROGRAM_MEASUREMENTS)
        .is_some_and(|b| measured_boot::sha256(b) != given.base)
    {
        disagreeing.push(Word::Base);
    }
    WordsSeal::Checked { disagreeing }
}

/// Milestone 104's half: does the archive's own measurement table describe the archive it is in?
fn inspect_table(fs: &nifefs::Fs<'_>) -> TableSeal {
    let Some(bytes) = fs.read(measured_boot::PROGRAM_MEASUREMENTS) else {
        return TableSeal::Absent;
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return TableSeal::Unreadable;
    };
    let mut listed: Vec<(&str, Digest)> = Vec::new();
    let mut disagreements = Vec::new();
    for entry in measured_boot::manifest_entries(text) {
        // The progenitor skips a line the one parser rejects, so only the program that line named
        // is lost. It is reported, with the parser's reason, and the rest of the table still counts.
        match entry {
            Ok(pair) => listed.push(pair),
            Err(bad) => disagreements.push(TableDisagreement::Unparsable {
                line: bad.line.to_string(),
                reason: format!("{:?}", bad.why),
            }),
        }
    }

    // The table measures every entry in the archive except itself, which cannot contain its own
    // digest (`initial_ramdisk_writer::measurement_table`).
    for e in fs.entries() {
        let Some(name) = e.name_str() else { continue };
        if name == measured_boot::PROGRAM_MEASUREMENTS {
            continue;
        }
        let bytes = fs.read(name).unwrap_or_default();
        let have = measured_boot::sha256(bytes);
        match listed.iter().find(|(n, _)| *n == name) {
            None => disagreements.push(TableDisagreement::Unlisted(name.to_string())),
            Some((_, want)) if *want != have => {
                disagreements.push(TableDisagreement::Changed(name.to_string()));
            }
            Some(_) => {}
        }
    }
    for (name, _) in &listed {
        if fs.read(name).is_none() {
            disagreements.push(TableDisagreement::Missing((*name).to_string()));
        }
    }
    disagreements.sort_by_key(|d| match d {
        TableDisagreement::Changed(n)
        | TableDisagreement::Unlisted(n)
        | TableDisagreement::Missing(n) => n.clone(),
        TableDisagreement::Unparsable { line, .. } => line.clone(),
    });
    TableSeal::Checked { disagreements }
}

/// **Find the archive inside a single boot file** (the UEFI stick's one file per
/// architecture, milestone 441 (the program that makes the stick)).
///
/// The stick carries no separate archive: `uefi_loader` embeds both halves, so a reader that wants
/// to judge the pair has to find the archive by its magic. Every occurrence is tried, because the
/// kernel is embedded in the same file and carries `nifefs::MAGIC` as a constant of its own; the
/// first one that parses and holds a `progenitor` is the archive.
pub fn embedded_archive(blob: &[u8]) -> Option<&[u8]> {
    blob.windows(nifefs::MAGIC.len())
        .enumerate()
        .filter(|(_, w)| *w == nifefs::MAGIC)
        .map(|(at, _)| &blob[at..])
        .find(|candidate| {
            nifefs::Fs::parse(candidate)
                .is_ok_and(|fs| fs.read(measured_boot::PROGENITOR).is_some())
        })
}

/// A digest as the hex a person compares against `shasum -a 256`.
pub fn hex_string(digest: &Digest) -> String {
    let bytes = measured_boot::hex(digest);
    String::from_utf8(bytes.to_vec()).expect("measured_boot::hex emits ascii")
}

/// Does `haystack` contain `needle`? The whole of the trust-root lookup, for the reason the module
/// header gives: the digest's 32 raw bytes are what the trust root compiles to, and finding them is
/// the same fact the kernel checks without needing to know where the array landed.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Pairs built in memory, for this crate's own doc examples and tests and for anyone else proving a
/// checker against a mismatch without a four-minute build.
///
/// It is public because the doc examples above are compiled as an external crate and a hidden
/// helper would leave them with twenty lines of archive construction in place of the assertion they
/// exist to make.
pub mod test_support {
    use super::{BOOT_PROGRAMS, measured_entries};

    /// The loader's two words for `archive`, as the card's `.loader-digests` file carries them.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by milestone/1101-1113-outside-review-fixes.
    pub fn words_for(archive: &[u8]) -> String {
        use measured_boot::loader_digests::{LoaderDigests, MAXIMUM_BYTES, encode};
        let fs = nifefs::Fs::parse(archive).expect("an archive");
        let digests = LoaderDigests {
            progenitor: measured_boot::sha256(fs.read(measured_boot::PROGENITOR).expect("packed")),
            base: measured_boot::sha256(
                fs.read(measured_boot::PROGRAM_MEASUREMENTS)
                    .expect("packed"),
            ),
        };
        let mut out = [0u8; MAXIMUM_BYTES];
        let n = encode(&digests, &mut out);
        String::from_utf8(out[..n].to_vec()).expect("ascii")
    }

    /// Pack an archive holding the named programs plus the measurement table that describes them,
    /// and a stand-in kernel image carrying exactly the digests a real one's trust root would.
    ///
    /// The stand-in kernel is padding with digests buried in it rather than a real image, which is
    /// the point: the check reads a kernel as bytes, so a test does not need one.
    pub fn pair(flavour: &str) -> (Vec<u8>, Vec<u8>) {
        let programs: Vec<(&str, Vec<u8>)> = BOOT_PROGRAMS
            .iter()
            .chain(&["console"])
            .map(|&name| (name, format!("the {flavour} {name}").into_bytes()))
            .collect();

        let mut table = String::from("# generated by test_support\n");
        for (name, bytes) in &programs {
            let digest = measured_boot::sha256(bytes);
            let text = measured_boot::digest_text(&digest);
            let text = core::str::from_utf8(&text).expect("a digest's text is ascii");
            table.push_str(&format!("{name} {text}\n"));
        }

        let mut files: Vec<(&str, &[u8])> = programs
            .iter()
            .map(|(name, bytes)| (*name, bytes.as_slice()))
            .collect();
        files.push((measured_boot::PROGRAM_MEASUREMENTS, table.as_bytes()));
        let mut archive = vec![0u8; nifefs::image_size(&files)];
        let written = nifefs::write_image(&files, &mut archive).expect("the archive fits");
        archive.truncate(written);

        let fs = nifefs::Fs::parse(&archive).expect("what we just packed parses");
        let mut kernel = vec![0xa5u8; 512];
        for name in measured_entries() {
            let bytes = fs.read(name).expect("packed above");
            kernel.extend_from_slice(&measured_boot::sha256(bytes));
            kernel.extend_from_slice(&[0x5a; 64]);
        }
        (kernel, archive)
    }

    /// One build's kernel and archive.
    pub fn matched_pair() -> (Vec<u8>, Vec<u8>) {
        pair("first")
    }

    /// A different build's, for proving that a mismatch is refused.
    pub fn other_pair() -> (Vec<u8>, Vec<u8>) {
        pair("second")
    }
}

#[cfg(test)]
mod tests {
    use test_support::{matched_pair, other_pair, pair};

    use super::*;

    #[test]
    fn a_pair_from_one_build_is_sealed() {
        let (kernel, archive) = matched_pair();
        let seal = inspect(&kernel, &archive).expect("readable");
        assert!(seal.is_sealed(), "{seal:?}");
        assert!(!seal.kernel_refuses());
        assert!(!seal.progenitor_refuses());
        let line = seal.explain("nife-vf2.img", "nife-initial-ramdisk.img");
        assert!(line.starts_with("SEALED:"), "{line}");
        assert_eq!(line.lines().count(), 1, "a yes is one line: {line}");
        let counted = format!("({} measured entries", measured_entries().len());
        assert!(line.contains(&counted), "counts the vouched: {line}");
    }

    /// Both ways a pair is unreadable say why, in words, rather than printing nothing.
    #[test]
    fn an_unreadable_pair_says_which_way() {
        let none = Unreadable::NoProgenitor.to_string();
        assert!(none.contains("no `progenitor`"), "{none}");
        let bad = Unreadable::NotAnArchive(nifefs::Error::Truncated).to_string();
        assert!(bad.starts_with("not a nifefs archive ("), "{bad}");
    }

    /// A kernel that refuses beside a table that checks out with no disagreements: the report is the
    /// kernel's refusal alone, and does not go on to announce a disagreement nobody found.
    #[test]
    fn a_clean_table_beside_a_refusing_kernel_reports_no_disagreement() {
        let seal = Seal {
            entries: vec![EntrySeal {
                name: "progenitor",
                vouch: Vouch::No(measured_boot::sha256(b"x")),
            }],
            table: TableSeal::Checked {
                disagreements: Vec::new(),
            },
            words: WordsSeal::NotGiven,
        };
        assert!(!seal.is_sealed());
        let text = seal.explain("kernel", "archive");
        assert!(text.contains("MEASURED BOOT REFUSED"), "{text}");
        assert!(!text.contains("disagrees"), "{text}");
    }

    /// The refusal's digest is for a person to compare against `shasum -a 256`, so it has to be
    /// that tool's spelling. The test above finds `hex_string`'s output in the report, which any
    /// spelling passes; this one pins it to FIPS 180-2's digest of `abc`.
    #[test]
    fn a_digest_is_printed_as_shasum_prints_it() {
        assert_eq!(
            hex_string(&measured_boot::sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_kernel_from_another_build_is_refused_and_the_refusal_names_the_entry() {
        let (kernel, _) = matched_pair();
        let (_, archive) = other_pair();
        let seal = inspect(&kernel, &archive).expect("readable");
        assert!(seal.kernel_refuses());
        // Every measured entry differs between the two builds, which is what a rebuild does.
        for e in &seal.entries {
            assert!(matches!(e.vouch, Vouch::No(_)), "{e:?}");
        }
        let text = seal.explain("nife-vf2.img", "nife-initial-ramdisk.img");
        assert!(text.contains("MEASURED BOOT REFUSED"), "{text}");
        assert!(text.contains("`progenitor`: it hashes to"), "{text}");
        assert!(text.contains("trust root"), "{text}");
        // The digest printed is the archive's, so a person can check it with shasum.
        let Vouch::No(digest) = seal.entries[0].vouch else {
            panic!("refused");
        };
        let want = hex_string(&digest);
        assert!(text.contains(&want), "{text}");
    }

    #[test]
    fn one_stale_half_is_named_on_its_own() {
        // A hand-copied card where only the progenitor moved: the table still matches, and the
        // report must point at the one entry that does not rather than at the whole pair.
        let (mut kernel, archive) = matched_pair();
        let fs = nifefs::Fs::parse(&archive).expect("packed");
        let digest = measured_boot::sha256(fs.read("progenitor").expect("packed"));
        let at = kernel
            .windows(digest.len())
            .position(|w| w == digest)
            .expect("the stand-in kernel carries it");
        kernel[at] ^= 0xff;

        let seal = inspect(&kernel, &archive).expect("readable");
        assert!(seal.kernel_refuses());
        assert!(matches!(seal.entries[0].vouch, Vouch::No(_)));
        assert!(
            seal.entries[1..]
                .iter()
                .all(|e| matches!(e.vouch, Vouch::Yes(_))),
            "{seal:?}"
        );
    }

    #[test]
    fn an_archive_that_does_not_parse_is_not_called_a_mismatch() {
        let (kernel, _) = matched_pair();
        assert!(matches!(
            inspect(&kernel, &[0u8; 64]),
            Err(Unreadable::NotAnArchive(_))
        ));
    }

    #[test]
    fn an_archive_with_no_progenitor_is_its_own_answer() {
        let (kernel, _) = matched_pair();
        let files: Vec<(&str, &[u8])> = vec![("console", b"only the console")];
        let mut archive = vec![0u8; nifefs::image_size(&files)];
        let n = nifefs::write_image(&files, &mut archive).expect("fits");
        archive.truncate(n);
        assert_eq!(inspect(&kernel, &archive), Err(Unreadable::NoProgenitor));
    }

    #[test]
    fn a_table_that_disagrees_with_its_archive_is_the_progenitors_refusal_not_the_kernels() {
        // Repack one build's programs with another build's table: the kernel still vouches for
        // every entry it measures, and the progenitor loses the console.
        let (_, other) = other_pair();
        let other_fs = nifefs::Fs::parse(&other).expect("packed");
        let stale_table = other_fs
            .read(measured_boot::PROGRAM_MEASUREMENTS)
            .expect("packed")
            .to_vec();

        let (_, mine) = matched_pair();
        let my_fs = nifefs::Fs::parse(&mine).expect("packed");
        let mut files: Vec<(&str, &[u8])> = BOOT_PROGRAMS
            .iter()
            .chain(&["console"])
            .map(|&name| (name, my_fs.read(name).expect("packed")))
            .collect();
        files.push((measured_boot::PROGRAM_MEASUREMENTS, &stale_table));
        let mut archive = vec![0u8; nifefs::image_size(&files)];
        let n = nifefs::write_image(&files, &mut archive).expect("fits");
        archive.truncate(n);

        let fs = nifefs::Fs::parse(&archive).expect("packed");
        let mut kernel = vec![0u8; 16];
        for name in measured_entries() {
            kernel.extend_from_slice(&measured_boot::sha256(fs.read(name).expect("packed")));
        }

        let seal = inspect(&kernel, &archive).expect("readable");
        assert!(!seal.kernel_refuses(), "{seal:?}");
        assert!(seal.progenitor_refuses(), "{seal:?}");
        let text = seal.explain("kernel", "archive");
        assert!(
            text.contains("`console` is in the archive with a different digest"),
            "{text}"
        );
    }

    #[test]
    fn an_archive_with_no_table_halts_as_surely_as_a_mismatch() {
        let progenitor = b"a progenitor".as_slice();
        let files: Vec<(&str, &[u8])> = vec![("progenitor", progenitor)];
        let mut archive = vec![0u8; nifefs::image_size(&files)];
        let n = nifefs::write_image(&files, &mut archive).expect("fits");
        archive.truncate(n);
        let kernel = measured_boot::sha256(progenitor).to_vec();

        let seal = inspect(&kernel, &archive).expect("readable");
        assert!(!seal.kernel_refuses());
        assert_eq!(seal.table, TableSeal::Absent);
        assert!(seal.progenitor_refuses());
        assert!(
            seal.explain("k", "a").contains("could vouch for nothing"),
            "{}",
            seal.explain("k", "a")
        );
    }

    /// Pack `files` into an archive.
    fn pack(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut archive = vec![0u8; nifefs::image_size(files)];
        let n = nifefs::write_image(files, &mut archive).expect("fits");
        archive.truncate(n);
        archive
    }

    /// One bad line beside a good console line costs only the program the bad line named. The old
    /// report called the whole table unreadable and said the console would be refused.
    #[test]
    fn a_bad_table_line_is_reported_and_does_not_condemn_the_console() {
        let progenitor = b"a progenitor".as_slice();
        let console = b"a console".as_slice();
        let digest = measured_boot::digest_text(&measured_boot::sha256(console));
        let digest = core::str::from_utf8(&digest).expect("ascii").to_string();
        let table = format!("console {digest}\nprogenitor not-a-digest\n");
        let archive = pack(&[
            ("progenitor", progenitor),
            ("console", console),
            (measured_boot::PROGRAM_MEASUREMENTS, table.as_bytes()),
        ]);
        let mut kernel = measured_boot::sha256(progenitor).to_vec();
        kernel.extend_from_slice(&measured_boot::sha256(table.as_bytes()));
        let seal = inspect(&kernel, &archive).expect("readable");
        let TableSeal::Checked { disagreements } = &seal.table else {
            panic!("the table was read: {seal:?}");
        };
        assert!(
            disagreements
                .iter()
                .any(|d| matches!(d, TableDisagreement::Unparsable { line, .. } if line.starts_with("progenitor"))),
            "{disagreements:?}"
        );
        assert!(seal.progenitor_refuses());
        let text = seal.explain("kernel", "archive");
        assert!(text.contains("progenitor not-a-digest"), "{text}");
        assert!(!text.contains("`console`"), "{text}");
        assert!(!text.contains("cannot be read"), "{text}");
    }

    #[test]
    fn a_table_that_is_not_utf8_is_still_unreadable() {
        let archive = pack(&[
            ("progenitor", b"p"),
            (measured_boot::PROGRAM_MEASUREMENTS, &[0xff, 0xfe]),
        ]);
        let seal = inspect(&measured_boot::sha256(b"p"), &archive).expect("readable");
        assert_eq!(seal.table, TableSeal::Unreadable);
    }

    /// A card whose kernel and archive are one build and whose loader words are another's reads as
    /// refused, and each kind of bad word is said in words.
    #[test]
    fn stale_malformed_and_missing_loader_words_are_refused() {
        let (kernel, archive) = matched_pair();
        let (_, other) = other_pair();

        let good = test_support::words_for(&archive);
        let seal = inspect_with_words(&kernel, &archive, Some(&good)).expect("readable");
        assert!(seal.is_sealed(), "{seal:?}");
        // The card file ends in a newline.
        let with_newline = format!("{good}\n");
        assert!(
            inspect_with_words(&kernel, &archive, Some(&with_newline))
                .unwrap()
                .is_sealed()
        );

        let stale = test_support::words_for(&other);
        let seal = inspect_with_words(&kernel, &archive, Some(&stale)).expect("readable");
        assert!(!seal.is_sealed());
        assert!(!seal.kernel_refuses() && !seal.progenitor_refuses());
        let text = seal.explain("k", "a");
        assert!(text.contains("`nife.progenitor` word"), "{text}");
        assert!(text.contains("`nife.base` word"), "{text}");

        for bad in ["", "nife.progenitor=zz nife.base=zz"] {
            let seal = inspect_with_words(&kernel, &archive, Some(bad)).expect("readable");
            assert!(matches!(seal.words, WordsSeal::Refused(_)), "{bad}");
            assert!(!seal.is_sealed(), "{bad}");
        }
        assert_eq!(
            inspect(&kernel, &archive).unwrap().words,
            WordsSeal::NotGiven
        );
    }

    #[test]
    fn an_unreadable_error_is_a_std_error_in_words() {
        let e: Box<dyn std::error::Error> =
            Box::new(Unreadable::NotAnArchive(nifefs::Error::Truncated));
        assert_eq!(
            e.to_string(),
            "not a nifefs archive (the image is shorter than the directory)"
        );
    }

    #[test]
    fn the_boot_programs_are_the_measured_boot_names() {
        assert_eq!(BOOT_PROGRAMS, [measured_boot::PROGENITOR]);
    }

    #[test]
    fn the_archive_is_found_inside_a_single_boot_file() {
        // The stick's shape: one file with the kernel and the archive inside it, and the kernel's
        // own copy of nifefs::MAGIC sitting in front of the real archive to be stepped over.
        let (kernel, archive) = pair("stick");
        let mut blob = Vec::new();
        blob.extend_from_slice(b"PE\0\0 firmware header");
        blob.extend_from_slice(&nifefs::MAGIC);
        blob.extend_from_slice(&kernel);
        blob.extend_from_slice(&archive);

        let found = embedded_archive(&blob).expect("the archive is in there");
        let seal = inspect(&blob, found).expect("readable");
        assert!(seal.is_sealed(), "{seal:?}");
    }
}
