//! **Long-name assembly**: the pieces of a long name, as a directory stores them, joined into one
//! name and checked against the short entry they belong to. This is the read half of the writer
//! in `long_name.rs`, and `verification.rs` proves it accepts only what the writer could have
//! written.
//!
//! The pieces sit last-first on disk. The first carries the highest ordinal with the last-piece
//! bit; each next one has the ordinal one below; the one nearest the short entry has ordinal 1.
//! Every piece carries the checksum of the short entry's eleven name bytes. After the name's
//! last unit comes one `0x0000` and then `0xFFFF` to the end of the piece, unless the name fills
//! its last piece exactly (Microsoft's "FAT: General Overview of On-Disk Format", version 1.03,
//! section 7). A run that breaks any of this is refused whole: no part of it becomes a name.
//!
//! Name: ratified 2026-11-01 (UTC), the assembly of a long file name from its directory pieces, as
//! the specification's long name directory entries are assembled; the module holds `LongFileName`
//! and its refusals.

use crate::directory::LongNameEntry;
use crate::long_name::{MAXIMUM_CODE_UNITS, UNITS_PER_ENTRY, checksum};

/// The most pieces a name holds: 255 units at thirteen to a piece.
const MAXIMUM_PIECES: usize = MAXIMUM_CODE_UNITS.div_ceil(UNITS_PER_ENTRY);

/// The units those pieces hold, which is five more than a name may have.
const MAXIMUM_UNITS: usize = MAXIMUM_PIECES * UNITS_PER_ENTRY;

/// **The assembler the crate uses**, sized for the longest name. The proof instantiates
/// [`Assembler`] with fewer pieces.
pub(crate) type LongNameAssembler = Assembler<MAXIMUM_UNITS>;

/// **A long file name, assembled and checked**: up to 255 UTF-16 code units.
///
/// Only the crate's long-name assembler makes one, so a value of this type came from a run whose
/// ordinals, checksum and padding were all as the specification says. The units are as stored;
/// the specification says UCS-2, and a writer such as Windows stores surrogate pairs, so
/// [`LongFileName::characters`] decodes them and replaces a lone surrogate.
///
/// Name: ratified 2026-11-01 (UTC), the specification's "long file name"; a file name, so not
/// `LongName`, which is the writer's checked-name type inside the crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LongFileName {
    units: [u16; MAXIMUM_CODE_UNITS],
    len: usize,
}

impl LongFileName {
    /// The name's UTF-16 code units, without the terminator and the padding.
    ///
    /// Name: ratified 2026-11-01 (UTC), matches `LongNameEntry::code_units`.
    pub fn code_units(&self) -> &[u16] {
        &self.units[..self.len]
    }

    /// The name's characters, with U+FFFD for a surrogate that has no partner.
    ///
    /// Name: ratified 2026-11-01 (UTC), the UTF-16 decode of `code_units`.
    pub fn characters(&self) -> impl Iterator<Item = char> + '_ {
        char::decode_utf16(self.code_units().iter().copied())
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
    }
}

/// **Why a run of long-name pieces was refused.**
///
/// Name: ratified 2026-11-01 (UTC), renamed from `LongNameError`: the error of a long file name,
/// beside `LongFileName`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LongFileNameError {
    /// A piece other than the first arrived with no first piece before it, or the first piece's
    /// ordinal is 0 or above 20.
    ///
    /// Name: ratified 2026-11-01 (UTC), a run with no valid beginning.
    BadStart,
    /// A piece's ordinal is not one below the piece before it.
    ///
    /// Name: ratified 2026-11-01 (UTC), the ordinals are out of sequence.
    OutOfOrder,
    /// The pieces do not carry one checksum.
    ///
    /// Name: ratified 2026-11-01 (UTC), the pieces disagree with each other.
    PiecesDisagree,
    /// The run ends before ordinal 1, so the short entry's nearest piece is missing.
    ///
    /// Name: ratified 2026-11-01 (UTC), a run that stops early.
    Incomplete,
    /// The pieces' checksum is not the checksum of the short entry's name, so the run belongs to
    /// another entry or the short name changed after it was written.
    ///
    /// Name: ratified 2026-11-01 (UTC), the checksum the specification binds the two with.
    ChecksumMismatch,
    /// The name is empty or over 255 units, a unit follows the terminator that is not `0xFFFF`
    /// padding, or the last piece holds no unit of the name.
    ///
    /// Name: ratified 2026-11-01 (UTC), a run whose padding is not the specification's.
    BadPadding,
}

