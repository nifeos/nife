//! Measured boot: the hash the kernel checks the progenitor against (milestone 22 phase B.1).
//! The kernel carries a trust root compiled into its image (`(name, digest)` pairs from
//! `kernel/build.rs`), hashes the archive entry it is about to load, and refuses to enter
//! userspace on a mismatch, so the unverified progenitor is not loaded on trust (DECISIONS §14).
//!
//! The hash is SHA-256, written here: collision- and preimage-resistant, about a hundred lines with
//! no dependency, allocation or unsafe (§46 (thin primitives or whole subsystems); calef,
//! 2026-10-22 UTC: "Keep our SHA-256."). The build and the kernel both hash through this crate, and
//! the tests are the published FIPS 180-4 vectors. See notes/trusted-init/measured-boot.md.
//!
//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `measure`. Refused `measure` (a verb,
//! and generic enough to name almost anything). "Measured boot" is the standard term for boot-time
//! hashing, so the new name also buys the recognition a reader arrives with.

#![no_std]

// The crate is `no_std` because the kernel links it. Its tests run on the host and want `Vec` and
// `String` to build tampered inputs, so the test build pulls `std` back in explicitly.
#[cfg(test)]
extern crate std;

/// A SHA-256 digest: 32 bytes.
pub const DIGEST_LEN: usize = 32;

/// The measurement of a program's bytes: the SHA-256 of all of them. The type does not carry its
/// algorithm (DECISIONS §197 (a package is one archive file)'s digest ruling, calef, 2026-10-07
/// UTC): the text form ([`digest_text`], [`parse_digest`]) carries the label, and a later Merkle
/// root gets its own type rather than a variant here. See notes/trusted-init/measured-boot.md.
pub type Digest = [u8; DIGEST_LEN];

/// The round constants of SHA-256 (FIPS 180-4 §4.2.2): the first 32 bits of the fractional parts of
/// the cube roots of the first 64 primes.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The initial hash value (FIPS 180-4 §5.3.3): the fractional parts of the square roots of the
/// first eight primes.
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// A streaming SHA-256, because the kernel hashes a mapped region it must not copy (there is no
/// heap to copy a 300 KiB ELF into).
#[derive(Clone)]
pub struct Sha256 {
    /// The eight working hash words.
    h: [u32; 8],
    /// Bytes not yet part of a full 64-byte block.
    block: [u8; 64],
    /// How many bytes of `block` are filled.
    filled: usize,
    /// Total message length in bytes; the padding encodes it in bits.
    len: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    /// The initial state: `H0` loaded, no bytes absorbed yet.
    pub const fn new() -> Self {
        Self {
            h: H0,
            block: [0; 64],
            filled: 0,
            len: 0,
        }
    }

    /// Absorb more of the message. Call as many times as you like; the result depends only on the
    /// concatenation, which is what lets the kernel hash a region page by page.
    pub fn update(&mut self, mut bytes: &[u8]) {
        self.len = self.len.wrapping_add(bytes.len() as u64);
        while !bytes.is_empty() {
            let want = 64 - self.filled;
            let take = if bytes.len() < want {
                bytes.len()
            } else {
                want
            };
            self.block[self.filled..self.filled + take].copy_from_slice(&bytes[..take]);
            self.filled += take;
            bytes = &bytes[take..];
            if self.filled == 64 {
                let block = self.block;
                self.compress(&block);
                self.filled = 0;
            }
        }
    }

    /// Pad, absorb the length, and produce the digest (FIPS 180-4 §5.1.1: a `0x80` byte, zeros, then
    /// the 64-bit big-endian bit length).
    pub fn finalize(mut self) -> Digest {
        let bits = self.len.wrapping_mul(8);
        self.update(&[0x80]);
        // `update` just bumped `len`, but from here we only care about `filled`.
        while self.filled != 56 {
            self.update(&[0]);
        }
        // The bit length goes in directly, not through `update`, so the recursion ends here.
        self.block[56..64].copy_from_slice(&bits.to_be_bytes());
        let block = self.block;
        self.compress(&block);

        let mut out = [0u8; DIGEST_LEN];
        for (i, word) in self.h.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    /// One 64-byte block through the compression function (FIPS 180-4 §6.2.2).
    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (dst, add) in self.h.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *dst = dst.wrapping_add(add);
        }
    }
}

