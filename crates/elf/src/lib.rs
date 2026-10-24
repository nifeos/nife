//! An ELF64 loader's front half: parse, validate, and hand back the segments to map.
//!
//! Pure logic that builds for the host: it parses static, little-endian `ET_EXEC` binaries for
//! this build's own machine and refuses everything else (no dynamic linking, relocations,
//! interpreter or PIE).
//!
//! See notes/elf.md and notes/elf/parser.md.
//!
//! Name: ratified 2026-09-18 (calef, `design/decisions/` §154 (the acronym test is whether the
//! phrase is spoken)): "executable and linkable format" is not a phrase people say, and an expert
//! meeting it translates it back to `elf`, so the name stays. Refused
//! `executable_and_linkable_format` (§154).

#![no_std]

mod note;
pub use note::{NoteError, NoteSearch, NoteSegment, NoteSegments};

/// Machine-checked proofs of the loader's front half (§14 (the project's direction), milestone 18
/// (verify the capability core, then spread inward)).
///
/// They prove the per-segment bounds check panic-free and in bounds. Totality of `Elf::parse` over
/// every byte string is past bounded model checking at a useful input size; the tests cover the
/// specific hostile files by example. See notes/verification/elf-parser-wall.md and
/// notes/elf/parser.md.
#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;

mod error;
mod machine;
mod parse;
mod segment;

pub use error::Error;
use machine::*;
pub use machine::{
    FOREIGN_MACHINES, KNOWN_MACHINES, NATIVE_MACHINE, machine_no_nife_build_accepts,
};
pub use parse::Elf;
use parse::*;
use segment::*;
pub use segment::{PF_R, PF_W, PF_X, Segment};
