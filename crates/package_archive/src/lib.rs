//! **`package_archive`**: the one file a nife package is, read and written by one definition. §197
//! (a package is one archive file) ruled the container, and §195 (a reviewed recipe vouches for a
//! package) puts the digest that decides whether the bytes may run in the recipe. A host tool
//! writes a package with [`write_package`] and a target reads it with [`Package::parse`], as with
//! `crates/nifefs`. Pure logic, `no_std` and allocation-free: a reader borrows out of its buffer,
//! because the first consumer's loader has no allocator. notes/packages.md, "The format".

//! ```text
//!   header, HEADER_LEN = 112 bytes; integers u32 LE, names NUL-padded to NAME_LEN = 32 bytes:
//!     magic "NIFEPKG1" (8 bytes, version in the last) | name | version | architecture
//!     member_count | reserved, zero, so the table starts 8-byte aligned
//!   the table of contents, member_count entries of MEMBER_LEN = 72 bytes:
//!     name | offset, absolute from the start of the file | len
//!     digest, DIGEST_LEN = 32 bytes, SHA-256 of that member's bytes
//!   member bytes, each starting at an 8-byte boundary
//! ```

//! # BUGS
//!
//! - Nothing installs a package: that is milestone 507 (installing a package)'s activation fork.
//! - The encoding and name are provisional until somebody else fetches a package (§197).
//! - Where a manifest travels, and whether the digest is a Merkle root, are open.
//! - No compression, which would be a second hostile-input parser; the size cost is unmeasured.
//! - Members and files are bounded by `u32` (4 GiB).
//! - A duplicate member name is refused by the writer and first-wins in the reader, as in `nifefs`.
//! - [`Package::parse`] allows overlapping members; accessors are bounds-checked, digests decide.
//! - Nothing here says where a package came from; §195's per-source trust is a client's job.

//! Name: provisional 2026-09-23 (milestone 198's rung 3a lane): an archive of a package's files, in
//! the field's words; `package` alone is too generic and `pkg` an abbreviation. Not ratified.

#![no_std]

pub use measured_boot::{DIGEST_LEN, Digest, sha256};

/// **The archive entry an image carries its own package source's catalog under** (milestone 198
/// rung 3a). One `<name>-<version>-<architecture> sha256:<64 hex>` line per package, which is
/// [`measured_boot`]'s manifest shape, so [`measured_boot::expected_in_manifest`] reads it. It is
/// packed *before* the measurement table, so the kernel's trust root vouches for it and a package
/// fetched over plain HTTP is checked against a digest that never crossed the network: DECISIONS
/// §195 (a reviewed recipe vouches for a package)'s "the image's measured table becomes the first
/// source". The host writer is `cargo xtask`'s archive build; the readers are the kernel's package
/// tests. Name: provisional 2026-09-24.
pub const CATALOG: &str = "package_catalog";

/// The magic, with the format version in the last byte, so a reader meeting a later package says
/// [`Error::BadMagic`] rather than striding a table whose entries have moved. `nifefs` records the
/// rule this follows: bump when a reader can tell.
pub const MAGIC: [u8; 8] = *b"NIFEPKG1";

/// The longest name, in bytes, for the package and for each member.
///
/// **32, which is `nifefs::NAME_LEN` and not a coincidence.** A member that cannot be an archive
/// entry is a member nothing in this system could install, so the two ceilings are one ceiling. It
/// is not a dependency on that crate: taking one to share a constant would put a filesystem in the
/// graph of every program that reads a package.
pub const NAME_LEN: usize = 32;

/// The header: magic, three names, the count, and four bytes of zero that keep the table 8-byte
/// aligned.
pub const HEADER_LEN: usize = 8 + NAME_LEN * 3 + 4 + 4;

/// One table-of-contents entry: the name, `offset` and `len` as `u32`s, then the member's digest.
///
/// `offset` is absolute rather than relative to the member area, so a reader never has to know
/// where the table ends. Each member carries its own digest, so a spawner handed one program can
/// ask whether that member is the reviewed one; the table of contents is `crates/measured_boot`'s
/// name-to-digest map travelling with its bytes. Members are 8-byte aligned because the largest is
/// an ELF. A name uses at most [`NAME_LEN`] bytes, unterminated when full, which is `nifefs`'s
/// ceiling on every spawnable name.
pub const MEMBER_LEN: usize = NAME_LEN + 4 + 4 + DIGEST_LEN;

/// The alignment every member's bytes start on. See the crate docs for why it is not 1.
pub const MEMBER_ALIGN: usize = 8;

/// The most members a package may hold.
///
/// **A ceiling exists so that a header claiming four billion members is refused before anything is
/// multiplied by it**, which is the arithmetic a hostile file reaches for first. 64 is a judgment:
/// the largest thing anyone has proposed packaging is one program, its license, its manifest and
/// its documentation bundle, which is four. Raising it costs nothing but the bound in this line.
pub const MAX_MEMBERS: usize = 64;

/// What a package says about itself: the identity §197 rules it is named by, plus the architecture,
/// because "per-architecture output is the normal case" (§197) and three triples means three files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attributes<'a> {
    /// The package's name, at most [`NAME_LEN`] bytes.
    pub name: &'a str,
    /// Its version, as the recipe spells it. Compared as bytes; nothing here orders versions.
    pub version: &'a str,
    /// The target triple's short name, as this tree spells it: `aarch64`, `riscv64`, `x86_64`.
    pub architecture: &'a str,
}

/// Why a package was refused, by [`Package::parse`] or by [`write_package`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The first eight bytes are not [`MAGIC`], or the file is shorter than the header.
    BadMagic,
    /// The header claims more members than [`MAX_MEMBERS`].
    TooManyMembers,
    /// The file is shorter than the table of contents it claims, or a member's `offset` and `len`
    /// run past its end. One variant for both, because from a reader's side they are one fact: the
    /// file does not contain what it says it contains.
    Truncated,
    /// A name longer than [`NAME_LEN`] bytes. Refused rather than truncated, for `nifefs`'s reason:
    /// two names agreeing in their first `NAME_LEN` bytes would become one member, and whichever
    /// was packed first would answer for both.
    NameTooLong,
    /// A name containing a NUL, which the padding cannot distinguish from the end of the name.
    NameHasNul,
    /// Two members written under one name. The writer refuses it; see the crate's BUGS.
    DuplicateName,
    /// The buffer handed to [`write_package`] is not exactly [`package_size`] bytes.
    BufferWrongSize,
}