/// The SHA-256 of one contiguous slice.
///
/// # Examples
///
/// ```
/// use measured_boot::sha256;
///
/// // The FIPS 180-4 vector for "abc", which is what pins this implementation.
/// let d = sha256(b"abc");
/// assert_eq!(
///     d[..4],
///     [0xba, 0x78, 0x16, 0xbf],
/// );
/// // The empty input is the other classic vector, and the one an off-by-one padding bug fails.
/// assert_eq!(sha256(b"")[..4], [0xe3, 0xb0, 0xc4, 0x42]);
/// ```
pub fn sha256(bytes: &[u8]) -> Digest {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize()
}

/// One entry of a trust root: "the program called `name` must hash to `digest`."
///
/// The kernel's copy is generated into its image by `kernel/build.rs` from the archive the build
/// just packed, so the check means "this kernel runs exactly this progenitor" with no key
/// management.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Measurement {
    /// The nifefs archive name this digest was measured for (`"progenitor"`, `"report_child_builder"`).
    pub name: &'static str,
    /// The expected SHA-256 of the program's bytes.
    pub digest: Digest,
}

/// Why a program was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// The trust root says nothing about a program by this name, so we cannot vouch for it. This
    /// is a refusal, not a pass: an empty or stale trust root must fail closed, or the whole check
    /// evaporates the first time someone builds the kernel without the build step that fills it in.
    Unmeasured,
    /// The bytes hashed to something other than what the trust root says they must.
    Mismatch,
}

/// The digest the trust root demands for `name`, if it names it at all.
pub fn expected(root: &[Measurement], name: &str) -> Option<Digest> {
    root.iter().find(|m| m.name == name).map(|m| m.digest)
}

/// Check an already-computed digest against the trust root.
///
/// Split out from [`verify`] so the kernel can hash a region in pieces. A plain `==`, because the
/// expected value is public and there is no secret for a timing side channel to leak.
pub fn verify_digest(root: &[Measurement], name: &str, digest: &Digest) -> Result<(), VerifyError> {
    match expected(root, name) {
        None => Err(VerifyError::Unmeasured),
        Some(want) if want == *digest => Ok(()),
        Some(_) => Err(VerifyError::Mismatch),
    }
}

/// Hash `bytes` and check them against the trust root's entry for `name`.
///
/// # Examples
///
/// Verification is a lookup by name and a comparison, and the two ways it can fail are distinct
/// on purpose: a component nobody measured is a different problem from one that changed.
///
/// ```
/// use measured_boot::{Measurement, sha256, verify, VerifyError};
///
/// let root = [Measurement { name: "progenitor", digest: sha256(b"the real progenitor") }];
///
/// assert!(verify(&root, "progenitor", b"the real progenitor").is_ok());
/// assert_eq!(verify(&root, "progenitor", b"a tampered progenitor"), Err(VerifyError::Mismatch));
///
/// // A name the trust root says nothing about is a REFUSAL, not a pass. An empty or stale root
/// // must fail closed, or the check evaporates the first time someone skips the build step that
/// // fills it in.
/// assert_eq!(verify(&root, "stowaway", b"anything"), Err(VerifyError::Unmeasured));
/// assert_eq!(verify(&[], "progenitor", b"the real progenitor"), Err(VerifyError::Unmeasured));
/// ```
pub fn verify(root: &[Measurement], name: &str, bytes: &[u8]) -> Result<(), VerifyError> {
    verify_digest(root, name, &sha256(bytes))
}

/// A digest as 64 lowercase hex characters (ASCII), with no label. No allocation, so the kernel can
/// print it in a diagnostic.
///
/// Not the text form of a digest that leaves a program; that is [`digest_text`]. Bare hex is for a
/// person comparing against `shasum -a 256`, and [`parse_digest`] refuses it.
pub fn hex(digest: &Digest) -> [u8; DIGEST_LEN * 2] {
    const HEXDIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; DIGEST_LEN * 2];
    for (i, byte) in digest.iter().enumerate() {
        out[i * 2] = HEXDIGITS[(byte >> 4) as usize];
        out[i * 2 + 1] = HEXDIGITS[(byte & 0xf) as usize];
    }
    out
}

/// The label a [`Digest`] carries in text: `sha256`, the container-image convention's name, then a
/// colon and the hex (DECISIONS §197's digest ruling, calef, 2026-10-07 UTC: "C"). It lets a second
/// kind of digest, such as a Merkle root, be added beside this one with its own label.
pub const SHA256_LABEL: &str = "sha256";

