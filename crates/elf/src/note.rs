//! Finding one ELF note by owner and type, through the program headers alone.
//!
//! Milestone 597 (a program carries its manifest in an ELF note), DECISIONS §197 (a package is one
//! archive file)'s option M2, ruled by calef on 2026-09-26 (UTC). Every public name here is
//! provisional; calef names public items.
//!
//! Nothing here runs inside [`Elf::parse`], and the kernel never calls [`Elf::note`]. A whole-image
//! reader uses [`Elf::note`]; a streaming reader uses [`NoteSegments::from_head`] and feeds each
//! segment to one [`NoteSearch`]. Both refuse a malformed note and a duplicate the same way. See
//! notes/elf/parser.md ("Finding a note") for the format, the padding rule and the hostile cases.

use super::{
    EHDR_SIZE, ELFCLASS64, ELFDATA2LSB, Elf, MAGIC, MAX_PHNUM, PHDR_SIZE, u16le, u32le, u64le,
};

/// `p_type` for a segment of notes.
const PT_NOTE: u32 = 4;

/// The three header words.
const NHDR_SIZE: usize = 12;

/// Why a note lookup refused the file. Provisional name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteError {
    /// The first bytes are not a little-endian ELF64 header whose program header table lies inside
    /// them. [`Elf::note`] never answers this for a file [`Elf::parse`] accepted.
    NotReadable,
    /// A `PT_NOTE` whose file range is not inside the file.
    SegmentOutOfBounds,
    /// A `PT_NOTE` whose `p_align` is not 0, 1, 4 or 8.
    BadAlignment,
    /// A note header, name or descriptor (with its padding) runs past the end of its segment.
    Truncated,
    /// The owner and type asked for appear more than once.
    Duplicate,
}

/// **Where one `PT_NOTE` lies in its file.** Provisional name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteSegment {
    /// `p_offset`: where the segment's bytes start in the file.
    pub offset: u64,
    /// `p_filesz`: how many bytes it has.
    pub size: u64,
    /// `p_align`, which says how the notes inside are padded.
    pub p_align: u64,
}

impl NoteSegment {
    /// The segment's bytes as a range of a file `file_len` long, or
    /// [`NoteError::SegmentOutOfBounds`] if any of it lies past the end.
    pub fn range(&self, file_len: usize) -> Result<core::ops::Range<usize>, NoteError> {
        let start = usize::try_from(self.offset).map_err(|_| NoteError::SegmentOutOfBounds)?;
        let size = usize::try_from(self.size).map_err(|_| NoteError::SegmentOutOfBounds)?;
        let end = start
            .checked_add(size)
            .ok_or(NoteError::SegmentOutOfBounds)?;
        if end > file_len {
            return Err(NoteError::SegmentOutOfBounds);
        }
        Ok(start..end)
    }
}

/// **Every `PT_NOTE` in a program header table**, read from a file's first bytes. Provisional
/// name.
///
/// It checks only what it needs to find the table: the magic, 64-bit, little-endian, an entry size
/// of at least 56 bytes, at most 64 entries, and a table that lies wholly inside `head`. It does not
/// check the machine or the file type, because it does not load anything; [`Elf::parse`] does that
/// for the bytes that are run.
pub struct NoteSegments<'a> {
    table: &'a [u8],
    phentsize: usize,
    phnum: usize,
    next: usize,
}

impl<'a> NoteSegments<'a> {
    /// Find the program header table in `head`, the first bytes of a file (all of it, or as much
    /// as the caller holds).
    pub fn from_head(head: &'a [u8]) -> Result<Self, NoteError> {
        if head.len() < EHDR_SIZE
            || head[0..4] != MAGIC
            || head[4] != ELFCLASS64
            || head[5] != ELFDATA2LSB
        {
            return Err(NoteError::NotReadable);
        }
        let phoff = usize::try_from(u64le(head, 32)).map_err(|_| NoteError::NotReadable)?;
        let phentsize = u16le(head, 54) as usize;
        let phnum = u16le(head, 56) as usize;
        if phentsize < PHDR_SIZE || phnum > MAX_PHNUM {
            return Err(NoteError::NotReadable);
        }
        // `phnum <= 64` and `phentsize <= u16::MAX`, so the product cannot wrap; the sum can.
        let end = phoff
            .checked_add(phnum * phentsize)
            .ok_or(NoteError::NotReadable)?;
        if end > head.len() {
            return Err(NoteError::NotReadable);
        }
        Ok(Self {
            table: &head[phoff..end],
            phentsize,
            phnum,
            next: 0,
        })
    }
}

