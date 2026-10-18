//! **Does this kernel vouch for this archive?** The host-side half of measured boot. A kernel image
//! and the userspace archive beside it are one sealed set: the build compiles the digests of the
//! archive entries the kernel may enter into its `.rodata` (`kernel/build.rs`), and the kernel
//! refuses userspace when the bytes do not match (`kernel::trust::require`). A mismatched pair
//! boots, prints `MEASURED BOOT REFUSED` and halts, discovered only after a walk to the board; this
//! asks the same question on the host, where the answer costs nothing.
//!
//! [`inspect`] hashes each entry the kernel would hand over (the [`BOOT_PROGRAMS`] and the
//! measurement table) and looks for the digest's 32 raw bytes inside the kernel image. That is what
//! the trust root compiles to, a chance match is not worth a sentence, and it needs no knowledge of
//! where the array landed, so one function reads a flat `booti` image, an ELF, and a UEFI boot file.
//! Finding a digest proves the kernel vouches for those bytes; not finding one cannot say what it
//! expected instead. It also checks the archive's measurement table against its own contents, a
//! separate refusal the progenitor would make one level up.
//!
//! # BUGS
//!
//! - It duplicates the kernel's check as an early warning; if they disagree, the kernel is right,
//!   and [`Seal::explain`] says so.
//! - A refusal names the entry and the digest the archive has, never the one the kernel wanted;
//!   `cargo xtask card-check` recovers that by comparing against the tree's own build.
//! - It says nothing about a boot script or `extlinux.conf`, and nothing forces anyone to run it on
//!   a hand-written card (`uefi_loader/build.rs` makes it a build failure where the pair is built).
//! - It asks whether a digest is present, not whether the kernel checks it, so a build carrying
//!   the digest without reaching the check would read as sealed. Every build reaches it today
//!   through `kernel::trust::require_program`; option C in
//!   `design/roadmap/0563-a-seal-check-that-reads-bytes-cannot-see-a-check-that-was-dropped.md`
//!   prices a build that says so itself.
//!
//! Name: provisional: the thing (a sealed pair), not the act. Refused `card_check` (three of the
//! checked places are not a card) and `seal` (a generic verb).

use std::fmt::Write as _;

use measured_boot::Digest;

/// The archive entries the kernel itself may enter as a boot program, and therefore the ones its
/// trust root names.
///
/// **One list, three readers.** `xtask::measure` writes the manifest from it, `uefi_loader`'s build
/// refuses an unsealed pair against it, and [`inspect`] reads a card with it. It was written out
/// twice before this crate existed, and a kernel measuring a name the checkers did not know about
/// would have been invisible to both of them.
///
/// `progenitor` comes first, and [`inspect`] reads it as the one program every archive must carry.
/// The six after it are not boot programs in the ordinary sense: they are milestone 19 (run a real
/// workload)'s ELF child builders, and `kernel::user::spawn_elf_child_builder` enters each of them
/// directly, so the trust root has to name them or the kernel could enter a program it never
/// measured.
pub const BOOT_PROGRAMS: [&str; 7] = [
    "progenitor",
    "report_child_builder",
    "device_child_builder",
    "interrupt_child_builder",
    "console_server_builder",
    "least_authority_demo_builder",
    "coremark_builder",
];

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
            Self::NotAnArchive(e) => write!(f, "not a nifefs archive ({e:?})"),
            Self::NoProgenitor => {
                write!(f, "carries no `progenitor`, so no kernel can enter it")
            }
        }
    }
}

/// What a kernel image says about one archive entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vouch {
    /// The digest of this entry's bytes is in the kernel image. The kernel will enter it.
    Yes,
    /// The kernel image does not carry this digest. The kernel halts at `MEASURED BOOT REFUSED`
    /// rather than enter it, which is the refusal this crate exists to move off the bench.
    No,
    /// The archive does not carry this entry. Not every archive carries every boot program, and a
    /// kernel refuses to enter a name it has no measurement for, so nothing is waved through.
    Absent,
}

/// One entry's verdict, with the digest the archive's bytes actually have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySeal {
    /// The archive entry's name, as the trust root spells it.
    pub name: &'static str,
    /// The SHA-256 of the entry's bytes, or `None` when the archive has no such entry.
    pub digest: Option<Digest>,
    /// Whether the kernel image carries that digest.
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
}

