//! **The parse**: the ELF64 header and program-header table, checked before anything is loaded,
//! and the little-endian reads it is built from.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// `\x7fELF`.
pub(crate) const MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

pub(crate) const ELFCLASS64: u8 = 2;

pub(crate) const ELFDATA2LSB: u8 = 1;

pub(crate) const EV_CURRENT: u8 = 1;

/// `e_type`. We accept only this one: a **static executable**, loaded where it says.
pub(crate) const ET_EXEC: u16 = 2;

/// `e_type` for a PIE / shared object. Needs relocation, which we do not do.
pub(crate) const ET_DYN: u16 = 3;

/// 64 bytes of ELF64 header, then program headers of 56 bytes each.
pub(crate) const EHDR_SIZE: usize = 64;

pub(crate) const PHDR_SIZE: usize = 56;

/// The most program headers a file may have. Bounds the header count before the O(n^2) overlap
/// check: a legitimate static executable has a few `PT_LOAD` segments, and 65535 headers exist only
/// to make validation stall. The note reader walks the same bound.
pub(crate) const MAX_PHNUM: usize = 64;

/// A parsed, **fully validated** ELF64 executable.
///
/// Everything is checked in [`Elf::parse`], not lazily: a loader that validates as it maps has
/// mapped half a bad program by the time it finds out.
///
/// # Examples
///
/// ```
/// use elf::{Elf, Error, FOREIGN_MACHINES, NATIVE_MACHINE, PF_R, PF_W, PF_X};
/// fn image(flags: u32, entry: u64) -> Vec<u8> {
///     const EHDR: usize = 64; const PHDR: usize = 56;
///     let mut v = vec![0u8; EHDR + PHDR];
///     v[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
///     v[4] = 2; v[5] = 1; v[6] = 1; // ELFCLASS64, ELFDATA2LSB, EV_CURRENT
///     v[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
///     v[18..20].copy_from_slice(&NATIVE_MACHINE.to_le_bytes());
///     v[24..32].copy_from_slice(&entry.to_le_bytes()); // e_entry
///     v[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes()); // e_phoff
///     v[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes()); // e_phentsize
///     v[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum
///     let p = EHDR;
///     v[p..p + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
///     v[p + 4..p + 8].copy_from_slice(&flags.to_le_bytes());
///     v[p + 8..p + 16].copy_from_slice(&((EHDR + PHDR) as u64).to_le_bytes()); // p_offset
///     v[p + 16..p + 24].copy_from_slice(&0x40_0000u64.to_le_bytes()); // p_vaddr
///     v[p + 24..p + 32].copy_from_slice(&0x40_0000u64.to_le_bytes()); // p_paddr, equal here
///     v[p + 40..p + 48].copy_from_slice(&4096u64.to_le_bytes()); // p_memsz; p_filesz stays 0
///     v
/// }
///
/// let bytes = image(PF_R | PF_X, 0x40_0000);
/// let elf = Elf::parse(&bytes).unwrap();
/// assert_eq!(elf.entry(), 0x40_0000);
/// let seg = elf.segments().next().unwrap();
/// assert!(seg.is_readable() && seg.is_executable() && !seg.is_writable());
/// assert_eq!(seg.page_range(4096), (0x40_0000, 0x40_1000));
///
/// // `paddr` equals `vaddr` here; a loader wants `vaddr` (see `Segment::paddr`).
/// assert_eq!(seg.paddr, seg.vaddr);
/// ```
pub struct Elf<'a> {
    pub(crate) bytes: &'a [u8],
    entry: u64,
    phoff: usize,
    phnum: usize,
    phentsize: usize,
}

