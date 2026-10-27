//! **`pmap`: what a process's mappings are when there is no `/proc/pid/maps`**
//! (notes/process-view.md). The program's logic, lifted out to run on the host;
//! `components/src/pmap.rs` is the syscall and the two output streams. It is handed a function that
//! reads one mapping through `abi::address_space::LIST`, which needs `ENUMERATE` where `MAP_INTO`
//! needs `WRITE` (§114 (`pmap` gets its listing)), so it cannot widen the space or learn that
//! another exists. [`collect`] takes the whole listing before anything prints, and the answers are
//! `ps`'s three. See notes/process-view/address-space-enumeration.md.

//! # BUGS
//!
//! - One row per page, no range or size: `LIST` walks the revocation log, which has no adjacency.
//! - A `DeviceFrame` mapping reads as `rw-`; the flags carry no device bit this crate can see.
//! - A resumed cursor can land on a recycled log slot: a log entry carries no generation.
//! - Nothing at the prompt grants this program an address space (no manifest field, no wiring),
//!   though since §249 (a running address space stays nameable) a builder can keep a copy.
//!
//! Name: ratified 2026-08-23 (calef): the name every reader knows, shared with
//! `components/src/pmap.rs` as one thing split at the IO boundary.

#![cfg_attr(not(test), no_std)]

/// The widest address space a listing can produce in one call: the kernel's whole thread table's
/// worth of page-table overhead plus content pages, rounded generously.
///
/// Unlike `ps::MAX_ROWS`, which is exactly the kernel's thread table (`sched::MAX_THREADS`) and so
/// makes truncation *unreachable*, there is no equally tight bound on one process's own mapping
/// count: a space's own budget (its backing region) is the only limit, and that is a runtime
/// quantity, not a compile-time one. `MAX_ROWS` is sized generously (256) rather than exactly, and
/// truncation is a real, reachable, and reported case here in a way it is not for `ps`. See `BUGS`
/// and [`Listing::is_complete`].
pub const MAX_ROWS: usize = 256;

/// One line of the listing: a mapped page and how it may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Row {
    /// The virtual address this page is mapped at, in the space the capability names.
    pub va: u64,
    /// One of `abi::address_space`'s `MAP_RO`/`MAP_RW`/`MAP_CODE` codes.
    pub kind: u64,
}

/// A finished walk of one address space: the rows, and whatever went wrong.
///
/// Built by [`collect`], which is the only constructor, so a `Listing` always describes a walk
/// that really happened. `ps::Survey`'s shape verbatim, one object type over.
pub struct Listing<'a> {
    rows: &'a [Row],
    refused: Option<i64>,
    stalled: bool,
    truncated: bool,
}

/// **Walk an address space to its end and keep what it says.**
///
/// `read(cursor)` is one `abi::address_space::LIST`: it answers `(next_cursor, va, kind)`, where a negative
/// first word is a refusal and `abi::survey::DONE` means the walk is over. `ps::collect`'s shape
/// verbatim, including its termination guarantee for a cursor that does not advance: this function
/// is total for every reader, which is what makes every case here reachable from a host test.
///
/// # Examples
///
/// ```
/// use pmap::{MAX_ROWS, Row, collect};
///
/// // A space of one mapping: cursor 0 yields it, cursor 1 says done.
/// let mut reader = |cursor: u64| match cursor {
///     0 => (1, 0x0040_0000, abi::address_space::MAP_CODE),
///     _ => (abi::survey::DONE as i64, 0, 0),
/// };
/// let mut rows = [Row::default(); MAX_ROWS];
/// let listing = collect(&mut rows, &mut reader);
/// assert_eq!(listing.rows().len(), 1);
/// assert_eq!(listing.rows()[0].va, 0x0040_0000);
///
/// let mut text = Vec::new();
/// listing.write_report(&mut |b| text.extend_from_slice(b));
/// assert!(String::from_utf8(text).unwrap().contains("r-x"));
/// ```
pub fn collect<'a>(
    rows: &'a mut [Row],
    read: &mut dyn FnMut(u64) -> (i64, u64, u64),
) -> Listing<'a> {
    let mut n = 0usize;
    let mut refused = None;
    let mut stalled = false;
    let mut truncated = false;
    let mut cursor = 0u64;
    loop {
        let (next, va, kind) = read(cursor);
        if next < 0 {
            refused = Some(next);
            break;
        }
        let next = next as u64;
        if next == abi::survey::DONE {
            break;
        }
        if next <= cursor {
            stalled = true;
            break;
        }
        if n == rows.len() {
            truncated = true;
            break;
        }
        rows[n] = Row { va, kind };
        n += 1;
        cursor = next;
    }
    Listing {
        rows: &rows[..n],
        refused,
        stalled,
        truncated,
    }
}

