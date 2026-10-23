//! **Replacing a running timetable's document, over one shared page** (milestone 129 (scheduled execution), DECISIONS
//! §222 (who holds a user's schedule)).
//!
//! Each durable session holds its own timetable and changes it by replacing the whole document,
//! with the one operation [`REPLACE`]. This module is all the registrar and the timetable agree on,
//! so it is one crate module rather than two copies (AGENTS.md rule 7).

//! # §222's five sub-rulings, and where each lives
//!
//! 1. One shared 4096-byte page carries the document in and the plan out ([`PAGE_BYTES`]).
//! 2. All or nothing: a replacement that does not parse, or names a program the timetable cannot
//!    load, leaves the schedule in force untouched ([`STATUS_PARSE`], [`STATUS_NO_IMAGE`]).
//! 3. An entry whose text is byte-identical keeps its beat ([`crate::Registry::arm_after`]).
//! 4. A job from a removed entry runs to completion: the timetable holds no capability to it.
//! 5. The session writes the store and then stages the replace; the timetable holds no directory.

//! # The page
//!
//! Control is a sequence word in the page, not a message as §10 (the capability-based microkernel
//! process model) would have it: a timetable yield-polls its clock from its one blocking wait
//! point, so a `RECEIVE` would stop it watching. Eight words of header, then bytes. The registrar
//! writes the document at [`BODY`] and its length at [`LEN`], then, **last** and with release
//! ordering, a new [`REQUEST`] word, and reads nothing but [`REPLY`] until that matches. The
//! timetable answers with [`STATUS`], [`DETAIL`], [`VERDICTS`] and the plan at [`BODY`]
//! ([`PLAN_LEN`]), then, last, [`REPLY`] set to the request's sequence number. With a page it says
//! nothing down its output endpoint ([`crate::contract`]), and it leaves its exit code at [`EXIT`].
//! See notes/scheduled-execution/the-crate.md, "The page".

//! Name: provisional, minted 2026-09-26 (UTC) by milestone 129's lane, for this module,
//! [`REPLACE`] and every constant here. Naming is calef's.

use crate::{Admission, Registry, Unbacked};

/// The page's size. One page, as §222's first sub-ruling set it.
pub const PAGE_BYTES: usize = 4096;

/// Byte offset of the request word: `(sequence << 8) | operation`. Written last by the registrar.
pub const REQUEST: usize = 0;
/// Byte offset of the document's length in bytes.
pub const LEN: usize = 8;
/// Byte offset of the reply word: the sequence number of the request answered. Written last.
pub const REPLY: usize = 16;
/// Byte offset of the outcome, one of the `STATUS_*` codes.
pub const STATUS: usize = 24;
/// Byte offset of the outcome's detail: a line number for [`STATUS_PARSE`], an entry index for
/// [`STATUS_NO_IMAGE`], and zero otherwise.
pub const DETAIL: usize = 32;
/// Byte offset of the verdict word: eight bits per entry, entry 0 in the low byte ([`verdict`]).
pub const VERDICTS: usize = 40;
/// Byte offset of the printed plan's length, which is at most [`BODY_MAX`].
pub const PLAN_LEN: usize = 48;
/// Byte offset of the exit word: zero while the timetable runs, and [`EXITED`] with its exit code
/// (one of [`crate::contract`]'s, or `0`) once it has stopped. Written just before it exits, so a
/// registrar that has reaped it reads why.
pub const EXIT: usize = 56;
/// The bit that marks the exit word as written.
pub const EXITED: u64 = 1 << 63;
/// Where the bytes start: the document on the way in, the printed plan on the way out.
pub const BODY: usize = 64;
/// The most bytes a document, or a plan, can occupy.
pub const BODY_MAX: usize = PAGE_BYTES - BODY;

/// **Replace the whole document.** The one operation §222 ruled on. Removing an entry is a replace
/// without its line; removing all of them is an empty document.
pub const REPLACE: u64 = 1;

/// The replacement is in force.
pub const STATUS_REPLACED: u64 = 0;
/// The replacement did not parse. [`DETAIL`] is the line. The schedule in force is unchanged.
pub const STATUS_PARSE: u64 = 1;
/// An admitted entry names a program the timetable's archive does not carry. [`DETAIL`] is the
/// entry's index. The schedule in force is unchanged.
pub const STATUS_NO_IMAGE: u64 = 2;
/// [`LEN`] is larger than [`BODY_MAX`], or the bytes are not UTF-8. Unchanged.
pub const STATUS_MALFORMED: u64 = 3;
/// The request word named an operation other than [`REPLACE`]. Unchanged.
pub const STATUS_UNKNOWN_OPERATION: u64 = 4;

