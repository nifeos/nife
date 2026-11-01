//! Runs of pieces, written by the writer in `long_name.rs` and read back by the assembler, and
//! every way a run can be wrong.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;
use crate::boot_sector::FileAllocationTableType;
use crate::directory::{DirectoryEntry, Slot};
use crate::long_name::{LongName, checksum};

/// The most pieces any test run holds.
const MOST: usize = 3;

/// The pieces the writer lays out for `name` with `checksum`, in directory order.
fn pieces(name: &str, checksum: u8) -> ([LongNameEntry; MOST], usize) {
    let name = LongName::new(name).expect("a long name");
    assert!(name.entries() <= MOST);
    let blank = LongNameEntry::from_parts(0, 0, [0; 13]);
    let mut out = [blank; MOST];
    for (index, piece) in out.iter_mut().enumerate().take(name.entries()) {
        let mut bytes = [0u8; 32];
        name.entry(index, checksum, &mut bytes);
        match Slot::parse(&bytes, FileAllocationTableType::Fat32) {
            Slot::Used(DirectoryEntry::LongName(parsed)) => *piece = parsed,
            other => panic!("the writer wrote {other:?}"),
        }
    }
    (out, name.entries())
}

/// Assemble `run` against `short`.
fn assemble(
    run: &[LongNameEntry],
    short: &[u8; 11],
) -> Result<Option<LongFileName>, LongFileNameError> {
    let mut assembler = LongNameAssembler::new();
    for piece in run {
        assembler.push(piece);
    }
    assembler.finish(short)
}

fn name_of(result: Result<Option<LongFileName>, LongFileNameError>) -> [char; 40] {
    let mut out = ['\0'; 40];
    let name = result
        .expect("the run is accepted")
        .expect("there is a name");
    for (slot, c) in out.iter_mut().zip(name.characters()) {
        *slot = c;
    }
    out
}

fn text(chars: &[char; 40]) -> usize {
    chars.iter().position(|c| *c == '\0').unwrap_or(40)
}

#[test]
fn what_the_writer_writes_the_assembler_reads_back() {
    for text_in in [
        "a",
        "ABCDEFGHIJKLM",
        "BOOTRISCV64.EFI",
        "Long File Name With Spaces.txt",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "ünïcödé.txt",
    ] {
        let short = b"LONGFI~1TXT";
        let (run, count) = pieces(text_in, checksum(short));
        let chars = name_of(assemble(&run[..count], short));
        let expected = text_in.chars();
        assert_eq!(text(&chars), expected.clone().count(), "{text_in}");
        assert!(
            chars.iter().zip(expected).all(|(a, b)| *a == b),
            "{text_in}"
        );
    }
}

#[test]
fn no_pieces_is_no_long_name() {
    assert_eq!(assemble(&[], b"HELLO   TXT"), Ok(None));
}

#[test]
fn an_assembler_is_empty_after_finish() {
    let short = b"LONGFI~1TXT";
    let (run, count) = pieces("Long File Name.txt", checksum(short));
    let mut assembler = LongNameAssembler::new();
    for piece in &run[..count] {
        assembler.push(piece);
    }
    assert!(matches!(assembler.finish(short), Ok(Some(_))));
    assert_eq!(assembler.finish(short), Ok(None));
}

#[test]
fn a_checksum_for_another_name_is_refused() {
    let (run, count) = pieces("Long File Name.txt", checksum(b"LONGFI~1TXT"));
    assert_eq!(
        assemble(&run[..count], b"LONGFI~2TXT"),
        Err(LongFileNameError::ChecksumMismatch)
    );
}

#[test]
fn a_run_missing_its_nearest_piece_is_incomplete() {
    let short = b"LONGFI~1TXT";
    let (run, count) = pieces("Long File Name With Spaces.txt", checksum(short));
    assert_eq!(count, 3);
    assert_eq!(
        assemble(&run[..2], short),
        Err(LongFileNameError::Incomplete)
    );
}

#[test]
fn a_run_missing_its_first_piece_has_no_start() {
    let short = b"LONGFI~1TXT";
    let (run, count) = pieces("Long File Name With Spaces.txt", checksum(short));
    assert_eq!(
        assemble(&run[1..count], short),
        Err(LongFileNameError::BadStart)
    );
}

#[test]
fn pieces_out_of_sequence_are_refused() {
    let short = b"LONGFI~1TXT";
    let (run, _) = pieces("Long File Name With Spaces.txt", checksum(short));
    let swapped = [run[0], run[2], run[1]];
    assert_eq!(
        assemble(&swapped, short),
        Err(LongFileNameError::OutOfOrder)
    );
    let repeated = [run[0], run[1], run[1]];
    assert_eq!(
        assemble(&repeated, short),
        Err(LongFileNameError::OutOfOrder)
    );
}

#[test]
fn pieces_that_disagree_about_the_checksum_are_refused() {
    let short = b"LONGFI~1TXT";
    let sum = checksum(short);
    let (run, _) = pieces("Long File Name.txt", sum);
    let odd = LongNameEntry::from_parts(1, sum ^ 1, run[1].code_units());
    assert_eq!(
        assemble(&[run[0], odd], short),
        Err(LongFileNameError::PiecesDisagree)
    );
}

