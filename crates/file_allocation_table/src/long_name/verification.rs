//! **Machine-checked proofs (Kani) of the long-name writer**: the checksum is the specification's,
//! every run is laid out as the specification lays it, and every alias is a legal short name.
//! Behind `#[cfg(kani)]`, so only `cargo kani` (script/verify) compiles them. See
//! notes/verification.md.
//!
//! The tests in `long_name.rs` and `lib.rs` pin worked examples and the host's driver reads one
//! back; these quantify over the name, which is what catches an off-by-one in the padding or the
//! ordinals for a length nobody wrote a test for.

use super::*;

/// **`ChkSum` is the specification's loop, for every short name.** The specification writes it
/// as a shift right with `0x80` added on a set low bit; [`checksum`] rotates. The two agree on
/// all 2^88 names, so the rotate is not a cleverness a reader has to trust.
/// Falsification: replayable `crates/file_allocation_table/falsifications/long_name.verification.the_checksum_is_the_specifications_loop.patch`
#[kani::proof]
#[kani::unwind(12)]
fn the_checksum_is_the_specifications_loop() {
    let short: [u8; 11] = kani::any();
    let mut sum = 0u8;
    for b in short {
        sum = (if sum & 1 != 0 { 0x80u8 } else { 0 })
            .wrapping_add(sum >> 1)
            .wrapping_add(b);
    }
    assert_eq!(checksum(&short), sum);
}

/// **Every long name's run is well formed, for every name up to [`MAX_UNITS`] units.**
///
/// For any units and any length, the run has `ceil(len / 13)` entries. The first on disk carries
/// the highest ordinal with [`LAST_LONG_ENTRY`], each later one the next ordinal down to 1, and
/// every one carries [`ATTR_LONG_NAME`], a zero type, a zero cluster and the checksum it was given.
/// Read back in ordinal order, the units are the name, then one `0x0000` unless the name fills its
/// last entry, then `0xFFFF` to the end.
/// Falsification: replayable `crates/file_allocation_table/falsifications/long_name.verification.every_long_name_run_is_well_formed.patch`
#[kani::proof]
#[kani::unwind(21)]
fn every_long_name_run_is_well_formed() {
    let len: usize = kani::any();
    kani::assume((1..=MAX_UNITS).contains(&len));
    let name = LongName {
        units: kani::any(),
        len,
    };
    let checksum: u8 = kani::any();
    let entries = name.entries();
    assert!(entries * UNITS_PER_ENTRY >= len && (entries - 1) * UNITS_PER_ENTRY < len);
    assert!(entries <= 20);

    // One entry and one unit, both chosen by the solver, rather than every entry in a loop: the
    // property is per unit, and a loop over 260 of them is a formula the solver need not build.
    let index: usize = kani::any();
    kani::assume(index < entries);
    let mut out = [0u8; 32];
    name.entry(index, checksum, &mut out);
    let ordinal = entries - index;
    let flag = if index == 0 { LAST_LONG_ENTRY } else { 0 };
    assert_eq!(out[0], ordinal as u8 | flag);
    assert_eq!([out[11], out[12], out[13]], [ATTR_LONG_NAME, 0, checksum]);
    assert_eq!([out[26], out[27]], [0, 0]);

    let k: usize = kani::any();
    kani::assume(k < UNITS_PER_ENTRY);
    let position = (ordinal - 1) * UNITS_PER_ENTRY + k;
    let at = UNIT_OFFSETS[k];
    let unit = u16::from_le_bytes([out[at], out[at + 1]]);
    let expected = if position < len {
        name.units[position]
    } else if position == len {
        0
    } else {
        0xFFFF
    };
    assert_eq!(unit, expected);
}

/// The longest name the alias harness quantifies over. The alias reads at most six base units and
/// three extension units after skipping spaces and periods, so fourteen leaves room for several
/// skipped units ahead of each part and every branch is reached. Measured: 31 seconds on an M-series
/// Mac on 2026-10-22 (UTC), and the solver time grows with the length.
const ALIAS_UNITS: usize = 14;

/// **Every alias is a legal short name.** For every name up to [`ALIAS_UNITS`] units, the alias
/// is `None` or eleven bytes a short entry admits: a base of one to six non-space bytes, then
/// `~1`, space padding to eight, and an extension that is left-aligned with no space inside it.
/// Falsification: replayable `crates/file_allocation_table/falsifications/long_name.verification.every_alias_is_a_legal_short_name.patch`
#[kani::proof]
#[kani::unwind(16)]
fn every_alias_is_a_legal_short_name() {
    let len: usize = kani::any();
    kani::assume((1..=ALIAS_UNITS).contains(&len));
    let mut units = [0u16; MAX_UNITS];
    let chosen: [u16; ALIAS_UNITS] = kani::any();
    units[..ALIAS_UNITS].copy_from_slice(&chosen);
    let name = LongName { units, len };
    let Some(alias) = name.alias() else {
        return;
    };
    assert!(alias.iter().all(|b| is_short_byte(*b)));
    let tilde = alias[..8]
        .iter()
        .rposition(|b| *b == b'~')
        .expect("a numeric tail");
    assert!((1..=6).contains(&tilde));
    assert!(alias[..tilde].iter().all(|b| *b != b' '));
    assert_eq!(alias[tilde + 1], b'1');
    assert!(alias[tilde + 2..8].iter().all(|b| *b == b' '));
    let ext_end = alias[8..]
        .iter()
        .position(|b| *b == b' ')
        .map_or(11, |i| 8 + i);
    assert!(alias[ext_end..].iter().all(|b| *b == b' '));
}