/// The replacement was empty, so the timetable is exiting: it answers, lets the jobs already
/// running finish, reports, and stops. A timetable holding nothing still holds its session up
/// under §16 (object revocation), whose rule is that a parent with live children refuses to be
/// destroyed, so an empty schedule has to end the process rather than leave it idling. The
/// verdict word is zero.
pub const STATUS_EMPTIED: u64 = 5;

/// The request word for `operation` at sequence number `seq`.
pub const fn request(operation: u64, seq: u64) -> u64 {
    (seq << 8) | (operation & 0xff)
}

/// The operation a request word carries.
pub const fn operation(request: u64) -> u64 {
    request & 0xff
}

/// The sequence number a request word carries.
pub const fn sequence(request: u64) -> u64 {
    request >> 8
}

/// Verdict kind in bits 0 and 1: no entry at this index.
pub const KIND_NONE: u8 = 0;
/// Verdict kind: planned, backed, armed.
pub const KIND_FIRES: u8 = 1;
/// Verdict kind: the program's own manifest refuses the line. The sentence is in the plan.
pub(crate) const KIND_REFUSED: u8 = 2;
/// Verdict kind: legal, and this timetable holds nothing to back it. Bits 2 to 4 say what.
pub const KIND_UNBACKED: u8 = 3;
/// Verdict bit 7: the entry was byte-identical to one already in force and kept its beat.
pub const KEPT_PHASE: u8 = 0x80;

/// **One entry's verdict byte.** Kind in bits 0 and 1; for [`KIND_UNBACKED`], the missing
/// authority in bits 2 to 4 ([`unbacked_code`]); [`KEPT_PHASE`] in bit 7.
///
/// A refusal's reason is not encoded. `grant_plan::Refusal` has thirty-six variants, some carrying
/// payloads, and a numbering of them would be a second wire format nobody asked for. The sentence
/// is in the printed plan, which travels in the same page.
pub fn verdict(admission: &Admission, kept: bool) -> u8 {
    let base = match admission {
        Admission::Fires(_) => KIND_FIRES,
        Admission::Refused(_) => KIND_REFUSED,
        Admission::Unbacked(u) => KIND_UNBACKED | (unbacked_code(*u) << 2),
    };
    if kept { base | KEPT_PHASE } else { base }
}

/// A missing authority's three-bit code, in [`Unbacked`]'s declaration order.
pub const fn unbacked_code(u: Unbacked) -> u8 {
    match u {
        Unbacked::File => 0,
        Unbacked::Directory => 1,
        Unbacked::Clock => 2,
        Unbacked::Domain => 3,
        Unbacked::Interrupt => 4,
        Unbacked::Memory => 5,
        Unbacked::WallClock => 6,
    }
}

/// **The verdict word for a whole registry**: entry `i`'s byte at bits `8i..8i+8`. `kept` is the
/// mask [`Registry::arm_after`] returned, bit `i` for entry `i`.
pub fn verdicts(reg: &Registry<'_>, kept: u8) -> u64 {
    let mut word = 0u64;
    for (i, row) in reg.rows().iter().enumerate() {
        let v = verdict(&row.admission, kept & (1 << i) != 0);
        word |= (v as u64) << (8 * i);
    }
    word
}

/// Entry `i`'s byte out of a verdict word.
pub const fn verdict_of(word: u64, i: usize) -> u8 {
    (word >> (8 * i)) as u8
}