impl Listing<'_> {
    /// The rows, in the order the kernel reported them (the space's own log order, not sorted by
    /// address).
    pub fn rows(&self) -> &[Row] {
        self.rows
    }

    /// Was this a refusal? Same meaning as `ps::Survey::is_refused`.
    pub fn is_refused(&self) -> bool {
        self.refused.is_some() || self.stalled
    }

    /// Is this listing the whole space? Same meaning as `ps::Survey::is_complete`.
    pub fn is_complete(&self) -> bool {
        !self.is_refused() && !self.truncated
    }

    /// What there is to complain about, or `None` when the walk succeeded and found mappings.
    /// `ps::Survey::complaint`'s shape verbatim.
    pub fn complaint(&self) -> Option<&'static str> {
        if self.stalled {
            return Some("the listing did not advance; the result is incomplete");
        }
        if let Some(code) = self.refused {
            return Some(refusal(code));
        }
        if self.truncated {
            return Some("the listing buffer filled; this address space has more mapped");
        }
        if self.rows.is_empty() {
            // Not an error: the caller held the capability and was allowed to look, and there was
            // nothing mapped (or, per this crate's BUGS, the space's registry entry is already
            // gone). Either way the answer is honestly "nothing", not "refused".
            return Some("this address space has nothing mapped");
        }
        None
    }

    /// Everything to complain about, said before a byte of output (DECISIONS §67).
    pub fn write_diagnostics(&self, out: &mut dyn FnMut(&[u8])) {
        if let Some(clause) = self.complaint() {
            out(b"pmap: ");
            out(clause.as_bytes());
            out(b"\n");
        }
    }

    /// The table itself, on the output stream. Nothing at all on a refusal.
    pub fn write_report(&self, out: &mut dyn FnMut(&[u8])) {
        if !self.is_complete() || self.rows.is_empty() {
            return;
        }
        out(b"              VA  PERM\n");
        for row in self.rows() {
            write_va(row.va, out);
            out(b"  ");
            out(kind_name(row.kind).as_bytes());
            out(b"\n");
        }
    }
}

/// What an `abi::address_space` kind code is called in the listing: the three permission letters a reader
/// of any Unix `pmap`/`/proc/pid/maps` already knows, `r--`/`rw-`/`r-x`. An unknown code prints as
/// `???` rather than panicking, the same defensiveness `ps::state_name` has for a kernel newer than
/// this program.
pub const fn kind_name(kind: u64) -> &'static str {
    match kind {
        abi::address_space::MAP_RO => "r--",
        abi::address_space::MAP_RW => "rw-",
        abi::address_space::MAP_CODE => "r-x",
        _ => "???",
    }
}

/// What a negated `abi::Error` means to somebody who typed `pmap`. `ps::refusal`'s catalogue, one
/// object type over.
pub fn refusal(code: i64) -> &'static str {
    match abi::Error::from_ret(code) {
        Some(abi::Error::NoSuchSlot) => "this process holds no address-space capability",
        Some(abi::Error::NotPermitted) => {
            "this address space may be mapped into, but not looked at: no ENUMERATE"
        }
        Some(abi::Error::WrongObject) => "the capability granted here is not an address space",
        Some(abi::Error::Gone) => "the address space this named has been destroyed",
        _ => "the address space could not be read",
    }
}