/// A member whose bytes do not hash to the digest the table of contents claims, by its index.
///
/// Its own type rather than an [`Error`] variant, because these are different questions asked at
/// different times: [`Package::parse`] asks whether a file can be read at all, and
/// [`Package::verify`] asks whether it is the file a recipe reviewed. A caller that has already
/// checked the whole file's digest against a recipe may skip the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyError {
    /// Which member failed, as an index into the table of contents.
    pub index: usize,
}

/// A parsed package: the bytes, with the table of contents proven to be inside them.
///
/// Holds the buffer and nothing else. Every accessor re-reads the table, which costs a few
/// arithmetic operations and buys a type that is four words wide and can be a local on a small
/// stack. `nifefs::Fs` arrived at the same shape by having to.
///
/// Equality is over the whole buffer, which is the only meaning that would not surprise: two
/// packages sharing a name are not the same package. The tests that assert a refusal need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Package<'a> {
    bytes: &'a [u8],
    count: usize,
}

impl<'a> Package<'a> {
    /// Read a package's structure, or say why it cannot be read.
    ///
    /// **This is the hostile-input entry point.** It proves, before any accessor runs, that the
    /// magic matches, that the count is within [`MAX_MEMBERS`], that the table of contents lies
    /// inside the buffer, and that every member's `offset .. offset + len` does too, with the
    /// addition done in `usize` on values widened from `u32` so it cannot wrap on any target this
    /// tree builds for. What it does not check is in the crate's BUGS.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() < HEADER_LEN || bytes[..8] != MAGIC {
            return Err(Error::BadMagic);
        }
        let count = u32::from_le_bytes([
            bytes[HEADER_LEN - 8],
            bytes[HEADER_LEN - 7],
            bytes[HEADER_LEN - 6],
            bytes[HEADER_LEN - 5],
        ]) as usize;
        if count > MAX_MEMBERS {
            return Err(Error::TooManyMembers);
        }
        let table_end = HEADER_LEN + count * MEMBER_LEN;
        if bytes.len() < table_end {
            return Err(Error::Truncated);
        }

        let package = Package { bytes, count };
        for index in 0..count {
            let (offset, len) = package.extent(index);
            // Widened to `usize` before adding: on a 64-bit target two `u32`s cannot overflow it,
            // and on a 32-bit one `checked_add` is what stops a file claiming a member that wraps
            // to a small, in-bounds slice.
            let end = offset.checked_add(len).ok_or(Error::Truncated)?;
            if end > bytes.len() {
                return Err(Error::Truncated);
            }
        }
        Ok(package)
    }

    /// The package's name, as the writer spelled it.
    pub fn name(&self) -> &'a str {
        field(self.bytes, 8)
    }

    /// Its version.
    pub fn version(&self) -> &'a str {
        field(self.bytes, 8 + NAME_LEN)
    }

    /// The architecture its members were built for.
    pub fn architecture(&self) -> &'a str {
        field(self.bytes, 8 + NAME_LEN * 2)
    }

    /// How many members it holds.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether it holds none, which a package with only attributes legitimately may.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The name of the member at `index`, or `None` past the end.
    pub fn member_name(&self, index: usize) -> Option<&'a str> {
        (index < self.count).then(|| field(self.bytes, HEADER_LEN + index * MEMBER_LEN))
    }

    /// The bytes of the member at `index`, or `None` past the end.
    pub fn member(&self, index: usize) -> Option<&'a [u8]> {
        if index >= self.count {
            return None;
        }
        let (offset, len) = self.extent(index);
        // `parse` proved this range; the slice is what it proved it for.
        Some(&self.bytes[offset..offset + len])
    }

    /// The digest the table of contents claims for the member at `index`.
    ///
    /// Claims, not proves: [`Package::verify`] is what turns a claim into a check.
    pub fn member_digest(&self, index: usize) -> Option<Digest> {
        if index >= self.count {
            return None;
        }
        let at = HEADER_LEN + index * MEMBER_LEN + NAME_LEN + 8;
        let mut digest = [0u8; DIGEST_LEN];
        digest.copy_from_slice(&self.bytes[at..at + DIGEST_LEN]);
        Some(digest)
    }

    /// The bytes of the first member called `name`, if there is one. See BUGS on duplicates.
    pub fn read(&self, name: &str) -> Option<&'a [u8]> {
        self.index_of(name).and_then(|index| self.member(index))
    }

    /// Where `name` sits in the table of contents, if it is there at all.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        (0..self.count).find(|&index| self.member_name(index) == Some(name))
    }

    /// Check every member's bytes against the digest beside it.
    ///
    /// The whole point of the per-member digest: a caller that trusts the file (a recipe vouched
    /// for its digest, §195) still learns *which* member it is holding, and a caller that does not
    /// learns whether anything in the file has moved.
    ///
    /// # Examples
    ///
    /// Refuse a file that has been edited since it was written:
    ///
    /// ```
    /// # use package_archive::{Attributes, Package, VerifyError, package_size, write_package};
    /// # let members: [(&str, &[u8]); 1] = [("uptime", b"\x7fELF...")];
    /// # let attributes = Attributes { name: "uptime", version: "0.1.0", architecture: "aarch64" };
    /// # let mut file = vec![0u8; package_size(&members)];
    /// # write_package(&attributes, &members, &mut file).unwrap();
    /// let last = file.len() - 1;
    /// file[last] ^= 0xff;
    /// assert_eq!(Package::parse(&file).unwrap().verify(), Err(VerifyError { index: 0 }));
    /// ```
    pub fn verify(&self) -> Result<(), VerifyError> {
        for index in 0..self.count {
            self.verify_member(index)?;
        }
        Ok(())
    }

    /// Check one member, by index. Returns `Ok(())` for an index past the end, because there is no
    /// member there to be wrong; callers that care use [`Package::len`].
    pub fn verify_member(&self, index: usize) -> Result<(), VerifyError> {
        let (Some(bytes), Some(claimed)) = (self.member(index), self.member_digest(index)) else {
            return Ok(());
        };
        if sha256(bytes) == claimed {
            Ok(())
        } else {
            Err(VerifyError { index })
        }
    }

    /// The `(offset, len)` of a member, read from a table `parse` has already bounds-checked.
    fn extent(&self, index: usize) -> (usize, usize) {
        let at = HEADER_LEN + index * MEMBER_LEN + NAME_LEN;
        let word = |o: usize| {
            u32::from_le_bytes([
                self.bytes[at + o],
                self.bytes[at + o + 1],
                self.bytes[at + o + 2],
                self.bytes[at + o + 3],
            ]) as usize
        };
        (word(0), word(4))
    }
}