impl Iterator for NoteSegments<'_> {
    type Item = NoteSegment;

    fn next(&mut self) -> Option<NoteSegment> {
        while self.next < self.phnum {
            let ph = &self.table[self.next * self.phentsize..][..PHDR_SIZE];
            self.next += 1;
            if u32le(ph, 0) == PT_NOTE {
                return Some(NoteSegment {
                    offset: u64le(ph, 8),
                    size: u64le(ph, 32),
                    p_align: u64le(ph, 48),
                });
            }
        }
        None
    }
}

/// Where one note's pieces lie, as offsets into its segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Extent {
    name_start: usize,
    name_end: usize,
    desc_start: usize,
    desc_end: usize,
    /// Where the next note's header starts: this note's padded end.
    next: usize,
}

/// The arithmetic of one note, loopless, so it can be proved for every value a file can hold.
///
/// `at` is where the note's header starts in a segment of `len` bytes; the caller has already
/// checked `at + 12 <= len` and read `namesz` and `descsz`, which is why that check is repeated
/// here: the proof must not lean on the caller.
fn note_extent(
    len: usize,
    at: usize,
    namesz: u32,
    descsz: u32,
    align: usize,
) -> Result<Extent, NoteError> {
    let name_start = at.checked_add(NHDR_SIZE).ok_or(NoteError::Truncated)?;
    if name_start > len {
        return Err(NoteError::Truncated);
    }
    let name_end = name_start
        .checked_add(namesz as usize)
        .ok_or(NoteError::Truncated)?;
    let desc_start = align_up(name_end, align).ok_or(NoteError::Truncated)?;
    let desc_end = desc_start
        .checked_add(descsz as usize)
        .ok_or(NoteError::Truncated)?;
    let next = align_up(desc_end, align).ok_or(NoteError::Truncated)?;
    if next > len {
        return Err(NoteError::Truncated);
    }
    Ok(Extent {
        name_start,
        name_end,
        desc_start,
        desc_end,
        next,
    })
}

/// `x` rounded up to a multiple of `align`, which is 4 or 8. `None` if that wraps.
fn align_up(x: usize, align: usize) -> Option<usize> {
    let mask = align - 1;
    Some(x.checked_add(mask)? & !mask)
}

/// `p_align` to note padding, by LLVM's `Elf_Note_Iterator` rule: the gABI says 8-byte padding for
/// ELF64, toolchains write 4 except under `p_align` 8. See notes/elf/parser.md.
fn padding(p_align: u64) -> Result<usize, NoteError> {
    match p_align {
        0 | 1 | 4 => Ok(4),
        8 => Ok(8),
        _ => Err(NoteError::BadAlignment),
    }
}

/// Walk one note segment and return the descriptor of the note owned by `owner` (compared with its
/// NUL) with type `kind`, refusing a malformed segment or a second match. `seen` says a match was
/// already found in an earlier segment, so a duplicate across segments is caught too.
fn find_in_segment<'a>(
    seg: &'a [u8],
    align: usize,
    owner: &[u8],
    kind: u32,
    mut seen: bool,
) -> Result<Option<&'a [u8]>, NoteError> {
    let mut found = None;
    let mut at = 0;
    while at < seg.len() {
        if seg.len() - at < NHDR_SIZE {
            return Err(NoteError::Truncated);
        }
        let namesz = u32le(seg, at);
        let descsz = u32le(seg, at + 4);
        let ntype = u32le(seg, at + 8);
        let e = note_extent(seg.len(), at, namesz, descsz, align)?;

        let name = &seg[e.name_start..e.name_end];
        if ntype == kind
            && name.len() == owner.len() + 1
            && name[name.len() - 1] == 0
            && &name[..owner.len()] == owner
        {
            if seen {
                return Err(NoteError::Duplicate);
            }
            seen = true;
            found = Some(&seg[e.desc_start..e.desc_end]);
        }
        at = e.next;
    }
    Ok(found)
}

