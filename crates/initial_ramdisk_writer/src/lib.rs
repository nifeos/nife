//! **Packing the initial ramdisk**: the programs and the entries every archive carries, as one
//! `nifefs` image.
//!
//! [`compose`] takes the programs' bytes, the image's package catalog and its index sources, and
//! returns the image. It refuses an archive without the `std` programs every image ships, and a
//! program whose ELF segments fall outside the address-space map's image band. It appends the
//! measurement table last, so the table measures everything above it. The result is a pure
//! function of its inputs, so the host composer and a device updater that call it with the same
//! inputs write the same bytes. Reading a file in, writing the image out and the kernel's
//! measurement manifest stay with the caller. See notes/adding-a-program/the-archives.md.

//! # BUGS
//!
//! - **The program list is the caller's.** This crate packs what it is given; which programs an
//!   image holds (the `[[bin]]` reader, the `packaged_only` list) is `xtask/src/archive.rs`, which a
//!   device does not have. The device updater composes from the packages it fetched.
//! - **The ELF check reads `PT_LOAD` headers by hand**, because `crates/elf` refuses a foreign
//!   `e_machine` and the host packs riscv64 and x86_64 archives alongside aarch64 ones.
//!
//! Name: provisional. The crate is named for what it writes, with the tree's spelled-out word for
//! the thing (the initial ramdisk, `memory::initialize`'s and the loaders' term). Considered
//! `archive_writer` (the milestone block's phrase, but `package_archive` already makes "archive"
//! mean a package) and `nifefs_composer` (names the format, which `nifefs` already owns, and not
//! the catalog, the table and the checks this adds). The public names are provisional too.

#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::{format, vec};
use core::fmt;

/// The `std` programs no image ships without, though they are built outside `components/`. `jig`
/// is the package client (milestone 809 (the package client becomes a program)); an image without
/// it boots, and the shell answers `jig` with a spawn failure, so the archive is refused instead.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/image-needs-jig; moved into this crate
/// 2026-11-01 (UTC) by lane/976-part-4b-the-archive-writer-becomes-a-crate.
pub const SHIPPED_STD_PROGRAMS: [&str; 2] = ["jig", "mdr"];

/// A packed image and how many entries it holds.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4b-the-archive-writer-becomes-a-crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// The `nifefs` image, ready to write to a file or a partition.
    pub bytes: Vec<u8>,
    /// How many files the image holds, the catalog, the sources and the measurement table included.
    pub entries: usize,
}

/// A program whose image does not fit the address-space map.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4b-the-archive-writer-becomes-a-crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Misfit {
    /// The archive entry's name.
    pub name: String,
    /// Where it falls outside the band.
    pub placement: address_space_map::ImagePlacement,
}

