//! **VFAT long names**: the short alias a long name still needs, the checksum that binds the two,
//! and the run of long-name entries that goes ahead of the short entry in a directory. Microsoft's
//! "FAT: General Overview of On-Disk Format" (version 1.03), section 7, is the format.
//!
//! A long name is up to 255 UCS-2 units, thirteen to an entry, written in reverse: the entry with
//! the name's end comes first on disk, its ordinal carrying [`LAST_LONG_ENTRY`]. A wrong checksum
//! fails silently, as a volume that lists `BOOTRI~1.EFI` and boots nothing; `verification.rs`
//! proves the checksum and the run's layout for every name. After the name's last unit comes one
//! `0x0000`, then `0xFFFF` to the end of the entry, unless the name fills its last entry exactly.
//!
//! Name: provisional, minted 2026-10-22 (UTC) by milestone/560-a-long-file-name-or-riscv64-cannot-be-installed,
//! for the thing the specification calls a long name. calef has not named it.

/// The most UCS-2 units a long name holds.
pub(crate) const MAX_UNITS: usize = 255;

/// UCS-2 units in one long-name entry: five in `LDIR_Name1`, six in `LDIR_Name2`, two in
/// `LDIR_Name3`.
pub(crate) const UNITS_PER_ENTRY: usize = 13;

/// `ATTR_LONG_NAME`: read-only, hidden, system and volume ID together, a combination no short
/// entry carries, which is how an old driver comes to skip a long-name entry.
pub(crate) const ATTR_LONG_NAME: u8 = 0x0F;

/// The bit in `LDIR_Ord` that marks the run's first entry on disk, which holds the name's end.
pub(crate) const LAST_LONG_ENTRY: u8 = 0x40;

/// Where each of an entry's thirteen units sits, in bytes from the entry's start. The gaps are
/// `LDIR_Ord` (0), `LDIR_Attr`, `LDIR_Type` and `LDIR_Chksum` (11 to 13), and `LDIR_FstClusLO` (26
/// and 27), which is zero so that a driver following it reaches nothing.
pub(crate) const UNIT_OFFSETS: [usize; UNITS_PER_ENTRY] =
    [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];

/// Whether `b` is punctuation a short name admits beside upper-case letters and digits. A
/// `matches!` rather than a scan of a byte string, so the alias harness's formula has no loop here.
const fn is_short_punctuation(b: u8) -> bool {
    matches!(
        b,
        b'$' | b'%'
            | b'\''
            | b'-'
            | b'_'
            | b'@'
            | b'~'
            | b'`'
            | b'!'
            | b'('
            | b')'
            | b'{'
            | b'}'
            | b'^'
            | b'#'
            | b'&'
    )
}

/// **A long name, checked and held as UCS-2 units.** Only [`LongName::from_units`] makes one, so
/// a value of this type is a name the specification admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LongName {
    units: [u16; MAX_UNITS],
    len: usize,
}

impl LongName {
    /// **Check `name` and encode it**, or `None` for a name no long-name entry may carry.
    ///
    /// A character outside the Basic Multilingual Plane is refused rather than written as a
    /// surrogate pair, because the specification says UCS-2 and not UTF-16.
    pub(crate) fn new(name: &str) -> Option<LongName> {
        let mut units = [0u16; MAX_UNITS];
        let mut len = 0;
        for c in name.chars() {
            let unit = u16::try_from(u32::from(c)).ok()?;
            *units.get_mut(len)? = unit;
            len += 1;
        }
        LongName::from_units(&units[..len])
    }

    /// **Check `units` as a long name**, or `None`.
    ///
    /// Refused: an empty name, one over [`MAX_UNITS`], a control character, any of `" * / : < > ?
    /// \ |`, and a name ending in a space or a period. The specification has a driver strip those
    /// last two, so writing one would make a file whose name is not the name asked for. Also
    /// refused is a name whose alias would have nothing before its dot (see [`LongName::alias`]).
    pub(crate) fn from_units(units: &[u16]) -> Option<LongName> {
        if units.is_empty() || units.len() > MAX_UNITS {
            return None;
        }
        let forbidden = |u: u16| u < 0x20 || b"\"*/:<>?\\|".iter().any(|b| u == u16::from(*b));
        if units.iter().any(|u| forbidden(*u)) {
            return None;
        }
        if matches!(units.last(), Some(&0x20) | Some(&0x2E)) {
            return None;
        }
        let mut name = LongName {
            units: [0; MAX_UNITS],
            len: units.len(),
        };
        name.units[..units.len()].copy_from_slice(units);
        name.alias()?;
        Some(name)
    }