/// **One lookup, fed a segment at a time.** Provisional name.
///
/// What [`Elf::note`] does over a whole image, for a reader that holds one segment at a time (the
/// shell). Every segment must be fed, in table order, for "the same note twice" to be caught; a
/// caller that stops at the first match has read a different rule.
pub struct NoteSearch<'o> {
    owner: &'o [u8],
    kind: u32,
    seen: bool,
}

impl<'o> NoteSearch<'o> {
    /// A search for the note owned by `owner` (without its NUL, `b"nife"`) with type `kind`.
    pub fn new(owner: &'o [u8], kind: u32) -> Self {
        Self {
            owner,
            kind,
            seen: false,
        }
    }

    /// Walk one whole segment, whose header said `p_align`. The descriptor if this segment holds
    /// the note, `None` if it does not, and a refusal if the segment is malformed or holds a second
    /// copy of a note already found, here or in an earlier segment.
    pub fn segment<'s>(
        &mut self,
        seg: &'s [u8],
        p_align: u64,
    ) -> Result<Option<&'s [u8]>, NoteError> {
        let found = find_in_segment(seg, padding(p_align)?, self.owner, self.kind, self.seen)?;
        self.seen |= found.is_some();
        Ok(found)
    }
}

impl<'a> Elf<'a> {
    /// **The descriptor of the one note owned by `owner` with type `kind`**, or `None` if the file
    /// has no such note. Provisional name.
    ///
    /// `owner` is the name without its NUL (`b"nife"`). Every `PT_NOTE` segment is walked in full,
    /// so a malformed note anywhere refuses the lookup, and so does a second copy of the one asked
    /// for; notes/elf/parser.md says why both are refusals rather than tolerances.
    pub fn note(&self, owner: &[u8], kind: u32) -> Result<Option<&'a [u8]>, NoteError> {
        let bytes: &'a [u8] = self.bytes;
        let mut search = NoteSearch::new(owner, kind);
        let mut found = None;
        for seg in NoteSegments::from_head(bytes)? {
            let range = seg.range(bytes.len())?;
            if let Some(desc) = search.segment(&bytes[range], seg.p_align)? {
                found = Some(desc);
            }
        }
        Ok(found)
    }
}

#[cfg(kani)]
mod verification {
    use super::*;

    /// **The extent arithmetic never panics**, for any segment length, cursor, sizes and either
    /// padding. The fields come straight from the file.
    ///
    /// Falsification: replayable `crates/elf/falsifications/note.verification.note_extent_never_panics.patch`
    #[kani::proof]
    fn note_extent_never_panics() {
        let align: usize = if kani::any() { 4 } else { 8 };
        let _ = note_extent(kani::any(), kani::any(), kani::any(), kani::any(), align);
    }

    /// **A passing extent is ordered, inside the segment, and moves the cursor forward.** The
    /// ordering is what makes the two slices in `find_in_segment` in bounds; `at < next` is what
    /// makes the walk end.
    ///
    /// Falsification: replayable `crates/elf/falsifications/note.verification.a_passing_extent_is_inside_and_advances.patch`
    #[kani::proof]
    fn a_passing_extent_is_inside_and_advances() {
        let len: usize = kani::any();
        let at: usize = kani::any();
        let align: usize = if kani::any() { 4 } else { 8 };
        if let Ok(e) = note_extent(len, at, kani::any(), kani::any(), align) {
            assert!(at < e.name_start);
            assert!(e.name_start <= e.name_end);
            assert!(e.name_end <= e.desc_start);
            assert!(e.desc_start <= e.desc_end);
            assert!(e.desc_end <= e.next);
            assert!(e.next <= len);
            assert!(e.next - at >= NHDR_SIZE);
        }
    }