impl<'a> Elf<'a> {
    /// Validate the whole file up front: header, program header table bounds, and every segment's
    /// bounds, permissions and address arithmetic. A `Ok(Elf)` has nothing left to check; every
    /// later accessor and [`segments`](Self::segments) iteration step trusts this pass completely.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() < EHDR_SIZE {
            return Err(Error::TooSmall);
        }

        if bytes[0..4] != MAGIC {
            return Err(Error::BadMagic);
        }
        if bytes[4] != ELFCLASS64 {
            return Err(Error::Not64Bit);
        }
        if bytes[5] != ELFDATA2LSB {
            return Err(Error::NotLittleEndian);
        }
        if bytes[6] != EV_CURRENT {
            return Err(Error::BadVersion);
        }

        let e_type = u16le(bytes, 16);
        let e_machine = u16le(bytes, 18);

        if e_machine != EXPECTED_MACHINE {
            return Err(Error::WrongMachine);
        }
        match e_type {
            ET_EXEC => {}
            ET_DYN => return Err(Error::NeedsRelocation),
            _ => return Err(Error::NotExecutable),
        }

        let entry = u64le(bytes, 24);
        let phoff = u64le(bytes, 32) as usize;
        let phentsize = u16le(bytes, 54) as usize;
        let phnum = u16le(bytes, 56) as usize;

        if phentsize < PHDR_SIZE {
            return Err(Error::BadProgramHeaders);
        }
        if phnum > MAX_PHNUM {
            return Err(Error::TooManyProgramHeaders);
        }

        // The bounds check on the program header table itself. `phoff` and `phnum` come out of
        // the file, so they are hostile input, and `phoff + phnum * phentsize` is exactly the
        // kind of arithmetic that wraps.
        let table_len = phnum
            .checked_mul(phentsize)
            .ok_or(Error::BadProgramHeaders)?;
        let table_end = phoff
            .checked_add(table_len)
            .ok_or(Error::BadProgramHeaders)?;
        if table_end > bytes.len() {
            return Err(Error::BadProgramHeaders);
        }

        let elf = Elf {
            bytes,
            entry,
            phoff,
            phnum,
            phentsize,
        };

        elf.validate()?;
        Ok(elf)
    }

    /// Every check, before the caller maps a single page.
    fn validate(&self) -> Result<(), Error> {
        let mut entry_ok = false;

        for i in 0..self.phnum {
            let Some(seg) = self.segment_at(i)? else {
                continue;
            };

            if seg.is_writable() && seg.is_executable() {
                return Err(Error::WritableAndExecutable);
            }
            if !seg.is_readable() && !seg.is_executable() {
                return Err(Error::SegmentUnreachable);
            }

            if seg.is_executable()
                && (self.entry >= seg.vaddr && self.entry < seg.vaddr + seg.memsz)
            {
                entry_ok = true;
            }

            // No two segments may claim the same page. See `Error::SegmentsOverlap`.
            for j in 0..i {
                if let Some(other) = self.segment_at(j)? {
                    let (a0, a1) = seg.page_range(4096);
                    let (b0, b1) = other.page_range(4096);
                    if a0 < b1 && b0 < a1 {
                        return Err(Error::SegmentsOverlap);
                    }
                }
            }
        }

        if !entry_ok {
            return Err(Error::EntryNotExecutable);
        }
        Ok(())
    }

    /// The `i`th program header, if it is a `PT_LOAD`.
    fn segment_at(&self, i: usize) -> Result<Option<Segment<'a>>, Error> {
        let off = self.phoff + i * self.phentsize;
        let ph = &self.bytes[off..off + PHDR_SIZE];

        if u32le(ph, 0) != PT_LOAD {
            return Ok(None);
        }

        let flags = u32le(ph, 4);
        let p_offset = u64le(ph, 8) as usize;
        let vaddr = u64le(ph, 16);
        let paddr = u64le(ph, 24);
        let filesz = u64le(ph, 32) as usize;
        let memsz = u64le(ph, 40);

        // Every check on the hostile fields; the slice below relies on its `end`.
        let end = check_segment_bounds(self.bytes.len(), p_offset, filesz, vaddr, memsz)?;

        Ok(Some(Segment {
            vaddr,
            paddr,
            memsz,
            flags,
            data: &self.bytes[p_offset..end],
        }))
    }

    /// Where execution begins. Validated to be inside an executable segment.
    pub fn entry(&self) -> u64 {
        self.entry
    }

    /// The segments to map, in file order.
    pub fn segments(&self) -> impl Iterator<Item = Segment<'a>> + '_ {
        (0..self.phnum).filter_map(|i| self.segment_at(i).ok().flatten())
    }
}

/// The per-segment bounds and overflow checks, as pure loopless arithmetic over a program header's
/// raw fields and the file length, so Kani proves it panic-free and in bounds for every input.
///
/// Returns the exclusive end offset of the segment's file data, so `[p_offset, end)` lies within a
/// `file_len`-byte file, or the error the fields earn: `SegmentTruncated`, `SegmentOutOfBounds`,
/// then `AddressOverflow`, in that order. See notes/elf/parser.md and
/// notes/verification/elf-parser-wall.md.
pub(crate) fn check_segment_bounds(
    file_len: usize,
    p_offset: usize,
    filesz: usize,
    vaddr: u64,
    memsz: u64,
) -> Result<usize, Error> {
    // The segment cannot claim less memory than it has bytes on disk.
    if (memsz as usize) < filesz {
        return Err(Error::SegmentTruncated);
    }

    // The file range must lie within the file. `p_offset` and `filesz` are hostile, so the add is
    // checked rather than trusted.
    let end = p_offset
        .checked_add(filesz)
        .ok_or(Error::SegmentOutOfBounds)?;
    if end > file_len {
        return Err(Error::SegmentOutOfBounds);
    }

    // The virtual range must not overflow: `vaddr + memsz` feeds the entry check and `page_range`,
    // and a near-`u64::MAX` memsz would wrap them, a PANIC under the dev profile's overflow checks,
    // i.e. a crafted binary halting the kernel.
    vaddr.checked_add(memsz).ok_or(Error::AddressOverflow)?;

    Ok(end)
}

pub(crate) fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

pub(crate) fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

pub(crate) fn u64le(b: &[u8], at: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(v)
}
