//! **Machine-checked proof (Kani) of long-name assembly**: the reader accepts a run only if the
//! specification would have written it. Behind `#[cfg(kani)]`, so only `cargo kani` compiles it.
//!
//! This is the reader's half of the property that long names are well formed; the writer's half
//! is in `long_name/verification.rs`.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;
use crate::long_name::checksum;

/// The most pieces the harness pushes: three pieces hold up to 39 units, which reaches every
/// branch (a first piece, a middle one, the last, and padding in each).
const RUN_PIECES: usize = 3;

/// **The reader accepts a run only if the specification would have written it.** For any pieces
/// (ordinal, checksum and thirteen units, all chosen by the solver) and any short name, when
/// assembly returns a name: the run starts at the last piece that carries the last-piece bit, with
/// the ordinal equal to the run's length, the ordinals fall by one to 1 and no later piece carries
/// the bit, every piece of the run carries the checksum of the short name, and the name is the
/// pieces' units in ordinal order up to a first `0x0000`, with only `0xFFFF` after it and at least
/// one unit in the last piece.
///
/// The bound is an assembler that holds [`RUN_PIECES`] pieces, so the padding scan unrolls over 39
/// units instead of 260. The ordinal bytes are the solver's, so a first piece naming more pieces
/// than the assembler holds is covered, and is refused. The code is the production assembler's,
/// with a smaller `UNITS`. The harness reads the checked units where the assembler keeps them;
/// copying them into a `LongFileName` is the unit tests'. They also hold the twentieth piece and
/// the 255-unit name.
/// Falsification: replayable `crates/file_allocation_table/falsifications/long_name_assembly.verification.a_run_is_accepted_only_when_it_is_well_formed.patch`
#[kani::proof]
#[kani::unwind(41)]
fn a_run_is_accepted_only_when_it_is_well_formed() {
    let short: [u8; 11] = kani::any();
    let count: usize = kani::any();
    kani::assume((1..=RUN_PIECES).contains(&count));
    let mut pieces = [LongNameEntry::from_parts(0, 0, [0; 13]); RUN_PIECES];
    for piece in &mut pieces {
        *piece = LongNameEntry::from_parts(kani::any(), kani::any(), kani::any());
    }
    let mut assembler = Assembler::<{ RUN_PIECES * UNITS_PER_ENTRY }>::new();
    // Loops with a fixed bound, so the solver unrolls three pushes and not forty-one.
    for (index, piece) in pieces.iter().enumerate() {
        if index < count {
            assembler.push(piece);
        }
    }
    let Ok(Some(length)) = assembler.finish_length(&short) else {
        return;
    };
    // A piece with the last-piece bit opens a run and drops what came before it, so the run the
    // short entry closes starts at the last such piece.
    let mut start = count;
    for (index, piece) in pieces.iter().enumerate() {
        if index < count && piece.is_last() {
            start = index;
        }
    }
    assert!(start < count);
    let run = count - start;
    for (index, piece) in pieces.iter().enumerate() {
        if index >= start && index < count {
            assert_eq!(usize::from(piece.ordinal()), run - (index - start));
            assert_eq!(piece.is_last(), index == start);
            assert_eq!(piece.checksum(), checksum(&short));
        }
    }
    let units = &assembler.units[..length];
    let total = run * UNITS_PER_ENTRY;
    assert!(units.len() > (run - 1) * UNITS_PER_ENTRY && units.len() <= total);
    let at = |position: usize| {
        pieces[start + run - 1 - position / UNITS_PER_ENTRY].code_units()
            [position % UNITS_PER_ENTRY]
    };
    let position: usize = kani::any();
    kani::assume(position < total);
    if position < units.len() {
        assert_eq!(units[position], at(position));
        assert!(at(position) != 0);
    } else if position == units.len() {
        assert_eq!(at(position), 0);
    } else {
        assert_eq!(at(position), 0xFFFF);
    }
}
