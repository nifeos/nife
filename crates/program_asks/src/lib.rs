//! **`program_asks`**: what a packaged program's manifest note asks the owner for, as the grant
//! record's vocabulary ([`installed_programs::Granted`]): the network, then the service ask of
//! note version 3, then each resolver zone. Two readers check these asks against one approval
//! file, so the reading is a crate and not a copy (AGENTS.md rule 7): the progenitor's install
//! (`system_initializer`) and `cargo xtask base-image`'s composition (milestone 909 (basalt
//! composes the bootable image from its packages) item 3) both call this one.
//!
//! [`asked_grants_of`] yields nothing for a program whose bytes or note cannot be read, which runs
//! with none of them either: the same rule `endowed_image` applies at spawn. The image composer
//! must not take that for "asks for nothing", so it calls [`checked_asked_grants_of`], which
//! refuses an unreadable note and reads the note of a program built for any machine. The asks are exactly what `jig install` lists
//! at its confirmation, and a grant record never holds less.
//!
//! # EXAMPLES
//!
//! ```
//! use program_asks::asked_grants_of;
//!
//! // A program whose bytes carry no readable manifest note asks for nothing, which is the rule
//! // the spawn path applies to the same bytes.
//! let program = package_archive::Program {
//!     name: "noteless",
//!     bytes: b"not an elf",
//!     digest: [0u8; 32],
//!     member: 0,
//! };
//! assert_eq!(asked_grants_of(&program).count(), 0);
//! ```
//!
//! The asking side of the same function, a note that asks for the network and one zone, is the
//! tests' first case; its bytes are an ELF with one `PT_NOTE`, which is too many bytes to type
//! into a comment.
//!
//! # BUGS
//!
//! - `jig` still spells the asks it lists at the confirmation from its own read of the note, into
//!   its own struct, because its list carries each program's name beside the asks and its words
//!   are the confirmation's. A third reader of the note is one already; lifting `jig`'s onto this
//!   one is follow-on work, not assumed here.
//!
//! Name: provisional, minted 2026-10-29 (UTC) by lane
//! milestone/909-item-3-composition-checks-the-approval. calef names crates. Considered
//! `installed_programs` (the table's grammar would carry the ELF parser under it, and every
//! consumer of the table would build one to ask a package a question), `approval_file` (the
//! owner's file's grammar is not a note's), and `capability_manifest_note` (the note's grammar
//! would carry the record's vocabulary, and every program build would link both).

#![no_std]

use installed_programs::{Grant, Granted};

/// **Why a program's note could not be read**, as opposed to the program having none.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane
/// milestone/909-composer-reads-any-machines-note. calef names public items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnreadableNote {
    /// The bytes are not a little-endian ELF64 file, or a `PT_NOTE` in them is malformed.
    Elf(elf::NoteError),
    /// The program's manifest note is there, and its descriptor does not decode.
    Manifest(capability_manifest_note::Error),
    /// A zone the manifest asks for is not text.
    ZoneNotText,
}

/// **What one packaged program's note asks for, or why it cannot be read.** The asks are those of
/// [`asked_grants_of`], in its order. A program with no manifest note at all is `Ok` and asks for
/// nothing, which is the rule the spawn path applies; a program whose bytes or note are malformed
/// is an `Err`, so a caller that must not guess (the image composer) refuses and names it.
///
/// The note is found through the program headers alone ([`elf::NoteSegments`] and
/// [`elf::NoteSearch`]), which check no machine: a program built for another machine than the
/// reader's has the same note to read. Nothing here runs the program, which is why
/// [`elf::Elf::parse`]'s machine check does not belong to it.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane
/// milestone/909-composer-reads-any-machines-note. calef names public items.
pub fn checked_asked_grants_of<'b>(
    p: &package_archive::Program<'b>,
) -> Result<impl Iterator<Item = Granted<'b>> + use<'b>, UnreadableNote> {
    let bytes: &'b [u8] = p.bytes;
    let mut search = elf::NoteSearch::new(
        capability_manifest_note::OWNER,
        capability_manifest_note::MANIFEST,
    );
    let mut note = None;
    for seg in elf::NoteSegments::from_head(bytes).map_err(UnreadableNote::Elf)? {
        let range = seg.range(bytes.len()).map_err(UnreadableNote::Elf)?;
        if let Some(d) = search
            .segment(&bytes[range], seg.p_align)
            .map_err(UnreadableNote::Elf)?
        {
            note = Some(d);
        }
    }
    let decoded = match note {
        None => None,
        Some(d) => Some((
            capability_manifest_note::decode(d).map_err(UnreadableNote::Manifest)?,
            capability_manifest_note::zones(d).map_err(UnreadableNote::Manifest)?,
            capability_manifest_note::service(d).map_err(UnreadableNote::Manifest)?,
        )),
    };
    if let Some((_, zones, _)) = decoded
        && zones.iter().any(|z| core::str::from_utf8(z).is_err())
    {
        return Err(UnreadableNote::ZoneNotText);
    }
    let network = decoded
        .is_some_and(|(m, _, _)| m.network)
        .then_some(Granted::Network);
    let service = decoded
        .is_some_and(|(_, _, s)| s)
        .then_some(Granted::Service);
    let zones = decoded.into_iter().flat_map(move |(_, zones, _)| {
        zones
            .iter()
            .filter_map(|z| Some(Granted::Zone(core::str::from_utf8(z).ok()?)))
    });
    Ok(network.into_iter().chain(service).chain(zones))
}

/// **What one packaged program's note asks for**, as the grants the owner's yes covers: the
/// network, if it asks, then the service ask, then each resolver zone, keyed on nothing. The
/// order is the record's (directories first, then the installer, the network, the service, the
/// zones), which is the order a check and a confirmation walk.
///
/// Nothing for a program whose note cannot be read ([`checked_asked_grants_of`] says why); a
/// caller that must tell that from "asks for nothing" calls that.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane
/// milestone/909-item-3-composition-checks-the-approval. calef names public items.
pub fn asked_grants_of<'b>(
    p: &package_archive::Program<'b>,
) -> impl Iterator<Item = Granted<'b>> + use<'b> {
    checked_asked_grants_of(p).ok().into_iter().flatten()
}

/// [`asked_grants_of`], keyed on the program's digest: what an install writes into the grant
/// record, one `Grant` per ask, in the same order.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane
/// milestone/909-item-3-composition-checks-the-approval. calef names public items.
pub fn asked_grants<'b>(
    p: &package_archive::Program<'b>,
) -> impl Iterator<Item = Grant<'b>> + use<'b> {
    let digest = p.digest;
    asked_grants_of(p).map(move |what| Grant { digest, what })
}

#[cfg(test)]
mod tests;