/// **Stage a replacement in `page`, for the registrar.** Writes the length and the document; the
/// caller then publishes `request(REPLACE, seq)` at [`REQUEST`] with release ordering, which is the
/// one step this function cannot take for it, because how a word is stored atomically depends on
/// where the page is mapped.
///
/// `None` when the document does not fit. It does not parse the document: the timetable does, and
/// a registrar that pre-checked would be a second opinion that could disagree with the first.
pub fn stage(page: &mut [u8], doc: &[u8]) -> Option<()> {
    if doc.len() > BODY_MAX || page.len() < PAGE_BYTES {
        return None;
    }
    page[LEN..LEN + 8].copy_from_slice(&(doc.len() as u64).to_le_bytes());
    page[BODY..BODY + doc.len()].copy_from_slice(doc);
    Some(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::contract::STAGING_BYTES;
    use crate::{Admission, Held, Unbacked, parse, registration};

    #[test]
    fn a_request_word_carries_an_operation_and_a_sequence_that_come_back_out() {
        let w = request(7, 0x1234);
        assert_eq!(w, 0x0012_3407);
        assert_eq!(operation(w), 7);
        assert_eq!(sequence(w), 0x1234);
        // Only the low byte is the operation, and a wider one does not leak into the sequence.
        assert_eq!(operation(request(0x1ff, 1)), 0xff);
        assert_eq!(sequence(request(0x1ff, 1)), 1);
    }

    #[test]
    fn each_missing_authority_has_its_own_three_bit_code() {
        let all = [
            (Unbacked::File, 0),
            (Unbacked::Directory, 1),
            (Unbacked::Clock, 2),
            (Unbacked::Domain, 3),
            (Unbacked::Interrupt, 4),
            (Unbacked::Memory, 5),
            (Unbacked::WallClock, 6),
        ];
        for (u, code) in all {
            assert_eq!(unbacked_code(u), code, "{u:?}");
            assert_eq!(
                verdict(&Admission::Unbacked(u), false),
                KIND_UNBACKED | (code << 2)
            );
        }
    }

    #[test]
    fn the_page_layout_adds_up() {
        assert_eq!(EXITED, 0x8000_0000_0000_0000);
        assert_eq!(BODY_MAX + BODY, PAGE_BYTES);
        assert_eq!(STAGING_BYTES, 131_072);
    }

    #[test]
    fn a_document_is_staged_into_a_page_exactly_as_big_as_the_page() {
        let doc = [b'x'; 5];
        let mut page = std::vec![0u8; PAGE_BYTES];
        assert_eq!(stage(&mut page, &doc), Some(()));
        assert_eq!(&page[BODY..BODY + 5], &doc);
        assert_eq!(&page[LEN..LEN + 8], &5u64.to_le_bytes());
        // A page larger than the contract is as good as one of exactly its size; a smaller one is
        // refused, and so is a document one byte too long.
        let mut big = std::vec![0u8; PAGE_BYTES + 16];
        assert_eq!(stage(&mut big, &doc), Some(()));
        let mut small = std::vec![0u8; PAGE_BYTES - 1];
        assert_eq!(stage(&mut small, &doc), None);
        let long = std::vec![b'y'; BODY_MAX + 1];
        assert_eq!(stage(&mut page, &long), None);
        let fits = std::vec![b'y'; BODY_MAX];
        assert_eq!(stage(&mut page, &fits), Some(()));
    }

    /// **The verdict word a registrar reads back**, and the request word it writes. Kind in the
    /// low two bits, the missing authority above it, the kept beat in bit 7.
    #[test]
    fn the_verdict_word_says_what_each_entry_became() {
        use registration::*;
        let doc = parse(
            "every 1s least_authority_demo 7\n\
             every 1s memory_grant_depleter\n\
             every 1s date\n",
        )
        .unwrap();
        let reg = Registry::register(&doc, Held::default());
        let word = verdicts(&reg, 0b001);
        assert_eq!(verdict_of(word, 0), KIND_FIRES | KEPT_PHASE);
        assert_eq!(verdict_of(word, 1), KIND_REFUSED);
        assert_eq!(
            verdict_of(word, 2),
            KIND_UNBACKED | (unbacked_code(Unbacked::Clock) << 2)
        );
        assert_eq!(verdict_of(word, 3), KIND_NONE, "no entry at index 3");

        let r = request(REPLACE, 41);
        assert_eq!((operation(r), sequence(r)), (REPLACE, 41));

        let mut page = [0u8; PAGE_BYTES];
        stage(&mut page, b"at-boot least_authority_demo 3\n").unwrap();
        assert_eq!(
            u64::from_le_bytes(page[LEN..LEN + 8].try_into().unwrap()),
            31
        );
        assert_eq!(&page[BODY..BODY + 31], b"at-boot least_authority_demo 3\n");
        assert!(
            stage(&mut page, &[b'#'; BODY_MAX + 1]).is_none(),
            "a document past the page"
        );
        assert!(
            stage(&mut page, &[b'#'; BODY_MAX]).is_some(),
            "and one that exactly fills it"
        );
    }

    // ---- survivor triage of the 2026-10-03 census ---------------------------------------------

    /// The kept mask is read bit by bit: entry `i` is kept when bit `i` is set. The existing check
    /// used bit 0, where a shift by zero cannot tell left from right.
    #[test]
    fn the_verdict_word_marks_the_kept_entry_and_only_that_one() {
        use registration::*;
        let doc = parse(
            "every 1s least_authority_demo 7\n\
             every 1s memory_grant_depleter\n\
             every 1s date\n",
        )
        .unwrap();
        let reg = Registry::register(&doc, Held::default());
        let word = verdicts(&reg, 0b010);
        assert_eq!(verdict_of(word, 0), KIND_FIRES);
        assert_eq!(verdict_of(word, 1), KIND_REFUSED | KEPT_PHASE);
        assert_eq!(
            verdict_of(word, 2),
            KIND_UNBACKED | (unbacked_code(Unbacked::Clock) << 2)
        );
    }
}
