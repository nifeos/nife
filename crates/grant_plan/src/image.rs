//! **What a file's bytes are bound and endowed with** (milestone 597 (a program carries its
//! manifest in an ELF note) and DECISIONS §219 (how the shell names an installed program to the
//! spawner) option D): the manifest a note declares, the vouch applied to it, and what an image
//! request can carry. The shell calls these to preview and the progenitor to decide, so the two
//! cannot differ. Moved out of the crate's root unchanged by milestone 809 (the package client
//! becomes a program), when the root outgrew §266 (a Rust source file stays under 2,000 lines)'s
//! ratchet; the root re-exports every item at its old path.

use crate::{
    ArgSpec, DirSpec, FileSpec, IndexStore, InputSpec, Manifest, MemSpec, OutputSpec, Program,
    Runtime, UNVOUCHED_MANIFEST, UNVOUCHED_STD_MANIFEST,
};

/// **What a file's bytes are bound and endowed with when they carry no manifest note** (milestone
/// 597, provisional: a program carries its manifest in an ELF note).
///
/// `uptime`'s manifest: output bytes to the caller and nothing else (no clock, no domain, no
/// config, no entropy, no network, no argument, no `--mem`). It was the ceiling every installed
/// program was held to (`INSTALLED_MANIFEST_OF`, #1320) until DECISIONS §197 (a package is one
/// archive file) was answered with M2, and it stays as the answer for a program that says
/// nothing: the least a program can be run with and still be heard from. A program that needs
/// more carries a note ([`image_manifest`]).
///
/// Name: provisional (milestone 597, 2026-09-26).
pub const NO_NOTE_MANIFEST: Manifest = Program::Uptime.manifest();

/// **The row an image's [`Endowment`](crate::Endowment) is filed under**, because an endowment names a `Program` and
/// a file's bytes have none.
///
/// An exception, and a foot gun: nothing about an image may be decided from this row. Every
/// decision is made from the manifest [`image_manifest`] returns, which is what the shell binds
/// the line against and what the progenitor endows from. A reader who calls
/// `e.prog.manifest()` on an image's endowment gets `uptime`'s manifest, which is the wrong one
/// whenever the program carries a note. The fix is an endowment that holds its manifest rather
/// than its row; that is wider than this milestone, and `spawnproto`'s BUGS records it.
///
/// Name: provisional (milestone 597, 2026-09-26).
pub const IMAGE_ROW: Program = Program::Uptime;

/// **Why a file's bytes are not run with the manifest they carry.** Provisional names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageRefusal {
    /// The manifest note is there and cannot be read: malformed, a version this system does not
    /// know, or the same note twice.
    Unreadable,
    /// The manifest declares something the image request cannot deliver yet
    /// ([`image_can_carry`]).
    NotCarried,
    /// Nobody vouched for the bytes, and their manifest asks for something the command line would
    /// have to designate: an argument, memory, a file, a directory, an input, an option. §219 lets
    /// unvouched bytes hold only what [`UNVOUCHED_MANIFEST`] names, so the line that bound those is
    /// refused rather than run with the designation silently dropped.
    ExceedsVouch,
}

