//! **The right-aligned number writer three table-printing programs share** (`free`, `vmstat` and
//! `slabtop`; milestone 793 (one crate for the right-aligned number writer three programs each
//! copy)). `write_right` prints a `u64` right-aligned in a column of the given width, and never
//! cuts a number wider than its column: the digits decide the width when they outrun it.
//! `write_signed` prints an `i64` with its sign, for an error code quoted inside a sentence. Output
//! goes to the caller's `&mut dyn FnMut(&[u8])`, the seam every program here writes its report
//! through, and the writer holds no state.

//! # EXAMPLES
//!
//! ```text
//! write_right(5, 3, out)        // "  5"
//! write_right(12345, 3, out)    // "12345": wider than its column, so it is printed whole
//! write_right(u64::MAX, 0, out) // "18446744073709551615": width 0 means no padding
//! write_signed(-3, out)         // "-3"
//! ```

//! # BUGS
//!
//! - A `width` above 24 pads to 24 and no further (twenty digits plus four spaces, fixed); no
//!   column in `free`, `vmstat` or `slabtop` is that wide.
//! - The unpadded writers that predate this crate (`swish`'s `write_num`, `pgrep`'s) are the same
//!   digit loop without the column; whether they become `write_right` calls at width `0` is no
//!   one's milestone yet.
//!
//! Name: provisional (milestone 793, 2026-10-15 UTC), for calef's naming pass. Considered and set
//! aside: `padnum` (floated in the block; abbreviates `num`, and reads as a verb phrase),
//! `decimal_writer` (suggests a fixed-point type), `padded_number` (names the output, not the
//! crate), `number_column` (reads as a column's ordinal).

#![cfg_attr(not(test), no_std)]

/// A `u64` right-aligned in `width` columns; `0` means no padding.
pub fn write_right(v: u64, width: usize, out: &mut dyn FnMut(&[u8])) {
    let mut buf = [b' '; 24];
    let mut i = buf.len();
    let mut v = v;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let start = buf.len().saturating_sub(width.max(buf.len() - i));
    out(&buf[start..]);
}

/// An `i64`, sign first: the shape of an error code quoted inside a sentence.
pub fn write_signed(v: i64, out: &mut dyn FnMut(&[u8])) {
    if v < 0 {
        out(b"-");
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut n = v.unsigned_abs();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    out(&buf[i..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn right(v: u64, width: usize) -> String {
        let mut out = Vec::new();
        write_right(v, width, &mut |b| out.extend_from_slice(b));
        String::from_utf8(out).unwrap()
    }

    /// A column pads short numbers and never cuts a long one: the digits decide the width when
    /// they outrun it.
    #[test]
    fn a_number_wider_than_its_column_is_printed_whole() {
        assert_eq!(right(5, 3), "  5");
        assert_eq!(right(0, 2), " 0");
        assert_eq!(right(12345, 3), "12345");
        assert_eq!(right(u64::MAX, 4), "18446744073709551615");
    }

    fn signed(v: i64) -> String {
        let mut out = Vec::new();
        write_signed(v, &mut |b| out.extend_from_slice(b));
        String::from_utf8(out).unwrap()
    }

    /// The sign goes first, once, and the magnitude follows digit for digit: a negative prints
    /// `-` then its digits, a positive prints no sign, and both `i64` extremes print their full
    /// nineteen-digit magnitude. `i64::MIN` is why the writer takes its absolute value unsigned:
    /// its magnitude does not fit back in an `i64`, so negating first would panic.
    #[test]
    fn a_signed_number_prints_its_sign_then_its_digits() {
        assert_eq!(signed(-9301), "-9301");
        assert_eq!(signed(9301), "9301");
        assert_eq!(signed(-3), "-3");
        assert_eq!(signed(i64::MIN), "-9223372036854775808");
        assert_eq!(signed(i64::MAX), "9223372036854775807");
    }

    /// Zero is not negative, so it prints one digit and no sign.
    #[test]
    fn zero_prints_one_digit_and_no_sign() {
        assert_eq!(signed(0), "0");
    }
}