/// The length of a digest's text form: the label, a colon, 64 hex characters.
pub const DIGEST_TEXT_LEN: usize = SHA256_LABEL.len() + 1 + DIGEST_LEN * 2;

/// A digest as the text every file and program in this tree writes it in: `sha256:` and 64
/// lowercase hex characters. One formatter and one parser, [`parse_digest`], so catalogs, recipes
/// and the measurement table cannot drift apart (rule 7). No allocation.
///
/// ```
/// use measured_boot::{digest_text, parse_digest, sha256};
///
/// let d = sha256(b"abc");
/// let text = digest_text(&d);
/// let text = core::str::from_utf8(&text).unwrap();
/// assert_eq!(
///     text,
///     "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
/// );
/// assert_eq!(parse_digest(text), Ok(d));
/// ```
///
/// Name: provisional 2026-10-07 (UTC), lane/digest-label, with [`parse_digest`],
/// [`DigestTextError`], [`SHA256_LABEL`], [`DIGEST_TEXT_LEN`] and [`BadLine`]. Not ratified.
pub fn digest_text(digest: &Digest) -> [u8; DIGEST_TEXT_LEN] {
    let mut out = [0u8; DIGEST_TEXT_LEN];
    let label = SHA256_LABEL.len();
    out[..label].copy_from_slice(SHA256_LABEL.as_bytes());
    out[label] = b':';
    out[label + 1..].copy_from_slice(&hex(digest));
    out
}

/// Why a digest's text did not parse. Each one is a refusal, and they are kept apart so the
/// diagnostic can say which: a digest from before the label existed, one from a format this
/// reader does not know, and one that is simply damaged are three different pieces of news.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DigestTextError {
    /// There is no digest at all: an empty field, or a manifest line with no space in it.
    Missing,
    /// There is no label, as in bare hex. Refused rather than read as SHA-256, because a
    /// reader that guessed the algorithm would be the format change the label exists to avoid.
    Unlabeled,
    /// A label this reader does not know, such as a Merkle root's, or `SHA256` in capitals. A
    /// reader meeting a newer digest says so instead of comparing it as though it were the old one.
    UnknownLabel,
    /// `sha256:` followed by something other than exactly 64 hex characters.
    NotHex,
}

/// Parse a digest's text form, `sha256:<64 hex>`, the inverse of [`digest_text`]. Strict about
/// the label and its case; the hex may be either case, since a person may paste it from a tool
/// that prints capitals.
///
/// ```
/// use measured_boot::{parse_digest, DigestTextError};
///
/// let bare = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
/// assert_eq!(parse_digest(bare), Err(DigestTextError::Unlabeled));
/// let merkle = std::format!("merkle-sha256-8k:{bare}");
/// assert_eq!(parse_digest(&merkle), Err(DigestTextError::UnknownLabel));
/// assert_eq!(parse_digest("sha256:abc"), Err(DigestTextError::NotHex));
/// assert!(parse_digest(&std::format!("sha256:{bare}")).is_ok());
/// ```
pub fn parse_digest(text: &str) -> Result<Digest, DigestTextError> {
    parse_digest_bytes(text.as_bytes())
}

/// [`parse_digest`] over bytes, which is what the Kani harnesses hand it: a symbolic buffer need
/// not be UTF-8, and proving `from_utf8` along the way would be proving the standard library.
fn parse_digest_bytes(text: &[u8]) -> Result<Digest, DigestTextError> {
    if text.is_empty() {
        return Err(DigestTextError::Missing);
    }
    let Some(colon) = text.iter().position(|&b| b == b':') else {
        return Err(DigestTextError::Unlabeled);
    };
    if text[..colon] != *SHA256_LABEL.as_bytes() {
        return Err(DigestTextError::UnknownLabel);
    }
    parse_hex(&text[colon + 1..]).ok_or(DigestTextError::NotHex)
}

/// The archive entry that carries the progenitor's measurement table (milestone 104). The
/// progenitor is in the archive it would measure, so its table is an archive entry the kernel's
/// trust root names, vouched for as the progenitor is. See notes/trusted-init/measured-boot.md.
///
/// The entry name is provisional and fits under `nifefs`'s `NAME_LEN = 32`. A string two programs
/// agree on sits outside `script/names`' four surfaces, so this paragraph is its whole record; it
/// does not wear the `Name:` spelling, which milestone 283 reserved for the crate's one block.
pub const PROGRAM_MEASUREMENTS: &str = "program_measurements";

