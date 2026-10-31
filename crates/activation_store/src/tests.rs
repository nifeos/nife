//! **The tests of the container**: the round trip, the torn write that is refused whole, the
//! `current` pointer that names a whole generation or nothing, and the generation that a second
//! one leaves untouched.
//!
//! Name: provisional, minted 2026-10-29 (UTC) by lane
//! milestone/909-item-3-composition-checks-the-approval.

extern crate std;

use std::vec;
use std::vec::Vec;

use installed_programs::Answered;

use super::*;

/// A one-row generation, in the crate's own encoding: the row, its pointer and the answer line.
/// Built with the table's writer, so a body here is what an install writes and not a lookalike.
fn generation(digest_byte: u8, program: &str) -> Vec<u8> {
    let digest = [digest_byte; 32];
    let entry = installed_programs::Entry {
        program,
        version: "0.1.0",
        package: program,
        digest,
        manager: installed_programs::OWNER,
    };
    let mut out = vec![0u8; 512];
    let n = installed_programs::with_entries_granting(
        "",
        || core::iter::once(entry),
        |_| false,
        core::iter::empty,
        Some(&Answered::Console),
        &mut out,
    )
    .unwrap();
    out[..n].to_vec()
}

/// A store of `generations` with `current` live, written through the crate's own writer.
fn written(generations: &[&[u8]], current: u32) -> Vec<u8> {
    let lens: Vec<usize> = generations.iter().map(|g| g.len()).collect();
    let mut out = vec![0u8; write_size(&lens)];
    let n = write(generations, current, &mut out).unwrap();
    out[..n].to_vec()
}

/// What `parse` refused about `image`, or the test fails: a container that parsed when a refusal
/// was expected is the louder half of a wrong answer.
fn refused(image: &[u8]) -> Error {
    match Store::parse(image) {
        Ok(_) => panic!("a container the test expected refused was accepted"),
        Err(e) => e,
    }
}

/// Hand-assemble a container with the given header words and bodies, in the documented layout, so
/// a test can name a field the writer would not write.
fn assembled(count: u32, current: u32, reserved: u32, bodies: &[&[u8]]) -> Vec<u8> {
    let lens: Vec<usize> = bodies.iter().map(|b| b.len()).collect();
    let mut out = vec![0u8; write_size(&lens)];
    out[0..8].copy_from_slice(&MAGIC);
    out[8..12].copy_from_slice(&VERSION.to_le_bytes());
    out[12..16].copy_from_slice(&count.to_le_bytes());
    out[16..20].copy_from_slice(&current.to_le_bytes());
    out[20..24].copy_from_slice(&reserved.to_le_bytes());
    let mut at = HEADER_LEN + count as usize * DESCRIPTOR_LEN;
    for (index, body) in bodies.iter().enumerate() {
        at = at.next_multiple_of(BODY_ALIGN);
        let descriptor = HEADER_LEN + index * DESCRIPTOR_LEN;
        out[descriptor..descriptor + 4].copy_from_slice(&(at as u32).to_le_bytes());
        out[descriptor + 4..descriptor + 8].copy_from_slice(&(body.len() as u32).to_le_bytes());
        out[at..at + body.len()].copy_from_slice(body);
        at += body.len();
    }
    out
}

#[test]
fn a_store_round_trips() {
    let first = generation(1, "progenitor");
    let second = generation(2, "swish");
    let bytes = written(&[&first, &second], 2);
    let store = Store::parse(&bytes).unwrap();
    assert_eq!(store.count(), 2);
    assert_eq!(store.current(), 2);
    assert_eq!(store.live(), second.as_slice());
    assert_eq!(store.generation(1), Some(first.as_slice()));
    assert_eq!(store.generation(2), Some(second.as_slice()));
    assert_eq!(store.generation(0), None);
    assert_eq!(store.generation(3), None);
}

#[test]
fn every_truncation_is_refused_whole() {
    let first = generation(1, "progenitor");
    let second = generation(2, "swish");
    let bytes = written(&[&first, &second], 1);
    // Cuts across the header, the descriptors and both bodies, plus the one-byte and the
    // just-short-of-complete cut. A truncated store must refuse, never yield `current` naming a
    // body whose end the file does not hold.
    for cut in [
        0,
        1,
        8,
        12,
        16,
        20,
        HEADER_LEN,
        HEADER_LEN + 4,
        HEADER_LEN + 8,
        HEADER_LEN + 2 * DESCRIPTOR_LEN,
        HEADER_LEN + 2 * DESCRIPTOR_LEN + first.len() / 2,
        bytes.len() - 1,
    ] {
        assert!(
            Store::parse(&bytes[..cut]).is_err(),
            "a store cut to {cut} bytes was accepted"
        );
    }
}