/// The longest `name-version-architecture` stem a package can have: three [`NAME_LEN`] fields and
/// two hyphens.
pub const STEM_LEN: usize = NAME_LEN * 3 + 2;

/// **A package the image vouches for, reduced to what installing it needs** (milestone 198 (a
/// package manager) rung 3a's installer; milestone 919 (a package carries several programs)): the
/// package, every one of whose programs [`installable`] has checked. Read them with
/// [`Installable::programs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Installable<'a> {
    package: Package<'a>,
}

/// **One program a package carries**: the name a person types, its bytes and their digest.
///
/// Name: provisional, milestone 919's lane, 2026-10-14 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Program<'a> {
    /// The program's name: the package's own for the member named after it, and the name after
    /// [`PROGRAM_PREFIX`] for a `bin/` member.
    pub name: &'a str,
    /// The executable member's bytes.
    pub bytes: &'a [u8],
    /// Their SHA-256, as the table of contents carries it and [`installable`] checked it. This is
    /// the digest the installed-programs table records and the spawner computes (DECISIONS §219 (how the
    /// shell names an installed program to the spawner) option D).
    pub digest: Digest,
    /// The member's index in the table of contents, which [`program_name`] reads back.
    pub member: usize,
}

impl<'a> Installable<'a> {
    /// The package the programs came from.
    pub fn package(&self) -> Package<'a> {
        self.package
    }

    /// **Every program the package carries**, in table-of-contents order. At least one, each name
    /// once, and each member's bytes matched its digest when [`installable`] accepted it.
    pub fn programs(&self) -> impl Iterator<Item = Program<'a>> + 'a {
        let package = self.package;
        (0..package.len()).filter_map(move |index| {
            let name = program_name(&package, index)?;
            Some(Program {
                name,
                bytes: package.member(index)?,
                digest: package.member_digest(index)?,
                member: index,
            })
        })
    }
}

/// **The program member `index` is, if it is one**: the member named after the package, or a
/// `bin/<program>` member. A `bin/` name that is empty or nested is returned as it is, for
/// [`installable`] to refuse. Public so an installer that sent a package can name the program a
/// refusal is about from the member index the reply carries.
pub fn program_name<'a>(package: &Package<'a>, index: usize) -> Option<&'a str> {
    let member = package.member_name(index)?;
    if member == package.name() {
        return Some(member);
    }
    member.strip_prefix(PROGRAM_PREFIX)
}

/// **Every program member, checked before any is installed** (milestone 919): at least one; no
/// `bin/` name empty or holding a `/`; no program named twice; every member's bytes matching its
/// table-of-contents digest.
fn check_programs(package: &Package<'_>) -> Result<(), Refusal> {
    let mut found = false;
    for index in 0..package.len() {
        let Some(name) = program_name(package, index) else {
            continue;
        };
        if name.is_empty() || name.contains('/') {
            return Err(Refusal::ProgramName);
        }
        let again = (0..index).any(|earlier| program_name(package, earlier) == Some(name));
        if again {
            return Err(Refusal::ProgramName);
        }
        let (Some(member), Some(digest)) = (package.member(index), package.member_digest(index))
        else {
            return Err(Refusal::NoProgram);
        };
        if sha256(member) != digest {
            return Err(Refusal::MemberMismatch);
        }
        found = true;
    }
    if found {
        Ok(())
    } else {
        Err(Refusal::NoProgram)
    }
}

/// Why [`installable`] refused a package. Each is a different thing a person should be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The bytes are not a package this parser can read.
    Unreadable,
    /// The catalog has no line for this package's stem, or has one and these bytes do not hash to
    /// it. One refusal for both, because to the installer they are one fact: the image does not
    /// vouch for these bytes (DECISIONS §195 (a reviewed recipe vouches for a package)).
    NotCataloged,
    /// The package carries no program: no member named after itself and no `bin/` member.
    NoProgram,
    /// **A `bin/` member's program name is empty or holds a `/`, or two members name one
    /// program** (milestone 919 (a package carries several programs)). The whole package is
    /// refused rather than installed without that program.
    ///
    /// Name: provisional, milestone 919's lane, 2026-10-14 (UTC).
    ProgramName,
    /// A program member does not hash to the digest its own table of contents claims. Only a
    /// cataloged package can reach this, so it means the catalog vouched for a file the producer
    /// would never have written.
    MemberMismatch,
    /// A package fetched by name ([`installable_as`]) is not the package that was asked for, though
    /// the catalog may vouch for it: a source serving `uptime` when asked for `greeting`.
    NotRequested,
}

/// **The installer's whole decision, on bytes alone** (milestone 198 rung 3a), in an order that is
/// the argument (notes/packages/package-archive.md):
/// 1. The header is parsed (hostile input before any digest; fuzzed and Kani-checked) for its stem.
/// 2. **The whole file's SHA-256 must be the image's catalog line for that stem** ([`CATALOG`]).
/// 3. **Every program member is found**: the one named after the package, and each
///    `bin/<program>` ([`PROGRAM_PREFIX`], a provisional convention); none is
///    [`Refusal::NoProgram`], a bad or repeated name [`Refusal::ProgramName`].
/// 4. Each program's bytes are checked against its table digest before any is returned.
///
/// `stem` is scratch for the `name-version-architecture` key ([`Package::stem`] reads it back).
pub fn installable<'a>(catalogue: &str, bytes: &'a [u8]) -> Result<Installable<'a>, Refusal> {
    let package = Package::parse(bytes).map_err(|_| Refusal::Unreadable)?;
    let mut stem = [0u8; STEM_LEN];
    let expected = measured_boot::expected_in_manifest(catalogue, package.stem(&mut stem))
        .ok_or(Refusal::NotCataloged)?;
    if sha256(bytes) != expected {
        return Err(Refusal::NotCataloged);
    }
    check_programs(&package)?;
    Ok(Installable { package })
}

/// **The prefix that marks a member as a program not named after its package** (milestone 810
/// (`ripgrep` is packaged in basalt)): `bin/rg` in the package `ripgrep` is the program `rg`.
/// Name: provisional 2026-10-12 (UTC), as is the convention, which [`installable`]'s step 3 states.
pub const PROGRAM_PREFIX: &str = "bin/";

