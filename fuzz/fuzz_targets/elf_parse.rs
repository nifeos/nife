//! Fuzz the ELF loader's front half with arbitrary bytes.
//!
//! The parser is the one a loader trusts to turn a file into (virtual address, size, flags, data)
//! tuples that become page-table entries. Kani proves the per-segment bounds check, and the
//! whole-parse totality proof did not return. This reaches the unproved shell: the header-count and
//! table-size arithmetic, the entry-point check, the overlap loop and the `segments()` iterator.
//! `Elf::parse` accepts the build's own machine only, so a host build accepts the host's machine;
//! `tests/fixtures/` holds one seed per machine, and `tests/fuzz_seed.rs` fails if none matches.

#![no_main]

use libfuzzer_sys::fuzz_target;
use static_elf::{Elf, PageBytes};

/// The note the target looks up. Any owner and type would do: the walk refuses a malformed or
/// duplicated note whatever it asks for, so the search exercises every segment either way.
const NOTE_OWNER: &[u8] = b"GNU";
const NOTE_TYPE: u32 = 1;

/// The larger of the two page sizes the walk rounds segments to.
const SIXTEEN_KIBIBYTES: PageBytes = PageBytes::new(16384).unwrap();

fuzz_target!(|data: &[u8]| {
    // The shell's streaming note reader runs on bytes nobody has parsed as a program yet, so it is
    // fuzzed before `parse` gets a chance to refuse them.
    if let Ok(segs) = static_elf::NoteSegments::from_head(data) {
        let mut search = static_elf::NoteSearch::new(NOTE_OWNER, NOTE_TYPE);
        for seg in segs {
            let Ok(r) = seg.range(data.len()) else { break };
            if search.segment(&data[r], seg.alignment).is_err() {
                break;
            }
        }
    }
    let Ok(elf) = Elf::parse(data) else {
        return;
    };

    // Everything a loader does with a parsed ELF, in the order it does it. Validation
    // already ran inside `parse`; this is the walk that happens afterwards, when the loader is
    // mapping and is past the point where refusing is easy.
    let _ = elf.entry();

    // The note lookup a loader makes for a program's manifest: every `PT_NOTE` walked in full, on
    // the same hostile bytes the loader accepted.
    let _ = elf.note(NOTE_OWNER, NOTE_TYPE);
    for seg in elf.segments() {
        let _ = seg.is_readable();
        let _ = seg.is_writable();
        let _ = seg.is_executable();
        // Two common page sizes. 4 KiB is the usual one; 16 KiB is used on some
        // the aarch64 roadmap and the arithmetic differs (`div_ceil` then `saturating_mul`), so
        // fuzzing only the one we ship would prove the less interesting half.
        let _ = seg.page_range(PageBytes::FOUR_KIBIBYTES);
        let _ = seg.page_range(SIXTEEN_KIBIBYTES);
        // The tail the loader must zero: `memsz` beyond `data.len()` is `.bss`. Recomputed here
        // because a segment whose `memsz` is under its `filesz` would make this wrap, and the check
        // that forbids it is in `check_segment_bounds` rather than in this type.
        let _ = seg.memory_bytes - seg.data.len() as u64;
    }
});