#[test]
fn trailing_bytes_are_refused() {
    let first = generation(1, "progenitor");
    let mut bytes = written(&[&first], 1);
    bytes.push(0);
    assert_eq!(refused(&bytes), Error::BadLayout);
}

#[test]
fn current_names_a_generation_the_container_holds() {
    let first = generation(1, "progenitor");
    for current in [0u32, 2] {
        let bytes = assembled(1, current, 0, &[&first]);
        assert_eq!(refused(&bytes), Error::BadCurrent);
    }
}

#[test]
fn a_reserved_word_that_is_not_zero_is_refused() {
    let first = generation(1, "progenitor");
    let bytes = assembled(1, 1, 1, &[&first]);
    assert_eq!(refused(&bytes), Error::Reserved);
}

#[test]
fn a_body_that_is_not_a_table_is_refused() {
    // The layout check runs first, so the bodies here are the aligned lengths the writer
    // produces: only their content is wrong.
    let bytes = assembled(1, 1, 0, &[b"not a ta"]);
    assert_eq!(refused(&bytes), Error::NotATable);
    // A torn row line, cut mid-word and padded to the writer's alignment: the same refusal.
    let whole = generation(3, "swish");
    let mut torn = whole[..whole.len() - 2].to_vec();
    while torn.len() % BODY_ALIGN != 0 {
        torn.push(b'x');
    }
    let bytes = assembled(1, 1, 0, &[&torn]);
    assert_eq!(refused(&bytes), Error::NotATable);
}

#[test]
fn a_body_laid_out_other_than_the_writer_lays_it_out_is_refused() {
    let first = generation(1, "progenitor");
    let second = generation(2, "swish");
    // Two bodies whose descriptors name one offset: the layout has one shape.
    let mut bytes = assembled(2, 1, 0, &[&first, &second]);
    let second_descriptor = HEADER_LEN + DESCRIPTOR_LEN;
    let first_offset = u32le(&bytes, HEADER_LEN);
    bytes[second_descriptor..second_descriptor + 4].copy_from_slice(&first_offset.to_le_bytes());
    assert_eq!(refused(&bytes), Error::BadLayout);
}

#[test]
fn a_generation_is_unchanged_when_the_next_one_is_added() {
    // The immutability the installer's rollback rests on: adding a generation copies the old
    // ones' bytes verbatim, so a rollback to generation 1 restores exactly what generation 1 was.
    let first = generation(1, "progenitor");
    let second = generation(2, "swish");
    let one_bytes = written(&[&first], 1);
    let two_bytes = written(&[&first, &second], 2);
    let one = Store::parse(&one_bytes).unwrap();
    let two = Store::parse(&two_bytes).unwrap();
    assert_eq!(one.generation(1), two.generation(1));
    // And the write is a function of its inputs alone, which is what a digest over the seed,
    // recorded in a measurement manifest, rests on.
    assert_eq!(
        written(&[&first, &second], 2),
        written(&[&first, &second], 2)
    );
}

#[test]
fn the_writer_refuses_what_it_cannot_represent() {
    let first = generation(1, "progenitor");
    let lens = [first.len()];
    let mut out = vec![0u8; write_size(&lens)];
    assert_eq!(write(&[], 1, &mut out), Err(Error::NoGenerations));
    assert_eq!(write(&[&first], 0, &mut out), Err(Error::BadCurrent));
    assert_eq!(write(&[&first], 2, &mut out), Err(Error::BadCurrent));
    let short = out.len() - 1;
    assert_eq!(write(&[&first], 1, &mut out[..short]), Err(Error::TooSmall));
    // A body that is not a table refuses before a byte is written: the buffer is untouched. The
    // buffer is sized for the body the refusal is about, so the size check is not what fires.
    let mut wrong = vec![0u8; write_size(&[8])];
    let before = wrong.clone();
    assert_eq!(write(&[b"not a ta"], 1, &mut wrong), Err(Error::NotATable));
    assert_eq!(wrong, before);
}