/// **Joins long-name pieces, in directory order, and checks them against their short entry.**
///
/// Call [`LongNameAssembler::push`] for each piece as the directory yields it, then
/// [`LongNameAssembler::finish`] with the next short entry's name. `finish` also clears the state
/// for the next run.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Assembler<const UNITS: usize> {
    /// Whole pieces, so a twentieth piece (which holds units 247 to 259) fits before its length
    /// is checked. `UNITS` is a whole number of pieces: the proof instantiates three.
    units: [u16; UNITS],
    /// How many pieces the run has, from its first piece's ordinal; 0 when no run is open.
    pieces: u8,
    /// The ordinal the next piece must have, which is 1 once the run is complete.
    next: u8,
    /// The first piece's checksum.
    checksum: u8,
    /// The first reason the open run is refused.
    error: Option<LongFileNameError>,
}

impl<const UNITS: usize> Assembler<UNITS> {
    /// The most pieces this assembler takes.
    const PIECES: usize = UNITS / UNITS_PER_ENTRY;

    /// An assembler with no run open.
    pub(crate) const fn new() -> Assembler<UNITS> {
        assert!(UNITS / UNITS_PER_ENTRY * UNITS_PER_ENTRY == UNITS);
        assert!(UNITS / UNITS_PER_ENTRY <= u8::MAX as usize);
        Assembler {
            units: [0; UNITS],
            pieces: 0,
            next: 0,
            checksum: 0,
            error: None,
        }
    }

    /// **Take the next piece of a run.** A first piece (last-piece bit set) opens a new run and
    /// drops any run still open, since a short entry never followed it.
    pub(crate) fn push(&mut self, piece: &LongNameEntry) {
        let ordinal = piece.ordinal();
        if piece.is_last() {
            *self = Assembler::<UNITS>::new();
            if ordinal == 0 || usize::from(ordinal) > Self::PIECES {
                self.error = Some(LongFileNameError::BadStart);
                return;
            }
            self.pieces = ordinal;
            self.checksum = piece.checksum();
        } else if self.pieces == 0 {
            self.error.get_or_insert(LongFileNameError::BadStart);
            return;
        } else {
            if ordinal != self.next || ordinal == 0 {
                self.error.get_or_insert(LongFileNameError::OutOfOrder);
                return;
            }
            if piece.checksum() != self.checksum {
                self.error.get_or_insert(LongFileNameError::PiecesDisagree);
                return;
            }
        }
        if self.error.is_some() {
            return;
        }
        let at = usize::from(ordinal - 1) * UNITS_PER_ENTRY;
        self.units[at..at + UNITS_PER_ENTRY].copy_from_slice(&piece.code_units());
        self.next = ordinal - 1;
    }

    /// **Close the run against the short entry whose name is `short_name`.**
    ///
    /// `Ok(None)` when no piece came before the entry, `Ok(Some(name))` for a run that is whole,
    /// ordered, bound to `short_name` by its checksum and padded as the specification says, and
    /// the first reason otherwise. The assembler is empty afterwards either way.
    pub(crate) fn finish(
        &mut self,
        short_name: &[u8; 11],
    ) -> Result<Option<LongFileName>, LongFileNameError> {
        let Some(len) = self.finish_length(short_name)? else {
            return Ok(None);
        };
        let mut units = [0u16; MAXIMUM_CODE_UNITS];
        units[..len].copy_from_slice(&self.units[..len]);
        Ok(Some(LongFileName { units, len }))
    }

    /// **Check the run as `finish` does and report the name's length**, leaving its units in
    /// place for `finish` to copy. The run's state is cleared; the units stay until the next first
    /// piece, which no later piece can reach without opening a run of its own.
    fn finish_length(&mut self, short_name: &[u8; 11]) -> Result<Option<usize>, LongFileNameError> {
        let pieces = self.pieces;
        let next = self.next;
        let run_checksum = self.checksum;
        let error = self.error.take();
        self.pieces = 0;
        self.next = 0;
        if pieces == 0 && error.is_none() {
            return Ok(None);
        }
        if let Some(error) = error {
            return Err(error);
        }
        if next != 0 {
            return Err(LongFileNameError::Incomplete);
        }
        if run_checksum != checksum(short_name) {
            return Err(LongFileNameError::ChecksumMismatch);
        }
        let total = usize::from(pieces) * UNITS_PER_ENTRY;
        let len = self.units[..total]
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(total);
        let padded = self.units[len..total]
            .iter()
            .skip(1)
            .all(|unit| *unit == 0xFFFF);
        let in_last = len > (usize::from(pieces) - 1) * UNITS_PER_ENTRY;
        if !padded || !in_last || len > MAXIMUM_CODE_UNITS {
            return Err(LongFileNameError::BadPadding);
        }
        Ok(Some(len))
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