    /// How many long-name entries the name takes: one per thirteen units, rounded up.
    pub(crate) const fn entries(&self) -> usize {
        self.len.div_ceil(UNITS_PER_ENTRY)
    }

    /// **The short alias written after the run**, as the eleven bytes of a short entry, or `None`
    /// when nothing would come before the dot.
    ///
    /// The specification's basis-name steps with a fixed numeric tail: drop leading periods and
    /// every space, upper-case ASCII, write `_` for any unit a short name cannot carry, keep six
    /// units before the last period and three after, and end the base with `~1`. The tail is
    /// always `~1` because the volume is created holding this one file, so no other alias can
    /// collide with it.
    pub(crate) fn alias(&self) -> Option<[u8; 11]> {
        let units = &self.units[..self.len];
        let start = units.iter().position(|u| *u != 0x2E).unwrap_or(units.len());
        let units = &units[start..];
        let dot = units.iter().rposition(|u| *u == 0x2E);
        let (base, ext) = match dot {
            Some(i) => (&units[..i], &units[i + 1..]),
            None => (units, &[][..]),
        };
        let mut out = [b' '; 11];
        let base_len = fold(base, &mut out[..6]);
        if base_len == 0 {
            return None;
        }
        out[base_len..base_len + 2].copy_from_slice(b"~1");
        fold(ext, &mut out[8..]);
        Some(out)
    }

    /// **Fill `out` with the long-name entry `index` places into the run**, counting from the
    /// first entry on disk, and stamp it with `checksum`. `index` must be below
    /// [`LongName::entries`] and `out` 32 bytes long.
    pub(crate) fn entry(&self, index: usize, checksum: u8, out: &mut [u8]) {
        assert!(index < self.entries() && out.len() == 32);
        let ordinal = self.entries() - index;
        out[0] = ordinal as u8 | if index == 0 { LAST_LONG_ENTRY } else { 0 };
        out[11] = ATTR_LONG_NAME;
        out[12] = 0; // LDIR_Type: zero for a name component
        out[13] = checksum;
        out[26..28].fill(0); // LDIR_FstClusLO
        let first = (ordinal - 1) * UNITS_PER_ENTRY;
        for (k, at) in UNIT_OFFSETS.iter().enumerate() {
            let unit = self.unit_or_padding(first + k);
            out[*at..*at + 2].copy_from_slice(&unit.to_le_bytes());
        }
    }

    /// The unit at `position` of the name as stored: the name, one terminating `0x0000`, then
    /// `0xFFFF` padding.
    const fn unit_or_padding(&self, position: usize) -> u16 {
        if position < self.len {
            self.units[position]
        } else if position == self.len {
            0x0000
        } else {
            0xFFFF
        }
    }
}

/// Fold `units` into short-name bytes in `out`, skipping spaces and periods and stopping when
/// `out` is full; returns how many bytes it wrote.
fn fold(units: &[u16], out: &mut [u8]) -> usize {
    let mut written = 0;
    for unit in units {
        if written == out.len() {
            break;
        }
        if *unit == 0x20 || *unit == 0x2E {
            continue;
        }
        out[written] = short_byte(*unit);
        written += 1;
    }
    written
}

/// The short-name byte for one long-name unit: ASCII folded to upper case, and `_` for anything a
/// short name does not admit (`+ , ; = [ ]` and every unit above ASCII).
fn short_byte(unit: u16) -> u8 {
    let Ok(b) = u8::try_from(unit) else {
        return b'_';
    };
    let b = b.to_ascii_uppercase();
    if b.is_ascii_uppercase() || b.is_ascii_digit() || is_short_punctuation(b) {
        b
    } else {
        b'_'
    }
}

/// **`ChkSum`, the specification's checksum of an eleven-byte short name**: rotate the running
/// byte right by one, then add the next name byte, wrapping.
pub(crate) fn checksum(short: &[u8; 11]) -> u8 {
    short
        .iter()
        .fold(0u8, |sum, b| sum.rotate_right(1).wrapping_add(*b))
}

