//! CRC-32 as GPT uses it, and the bitwise definition it is proved against.
//!
//! The IEEE 802.3 / zlib / PNG variant: reflected, polynomial `0x04C11DB7` written reversed as
//! `0xEDB88320`, initialised to all ones and complemented at the end. UEFI 2.10 §5.3.2 specifies it
//! by reference to "the 32-bit CRC algorithm". Four CRCs guard a GPT: each header's, over the size
//! it declares (92 bytes in practice), and each copy of the entry array's. [`crc32`] is
//! table-driven for the exhaustive tests; `crc32_bitwise` keeps the definition, and
//! `verification::crc32_matches_its_bitwise_definition` proves the two agree up to a bound. See
//! notes/globally-unique-identifier-partition-table/the-modules.md, "CRC-32".

/// The reversed CRC-32 polynomial. `0x04C11DB7` with its bits in the other order, which is what a
/// right-shifting (reflected) implementation uses.
const POLY: u32 = 0xEDB8_8320;

/// One table entry per possible byte, each holding eight rounds of the bitwise loop pre-computed.
/// Built by `const` evaluation from [`POLY`], so the polynomial appears exactly once in this crate.
const TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut bit = 0;
        while bit < 8 {
            c = if c & 1 != 0 { POLY ^ (c >> 1) } else { c >> 1 };
            bit += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
};

/// CRC-32 over `bytes`.
///
/// Table-driven. Equal to `crc32_bitwise` below on every input, which is proved rather than
/// assumed. (Not a doc link: that function is compiled only under `cfg(test)` and `cfg(kani)`, so
/// rustdoc cannot see it.)
pub fn crc32(bytes: &[u8]) -> u32 {
    !update(!0, bytes)
}

/// CRC-32 over several slices, as though they were one.
///
/// The GPT header's CRC covers the header with its own CRC field taken as zero. Feeding three
/// pieces to one running CRC makes that substitution without a block-sized scratch buffer on a
/// kernel stack.
pub(crate) fn crc32_pieces(pieces: &[&[u8]]) -> u32 {
    let mut crc = !0u32;
    for piece in pieces {
        crc = update(crc, piece);
    }
    !crc
}

/// One pass of the table loop over `bytes`, carrying `crc` in its internal (un-complemented) form.
fn update(mut crc: u32, bytes: &[u8]) -> u32 {
    for &b in bytes {
        crc = TABLE[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    crc
}

/// The same CRC-32, written as its definition: shift a bit out, and conditionally fold the
/// polynomial back in.
///
/// Not the fast path and not public: it is what the table-driven version is proved against, the
/// only defence against a subtly wrong table.
#[cfg(any(test, kani))]
pub(crate) fn crc32_bitwise(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            // Branch-free: `0 - (crc & 1)` is all ones when the low bit is set and zero otherwise,
            // so the polynomial is folded in exactly when the shifted-out bit was one.
            crc = (crc >> 1) ^ (POLY & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published check values for this CRC-32 variant. If these three pass, the algorithm is the
    /// one the rest of the world calls CRC-32, and every byte-order question is already settled.
    #[test]
    fn the_published_check_values() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
    }

    #[test]
    fn the_table_and_the_definition_agree_on_real_lengths() {
        // Every length up to a header's worth, with a byte pattern that exercises the whole table.
        let data: [u8; 128] = core::array::from_fn(|i| (i as u8).wrapping_mul(37).wrapping_add(11));
        for len in 0..=data.len() {
            assert_eq!(
                crc32(&data[..len]),
                crc32_bitwise(&data[..len]),
                "length {len}"
            );
        }
    }

    /// Splitting the input changes nothing, which is the property the header CRC's three-piece
    /// substitution depends on.
    #[test]
    fn pieces_are_the_same_as_the_whole() {
        let data = b"123456789";
        assert_eq!(crc32_pieces(&[data]), crc32(data));
        for split in 0..=data.len() {
            let (a, b) = data.split_at(split);
            assert_eq!(crc32_pieces(&[a, b]), 0xCBF4_3926, "split at {split}");
        }
        assert_eq!(crc32_pieces(&[]), crc32(b""));
    }
}