/// A manifest line [`manifest_entries`] could not read, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadLine<'a> {
    /// The line itself, trimmed, so a caller can name it.
    pub line: &'a str,
    /// What was wrong with its digest.
    pub why: DigestTextError,
}

/// The measurement manifest format, one entry per line: a name, one space, and the digest's text
/// form ([`digest_text`]); blank lines and `#` comments are skipped. Yields borrowed names and
/// never allocates. `Err` carries the line and the reason; a caller that skips errors still fails
/// closed ([`VerifyError::Unmeasured`]). See notes/trusted-init/measured-boot.md.
///
/// ```
/// use measured_boot::{manifest_entries, DigestTextError};
///
/// let text = "# a comment\n\
///             console sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n\
///             \n\
///             old e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n\
///             oops not-a-digest\n";
/// let mut it = manifest_entries(text);
/// assert_eq!(it.next().unwrap().unwrap().0, "console");
/// let old = it.next().unwrap().unwrap_err();
/// assert_eq!(old.why, DigestTextError::Unlabeled);
/// assert_eq!(it.next().unwrap().unwrap_err().line, "oops not-a-digest");
/// assert!(it.next().is_none());
/// ```
pub fn manifest_entries(text: &str) -> impl Iterator<Item = Result<(&str, Digest), BadLine<'_>>> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let bad = |why| BadLine { line, why };
            let (name, digest) = line.split_once(' ').ok_or(bad(DigestTextError::Missing))?;
            let digest = parse_digest(digest.trim()).map_err(bad)?;
            Ok((name, digest))
        })
}

/// The digest a manifest demands for `name`, if it names it at all. The runtime counterpart of
/// [`expected`]: same question, asked of text read at boot rather than of a compiled-in table.
pub fn expected_in_manifest(text: &str, name: &str) -> Option<Digest> {
    manifest_entries(text)
        .flatten()
        .find(|(n, _)| *n == name)
        .map(|(_, d)| d)
}

/// Hash `bytes` and check them against a manifest's entry for `name`.
///
/// The lookup happens before the hash, so a name the table does not mention is a refusal that
/// costs nothing to reach. The verdicts are [`verify`]'s: a name the manifest says nothing about is
/// [`VerifyError::Unmeasured`], so a table that failed to generate vouches for nothing.
pub fn verify_in_manifest(text: &str, name: &str, bytes: &[u8]) -> Result<(), VerifyError> {
    match expected_in_manifest(text, name) {
        None => Err(VerifyError::Unmeasured),
        Some(want) if want == sha256(bytes) => Ok(()),
        Some(_) => Err(VerifyError::Mismatch),
    }
}

/// What a loader found when it asked an archive for a program, after the table had its say.
///
/// Nothing runs that the table will not vouch for, so `elf` is `None` whenever the entry is
/// missing, refused or not an ELF. `unvouched` tells the absence (a build left it out) from the
/// refusal (a security event the boot reports). `Default` is the absence, derived on purpose
/// (milestone 246): it is fail-safe, and it lets `cargo mutants` test [`verdict`]. See
/// notes/trusted-init/measured-boot.md.
#[derive(Default)]
pub struct Verdict<'a> {
    /// The parsed image, if and only if the table vouched for the bytes and they are an ELF.
    pub elf: Option<elf::Elf<'a>>,
    /// The archive had this entry and the table would not vouch for it.
    pub unvouched: bool,
}