/// **The catalog's stem for a package asked for by name** (milestone 198 (a package manager)
/// rung 3a's fetch): the line whose stem is `<name>-<version>-<architecture>`, for a version with
/// no hyphen in it. [`CatalogMiss::NoSuchPackage`] when the image vouches for no package of that
/// name on this architecture, the refusal a fetch gives before it opens a connection.
///
/// **`name@version` asks for one version; a bare name must mean exactly one** (milestone 614 (two
/// installed versions of one program, each runnable, and a caller granted the one it needs)), else
/// [`CatalogMiss::SeveralVersions`]: nothing here orders versions to say which is newest. The
/// catalog is the image's own ([`CATALOG`]), never what a source offers. A version with a hyphen
/// cannot be told from the name before it and is not matched.
pub fn cataloged_stem<'c>(
    catalogue: &'c str,
    name: &str,
    architecture: &str,
) -> Result<&'c str, CatalogMiss> {
    matching_stem(
        measured_boot::manifest_entries(catalogue)
            .flatten()
            .map(|(stem, _)| stem),
        name,
        architecture,
    )
}

/// **[`cataloged_stem`]'s rule over any list of stems**, for a reader whose lines are not the
/// catalog's: milestone 801 (packages over the internet)'s index carries a location after each
/// stem and digest, so `measured_boot::manifest_entries` would refuse its lines, and the rule for
/// what `name` and `name@version` mean must still be this one, written once.
///
/// Name: provisional 2026-10-09 (UTC), milestone 801's lane.
pub fn matching_stem<'c>(
    stems: impl IntoIterator<Item = &'c str>,
    name: &str,
    architecture: &str,
) -> Result<&'c str, CatalogMiss> {
    let (name, wanted) = match name.split_once('@') {
        Some((name, version)) => (name, Some(version)),
        None => (name, None),
    };
    if name.is_empty() || wanted == Some("") {
        return Err(CatalogMiss::NoSuchPackage);
    }
    let mut found: Option<&'c str> = None;
    for stem in stems {
        let version = stem
            .strip_suffix(architecture)
            .and_then(|s| s.strip_suffix('-'))
            .and_then(|s| s.strip_prefix(name))
            .and_then(|s| s.strip_prefix('-'))
            .filter(|version| !version.is_empty() && !version.contains('-'));
        let Some(version) = version else { continue };
        if wanted.is_some_and(|wanted| wanted != version) {
            continue;
        }
        match found {
            // The same stem twice is one package written twice, not two versions.
            Some(earlier) if earlier != stem => return Err(CatalogMiss::SeveralVersions),
            _ => found = Some(stem),
        }
    }
    found.ok_or(CatalogMiss::NoSuchPackage)
}

/// Why [`cataloged_stem`] found no one stem.
///
/// Name: ratified 2026-10-07 (calef, §258 (names for two installed versions of one program)).
/// Refused `StemMiss` ("stem" is internal jargon; calef: "`StemMiss` is horrible"). Minted as
/// `StemMiss` by milestone 614 (two installed versions of one program, each runnable)'s lane,
/// 2026-10-02.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogMiss {
    /// No line names a package of that name (at that version, if one was asked for) on this
    /// architecture.
    NoSuchPackage,
    /// A bare name, and the catalog vouches for more than one version of it.
    SeveralVersions,
}

/// **[`installable`], for bytes fetched by name**: the package must also be the one whose stem was
/// asked for. A source can serve a *different* package the catalog vouches for, and
/// [`installable`] alone would install it; the stem is checked first, so such a package is refused
/// before it is hashed.
pub fn installable_as<'a>(
    catalogue: &str,
    stem: &str,
    bytes: &'a [u8],
) -> Result<Installable<'a>, Refusal> {
    let package = Package::parse(bytes).map_err(|_| Refusal::Unreadable)?;
    let mut buf = [0u8; STEM_LEN];
    if package.stem(&mut buf) != stem {
        return Err(Refusal::NotRequested);
    }
    installable(catalogue, bytes)
}

impl<'a> Package<'a> {
    /// **The package's `name-version-architecture` stem**, the key its catalog line and its file
    /// name share, composed into `out`.
    pub fn stem<'b>(&self, out: &'b mut [u8; STEM_LEN]) -> &'b str {
        let mut at = 0;
        for (i, part) in [self.name(), self.version(), self.architecture()]
            .iter()
            .enumerate()
        {
            if i > 0 {
                out[at] = b'-';
                at += 1;
            }
            out[at..at + part.len()].copy_from_slice(part.as_bytes());
            at += part.len();
        }
        // Three fields read as UTF-8 and two ASCII hyphens: this cannot fail, and `unwrap_or`
        // keeps the path panic-free.
        core::str::from_utf8(&out[..at]).unwrap_or("")
    }
}

/// Read a NUL-padded name of [`NAME_LEN`] bytes at `at`.
///
/// Invalid UTF-8 comes back as the empty string rather than as an error, which is deliberate and is
/// the one place this file is lenient: a name is compared, and a name that cannot be spelled matches
/// nothing, so a caller looking one up fails to find it. `parse`'s job is bounds, not spelling.
fn field(bytes: &[u8], at: usize) -> &str {
    let raw = &bytes[at..at + NAME_LEN];
    let end = raw.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
    core::str::from_utf8(&raw[..end]).unwrap_or("")
}

/// How many bytes [`write_package`] needs for these members, including the padding that aligns each
/// one. Deterministic: the same inputs give the same size and the same bytes, which is what lets a
/// recipe record a digest and a rebuild reproduce it.
pub fn package_size(members: &[(&str, &[u8])]) -> usize {
    let mut at = HEADER_LEN + members.len() * MEMBER_LEN;
    for (_, bytes) in members {
        at = at.next_multiple_of(MEMBER_ALIGN) + bytes.len();
    }
    at
}