/// A virtual address, right-aligned in fourteen hex columns (`0x` plus sixteen digits and a margin
/// to spare): wide enough for the full 48-bit user half this kernel's `Half::Low` admits, without
/// ever truncating a real one.
fn write_va(va: u64, out: &mut dyn FnMut(&[u8])) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..16 {
        buf[17 - i] = HEX[((va >> (i * 4)) & 0xf) as usize];
    }
    // Strip leading zero digits after the `0x`, keeping at least one.
    let mut start = 2;
    while start < 17 && buf[start] == b'0' {
        start += 1;
    }
    let digits = &buf[start..];
    for _ in 0..(14usize.saturating_sub(2 + digits.len())) {
        out(b" ");
    }
    out(b"0x");
    out(digits);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader over a canned space: `entries` in slot order, then done.
    fn space(entries: &'static [(u64, u64)]) -> impl FnMut(u64) -> (i64, u64, u64) {
        move |cursor: u64| match entries.get(cursor as usize) {
            Some(&(va, kind)) => (cursor as i64 + 1, va, kind),
            None => (abi::survey::DONE as i64, 0, 0),
        }
    }

    fn shown(f: impl FnOnce(&mut dyn FnMut(&[u8]))) -> String {
        let mut v = Vec::new();
        f(&mut |b| v.extend_from_slice(b));
        String::from_utf8(v).unwrap()
    }

    /// Every kind has its three letters, and the one with no test of its own (`MAP_RO`) cannot be
    /// deleted into `???`.
    #[test]
    fn every_kind_has_its_permission_letters() {
        assert_eq!(kind_name(abi::address_space::MAP_RO), "r--");
        assert_eq!(kind_name(abi::address_space::MAP_RW), "rw-");
        assert_eq!(kind_name(abi::address_space::MAP_CODE), "r-x");
        assert_eq!(kind_name(u64::MAX), "???");
    }

    /// Each refusal is explained in its own words, not folded into the generic last line.
    #[test]
    fn each_refusal_is_explained_in_its_own_words() {
        let say = |e: abi::Error| refusal(e as i64);
        assert!(say(abi::Error::NoSuchSlot).contains("holds no address-space capability"));
        assert!(say(abi::Error::WrongObject).contains("not an address space"));
        assert!(say(abi::Error::NotPermitted).contains("no ENUMERATE"));
        assert!(say(abi::Error::Gone).contains("destroyed"));
        assert!(refusal(0).contains("could not be read"));
    }

    /// The address column is right-aligned in fourteen columns, and an address of zero keeps its
    /// one digit rather than stripping all of them.
    #[test]
    fn an_address_is_aligned_and_zero_keeps_a_digit() {
        let va = |v| shown(|o| write_va(v, o));
        assert_eq!(va(0), "           0x0");
        assert_eq!(va(0x40_0000), "      0x400000");
        assert_eq!(va(0x1234), "        0x1234");
        assert_eq!(va(0xffff_ffff_ffff), "0xffffffffffff");
        assert_eq!(va(u64::MAX), "0xffffffffffffffff");
    }

    #[test]
    fn a_space_walks_to_its_end() {
        let mut rows = [Row::default(); MAX_ROWS];
        let l = collect(
            &mut rows,
            &mut space(&[
                (0x0040_0000, abi::address_space::MAP_CODE),
                (0x0050_0000, abi::address_space::MAP_RW),
                (0x0060_0000, abi::address_space::MAP_RO),
            ]),
        );
        assert!(!l.is_refused());
        assert_eq!(l.rows().len(), 3);
        assert_eq!(
            l.rows()[2],
            Row {
                va: 0x0060_0000,
                kind: abi::address_space::MAP_RO
            }
        );
    }

    /// **The claim the whole program exists to make**, `ps`'s test verbatim, one object type over:
    /// an empty space and a refused one must not produce the same thing on any stream.
    #[test]
    fn an_empty_space_and_a_refusal_are_different_answers() {
        let mut rows_a = [Row::default(); MAX_ROWS];
        let empty = collect(&mut rows_a, &mut space(&[]));
        let mut rows_b = [Row::default(); MAX_ROWS];
        let refused = collect(&mut rows_b, &mut |_| {
            (abi::Error::NotPermitted as i64, 0, 0)
        });

        assert!(!empty.is_refused(), "an empty space is not a refusal");
        assert!(refused.is_refused());

        let empty_diag = shown(|o| empty.write_diagnostics(o));
        let refused_diag = shown(|o| refused.write_diagnostics(o));
        assert_ne!(empty_diag, refused_diag);
        assert!(empty_diag.contains("nothing mapped"), "{empty_diag}");
        assert!(refused_diag.contains("not looked at"), "{refused_diag}");

        assert_eq!(shown(|o| empty.write_report(o)), "");
        assert_eq!(shown(|o| refused.write_report(o)), "");
    }

    #[test]
    fn a_refusal_halfway_through_discards_the_partial_table() {
        let mut rows = [Row::default(); MAX_ROWS];
        let l = collect(&mut rows, &mut |cursor| match cursor {
            0 => (1, 0x0040_0000, abi::address_space::MAP_CODE),
            _ => (abi::Error::Gone as i64, 0, 0),
        });
        assert!(l.is_refused());
        assert_eq!(l.rows().len(), 1);
        assert_eq!(shown(|o| l.write_report(o)), "");
        assert!(shown(|o| l.write_diagnostics(o)).contains("destroyed"));
    }

    #[test]
    fn a_cursor_that_does_not_advance_ends_the_walk() {
        let mut rows = [Row::default(); MAX_ROWS];
        let l = collect(&mut rows, &mut |_| {
            (1, 0x0040_0000, abi::address_space::MAP_RO)
        });
        assert!(l.is_refused());
        assert!(shown(|o| l.write_diagnostics(o)).contains("did not advance"));
    }

    #[test]
    fn a_buffer_too_small_for_the_space_says_so_and_prints_nothing() {
        let mut rows = [Row::default(); 2];
        let l = collect(&mut rows, &mut |cursor| {
            (
                cursor as i64 + 1,
                0x0040_0000 + cursor * 0x1000,
                abi::address_space::MAP_RW,
            )
        });
        assert_eq!(l.rows().len(), 2);
        assert!(!l.is_complete());
        assert!(!l.is_refused(), "running out of room is not a refusal");
        assert!(shown(|o| l.write_diagnostics(o)).contains("more mapped"));
        assert_eq!(shown(|o| l.write_report(o)), "");
    }

    #[test]
    fn the_table_has_a_header_and_one_line_per_mapping() {
        let mut rows = [Row::default(); MAX_ROWS];
        let l = collect(
            &mut rows,
            &mut space(&[
                (0x0040_0000, abi::address_space::MAP_CODE),
                (0x0050_0000, abi::address_space::MAP_RW),
            ]),
        );
        let out = shown(|o| l.write_report(o));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3, "header plus two rows: {out}");
        assert!(lines[0].contains("VA") && lines[0].contains("PERM"));
        assert!(lines[1].ends_with("0x400000  r-x"), "{}", lines[1]);
        assert!(lines[2].ends_with("0x500000  rw-"), "{}", lines[2]);
        assert_eq!(shown(|o| l.write_diagnostics(o)), "");
    }

    #[test]
    fn an_unknown_kind_is_shown_as_unknown() {
        assert_eq!(kind_name(99), "???");
        let mut rows = [Row::default(); MAX_ROWS];
        let l = collect(&mut rows, &mut space(&[(0x0040_0000, 99)]));
        assert!(shown(|o| l.write_report(o)).contains("???"));
    }

    #[test]
    fn every_refusal_names_what_is_not_held() {
        for code in [
            abi::Error::NoSuchSlot as i64,
            abi::Error::NotPermitted as i64,
            abi::Error::WrongObject as i64,
            abi::Error::Gone as i64,
            -99,
        ] {
            let m = refusal(code);
            assert!(!m.is_empty());
            assert!(
                m.chars().next().is_some_and(|c| c.is_lowercase()),
                "a refusal is a clause the program name prefixes: {m}",
            );
        }
    }

    /// A `va` at the very top of the user half prints in full and does not overflow the column.
    #[test]
    fn a_wide_va_prints_in_full() {
        let mut rows = [Row::default(); MAX_ROWS];
        let l = collect(
            &mut rows,
            &mut space(&[(0x0000_7fff_ffff_f000, abi::address_space::MAP_RW)]),
        );
        let out = shown(|o| l.write_report(o));
        assert!(out.contains("0x7ffffffff000"), "{out}");
    }
}
