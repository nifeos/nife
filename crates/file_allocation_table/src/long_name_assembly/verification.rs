//! **Machine-checked proof (Kani) of long-name assembly**: the reader accepts a run only if the
//! specification would have written it. Behind `#[cfg(kani)]`, so only `cargo kani` compiles it.
//!
//! This is the read half of the fourth property in milestone 813's table; the writer's half is in
//! `long_name/verification.rs`.
//!
//! Name: provisional.

use super::*;
use crate::long_name::checksum;

/// The most pieces the harness pushes: three pieces hold up to 39 units, which reaches every
/// branch (a first piece, a middle one, the last, and padding in each).
const RUN_PIECES: usize = 3;

/// **The reader accepts a run only if the specification would have written it.** For any pieces
/// (ordinal, checksum and thirteen units, all chosen by the solver) and any short name, when
/// assembly returns a name: the first piece carries the last-piece bit and the ordinal equal to
/// the run's length, the ordinals fall by one to 1 and no later piece carries the bit, every piece
/// carries the checksum of the short name, and the name is the pieces' units in ordinal order up
/// to a first `0x0000`, with only `0xFFFF` after it and at least one unit in the last piece.
///
/// The bound is [`RUN_PIECES`] pieces, and the first piece's ordinal is at most that (every other
/// byte, the last-piece bit included, is the solver's), because the padding scan unrolls once per
/// unit and 260 units cost the solver more than the property is worth. The unit tests hold the
/// twentieth piece and the 255-unit name.
/// Falsification: replayable `crates/file_allocation_table/falsifications/long_name_assembly.verification.a_run_is_accepted_only_when_it_is_well_formed.patch`
#[kani::proof]
#[kani::unwind(42)]
fn a_run_is_accepted_only_when_it_is_well_formed() {
    let short: [u8; 11] = kani::any();
    let count: usize = kani::any();
    kani::assume((1..=RUN_PIECES).contains(&count));
    let mut pieces = [LongNameEntry::from_parts(0, 0, [0; 13]); RUN_PIECES];
    for piece in &mut pieces {
        *piece = LongNameEntry::from_parts(kani::any(), kani::any(), kani::any());
    }
    kani::assume(pieces[0].ordinal() <= RUN_PIECES as u8);
    let mut assembler = LongNameAssembler::new();
    for piece in &pieces[..count] {
        assembler.push(piece);
    }
    let Ok(Some(name)) = assembler.finish(&short) else {
        return;
    };
    assert!(pieces[0].is_last());
    for (index, piece) in pieces[..count].iter().enumerate() {
        assert_eq!(usize::from(piece.ordinal()), count - index);
        assert_eq!(piece.is_last(), index == 0);
        assert_eq!(piece.checksum(), checksum(&short));
    }
    let units = name.code_units();
    let total = count * UNITS_PER_ENTRY;
    assert!(units.len() > (count - 1) * UNITS_PER_ENTRY && units.len() <= total);
    let at = |position: usize| {
        pieces[count - 1 - position / UNITS_PER_ENTRY].code_units()[position % UNITS_PER_ENTRY]
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