#[test]
fn a_first_piece_with_a_bad_ordinal_is_refused() {
    let units = [0x41u16; 13];
    for ordinal in [0x40u8, 0x40 | 21, 0x40 | 0x3F] {
        let piece = LongNameEntry::from_parts(ordinal, 0, units);
        assert_eq!(
            assemble(&[piece], b"A          "),
            Err(LongFileNameError::BadStart)
        );
    }
}

#[test]
fn padding_that_is_not_the_specifications_is_refused() {
    let short = b"A       TXT";
    let sum = checksum(short);
    let one = |units: [u16; 13]| assemble(&[LongNameEntry::from_parts(0x41, sum, units)], short);
    let mut units = [0xFFFFu16; 13];
    units[0] = 0x41;
    units[1] = 0;
    assert!(matches!(one(units), Ok(Some(_))));
    // A unit after the terminator that is not padding.
    units[5] = 0x42;
    assert_eq!(one(units), Err(LongFileNameError::BadPadding));
    // An empty name.
    let mut empty = [0xFFFFu16; 13];
    empty[0] = 0;
    assert_eq!(one(empty), Err(LongFileNameError::BadPadding));
    // No terminator is fine when the name fills the piece.
    assert!(matches!(one([0x41; 13]), Ok(Some(_))));
}

#[test]
fn a_last_piece_with_no_name_in_it_is_refused() {
    let short = b"A       TXT";
    let sum = checksum(short);
    let mut second = [0xFFFFu16; 13];
    second[0] = 0;
    let run = [
        LongNameEntry::from_parts(0x42, sum, second),
        LongNameEntry::from_parts(1, sum, [0x41; 13]),
    ];
    assert_eq!(assemble(&run, short), Err(LongFileNameError::BadPadding));
}

#[test]
fn a_new_first_piece_drops_a_run_that_never_reached_its_short_entry() {
    let short = b"LONGFI~1TXT";
    let sum = checksum(short);
    let (stale, _) = pieces("Long File Name With Spaces.txt", sum ^ 0xFF);
    let (fresh, count) = pieces("Long File Name.txt", sum);
    let mut assembler = LongNameAssembler::new();
    assembler.push(&stale[0]);
    for piece in &fresh[..count] {
        assembler.push(piece);
    }
    assert!(matches!(assembler.finish(short), Ok(Some(_))));
}

#[test]
fn the_longest_name_fits() {
    let mut units = [0x61u16; MAXIMUM_CODE_UNITS];
    units[MAXIMUM_CODE_UNITS - 1] = 0x62;
    let mut assembler = LongNameAssembler::new();
    let short = b"AAAAAA~1   ";
    let sum = checksum(short);
    for ordinal in (1..=MAXIMUM_PIECES as u8).rev() {
        let first = usize::from(ordinal - 1) * 13;
        let mut chunk = [0xFFFFu16; 13];
        for (k, slot) in chunk.iter_mut().enumerate() {
            if first + k < MAXIMUM_CODE_UNITS {
                *slot = units[first + k];
            } else if first + k == MAXIMUM_CODE_UNITS {
                *slot = 0;
            }
        }
        let flag = if usize::from(ordinal) == MAXIMUM_PIECES {
            0x40
        } else {
            0
        };
        assembler.push(&LongNameEntry::from_parts(ordinal | flag, sum, chunk));
    }
    let name = assembler.finish(short).unwrap().unwrap();
    assert_eq!(name.code_units(), &units[..]);
}

#[test]
fn an_over_long_name_is_refused() {
    // Twenty pieces with no terminator hold 260 units, five more than a name may.
    let short = b"AAAAAA~1   ";
    let sum = checksum(short);
    let mut assembler = LongNameAssembler::new();
    for ordinal in (1..=20u8).rev() {
        let flag = if ordinal == 20 { 0x40 } else { 0 };
        assembler.push(&LongNameEntry::from_parts(ordinal | flag, sum, [0x61; 13]));
    }
    assert_eq!(assembler.finish(short), Err(LongFileNameError::BadPadding));
}

#[test]
fn surrogate_pairs_decode_and_a_lone_surrogate_is_replaced() {
    let short = b"A       TXT";
    let sum = checksum(short);
    let mut units = [0xFFFFu16; 13];
    units[..5].copy_from_slice(&[0x61, 0xD83E, 0xDD80, 0xD800, 0x62]);
    units[5] = 0;
    let name = assemble(&[LongNameEntry::from_parts(0x41, sum, units)], short)
        .unwrap()
        .unwrap();
    let mut chars = name.characters();
    assert_eq!(chars.next(), Some('a'));
    assert_eq!(chars.next(), Some('\u{1F980}'));
    assert_eq!(chars.next(), Some(char::REPLACEMENT_CHARACTER));
    assert_eq!(chars.next(), Some('b'));
    assert_eq!(chars.next(), None);
}