/// Write a package into `buffer`, which must be exactly [`package_size`] bytes.
///
/// Refuses rather than truncates or mangles: a name too long, with a NUL, or repeated, too many
/// members, or a buffer of the wrong size. **No timestamp, no ordering pass, no nonzero padding**:
/// the bytes are a function of the inputs alone, so two hosts building one recipe produce one
/// digest, which §195's reviewed recipe rests on.
///
/// # Examples
///
/// Write a package, read it back, and check a member against its digest:
///
/// ```
/// # use package_archive::{Attributes, Package, package_size, write_package};
/// let members: [(&str, &[u8]); 2] = [("uptime", b"\x7fELF..."), ("uptime.license", b"MIT")];
/// let attributes = Attributes { name: "uptime", version: "0.1.0", architecture: "aarch64" };
///
/// let mut file = vec![0u8; package_size(&members)];
/// write_package(&attributes, &members, &mut file).unwrap();
///
/// let package = Package::parse(&file).unwrap();
/// assert_eq!(package.name(), "uptime");
/// assert_eq!(package.version(), "0.1.0");
/// assert_eq!(package.read("uptime"), Some(&b"\x7fELF..."[..]));
/// package.verify().unwrap();
/// ```
pub fn write_package(
    attributes: &Attributes<'_>,
    members: &[(&str, &[u8])],
    buffer: &mut [u8],
) -> Result<(), Error> {
    if members.len() > MAX_MEMBERS {
        return Err(Error::TooManyMembers);
    }
    if buffer.len() != package_size(members) {
        return Err(Error::BufferWrongSize);
    }
    check_name(attributes.name)?;
    check_name(attributes.version)?;
    check_name(attributes.architecture)?;
    for (index, (name, _)) in members.iter().enumerate() {
        check_name(name)?;
        if members[..index].iter().any(|(earlier, _)| earlier == name) {
            return Err(Error::DuplicateName);
        }
    }

    buffer.fill(0);
    buffer[..8].copy_from_slice(&MAGIC);
    put_name(buffer, 8, attributes.name);
    put_name(buffer, 8 + NAME_LEN, attributes.version);
    put_name(buffer, 8 + NAME_LEN * 2, attributes.architecture);
    let count = members.len() as u32;
    buffer[HEADER_LEN - 8..HEADER_LEN - 4].copy_from_slice(&count.to_le_bytes());

    let mut at = HEADER_LEN + members.len() * MEMBER_LEN;
    for (index, (name, bytes)) in members.iter().enumerate() {
        at = at.next_multiple_of(MEMBER_ALIGN);
        let entry = HEADER_LEN + index * MEMBER_LEN;
        put_name(buffer, entry, name);
        buffer[entry + NAME_LEN..entry + NAME_LEN + 4].copy_from_slice(&(at as u32).to_le_bytes());
        buffer[entry + NAME_LEN + 4..entry + NAME_LEN + 8]
            .copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        buffer[entry + NAME_LEN + 8..entry + MEMBER_LEN].copy_from_slice(&sha256(bytes));
        buffer[at..at + bytes.len()].copy_from_slice(bytes);
        at += bytes.len();
    }
    Ok(())
}

fn check_name(name: &str) -> Result<(), Error> {
    if name.len() > NAME_LEN {
        Err(Error::NameTooLong)
    } else if name.as_bytes().contains(&0) {
        Err(Error::NameHasNul)
    } else {
        Ok(())
    }
}