/// What the archive's own measurement table says about the archive (milestone 104's half of the
/// chain, which is the progenitor's refusal rather than the kernel's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableSeal {
    /// The archive carries no table. The kernel refuses to start a progenitor that can vouch for
    /// nothing, so this halts as surely as a digest mismatch does.
    Absent,
    /// The table is present but is not UTF-8, or holds a line the one manifest parser rejects. The
    /// progenitor reads an empty table, which vouches for nothing, and the console is refused.
    Unreadable,
    /// The table was read. An empty `disagreements` is the healthy case.
    Checked {
        /// Every way the table and the archive fail to describe each other, sorted by name.
        disagreements: Vec<TableDisagreement>,
    },
}

/// The whole verdict on one kernel-and-archive pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seal {
    /// One verdict per name in [`measured_entries`], in that order.
    pub entries: Vec<EntrySeal>,
    /// The progenitor's side of the chain.
    pub table: TableSeal,
}

impl Seal {
    /// **The kernel's decision**: would this kernel halt at `MEASURED BOOT REFUSED` on this archive?
    ///
    /// True when any entry the archive carries is one the kernel does not vouch for. An entry the
    /// archive does not carry is not a refusal here, because the kernel only measures what it is
    /// about to enter; a missing `progenitor` is refused earlier, by [`inspect`] itself.
    pub fn kernel_refuses(&self) -> bool {
        self.entries.iter().any(|e| e.vouch == Vouch::No)
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

    /// Whether this pair boots: neither side refuses.
    pub fn is_sealed(&self) -> bool {
        !self.kernel_refuses() && !self.progenitor_refuses()
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
                .filter(|e| e.vouch == Vouch::Yes)
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
                "NOT SEALED: {kernel} does not vouch for {archive}. They are from different \
                 builds, and this pair halts at MEASURED BOOT REFUSED after the power cycle."
            );
            for e in self.entries.iter().filter(|e| e.vouch == Vouch::No) {
                let _ = writeln!(
                    out,
                    "  entry `{}` hashes to {}, which is not in the kernel image's trust root",
                    e.name,
                    e.digest.as_ref().map_or_else(String::new, hex_string)
                );
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
/// with the pair embedded. Nothing is parsed out of it; only its bytes are searched, for the reason
/// the module header gives.
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
    let fs = nifefs::Fs::parse(archive).map_err(Unreadable::NotAnArchive)?;
    if fs.read(BOOT_PROGRAMS[0]).is_none() {
        return Err(Unreadable::NoProgenitor);
    }

    let entries = measured_entries()
        .into_iter()
        .map(|name| match fs.read(name) {
            None => EntrySeal {
                name,
                digest: None,
                vouch: Vouch::Absent,
            },
            Some(bytes) => {
                let digest = measured_boot::sha256(bytes);
                let vouch = if contains(kernel, &digest) {
                    Vouch::Yes
                } else {
                    Vouch::No
                };
                EntrySeal {
                    name,
                    digest: Some(digest),
                    vouch,
                }
            }
        })
        .collect();

    Ok(Seal {
        entries,
        table: inspect_table(&fs),
    })
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
    for entry in measured_boot::manifest_entries(text) {
        // A line the one parser rejects is what the progenitor turns into an empty table, so it is
        // reported as unreadable rather than skipped: skipping would make this check pass a card
        // whose console the progenitor will refuse.
        match entry {
            Ok(pair) => listed.push(pair),
            Err(_) => return TableSeal::Unreadable,
        }
    }

    let mut disagreements = Vec::new();
    // The table measures every entry in the archive except itself, which cannot contain its own
    // digest (`xtask::measure::measurement_table`).
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
            nifefs::Fs::parse(candidate).is_ok_and(|fs| fs.read(BOOT_PROGRAMS[0]).is_some())
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
                digest: None,
                vouch: Vouch::No,
            }],
            table: TableSeal::Checked {
                disagreements: Vec::new(),
            },
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
            assert_eq!(e.vouch, Vouch::No, "{e:?}");
        }
        let text = seal.explain("nife-vf2.img", "nife-initial-ramdisk.img");
        assert!(text.contains("MEASURED BOOT REFUSED"), "{text}");
        assert!(text.contains("`progenitor` hashes to"), "{text}");
        assert!(text.contains("trust root"), "{text}");
        // The digest printed is the archive's, so a person can check it with shasum.
        let want = hex_string(&seal.entries[0].digest.expect("present"));
        assert!(text.contains(&want), "{text}");
    }

    #[test]
    fn one_stale_half_is_named_on_its_own() {
        // A hand-copied card where only the progenitor moved: the other entries still match, and
        // the report must point at the one that does not rather than at the whole pair.
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
        assert_eq!(seal.entries[0].vouch, Vouch::No);
        assert_eq!(seal.entries[1].vouch, Vouch::Yes);
        assert_eq!(seal.entries[2].vouch, Vouch::Yes);
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