/// Whether `b` may appear in a short name at all, padding space included.
pub(crate) fn is_short_byte(b: u8) -> bool {
    b == b' ' || b.is_ascii_uppercase() || b.is_ascii_digit() || is_short_punctuation(b)
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> LongName {
        LongName::new(s).unwrap_or_else(|| panic!("{s:?} is a long name"))
    }

    /// The checksum of `BOOTRI~1EFI`, worked by hand with the specification's own loop, which
    /// shifts and adds `0x80` on a set low bit rather than rotating.
    #[test]
    fn the_checksum_of_the_riscv64_alias_is_the_specifications() {
        let mut sum = 0u8;
        for b in b"BOOTRI~1EFI" {
            sum = (if sum & 1 != 0 { 0x80u8 } else { 0 })
                .wrapping_add(sum >> 1)
                .wrapping_add(*b);
        }
        assert_eq!(checksum(b"BOOTRI~1EFI"), sum);
        // A changed byte anywhere changes it, so the checksum covers the whole name.
        assert_ne!(checksum(b"BOOTRI~1EFI"), checksum(b"BOOTRI~2EFI"));
        assert_ne!(checksum(b"BOOTRI~1EFI"), checksum(b"COOTRI~1EFI"));
    }

    #[test]
    fn aliases_follow_the_basis_name_steps() {
        assert_eq!(name("BOOTRISCV64.EFI").alias(), Some(*b"BOOTRI~1EFI"));
        assert_eq!(name("bootx64.efi").alias(), Some(*b"BOOTX6~1EFI"));
        assert_eq!(name("a b.c d").alias(), Some(*b"AB~1    CD "));
        assert_eq!(name("..hidden").alias(), Some(*b"HIDDEN~1   "));
        assert_eq!(name("x+y,z.tar.gz").alias(), Some(*b"X_Y_ZT~1GZ "));
        assert_eq!(name("é.txt").alias(), Some(*b"_~1     TXT"));
        assert_eq!(name("abc.longext").alias(), Some(*b"ABC~1   LON"));
    }

    #[test]
    fn a_name_no_entry_may_carry_is_refused() {
        for bad in [
            "",
            "a:b",
            "a*b",
            "a\u{7}b",
            "trailing.",
            "trailing ",
            " .efi",
            "🦀.efi",
        ] {
            assert_eq!(LongName::new(bad), None, "{bad:?}");
        }
        assert!(LongName::new(&"a".repeat(255)).is_some());
        assert_eq!(LongName::new(&"a".repeat(256)), None);
        // `from_units` holds the length limit itself, since a caller with units skips `new`.
        assert_eq!(
            LongName::from_units(&[u16::from(b'a'); MAX_UNITS + 1]),
            None
        );
    }

    /// `BOOTRISCV64.EFI` is fifteen units, so two entries: the first on disk holds units 13 and 14,
    /// the terminating zero and ten units of padding.
    #[test]
    fn the_riscv64_name_takes_two_entries_with_the_end_marked_and_padded() {
        let n = name("BOOTRISCV64.EFI");
        assert_eq!(n.entries(), 2);
        let mut first = [0u8; 32];
        n.entry(0, 0xA5, &mut first);
        assert_eq!(first[0], 0x42);
        assert_eq!(&first[1..7], &[b'F', 0, b'I', 0, 0, 0]);
        assert_eq!(&first[7..11], &[0xFF; 4]);
        assert_eq!([first[11], first[12], first[13]], [0x0F, 0, 0xA5]);
        assert!(first[14..26].iter().all(|b| *b == 0xFF));
        assert_eq!(&first[26..28], &[0, 0]);
        assert!(first[28..32].iter().all(|b| *b == 0xFF));

        let mut second = [0u8; 32];
        n.entry(1, 0xA5, &mut second);
        assert_eq!(second[0], 0x01);
        assert_eq!(&second[1..11], b"B\0O\0O\0T\0R\0");
        assert_eq!(&second[14..26], b"I\0S\0C\0V\x006\x004\0");
        assert_eq!(&second[28..32], b".\0E\0");
    }

    /// A name that fills its last entry exactly carries no terminating zero and no padding.
    #[test]
    fn a_name_of_exactly_thirteen_units_has_no_terminator() {
        let n = name("ABCDEFGHIJKLM");
        assert_eq!(n.entries(), 1);
        let mut e = [0u8; 32];
        n.entry(0, 0, &mut e);
        assert_eq!(e[0], 0x41);
        assert_eq!(&e[30..32], b"M\0");
    }

    #[test]
    fn short_bytes_fold_case_and_replace_what_a_short_name_refuses() {
        assert_eq!(short_byte(u16::from(b'a')), b'A');
        assert_eq!(short_byte(u16::from(b'~')), b'~');
        assert_eq!(short_byte(u16::from(b'+')), b'_');
        assert_eq!(short_byte(0x00E9), b'_');
        assert!(is_short_byte(b' ') && is_short_byte(b'~') && !is_short_byte(b'+'));
        assert!(!is_short_byte(b'a') && !is_short_byte(b'.'));
    }
}