/// Decide whether a program may be loaded: measure `bytes` against `table` under `name`, and hand
/// back the parsed image or a refusal. `bytes` is `None` when the archive lacks the entry (the
/// caller's `fs.read(name)`), which is not a refusal. `Unmeasured` and `Mismatch` both refuse. A
/// vouched program that is not an ELF reports `unvouched: false` with no image: a build defect.
///
/// # Examples
///
/// ```
/// use measured_boot::{sha256, digest_text, verdict};
///
/// let swish = b"the real swish";
/// let d = digest_text(&sha256(swish));
/// let table = std::format!("swish {}\n", std::str::from_utf8(&d).unwrap());
///
/// // Substituted bytes: the archive had it, and the table would not vouch for it.
/// let v = verdict(&table, "swish", Some(b"a tampered swish"));
/// assert!(v.unvouched);
/// assert!(v.elf.is_none());
///
/// // A name the table never mentions is the same refusal, not a pass.
/// let v = verdict(&table, "stowaway", Some(&b"anything"[..]));
/// assert!(v.unvouched);
///
/// // Absent from the archive is a DIFFERENT answer: nothing was substituted.
/// let v = verdict(&table, "swish", None);
/// assert!(!v.unvouched);
/// assert!(v.elf.is_none());
/// ```
///
/// Names **provisional** (milestone 246 (measured boot's refusal path is tested by nothing)'s
/// lane): `verdict` and [`Verdict`] sit beside [`verify`] and [`verify_in_manifest`]; public names
/// are an architect's (milestone 160 (review the public function names)).
pub fn verdict<'a>(table: &str, name: &str, bytes: Option<&'a [u8]>) -> Verdict<'a> {
    let Some(bytes) = bytes else {
        return Verdict {
            elf: None,
            unvouched: false,
        };
    };
    if verify_in_manifest(table, name, bytes).is_err() {
        return Verdict {
            elf: None,
            unvouched: true,
        };
    }
    Verdict {
        elf: elf::Elf::parse(bytes).ok(),
        unvouched: false,
    }
}