/// **Whether an image request can deliver what `m` declares**, today.
///
/// An image travels as a plain line (DECISIONS §219 option D's first cut): the request words carry
/// an argument and a `--mem` count, and the progenitor endows the pages and endpoints a manifest
/// names on its own. Nothing else is on that wire yet. So a manifest that needs a file, a
/// directory, an input, an option, a supervised job, a declared second stream or a silent output
/// is refused at the prompt and again at the progenitor, rather than run without what it said it
/// needs. `spawnproto`'s BUGS carries each.
///
/// **A `std` image is carried when it hears words, and only then** (milestone 205 (how a foreign
/// program is told what to do)): `Runtime::Std` and [`ArgSpec::Words`] go together or not at all.
/// The progenitor sizes an image's region before its frames arrive, so before it can read the note,
/// and the one thing the request tells it is whether an argv follows (`spawnproto::ARGS_BIT`). Tying
/// the two makes that bit the region's size: [`image_hears_words`]. A `std` image also declares no
/// memory grant, domain or network, which is what the `std` layout can hold.
pub fn image_can_carry(m: &Manifest) -> bool {
    let std = m.runtime == Runtime::Std;
    matches!(m.output, OutputSpec::Bytes | OutputSpec::Words)
        && m.file == FileSpec::Forbidden
        && m.dir == DirSpec::Forbidden
        && m.input == InputSpec::Forbidden
        && m.flags.letters().is_empty()
        && !m.interruptible
        // Milestone 805: the reboot object is endowed to the one program in this table that
        // declares it, never to installed bytes, whatever their note asks.
        && !m.reboot
        && !m.sync
        // Milestone 809: only the owner's console grants the installer endpoint, and only to a
        // program this table names; installed bytes asking for it, or for the catalog that goes
        // with it, are refused.
        && !m.installer
        && !m.catalog
        // Milestone 801 (packages over the internet): the index store is the owner's console's to
        // grant, as the installer is.
        && m.indexes == IndexStore::Forbidden
        && std == (m.arg.hears_words())
        && (!std
            || (m.output == OutputSpec::Bytes
                && m.mem == MemSpec::Forbidden
                && !m.domain
                && !m.network))
}

/// **Whether an image is sent its line as an argv**, which for an image is also whether it is built
/// in the `std` layout ([`image_can_carry`] ties the two). The shell asks it of the note it read and
/// sets `spawnproto::ARGS_BIT`; the progenitor sizes the region from that bit and then checks its
/// own reading of the note agrees ([`image_request_fits`]). Name: provisional.
pub const fn image_hears_words(m: &Manifest) -> bool {
    m.arg.hears_words()
}

/// **The manifest a file's bytes are bound and endowed with** (milestone 597, provisional), given
/// what their note declared (`None` for no note) and whether the installed-programs table vouches for them.
///
/// Vouched bytes get what they declare, because the digest that vouched covers the note: a person
/// who installed the package installed its manifest. Unvouched bytes get [`UNVOUCHED_MANIFEST`]
/// whatever they declare (§219: a note from bytes nobody vouched for grants nothing), and are
/// refused if the declaration asks for anything a command line would have designated, since the
/// shell has already bound the line against it.
///
/// The shell calls this to preview (`caps`) and the progenitor to decide, so the two cannot differ.
pub fn image_manifest(declared: Option<Manifest>, vouched: bool) -> Result<Manifest, ImageRefusal> {
    let declared = declared.unwrap_or(NO_NOTE_MANIFEST);
    if !image_can_carry(&declared) {
        return Err(ImageRefusal::NotCarried);
    }
    if vouched {
        return Ok(declared);
    }
    // **An unvouched `std` program still hears its words** (§170 (how a foreign program is told
    // what to do)): the argv carries no authority, and `std` is how the bytes were built rather than
    // anything they are granted. So it gets the unvouched grants in the `std` layout, and nothing
    // more; its note's entropy or clock requests grant nothing, as §219 says.
    if declared.runtime == Runtime::Std {
        return Ok(UNVOUCHED_STD_MANIFEST);
    }
    let u = UNVOUCHED_MANIFEST;
    if declared.arg != u.arg || declared.mem != u.mem {
        return Err(ImageRefusal::ExceedsVouch);
    }
    Ok(u)
}

/// **Whether an image request's words fit the manifest it will be endowed with.** The shell bound
/// the line against the note it read, and the progenitor judges its own copy of the bytes, so a
/// file changed in between (or a shell that lies) can send an argument or a `--mem` grant the
/// endowed manifest forbids. That is refused rather than half-honored.
///
/// `words` is whether the request carried an argv (`spawnproto::ARGS_BIT`), which sized the
/// region: it must agree with [`image_hears_words`] of the manifest, or a `std` program would be
/// built in a native job's forty pages.
pub fn image_request_fits(m: &Manifest, arg: u64, mem_pages: u64, words: bool) -> bool {
    if words != image_hears_words(m) {
        return false;
    }
    let arg_ok = m.arg == ArgSpec::Required || arg == 0;
    let mem_ok = match m.mem {
        MemSpec::Forbidden => mem_pages == 0,
        MemSpec::Required { min, max } => mem_pages >= min && mem_pages <= max,
    };
    arg_ok && mem_ok
}

#[cfg(test)]
mod tests;
