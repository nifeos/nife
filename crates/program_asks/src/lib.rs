//! **`program_asks`**: what a packaged program's manifest note asks the owner for, as the grant
//! record's vocabulary ([`installed_programs::Granted`]): the network, then the service ask of
//! note version 3, then each resolver zone. Two readers check these asks against one approval
//! file, so the reading is a crate and not a copy (AGENTS.md rule 7): the progenitor's install
//! (`system_initializer`) and `cargo xtask base-image`'s composition (milestone 909 (basalt
//! composes the bootable image from its packages) item 3) both call this one.
//!
//! Nothing for a program whose bytes or note cannot be read, which runs with none of them either:
//! the same rule `endowed_image` applies at spawn. The asks are exactly what `jig install` lists
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

/// **What one packaged program's note asks for**, as the grants the owner's yes covers: the
/// network, if it asks, then the service ask, then each resolver zone, keyed on nothing. The
/// order is the record's (directories first, then the installer, the network, the service, the
/// zones), which is the order a check and a confirmation walk.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane
/// milestone/909-item-3-composition-checks-the-approval. calef names public items.
pub fn asked_grants_of<'b>(
    p: &package_archive::Program<'b>,
) -> impl Iterator<Item = Granted<'b>> + use<'b> {
    let note = elf::Elf::parse(p.bytes).ok().and_then(|elf| {
        elf.note(
            capability_manifest_note::OWNER,
            capability_manifest_note::MANIFEST,
        )
        .ok()
        .flatten()
    });
    let decoded = note.and_then(|d| {
        Some((
            capability_manifest_note::decode(d).ok()?,
            capability_manifest_note::zones(d).ok()?,
            capability_manifest_note::service(d).ok()?,
        ))
    });
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
    network.into_iter().chain(service).chain(zones)
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