/// Parse 64 hex characters back into a digest: the part of [`parse_digest`] after the label.
/// Private, so nothing outside this crate can read a digest without its label; strict, because a
/// manifest we cannot parse must not silently become an empty trust root (see
/// [`VerifyError::Unmeasured`]).
fn parse_hex(bytes: &[u8]) -> Option<Digest> {
    if bytes.len() != DIGEST_LEN * 2 {
        return None;
    }
    let nibble = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    let mut out = [0u8; DIGEST_LEN];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = (nibble(bytes[i * 2])? << 4) | nibble(bytes[i * 2 + 1])?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use std::string::ToString;
    use std::vec;
    use std::vec::Vec;

    use super::*;

    /// The published FIPS 180-4 / NIST CAVS vectors, **not** self-consistency checks. Both sides of
    /// the boot check hash through this one implementation, so an implementation that agreed only
    /// with itself would still boot happily while measuring nothing real. These three (and the
    /// multi-`update` case below) are what make the digest a SHA-256 rather than a private function.
    #[test]
    fn matches_the_published_sha256_vectors() {
        let hexed = |d: &Digest| {
            let h = hex(d);
            core::str::from_utf8(&h).unwrap().to_string()
        };
        assert_eq!(
            hexed(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hexed(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // 448 bits: the vector that exercises padding into a second block.
        assert_eq!(
            hexed(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // A million 'a's, streamed: the classic long-message vector, which is also the case that
        // catches a length counter that overflows or is fed in bits instead of bytes.
        let mut h = Sha256::new();
        for _ in 0..1000 {
            h.update(&[b'a'; 1000]);
        }
        assert_eq!(
            hexed(&h.finalize()),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    /// Streaming must equal one-shot for any split, because the kernel hashes a mapped region in
    /// whatever chunks it has and the build hashes one contiguous buffer. If those disagreed the
    /// boot would fail on correct bytes, which is the worst kind of fail-closed.
    #[test]
    fn streaming_in_any_chunking_equals_one_shot() {
        let data: Vec<u8> = (0..300u32).map(|i| (i * 7) as u8).collect();
        let want = sha256(&data);
        for chunk in [1usize, 3, 55, 64, 65, 128, 299, 300] {
            let mut h = Sha256::new();
            for piece in data.chunks(chunk) {
                h.update(piece);
            }
            assert_eq!(h.finalize(), want, "chunking by {chunk} changed the digest");
        }
    }

    /// The three verdicts the boot path can get, including the one that matters most: a trust root
    /// that does not mention the program is a **refusal**. A kernel built without the measurement
    /// step must not boot an unmeasured progenitor.
    #[test]
    fn verification_accepts_the_measured_bytes_and_refuses_everything_else() {
        let root = [Measurement {
            name: "progenitor",
            digest: sha256(b"the real progenitor"),
        }];
        assert_eq!(verify(&root, "progenitor", b"the real progenitor"), Ok(()));
        assert_eq!(
            verify(&root, "progenitor", b"the real progenitor!"),
            Err(VerifyError::Mismatch)
        );
        assert_eq!(
            verify(&root, "hello", b"the real progenitor"),
            Err(VerifyError::Unmeasured)
        );
        assert_eq!(
            verify(&[], "progenitor", b"the real progenitor"),
            Err(VerifyError::Unmeasured)
        );
    }

    /// A single flipped bit anywhere must change the digest. This is the property the whole
    /// milestone rests on: an attacker who rewrites one byte of the progenitor's code cannot keep the
    /// measurement.
    #[test]
    fn one_flipped_bit_changes_the_digest() {
        let mut data = vec![0u8; 200];
        for (i, b) in data.iter_mut().enumerate() {
            *b = i as u8;
        }
        let base = sha256(&data);
        for i in [0usize, 1, 63, 64, 100, 199] {
            let mut tampered = data.clone();
            tampered[i] ^= 1;
            assert_ne!(sha256(&tampered), base, "flipping byte {i} kept the digest");
        }
    }

    /// Text out, text in: the manifest the build writes must read back as the digest the kernel
    /// checks.
    #[test]
    fn digest_text_round_trips_and_rejects_malformed_text() {
        let d = sha256(b"round trip");
        let text = digest_text(&d);
        let text = core::str::from_utf8(&text).unwrap();
        assert!(text.starts_with("sha256:"));
        assert_eq!(parse_digest(text), Ok(d));
        assert_eq!(
            parse_digest("sha256:abc"),
            Err(DigestTextError::NotHex),
            "a short string is not a digest"
        );
        assert_eq!(
            parse_digest(&std::format!("sha256:{}", "g".repeat(64))),
            Err(DigestTextError::NotHex),
            "non-hex characters must not parse as zeros"
        );
        assert_eq!(parse_digest(""), Err(DigestTextError::Missing));
    }

    /// **The ruling's whole point, as a test** (DECISIONS §197, 2026-10-07): a digest written before
    /// the label existed, and one written by a format that comes after it, are both refused by
    /// name rather than compared. Bare hex read as SHA-256 would be a guess; a Merkle root compared
    /// as a whole-file hash would be a guaranteed mismatch reported as tampering.
    #[test]
    fn an_unlabeled_or_unknown_digest_is_refused_by_name() {
        let hexed = hex(&sha256(b"x"));
        let hexed = core::str::from_utf8(&hexed).unwrap();
        assert_eq!(parse_digest(hexed), Err(DigestTextError::Unlabeled));
        for label in ["merkle-sha256-8k", "SHA256", "sha512", "sha256 ", ""] {
            assert_eq!(
                parse_digest(&std::format!("{label}:{hexed}")),
                Err(DigestTextError::UnknownLabel),
                "{label:?} is not the label this reader knows"
            );
        }
        // The colon is the separator, so a second one belongs to the value and fails as hex.
        assert_eq!(
            parse_digest(&std::format!("sha256::{hexed}")),
            Err(DigestTextError::NotHex)
        );
    }

    /// The manifest is the second link of the chain (milestone 104): the kernel vouches for this
    /// text, and the progenitor decides what to load from it. So the same three verdicts have to hold when
    /// the trust root is *text read at boot* rather than a table compiled into an image, and the
    /// `Unmeasured` one has to hold hardest: a table that failed to generate, or one whose line for
    /// a program is malformed, must refuse that program rather than wave it through.
    #[test]
    fn a_manifest_gives_the_same_three_verdicts_a_compiled_in_trust_root_does() {
        let console = b"the real console".as_slice();
        let hexed = digest_text(&sha256(console));
        let hexed = core::str::from_utf8(&hexed).unwrap();
        let bare = &hexed[SHA256_LABEL.len() + 1..];
        let text = std::format!(
            "# generated by cargo xtask\nconsole {hexed}\nbroken deadbeef\nunlabeled {bare}\n"
        );

        assert_eq!(verify_in_manifest(&text, "console", console), Ok(()));
        assert_eq!(
            verify_in_manifest(&text, "console", b"a tampered console"),
            Err(VerifyError::Mismatch)
        );
        assert_eq!(
            verify_in_manifest(&text, "swish", console),
            Err(VerifyError::Unmeasured),
            "a name the table does not mention is a refusal"
        );
        assert_eq!(
            verify_in_manifest(&text, "broken", b"deadbeef"),
            Err(VerifyError::Unmeasured),
            "a malformed line must refuse its program, not vouch for it"
        );
        assert_eq!(
            verify_in_manifest(&text, "unlabeled", console),
            Err(VerifyError::Unmeasured),
            "a digest from before the label existed vouches for nothing"
        );
        assert_eq!(
            verify_in_manifest("", "console", console),
            Err(VerifyError::Unmeasured),
            "an empty table vouches for nothing"
        );
    }

    /// A minimal, genuinely parseable ELF64 for this host's architecture.
    ///
    /// Hand-rolled rather than borrowed from `elf`'s own test builder, which is private to that
    /// crate. One `PT_LOAD` segment, readable and executable, with the entry point inside it, which
    /// is the least `elf::Elf::parse` accepts. `elf::NATIVE_MACHINE` exists for exactly this: a
    /// forged header has to write *some* machine number and the native one is what gets past the
    /// machine check.
    fn tiny_elf() -> Vec<u8> {
        const EHDR: usize = 64;
        const PHDR: usize = 56;
        let entry: u64 = 0x40_0000;
        let code = [0xaau8; 16];

        let mut out = vec![0u8; EHDR + PHDR];
        out[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
        out[4] = 2; // ELFCLASS64
        out[5] = 1; // ELFDATA2LSB
        out[6] = 1; // EV_CURRENT
        out[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
        out[18..20].copy_from_slice(&elf::NATIVE_MACHINE.to_le_bytes());
        out[24..32].copy_from_slice(&entry.to_le_bytes());
        out[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes()); // e_phoff
        out[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes());
        out[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum

        let ph = &mut out[EHDR..EHDR + PHDR];
        ph[0..4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
        ph[4..8].copy_from_slice(&(elf::PF_R | elf::PF_X).to_le_bytes());
        ph[8..16].copy_from_slice(&((EHDR + PHDR) as u64).to_le_bytes()); // p_offset
        ph[16..24].copy_from_slice(&entry.to_le_bytes()); // p_vaddr
        ph[24..32].copy_from_slice(&entry.to_le_bytes()); // p_paddr
        ph[32..40].copy_from_slice(&(code.len() as u64).to_le_bytes()); // p_filesz
        ph[40..48].copy_from_slice(&(code.len() as u64).to_le_bytes()); // p_memsz

        out.extend_from_slice(&code);
        out
    }

    /// A one-line table vouching for `name`'s real bytes.
    fn table_for(name: &str, bytes: &[u8]) -> std::string::String {
        let d = digest_text(&sha256(bytes));
        std::format!("{name} {}\n", core::str::from_utf8(&d).unwrap())
    }

    /// **The test milestone 246 exists for.** Nothing in this tree took the refusing branch of the
    /// load-or-refuse decision until this ran: the boot gate proves it by booting a system whose
    /// table happens to be right, which exercises the accept path and never this one.
    ///
    /// Falsified by hand before it was committed: changing `unvouched: true` to `false` in
    /// [`verdict`]'s refusal arm turns this red, and turns nothing else in the tree red.
    #[test]
    fn substituted_bytes_are_refused_and_reported_as_unvouched() {
        let real = tiny_elf();
        let table = table_for("swish", &real);

        let mut tampered = real.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;

        let v = verdict(&table, "swish", Some(&tampered));
        assert!(
            v.unvouched,
            "a substituted program must be reported unvouched"
        );
        assert!(
            v.elf.is_none(),
            "a refused program must not come back as a loadable image"
        );

        // The other refusal, and it is the one an empty or stale table produces for everything: the
        // table says nothing about this name. `Unmeasured` and `Mismatch` are one answer here.
        let v = verdict(&table, "stowaway", Some(&real));
        assert!(v.unvouched);
        assert!(v.elf.is_none());

        let v = verdict("", "swish", Some(&real));
        assert!(v.unvouched, "an empty table vouches for nothing");
    }

    /// Absent and refused are different answers, and the difference is the whole reason [`Verdict`]
    /// has two fields instead of being an `Option`. A build that did not pack a program is not a
    /// security event; a program whose bytes were substituted is.
    #[test]
    fn an_absent_program_is_not_an_unvouched_one() {
        let real = tiny_elf();
        let table = table_for("swish", &real);

        let v = verdict(&table, "swish", None);
        assert!(
            !v.unvouched,
            "an entry the archive does not have was not substituted, and must not be announced as if it were"
        );
        assert!(v.elf.is_none());

        // And a name the table would refuse is *still* not unvouched when the archive lacks it:
        // there are no bytes to measure, so the table is never consulted.
        let v = verdict("", "absent", None);
        assert!(!v.unvouched);
    }

    /// The accept path, and the third `elf: None` case, which is neither of the other two. Bytes the
    /// table vouches for that are not an ELF report `unvouched: false`: the measurement did its job
    /// and the packaging did not, and calling that a refusal would put a security word on a build
    /// defect.
    #[test]
    fn a_vouched_program_parses_and_vouched_junk_is_not_called_unvouched() {
        let real = tiny_elf();
        let table = table_for("swish", &real);

        let v = verdict(&table, "swish", Some(&real));
        assert!(!v.unvouched);
        assert!(v.elf.is_some(), "vouched, well-formed bytes must load");

        let junk = b"not an elf at all";
        let table = table_for("swish", junk);
        let v = verdict(&table, "swish", Some(junk));
        assert!(
            !v.unvouched,
            "vouched-for junk is a build defect, not a substitution"
        );
        assert!(v.elf.is_none());
    }

    /// The format has exactly one definition, and `kernel/build.rs` reads it through this iterator
    /// too. A `#` comment, a blank line and CRLF-ish trailing space all appear in files people edit
    /// or diff, so they are skipped rather than reported; anything else that does not parse is
    /// reported with the line, because the build turns that into a hard error.
    #[test]
    fn manifest_parsing_skips_comments_and_reports_the_line_it_could_not_read() {
        let d = digest_text(&sha256(b"x"));
        let d = core::str::from_utf8(&d).unwrap();
        let text = std::format!("# header\n\n  wc {d}  \nno-space-here\nshort ab\n");
        let got: Vec<Result<(&str, Digest), BadLine>> = manifest_entries(&text).collect();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].unwrap().0, "wc");
        let missing = BadLine {
            line: "no-space-here",
            why: DigestTextError::Missing,
        };
        assert_eq!(got[1].unwrap_err(), missing);
        let short = BadLine {
            line: "short ab",
            why: DigestTextError::Unlabeled,
        };
        assert_eq!(got[2].unwrap_err(), short);
        assert_eq!(expected_in_manifest(&text, "wc"), Some(sha256(b"x")));
    }

    /// Uppercase hex is hex. Every other test round-trips through our own lowercase `to_hex`, so
    /// the `b'A'..=b'F'` arm (and its `+ 10` arithmetic) was dead in the suite (milestone 85), and
    /// a digest pasted from a tool that prints uppercase would have been refused.
    #[test]
    fn uppercase_hex_parses_to_the_same_digest() {
        let lower = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let upper = std::format!("sha256:{}", lower[7..].to_uppercase());
        assert_eq!(parse_digest(&upper), parse_digest(lower));
        assert_eq!(parse_digest(&upper).unwrap()[0], 0xBA);
    }
}

#[cfg(kani)]
mod verification {
    use super::*;

    /// **Every digest reads back as itself.** The formatter and the parser are the one definition
    /// of the text that a catalog, a recipe, an activation row and the measurement table share,
    /// so a digest that wrote out and read back as something else would make a vouched package
    /// unrunnable, or a different one runnable. Proved for all 2^256 digests rather than the few
    /// the tests hash.
    ///
    /// Falsification: replayable `crates/measured_boot/falsifications/verification.every_digest_reads_back_as_itself.patch`
    #[kani::proof]
    #[kani::unwind(72)]
    fn every_digest_reads_back_as_itself() {
        let digest: Digest = kani::any();
        assert_eq!(parse_digest_bytes(&digest_text(&digest)), Ok(digest));
    }

    /// **Only `sha256:` and 64 hex characters parse, whatever the bytes and however many.** The
    /// property the ruling needs from the parser: an unlabeled digest and a foreign label are
    /// refused, never read as a SHA-256, and no input panics. Up to one byte longer than the text
    /// form, so the too-long case is in the space too.
    ///
    /// Falsification: replayable `crates/measured_boot/falsifications/verification.only_a_labeled_sha256_parses.patch`
    #[kani::proof]
    #[kani::unwind(73)]
    fn only_a_labeled_sha256_parses() {
        let bytes: [u8; DIGEST_TEXT_LEN + 1] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= bytes.len());
        let text = &bytes[..len];
        if let Ok(digest) = parse_digest_bytes(text) {
            assert_eq!(len, DIGEST_TEXT_LEN);
            assert_eq!(&text[..SHA256_LABEL.len() + 1], b"sha256:");
            // What parsed is what the text says, up to the case of the hex.
            assert!(digest_text(&digest).eq_ignore_ascii_case(text));
        }
    }
}