    /// **Finding the note headers in a file's first bytes never panics, and every segment it
    /// yields was read from inside them.** The head is hostile (the shell reads it from a file
    /// anyone may have written), and `phoff` places the table at a symbolic position. 176 bytes is
    /// the header plus two program headers, so at most three entries of the smallest size fit; the
    /// unwind bound covers that and the four-byte magic comparison.
    ///
    /// Falsification: replayable `crates/elf/falsifications/note.verification.note_segments_from_a_head_never_panic.patch`
    #[kani::proof]
    #[kani::unwind(5)]
    fn note_segments_from_a_head_never_panic() {
        const LEN: usize = EHDR_SIZE + 2 * PHDR_SIZE;
        let head: [u8; LEN] = kani::any();
        let cut: usize = kani::any();
        kani::assume(cut <= LEN);
        if let Ok(segs) = NoteSegments::from_head(&head[..cut]) {
            for seg in segs {
                let _ = seg.range(kani::any());
            }
        }
    }

    /// **A note segment the solver chose is refused or read entirely inside itself**, walked by
    /// the real loop. 40 bytes holds at most three notes, so the loop runs at most three times and
    /// the owner comparison at most five bytes; the unwind bound covers both.
    ///
    /// Falsification: replayable `crates/elf/falsifications/note.verification.a_note_segment_is_read_inside_itself.patch`
    #[kani::proof]
    #[kani::unwind(7)]
    fn a_note_segment_is_read_inside_itself() {
        const LEN: usize = 40;
        let seg: [u8; LEN] = kani::any();
        let align: usize = if kani::any() { 4 } else { 8 };
        if let Ok(Some(desc)) = find_in_segment(&seg, align, b"nife", kani::any(), false) {
            let start = desc.as_ptr() as usize - seg.as_ptr() as usize;
            assert!(start + desc.len() <= LEN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NATIVE_MACHINE, PF_R, PF_X};
    extern crate std;
    use std::vec;
    use std::vec::Vec;

    /// One note, 4-byte padded.
    fn note(owner: &[u8], kind: u32, desc: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&((owner.len() + 1) as u32).to_le_bytes());
        v.extend_from_slice(&(desc.len() as u32).to_le_bytes());
        v.extend_from_slice(&kind.to_le_bytes());
        v.extend_from_slice(owner);
        v.push(0);
        while v.len() % 4 != 0 {
            v.push(0);
        }
        v.extend_from_slice(desc);
        while v.len() % 4 != 0 {
            v.push(0);
        }
        v
    }

    /// A one-`PT_LOAD` ELF plus one `PT_NOTE` per entry of `notes`, each `(p_align, bytes)`.
    fn image(notes: &[(u64, Vec<u8>)]) -> Vec<u8> {
        let phnum = 1 + notes.len();
        let mut body_off = 64 + phnum * 56;
        let mut v = vec![0u8; 64];
        v[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
        v[4] = 2;
        v[5] = 1;
        v[6] = 1;
        v[16..18].copy_from_slice(&2u16.to_le_bytes());
        v[18..20].copy_from_slice(&NATIVE_MACHINE.to_le_bytes());
        v[24..32].copy_from_slice(&0x40_0000u64.to_le_bytes());
        v[32..40].copy_from_slice(&64u64.to_le_bytes());
        v[54..56].copy_from_slice(&56u16.to_le_bytes());
        v[56..58].copy_from_slice(&(phnum as u16).to_le_bytes());

        let code = [0xaau8; 16];
        let mut ph = vec![0u8; 56];
        ph[0..4].copy_from_slice(&1u32.to_le_bytes());
        ph[4..8].copy_from_slice(&(PF_R | PF_X).to_le_bytes());
        ph[8..16].copy_from_slice(&(body_off as u64).to_le_bytes());
        ph[16..24].copy_from_slice(&0x40_0000u64.to_le_bytes());
        ph[24..32].copy_from_slice(&0x40_0000u64.to_le_bytes());
        ph[32..40].copy_from_slice(&16u64.to_le_bytes());
        ph[40..48].copy_from_slice(&16u64.to_le_bytes());
        v.extend_from_slice(&ph);
        body_off += code.len();

        let mut body = code.to_vec();
        for (align, bytes) in notes {
            let mut ph = vec![0u8; 56];
            ph[0..4].copy_from_slice(&PT_NOTE.to_le_bytes());
            ph[4..8].copy_from_slice(&PF_R.to_le_bytes());
            ph[8..16].copy_from_slice(&(body_off as u64).to_le_bytes());
            ph[32..40].copy_from_slice(&(bytes.len() as u64).to_le_bytes());
            ph[40..48].copy_from_slice(&(bytes.len() as u64).to_le_bytes());
            ph[48..56].copy_from_slice(&align.to_le_bytes());
            v.extend_from_slice(&ph);
            body.extend_from_slice(bytes);
            body_off += bytes.len();
        }
        v.extend_from_slice(&body);
        v
    }

    #[test]
    fn a_note_is_found_among_others() {
        let mut seg = note(b"GNU", 3, &[1; 20]);
        seg.extend(note(b"nife", 1, b"manifest bytes"));
        seg.extend(note(b"nife", 2, b"other type"));
        let bytes = image(&[(4, seg)]);
        let elf = Elf::parse(&bytes).expect("a PT_NOTE does not disturb the loader");
        assert_eq!(elf.note(b"nife", 1), Ok(Some(&b"manifest bytes"[..])));
        assert_eq!(elf.note(b"nife", 9), Ok(None));
        assert_eq!(elf.segments().count(), 1, "PT_NOTE is not a load segment");
    }

    /// The shell's path: the headers from the first bytes, each segment read on its own, one
    /// search across them. It must answer what [`Elf::note`] answers, including the refusal.
    fn streamed(bytes: &[u8]) -> Result<Option<Vec<u8>>, NoteError> {
        let mut search = NoteSearch::new(b"nife", 1);
        let mut found = None;
        for seg in NoteSegments::from_head(bytes)? {
            let r = seg.range(bytes.len())?;
            let copy = bytes[r].to_vec();
            if let Some(d) = search.segment(&copy, seg.p_align)? {
                found = Some(d.to_vec());
            }
        }
        Ok(found)
    }

    #[test]
    fn the_streaming_reader_agrees_with_the_whole_image_reader() {
        let cases = [
            image(&[(4, note(b"nife", 1, b"manifest bytes"))]),
            image(&[(4, note(b"GNU", 3, b"x"))]),
            image(&[(4, note(b"nife", 1, b"a")), (4, note(b"nife", 1, b"b"))]),
            image(&[(16, note(b"nife", 1, b"a"))]),
            image(&[(4, note(b"GNU", 3, b"x")), (4, note(b"nife", 1, b"second"))]),
        ];
        for bytes in cases {
            let whole = Elf::parse(&bytes)
                .unwrap()
                .note(b"nife", 1)
                .map(|d| d.map(|d| d.to_vec()));
            assert_eq!(streamed(&bytes), whole);
        }
    }

    #[test]
    fn a_head_that_does_not_hold_the_header_table_is_not_readable() {
        let bytes = image(&[(4, note(b"nife", 1, b"a"))]);
        let table_end = 64 + 2 * 56;
        assert!(NoteSegments::from_head(&bytes[..table_end]).is_ok());
        assert_eq!(
            NoteSegments::from_head(&bytes[..table_end - 1]).err(),
            Some(NoteError::NotReadable)
        );
        let mut not_elf = bytes.clone();
        not_elf[0] = 0;
        assert_eq!(
            NoteSegments::from_head(&not_elf).err(),
            Some(NoteError::NotReadable)
        );
    }

    #[test]
    fn an_owner_that_is_a_prefix_is_not_a_match() {
        let bytes = image(&[(4, note(b"nife0", 1, b"x"))]);
        assert_eq!(Elf::parse(&bytes).unwrap().note(b"nife", 1), Ok(None));
    }

    #[test]
    fn the_same_note_twice_is_refused_within_and_across_segments() {
        let mut seg = note(b"nife", 1, b"a");
        seg.extend(note(b"nife", 1, b"b"));
        let bytes = image(&[(4, seg)]);
        assert_eq!(
            Elf::parse(&bytes).unwrap().note(b"nife", 1),
            Err(NoteError::Duplicate)
        );

        let bytes = image(&[(4, note(b"nife", 1, b"a")), (4, note(b"nife", 1, b"b"))]);
        assert_eq!(
            Elf::parse(&bytes).unwrap().note(b"nife", 1),
            Err(NoteError::Duplicate)
        );
    }

    #[test]
    fn a_descriptor_that_runs_past_its_segment_is_refused() {
        let mut seg = note(b"nife", 1, b"abcd");
        seg[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let bytes = image(&[(4, seg)]);
        assert_eq!(
            Elf::parse(&bytes).unwrap().note(b"nife", 1),
            Err(NoteError::Truncated)
        );
    }

    #[test]
    fn a_missing_final_pad_is_refused() {
        let mut seg = note(b"nife", 1, b"abc"); // descriptor 3 bytes, padded to 4
        seg.pop();
        let bytes = image(&[(4, seg)]);
        assert_eq!(
            Elf::parse(&bytes).unwrap().note(b"nife", 1),
            Err(NoteError::Truncated)
        );
    }

    #[test]
    fn a_header_cut_short_is_refused() {
        let mut seg = note(b"nife", 1, b"abcd");
        seg.extend_from_slice(&[0; 8]);
        let bytes = image(&[(4, seg)]);
        assert_eq!(
            Elf::parse(&bytes).unwrap().note(b"nife", 1),
            Err(NoteError::Truncated)
        );
    }

    #[test]
    fn eight_byte_padding_follows_p_align() {
        // `.note.gnu.property`'s padding. With "GNU\0" (namesz 4) the name ends on 16, which is
        // 8-aligned already, so the rule is invisible; a 5-byte owner shows the difference.
        let mut seg = Vec::new();
        seg.extend_from_slice(&5u32.to_le_bytes());
        seg.extend_from_slice(&3u32.to_le_bytes());
        seg.extend_from_slice(&1u32.to_le_bytes());
        seg.extend_from_slice(b"nife\0\0\0\0\0\0\0\0"); // 12 + 5 pads to 24 under 8, to 20 under 4
        seg.extend_from_slice(b"abc\0\0\0\0\0");
        let bytes = image(&[(8, seg.clone())]);
        assert_eq!(
            Elf::parse(&bytes).unwrap().note(b"nife", 1),
            Ok(Some(&b"abc"[..]))
        );
        // Read with 4-byte padding, the same bytes put the descriptor at 20 and read the padding.
        let bytes = image(&[(4, seg)]);
        assert_ne!(
            Elf::parse(&bytes).unwrap().note(b"nife", 1),
            Ok(Some(&b"abc"[..]))
        );
    }

    #[test]
    fn an_alignment_that_is_not_four_or_eight_is_refused() {
        let bytes = image(&[(16, note(b"nife", 1, b"a"))]);
        assert_eq!(
            Elf::parse(&bytes).unwrap().note(b"nife", 1),
            Err(NoteError::BadAlignment)
        );
    }

    #[test]
    fn a_note_segment_outside_the_file_is_refused() {
        let mut bytes = image(&[(4, note(b"nife", 1, b"a"))]);
        let ph = 64 + 56;
        bytes[ph + 8..ph + 16].copy_from_slice(&(u64::MAX - 2).to_le_bytes());
        assert_eq!(
            Elf::parse(&bytes).unwrap().note(b"nife", 1),
            Err(NoteError::SegmentOutOfBounds)
        );
    }

    #[test]
    fn an_empty_note_segment_has_no_note() {
        let bytes = image(&[(4, Vec::new())]);
        assert_eq!(Elf::parse(&bytes).unwrap().note(b"nife", 1), Ok(None));
    }

    /// A bare ELF header (64 bytes) that claims `phnum` program headers of `phentsize` bytes at
    /// offset 64, followed by `tail` zero bytes. Mutation-survivor triage helper; provisional name.
    fn head_with_table(class: u8, data: u8, phentsize: u16, phnum: u16, tail: usize) -> Vec<u8> {
        let mut v = vec![0u8; 64 + tail];
        v[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
        v[4] = class;
        v[5] = data;
        v[32..40].copy_from_slice(&64u64.to_le_bytes());
        v[54..56].copy_from_slice(&phentsize.to_le_bytes());
        v[56..58].copy_from_slice(&phnum.to_le_bytes());
        v
    }

    /// Each of the three identity checks refuses on its own: a head wrong in exactly one of magic,
    /// class or data encoding, with a table that would otherwise fit, is not readable.
    #[test]
    fn each_identity_byte_is_checked_on_its_own() {
        assert!(NoteSegments::from_head(&head_with_table(2, 1, 56, 1, 56)).is_ok());
        assert_eq!(
            NoteSegments::from_head(&head_with_table(1, 1, 56, 1, 56)).err(),
            Some(NoteError::NotReadable),
            "32-bit class"
        );
        assert_eq!(
            NoteSegments::from_head(&head_with_table(2, 2, 56, 1, 56)).err(),
            Some(NoteError::NotReadable),
            "big-endian data"
        );
        let mut bad_magic = head_with_table(2, 1, 56, 1, 56);
        bad_magic[3] = b'G';
        assert_eq!(
            NoteSegments::from_head(&bad_magic).err(),
            Some(NoteError::NotReadable)
        );
    }

    /// The header is 64 bytes: 64 is enough for an empty table, every shorter prefix is refused
    /// without reading past its end.
    #[test]
    fn a_head_is_readable_from_exactly_one_header_long() {
        let full = head_with_table(2, 1, 56, 0, 0);
        assert_eq!(NoteSegments::from_head(&full).unwrap().count(), 0);
        for cut in 0..64 {
            assert_eq!(
                NoteSegments::from_head(&full[..cut]).err(),
                Some(NoteError::NotReadable),
                "{cut} bytes"
            );
        }
    }

    /// Entry size 56 is the least that holds a program header; 55 is refused even when the table
    /// would fit, and a larger entry size is fine.
    #[test]
    fn a_program_header_entry_smaller_than_a_header_is_refused() {
        assert_eq!(
            NoteSegments::from_head(&head_with_table(2, 1, 55, 1, 200)).err(),
            Some(NoteError::NotReadable)
        );
        assert!(NoteSegments::from_head(&head_with_table(2, 1, 56, 1, 200)).is_ok());
        assert!(NoteSegments::from_head(&head_with_table(2, 1, 64, 1, 200)).is_ok());
    }

    /// 64 entries is the most read; 65 is refused even when the head is long enough to hold them.
    #[test]
    fn at_most_sixty_four_program_headers_are_read() {
        assert_eq!(
            NoteSegments::from_head(&head_with_table(2, 1, 56, 64, 64 * 56))
                .unwrap()
                .count(),
            0,
            "64 all-zero entries are none of them PT_NOTE"
        );
        assert_eq!(
            NoteSegments::from_head(&head_with_table(2, 1, 56, 65, 65 * 56)).err(),
            Some(NoteError::NotReadable)
        );
    }

    /// A note with no name and no descriptor is exactly its 12-byte header, and a segment holding
    /// just that is whole, not truncated.
    #[test]
    fn a_bare_note_header_filling_the_segment_is_not_truncated() {
        assert!(note_extent(12, 0, 0, 0, 4).is_ok());
        assert_eq!(
            note_extent(11, 0, 0, 0, 4),
            Err(NoteError::Truncated),
            "one byte short of the header"
        );
        let bytes = image(&[(4, vec![0u8; 12])]);
        assert_eq!(Elf::parse(&bytes).unwrap().note(b"nife", 1), Ok(None));
    }

    #[test]
    fn an_empty_descriptor_is_found_and_empty() {
        let bytes = image(&[(4, note(b"nife", 1, b""))]);
        assert_eq!(
            Elf::parse(&bytes).unwrap().note(b"nife", 1),
            Ok(Some(&b""[..]))
        );
    }
}