fn put_name(buffer: &mut [u8], at: usize, name: &str) {
    buffer[at..at + name.len()].copy_from_slice(name.as_bytes());
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec;

    use super::*;

    fn written(members: &[(&str, &[u8])]) -> std::vec::Vec<u8> {
        let attributes = Attributes {
            name: "uptime",
            version: "0.1.0",
            architecture: "aarch64",
        };
        let mut buffer = vec![0u8; package_size(members)];
        write_package(&attributes, members, &mut buffer).unwrap();
        buffer
    }

    #[test]
    fn a_package_reads_back_what_was_written() {
        let members: [(&str, &[u8]); 3] = [
            ("uptime", b"\x7fELF and then some"),
            ("uptime.licence", b"MIT OR Apache-2.0"),
            ("uptime.manual", b""),
        ];
        let file = written(&members);
        let package = Package::parse(&file).unwrap();

        assert_eq!(package.name(), "uptime");
        assert_eq!(package.version(), "0.1.0");
        assert_eq!(package.architecture(), "aarch64");
        assert_eq!(package.len(), 3);
        for (name, bytes) in members {
            assert_eq!(package.read(name), Some(bytes));
        }
        assert_eq!(package.read("absent"), None);
        package.verify().unwrap();
    }

    #[test]
    fn every_member_starts_aligned() {
        let members: [(&str, &[u8]); 3] = [("a", b"1"), ("b", b"22"), ("c", b"333")];
        let file = written(&members);
        let package = Package::parse(&file).unwrap();
        for index in 0..package.len() {
            let (offset, _) = package.extent(index);
            assert_eq!(offset % MEMBER_ALIGN, 0, "member {index} at {offset}");
        }
    }

    #[test]
    fn the_same_inputs_give_the_same_bytes() {
        // The property a recipe's recorded digest rests on. If this ever fails, §195's whole
        // arrangement fails with it: a reviewer would be vouching for bytes nobody can reproduce.
        let members: [(&str, &[u8]); 2] = [("uptime", b"\x7fELF"), ("uptime.licence", b"MIT")];
        assert_eq!(written(&members), written(&members));
    }

    #[test]
    fn a_name_that_fills_the_field_has_no_terminator() {
        let full = "n".repeat(NAME_LEN);
        let members: [(&str, &[u8]); 1] = [(full.as_str(), b"x")];
        let file = written(&members);
        let package = Package::parse(&file).unwrap();
        assert_eq!(package.member_name(0), Some(full.as_str()));
        assert_eq!(package.read(&full), Some(&b"x"[..]));
    }

    #[test]
    fn the_writer_refuses_what_it_cannot_represent() {
        let long = "n".repeat(NAME_LEN + 1);
        let attributes = Attributes {
            name: "uptime",
            version: "0.1.0",
            architecture: "aarch64",
        };
        let cases: [(&str, Error); 3] = [
            (long.as_str(), Error::NameTooLong),
            ("has\0nul", Error::NameHasNul),
            ("uptime", Error::DuplicateName),
        ];
        for (name, expected) in cases {
            let members: [(&str, &[u8]); 2] = [("uptime", b"x"), (name, b"y")];
            let mut buffer = vec![0u8; package_size(&members)];
            assert_eq!(
                write_package(&attributes, &members, &mut buffer),
                Err(expected)
            );
        }
    }

    #[test]
    fn a_buffer_of_the_wrong_size_is_refused() {
        let members: [(&str, &[u8]); 1] = [("uptime", b"x")];
        let attributes = Attributes {
            name: "uptime",
            version: "0.1.0",
            architecture: "aarch64",
        };
        let mut buffer = vec![0u8; package_size(&members) + 1];
        assert_eq!(
            write_package(&attributes, &members, &mut buffer),
            Err(Error::BufferWrongSize)
        );
    }

    #[test]
    fn a_truncated_file_is_refused_not_indexed() {
        let members: [(&str, &[u8]); 2] = [("uptime", b"\x7fELF"), ("uptime.licence", b"MIT")];
        let file = written(&members);
        for cut in [0, HEADER_LEN - 1, HEADER_LEN, file.len() - 1] {
            let expected = if cut < HEADER_LEN {
                Error::BadMagic
            } else {
                Error::Truncated
            };
            assert_eq!(Package::parse(&file[..cut]), Err(expected), "cut at {cut}");
        }
    }

    #[test]
    fn a_count_beyond_the_ceiling_is_refused_before_it_is_multiplied() {
        let mut file = written(&[("uptime", b"x")]);
        file[HEADER_LEN - 8..HEADER_LEN - 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(Package::parse(&file), Err(Error::TooManyMembers));
    }

    #[test]
    fn a_member_pointing_outside_the_file_is_refused() {
        let mut file = written(&[("uptime", b"\x7fELF")]);
        let at = HEADER_LEN + NAME_LEN;
        file[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(Package::parse(&file), Err(Error::Truncated));
    }

    #[test]
    fn an_edited_member_fails_verification_and_names_itself() {
        let members: [(&str, &[u8]); 2] = [("uptime", b"\x7fELF"), ("uptime.licence", b"MIT")];
        let mut file = written(&members);
        let last = file.len() - 1;
        file[last] ^= 0xff;
        let package = Package::parse(&file).unwrap();
        assert_eq!(package.verify(), Err(VerifyError { index: 1 }));
        package.verify_member(0).unwrap();
    }

    fn catalogue_for(file: &[u8]) -> std::string::String {
        let hex = measured_boot::digest_text(&sha256(file));
        std::format!(
            "other-1.0-aarch64 sha256:{}\nuptime-0.1.0-aarch64 {}\n",
            "0".repeat(64),
            core::str::from_utf8(&hex).unwrap()
        )
    }

    /// **The installer installs the program a catalog vouches for, and nothing else** (milestone
    /// 198 rung 3a). The accepted case first, so each refusal below is a change of one thing.
    #[test]
    fn a_catalogued_package_yields_its_program_and_that_programs_digest() {
        let members: [(&str, &[u8]); 2] = [("uptime", b"\x7fELF"), ("uptime.licence", b"MIT")];
        let file = written(&members);
        let got = installable(&catalogue_for(&file), &file).unwrap();
        let programs: std::vec::Vec<Program> = got.programs().collect();
        assert_eq!(
            programs,
            [Program {
                name: "uptime",
                bytes: b"\x7fELF",
                digest: sha256(b"\x7fELF"),
                member: 0,
            }]
        );
        let mut stem = [0u8; STEM_LEN];
        assert_eq!(
            Package::parse(&file).unwrap().stem(&mut stem),
            "uptime-0.1.0-aarch64"
        );
    }

    /// **One flipped byte anywhere is refused by the catalog**, before the member is looked at:
    /// the flip is in the license, which no member check of the program would ever read.
    #[test]
    fn a_byte_the_catalogue_did_not_vouch_for_is_refused() {
        let members: [(&str, &[u8]); 2] = [("uptime", b"\x7fELF"), ("uptime.licence", b"MIT")];
        let file = written(&members);
        let catalogue = catalogue_for(&file);
        let mut tampered = file.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert_eq!(
            installable(&catalogue, &tampered),
            Err(Refusal::NotCataloged)
        );
        // And a catalog without the stem vouches for nothing, however good the bytes are.
        assert_eq!(
            installable("uptime-0.2.0-aarch64 00", &file),
            Err(Refusal::NotCataloged)
        );
        assert_eq!(
            installable(&catalogue, b"garbage"),
            Err(Refusal::Unreadable)
        );
    }

    /// **A vouched package with no member named after it installs nothing**, rather than guessing
    /// which member is the program.
    #[test]
    fn a_package_with_no_program_of_its_own_name_is_refused() {
        let file = written(&[("uptime.licence", b"MIT")]);
        assert_eq!(
            installable(&catalogue_for(&file), &file),
            Err(Refusal::NoProgram)
        );
    }

    /// The programs a vouched package yields, as `(name, length)` pairs, or why it yields none.
    fn programs_of(file: &[u8]) -> Result<std::vec::Vec<(std::string::String, usize)>, Refusal> {
        let digest = measured_boot::digest_text(&sha256(file));
        let vouched = std::format!(
            "uptime-0.1.0-aarch64 {}",
            core::str::from_utf8(&digest).unwrap()
        );
        installable(&vouched, file).map(|g| {
            g.programs()
                .map(|p| (p.name.into(), p.bytes.len()))
                .collect()
        })
    }

    /// **Every program member is a program** (milestones 810 and 919 (a package carries several
    /// programs)): the member named after the package and each `bin/<program>` member, in table
    /// order, named unlike the package or not. Files that are neither are not programs.
    #[test]
    fn every_bin_member_and_the_one_named_after_the_package_is_a_program() {
        let file = written(&[("bin/up", b"\x7fELF up"), ("uptime.license", b"MIT")]);
        assert_eq!(programs_of(&file), Ok(std::vec![("up".into(), 7)]));
        let file = written(&[
            ("bin/a", b"\x7fELF a"),
            ("uptime", b"\x7fELF"),
            ("uptime.license", b"MIT"),
            ("bin/bb", b"\x7fELF bb"),
        ]);
        assert_eq!(
            programs_of(&file),
            Ok(std::vec![
                ("a".into(), 6),
                ("uptime".into(), 4),
                ("bb".into(), 7)
            ])
        );
        let package = Package::parse(&file).unwrap();
        assert_eq!(program_name(&package, 3), Some("bb"));
        assert_eq!(program_name(&package, 2), None);
    }

    /// **One bad program refuses the whole package** (milestone 919): an empty or nested `bin/`
    /// name beside a good program, or the member named after the package and its `bin/` twin
    /// naming one program twice, installs nothing. Two members of one name the writer refuses.
    #[test]
    fn a_bad_program_name_refuses_the_whole_package() {
        let good = ("bin/ok", &b"\x7fELF"[..]);
        for bad in [
            &[good, ("bin/", &b"a"[..])][..],
            &[good, ("bin/x/y", &b"a"[..])][..],
            &[("uptime", &b"a"[..]), ("bin/uptime", &b"b"[..])][..],
        ] {
            assert_eq!(programs_of(&written(bad)), Err(Refusal::ProgramName));
        }
    }

    /// **Every program's bytes are checked, not only the first's** (milestone 919): a second
    /// program whose table-of-contents digest is wrong refuses the package. The catalog vouches
    /// for the whole file, so only a producer bug writes this; it is built here by hand.
    #[test]
    fn a_second_program_that_does_not_match_its_digest_is_refused() {
        let mut file = written(&[("bin/a", b"\x7fELF a"), ("bin/b", b"\x7fELF b")]);
        let (at, _) = Package::parse(&file).unwrap().extent(1);
        file[at] ^= 1;
        assert_eq!(programs_of(&file), Err(Refusal::MemberMismatch));
    }

    /// **A name finds its package's stem on this architecture and nowhere else** (rung 3a's
    /// fetch). A prefix of another package's name is not that package, and another
    /// architecture's line is not this machine's.
    #[test]
    fn a_name_finds_its_stem_in_the_catalogue() {
        let d = std::format!("sha256:{}", "0".repeat(64));
        let catalogue = std::format!(
            "uptime-0.1.0-aarch64 {d}\ngreeting-0.1.0-riscv64 {d}\ngreeting-0.1.0-aarch64 {d}\n"
        );
        assert_eq!(
            cataloged_stem(&catalogue, "greeting", "aarch64"),
            Ok("greeting-0.1.0-aarch64")
        );
        assert_eq!(
            cataloged_stem(&catalogue, "uptime", "aarch64"),
            Ok("uptime-0.1.0-aarch64")
        );
        let none = Err(CatalogMiss::NoSuchPackage);
        assert_eq!(cataloged_stem(&catalogue, "uptime", "riscv64"), none);
        assert_eq!(cataloged_stem(&catalogue, "greet", "aarch64"), none);
        assert_eq!(cataloged_stem(&catalogue, "", "aarch64"), none);
        assert_eq!(cataloged_stem(&catalogue, "nosuch", "aarch64"), none);
    }

    /// **Two cataloged versions: a bare name is refused, and `name@version` picks one**
    /// (milestone 614). The 0.2.0 line comes first, as it does in the image's own catalog,
    /// because its recipe's filename sorts first; the order must not decide what is fetched.
    #[test]
    fn a_bare_name_with_two_catalogued_versions_is_refused_and_a_qualified_one_is_not() {
        let d = std::format!("sha256:{}", "0".repeat(64));
        let catalogue = std::format!(
            "greeting-0.2.0-aarch64 {d}\ngreeting-0.1.0-aarch64 {d}\ngreeting-0.1.0-riscv64 {d}\n"
        );
        assert_eq!(
            cataloged_stem(&catalogue, "greeting", "aarch64"),
            Err(CatalogMiss::SeveralVersions)
        );
        assert_eq!(
            cataloged_stem(&catalogue, "greeting@0.1.0", "aarch64"),
            Ok("greeting-0.1.0-aarch64")
        );
        assert_eq!(
            cataloged_stem(&catalogue, "greeting@0.2.0", "aarch64"),
            Ok("greeting-0.2.0-aarch64")
        );
        // One version on this architecture is not ambiguous, whatever another one carries.
        assert_eq!(
            cataloged_stem(&catalogue, "greeting", "riscv64"),
            Ok("greeting-0.1.0-riscv64")
        );
        let none = Err(CatalogMiss::NoSuchPackage);
        assert_eq!(
            cataloged_stem(&catalogue, "greeting@0.3.0", "aarch64"),
            none
        );
        assert_eq!(cataloged_stem(&catalogue, "greeting@", "aarch64"), none);
        assert_eq!(cataloged_stem(&catalogue, "@0.1.0", "aarch64"), none);
        // A line written twice is one package.
        let twice = std::format!("greeting-0.1.0-aarch64 {d}\ngreeting-0.1.0-aarch64 {d}\n");
        assert_eq!(
            cataloged_stem(&twice, "greeting", "aarch64"),
            Ok("greeting-0.1.0-aarch64")
        );
    }

    /// **Bytes fetched by name must be the package asked for**, even when the catalog vouches
    /// for them: the source answered `greeting` with a genuine `uptime`.
    #[test]
    fn a_vouched_package_that_was_not_asked_for_is_refused() {
        let file = written(&[("uptime", b"\x7fELF")]);
        let catalogue = catalogue_for(&file);
        assert!(installable_as(&catalogue, "uptime-0.1.0-aarch64", &file).is_ok());
        assert_eq!(
            installable_as(&catalogue, "greeting-0.1.0-aarch64", &file),
            Err(Refusal::NotRequested)
        );
        // And the digest is still checked when the stem is right.
        let mut tampered = file.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert_eq!(
            installable_as(&catalogue, "uptime-0.1.0-aarch64", &tampered),
            Err(Refusal::NotCataloged)
        );
    }

    /// **The layout is the documented one, pinned by hand** and not by the constants the writer
    /// and the reader share: both would agree with each other after any change to them. A third
    /// party writing this format from the crate docs lands on exactly these offsets.
    #[test]
    fn the_header_and_table_sit_where_the_format_says() {
        assert_eq!(HEADER_LEN, 112);
        assert_eq!(MEMBER_LEN, 72);
        let file = written(&[("a", b"xyz"), ("b", b"")]);
        assert_eq!(&file[..8], b"NIFEPKG1");
        assert_eq!(&file[8..14], b"uptime");
        assert_eq!(&file[40..45], b"0.1.0");
        assert_eq!(&file[72..79], b"aarch64");
        assert_eq!(&file[104..108], 2u32.to_le_bytes());
        assert_eq!(&file[108..112], [0; 4]);
        assert_eq!(file[112], b'a');
        assert_eq!(file[112 + 72], b'b');
        let offset = u32::from_le_bytes(file[112 + 32..112 + 36].try_into().unwrap());
        assert_eq!(offset as usize % MEMBER_ALIGN, 0);
        assert!(offset as usize >= 112 + 2 * 72);
        assert_eq!(&file[112 + 36..112 + 40], 3u32.to_le_bytes());
        assert_eq!(&file[112 + 40..112 + 72], sha256(b"xyz"));
    }

    /// The count is read from where the header puts it, and the ceiling is inclusive: 64 members
    /// parse and write, 65 are refused by both.
    #[test]
    fn the_member_ceiling_is_inclusive_in_the_reader_and_the_writer() {
        let names: std::vec::Vec<std::string::String> =
            (0..=MAX_MEMBERS).map(|i| std::format!("m{i}")).collect();
        let all: std::vec::Vec<(&str, &[u8])> =
            names.iter().map(|n| (n.as_str(), &b"x"[..])).collect();
        let at_max = written(&all[..MAX_MEMBERS]);
        assert_eq!(Package::parse(&at_max).unwrap().len(), MAX_MEMBERS);

        let attributes = Attributes {
            name: "uptime",
            version: "0.1.0",
            architecture: "aarch64",
        };
        let mut buffer = vec![0u8; package_size(&all)];
        assert_eq!(
            write_package(&attributes, &all, &mut buffer),
            Err(Error::TooManyMembers)
        );

        // The reader's side: the same file with its count word raised by one.
        let mut over = at_max.clone();
        over[104..108].copy_from_slice(&(MAX_MEMBERS as u32 + 1).to_le_bytes());
        over.resize(over.len() + MEMBER_LEN, 0);
        assert_eq!(Package::parse(&over), Err(Error::TooManyMembers));
    }

    /// A package with no members is exactly a header, and that is a package. The count of the
    /// table is checked against the buffer: a file that ends inside it is truncated.
    #[test]
    fn an_empty_package_is_a_bare_header_and_a_short_table_is_truncated() {
        let empty = written(&[]);
        assert_eq!(empty.len(), HEADER_LEN);
        let package = Package::parse(&empty).unwrap();
        assert!(package.is_empty());
        assert_eq!(package.len(), 0);
        assert_eq!(package.member_name(0), None);

        let one = written(&[("a", b"x")]);
        assert!(!Package::parse(&one).unwrap().is_empty());
        assert_eq!(
            Package::parse(&one[..HEADER_LEN + MEMBER_LEN - 1]),
            Err(Error::Truncated)
        );
    }

    /// An index equal to the count is past the end, for the name as for the bytes.
    #[test]
    fn the_member_after_the_last_is_not_there() {
        let file = written(&[("a", b"x"), ("b", b"y")]);
        let package = Package::parse(&file).unwrap();
        assert_eq!(package.member_name(1), Some("b"));
        assert_eq!(package.member_name(2), None);
        assert_eq!(package.member(2), None);
    }

    /// The longest stem is exactly [`STEM_LEN`], so a package whose three fields all fill their
    /// field still fits the scratch the installer gives it.
    #[test]
    fn the_longest_stem_fills_its_scratch_exactly() {
        let long = "n".repeat(NAME_LEN);
        let attributes = Attributes {
            name: &long,
            version: &long,
            architecture: &long,
        };
        let mut file = vec![0u8; package_size(&[])];
        write_package(&attributes, &[], &mut file).unwrap();
        let mut stem = [0u8; STEM_LEN];
        let text = Package::parse(&file).unwrap().stem(&mut stem);
        assert_eq!(text.len(), STEM_LEN);
        assert_eq!(STEM_LEN, 98);
    }

    /// **A stem names one version**: an empty version and a version with a hyphen in it are not
    /// versions, and an empty name is not a package, whatever a catalog line says.
    #[test]
    fn a_stem_with_an_empty_or_hyphenated_version_or_an_empty_name_is_never_found() {
        let d = std::format!("sha256:{}", "0".repeat(64));
        let none = Err(CatalogMiss::NoSuchPackage);
        let empty_version = std::format!("foo--aarch64 {d}\n");
        assert_eq!(cataloged_stem(&empty_version, "foo", "aarch64"), none);
        let hyphenated = std::format!("foo-1.0-rc1-aarch64 {d}\n");
        assert_eq!(cataloged_stem(&hyphenated, "foo", "aarch64"), none);
        let nameless = std::format!("-1.0-aarch64 {d}\n");
        assert_eq!(cataloged_stem(&nameless, "", "aarch64"), none);
        assert_eq!(cataloged_stem(&nameless, "@1.0", "aarch64"), none);
    }

    #[test]
    fn wrong_magic_is_refused() {
        let mut file = written(&[("uptime", b"x")]);
        file[7] = b'2';
        assert_eq!(Package::parse(&file), Err(Error::BadMagic));
    }
}

#[cfg(kani)]
mod verification {
    use super::*;

    /// **A file the solver chose is either refused or entirely inside itself.**
    ///
    /// The property that matters for a parser on the install path: whatever bytes arrive, every
    /// accessor `parse` blesses stays in the buffer. Kani checks the slicing in `member` for out of
    /// bounds directly, so the harness's assertion is the arithmetic claim and the panic-freedom is
    /// the proof's own.
    ///
    /// Falsification: replayable `crates/package_archive/falsifications/verification.a_parsed_package_reads_only_inside_itself.patch`
    // **9, and the reason is `memcmp` rather than any loop in this file.** The magic comparison is
    // eight bytes, which the model checker unwinds as a loop; at 4 it reports an unwinding
    // assertion in `<builtin-library-memcmp>` and leaves 270 of 271 checks undetermined, which
    // looks like a failed proof and is a bound that is too small. The member loop is bounded by the
    // buffer: 272 bytes holds a table of at most two entries.
    #[kani::proof]
    #[kani::unwind(9)]
    fn a_parsed_package_reads_only_inside_itself() {
        const LEN: usize = HEADER_LEN + MEMBER_LEN * 2 + 16;
        let bytes: [u8; LEN] = kani::any();
        if let Ok(package) = Package::parse(&bytes) {
            assert!(package.len() <= MAX_MEMBERS);
            for index in 0..2 {
                if let Some(member) = package.member(index) {
                    assert!(member.len() <= LEN);
                }
            }
        }
    }

    /// **A file shorter than the header is refused rather than indexed.** `nifefs` proves the same
    /// boundary and for the same reason: the header read is the one place a short buffer would be
    /// indexed before anything had checked its length. `unwind(9)` lets the 8-byte magic compare
    /// finish and cuts no path on the correct tree, where the guard returns before any loop; with
    /// no bound the falsification took 20 to 31 minutes, unrolling the member loop.
    ///
    /// Falsification: replayable `crates/package_archive/falsifications/verification.a_short_file_is_refused.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn a_short_file_is_refused() {
        let bytes: [u8; HEADER_LEN - 1] = kani::any();
        assert_eq!(Package::parse(&bytes), Err(Error::BadMagic));
    }
}