impl fmt::Display for Misfit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` does not fit the address-space map: {}",
            self.name, self.placement
        )
    }
}

/// Why [`compose`] refused to write an image.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4b-the-archive-writer-becomes-a-crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A program in [`SHIPPED_STD_PROGRAMS`] is not among the entries.
    MissingShippedProgram {
        /// The architecture the archive is for, as the caller spelled it.
        arch: String,
        /// The program that is absent.
        program: &'static str,
    },
    /// At least one program's image falls outside the address-space map's image band.
    OutsideImageBand(Vec<Misfit>),
    /// `nifefs` refused the entries: too many files or a bad name.
    Pack {
        /// What `nifefs::write_image` returned.
        error: nifefs::Error,
        /// How many entries were being packed.
        files: usize,
        /// The image size `nifefs::image_size` asked for.
        size: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MissingShippedProgram { arch, program } => {
                write!(
                    f,
                    "the {arch} archive has no `{program}`, which every image ships"
                )
            }
            Error::OutsideImageBand(misfits) => {
                for (i, misfit) in misfits.iter().enumerate() {
                    if i > 0 {
                        f.write_str("\n")?;
                    }
                    write!(f, "{misfit}")?;
                }
                Ok(())
            }
            Error::Pack { error, files, size } => {
                write!(f, "{error:?} ({files} files, {size} bytes)")
            }
        }
    }
}

/// **Pack `blobs`, the catalog, the index sources and the measurement table into one image.**
///
/// The entries are `blobs` in the order given, then the catalog under
/// [`package_archive::CATALOG`], then the sources under [`package_index::SOURCES_ENTRY`], then the
/// table under [`measured_boot::PROGRAM_MEASUREMENTS`]. The progenitor reads that last entry out of
/// the archive it already holds and refuses to load a program whose bytes it does not match; the
/// kernel's trust root vouches for the entry. `arch` only names the architecture in a refusal.
///
/// Refuses, before packing, an archive missing a [`SHIPPED_STD_PROGRAMS`] entry and one holding a
/// program that does not fit the image band (milestone 206); the band check names every
/// program that fails.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4b-the-archive-writer-becomes-a-crate.
pub fn compose<N: AsRef<str>>(
    arch: &str,
    blobs: &[(N, Vec<u8>)],
    catalog: &str,
    sources: &str,
) -> Result<Image, Error> {
    let mut files: Vec<(&str, &[u8])> = blobs
        .iter()
        .map(|(name, bytes)| (name.as_ref(), bytes.as_slice()))
        .collect();
    files.push((package_archive::CATALOG, catalog.as_bytes()));
    files.push((package_index::SOURCES_ENTRY, sources.as_bytes()));
    shipped_std_programs_present(arch, &files)?;
    programs_fit_the_address_space_map(&files)?;
    let table = measurement_table(&files);
    files.push((measured_boot::PROGRAM_MEASUREMENTS, table.as_bytes()));

    let size = nifefs::image_size(&files);
    let mut bytes = vec![0u8; size];
    // Carry the error: it names which of `MAXIMUM_FILES`, `image_size` and `write_image`'s bounds
    // check refused.
    if let Err(error) = nifefs::write_image(&files, &mut bytes) {
        return Err(Error::Pack {
            error,
            files: files.len(),
            size,
        });
    }
    Ok(Image {
        bytes,
        entries: files.len(),
    })
}

/// **The table the progenitor measures its own loads against** (milestone 104), packed as an
/// entry under [`measured_boot::PROGRAM_MEASUREMENTS`]. `files` is the archive's entries; returns
/// the table's text.
///
/// One `name sha256:<hex>` line, in the kernel manifest's format and parser
/// (`measured_boot::manifest_entries`), for every entry but the table itself, which cannot hold its
/// own digest. Sorted, so reordering the file list does not rewrite the table and relink the
/// kernel. An archive entry and not compiled into the progenitor, because the progenitor is in the
/// archive it measures: the kernel's trust root names the table instead, which keeps the build one
/// pass.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-4b-the-archive-writer-becomes-a-crate.
pub fn measurement_table(files: &[(&str, &[u8])]) -> String {
    let mut lines: Vec<String> = files
        .iter()
        .filter(|(name, _)| *name != measured_boot::PROGRAM_MEASUREMENTS)
        .map(|(name, bytes)| {
            let hex = measured_boot::digest_text(&measured_boot::sha256(bytes));
            let hex = core::str::from_utf8(&hex).expect("a digest's text is ascii");
            format!("{name} {hex}\n")
        })
        .collect();
    lines.sort();
    let mut text = String::from(
        "# generated by cargo xtask: every program in this archive, for the progenitor to measure what it \
         loads\n",
    );
    for line in lines {
        text.push_str(&line);
    }
    text
}

/// Refuse `files` if one of [`SHIPPED_STD_PROGRAMS`] is not among them.
fn shipped_std_programs_present(arch: &str, files: &[(&str, &[u8])]) -> Result<(), Error> {
    match SHIPPED_STD_PROGRAMS
        .iter()
        .find(|p| !files.iter().any(|(name, _)| name == *p))
    {
        None => Ok(()),
        Some(program) => Err(Error::MissingShippedProgram {
            arch: String::from(arch),
            program,
        }),
    }
}

/// **Every program in an archive fits the address-space map's image band** (milestone 206),
/// checked at packing because the kernel's loader only judges the programs a boot loads. Entries
/// that are not ELF (the catalog, the measurement table) are skipped.
fn programs_fit_the_address_space_map(files: &[(&str, &[u8])]) -> Result<(), Error> {
    let mut misfits = Vec::new();
    for (name, bytes) in files {
        let Some((lo, hi)) = image_span(bytes) else {
            continue;
        };
        if let Err(placement) = address_space_map::check_image(lo, hi) {
            misfits.push(Misfit {
                name: String::from(*name),
                placement,
            });
        }
    }
    if misfits.is_empty() {
        Ok(())
    } else {
        Err(Error::OutsideImageBand(misfits))
    }
}

/// The page-rounded span of a 64-bit little-endian ELF's `PT_LOAD` segments, or `None` if `bytes`
/// is not one or has no loadable segment.
fn image_span(bytes: &[u8]) -> Option<(u64, u64)> {
    const PAGE: u64 = address_space_map::PAGE;
    let u16_at = |o: usize| Some(u16::from_le_bytes(bytes.get(o..o + 2)?.try_into().ok()?));
    let u32_at = |o: usize| Some(u32::from_le_bytes(bytes.get(o..o + 4)?.try_into().ok()?));
    let u64_at = |o: usize| Some(u64::from_le_bytes(bytes.get(o..o + 8)?.try_into().ok()?));
    if bytes.get(0..6)? != [0x7f, b'E', b'L', b'F', 2, 1] {
        return None;
    }
    let phoff = usize::try_from(u64_at(32)?).ok()?;
    let phentsize = usize::from(u16_at(54)?);
    let phnum = usize::from(u16_at(56)?);
    let (mut lo, mut hi) = (u64::MAX, 0u64);
    for i in 0..phnum {
        let ph = phoff.checked_add(i.checked_mul(phentsize)?)?;
        if u32_at(ph)? != 1 {
            continue; // not PT_LOAD
        }
        let vaddr = u64_at(ph + 16)?;
        let end = vaddr.checked_add(u64_at(ph + 40)?)?;
        lo = lo.min(vaddr / PAGE * PAGE);
        hi = hi.max(end.div_ceil(PAGE) * PAGE);
    }
    (lo < hi).then_some((lo, hi))
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    /// A 64-bit little-endian ELF with one `PT_LOAD` segment of 0x1800 bytes at `vaddr`.
    fn elf_at(vaddr: u64) -> Vec<u8> {
        let mut v = vec![0u8; 64 + 56];
        v[0..6].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1]);
        v[32..40].copy_from_slice(&64u64.to_le_bytes());
        v[54..56].copy_from_slice(&56u16.to_le_bytes());
        v[56..58].copy_from_slice(&1u16.to_le_bytes());
        v[64..68].copy_from_slice(&1u32.to_le_bytes());
        v[64 + 16..64 + 24].copy_from_slice(&vaddr.to_le_bytes());
        v[64 + 40..64 + 48].copy_from_slice(&0x1800u64.to_le_bytes());
        v
    }

    /// The fixture archive: a program in the band, the two `std` programs every image ships, and
    /// 5000 bytes from a fixed xorshift stream.
    fn fixture() -> Vec<(&'static str, Vec<u8>)> {
        let mut noise = Vec::new();
        let mut x = 0x2545_f491u32;
        for _ in 0..5000 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            noise.push((x >> 24) as u8);
        }
        vec![
            ("progenitor", elf_at(address_space_map::IMAGE_BASE)),
            ("jig", b"jig".to_vec()),
            ("mdr", b"mdr".to_vec()),
            ("wc", noise),
        ]
    }

    /// **The bytes this crate writes are the bytes `xtask/src/archive.rs` wrote** before the packer
    /// moved here. The digest was taken from the old `write_archive_with` over this fixture on
    /// 2026-11-01 (UTC); the 15,872 bytes are the five fixture entries, the catalog, the sources and
    /// the measurement table. A change to the format, the entry order or the table moves it.
    #[test]
    fn the_fixture_archive_has_the_digest_the_old_packer_gave_it() {
        let image = compose("fixture", &fixture(), "catalog\n", "sources\n").unwrap();
        assert_eq!(image.bytes.len(), 15_872);
        assert_eq!(image.entries, 7);
        let digest = measured_boot::digest_text(&measured_boot::sha256(&image.bytes));
        assert_eq!(
            core::str::from_utf8(&digest).unwrap(),
            "sha256:bafba156b0342c87bd10fc500cc696ec045f7ee8569dabef3abf0518fde75101"
        );
    }

    /// The image reads back through the reader the kernel uses, and its table names every entry
    /// but itself, sorted.
    #[test]
    fn the_image_reads_back_and_its_table_measures_every_other_entry() {
        let image = compose("fixture", &fixture(), "catalog\n", "sources\n").unwrap();
        let fs = nifefs::Fs::parse(&image.bytes).unwrap();
        assert_eq!(fs.read("jig"), Some(&b"jig"[..]));
        assert_eq!(fs.read(package_archive::CATALOG), Some(&b"catalog\n"[..]));
        let table =
            core::str::from_utf8(fs.read(measured_boot::PROGRAM_MEASUREMENTS).unwrap()).unwrap();
        let names: Vec<&str> = table
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| l.split(' ').next().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "jig",
                "mdr",
                "package_catalog",
                "package_index_sources",
                "progenitor",
                "wc"
            ]
        );
    }

    /// An archive without `jig`, or without `mdr`, is refused with the program's name rather than
    /// packed into an image whose shell answers the name with a spawn failure.
    #[test]
    fn an_archive_without_jig_or_mdr_is_refused_by_name() {
        let e = shipped_std_programs_present("x86_64", &[("progenitor", b"")]).unwrap_err();
        assert!(e.to_string().contains("no `jig`"), "{e}");
        let e = shipped_std_programs_present("x86_64", &[("jig", b"")]).unwrap_err();
        assert!(e.to_string().contains("no `mdr`"), "{e}");
        assert!(shipped_std_programs_present("x86_64", &[("jig", b""), ("mdr", b"")]).is_ok());
        let mut blobs = fixture();
        blobs.retain(|(name, _)| *name != "mdr");
        assert!(matches!(
            compose("x86_64", &blobs, "", ""),
            Err(Error::MissingShippedProgram { program: "mdr", .. })
        ));
    }

    /// A program linked where the map says passes, and one linked at lld's default base, where
    /// `mkfs` was, is named.
    #[test]
    fn an_archive_program_outside_the_image_band_is_named() {
        let good = elf_at(address_space_map::IMAGE_BASE);
        let mkfs_as_it_was = elf_at(0x20_0000);
        assert_eq!(
            image_span(&good),
            Some((
                address_space_map::IMAGE_BASE,
                address_space_map::IMAGE_BASE + 0x2000
            ))
        );
        assert!(
            programs_fit_the_address_space_map(&[("good", &good), ("catalog", b"text")]).is_ok()
        );
        let Err(Error::OutsideImageBand(misfits)) =
            programs_fit_the_address_space_map(&[("mkfs", &mkfs_as_it_was)])
        else {
            panic!("a program at 0x200000 fits the band");
        };
        assert_eq!(misfits.len(), 1);
        assert_eq!(misfits[0].name, "mkfs");
    }

    /// A name `nifefs` cannot hold is carried out with the file count and size, which are what a
    /// reader hunts for otherwise.
    #[test]
    fn a_name_nifefs_refuses_comes_back_with_the_counts() {
        let mut blobs = fixture();
        blobs.push(("a\0b", Vec::new()));
        let Err(Error::Pack { error, files, .. }) = compose("x86_64", &blobs, "", "") else {
            panic!("a name with a NUL was packed");
        };
        assert_eq!(error, nifefs::Error::NameHasNul);
        assert_eq!(files, 8);
    }
}
