//! An ELF64 loader's front half: parse, validate, and hand back the segments to map.
//!
//! Pure logic that builds for the host: it parses static, little-endian `ET_EXEC` binaries for
//! this build's own machine and refuses everything else (no dynamic linking, relocations,
//! interpreter or position-independent executable).
//!
//! The README says what it refuses and why, and what it does not check.
//!
//! Name: ratified 2026-11-14 (UTC) (block 1088, option A): `static_elf`, the one kind of file the
//! crate accepts. The crates.io name `elf` belongs to a different, widely used parser, so the
//! package takes the same name as the code, with no `package =` rename and no `doc(alias)` of the
//! old spelling. Refused `executable_and_linkable_format` (the acronym test, decision 154).

#![no_std]

mod note;
pub use note::{NoteError, NoteSearch, NoteSegment, NoteSegments};

/// Machine-checked proofs of the loader's front half.
///
/// They prove the per-segment bounds check and the program-header offset panic-free and in bounds,
/// and `Segment::page_range` panic-free and ordered for every page size. Totality of `Elf::parse`
/// over every byte string is past bounded model checking at a useful input size: the overlap check
/// is a loop over the headers inside a loop over the same, and the header table sits at a symbolic
/// offset in a symbolic file. So the arithmetic a hostile header can weaponize is factored into
/// loopless functions the proofs reach, and the tests cover the specific hostile files by example.
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
pub use machine::{MACHINE_AARCH64, MACHINE_RISCV, MACHINE_X86_64, NATIVE_MACHINE};
pub use parse::Elf;
use parse::*;
use segment::*;
pub use segment::{EXECUTABLE, PageBytes, READABLE, Segment, WRITABLE};
