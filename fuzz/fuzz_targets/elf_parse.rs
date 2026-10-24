//! Fuzz the ELF loader's front half with arbitrary bytes.
//!
//! `crates/elf` is the only parser that loads what it parses: its (vaddr, memsz, flags, data) tuples
//! become page-table entries. Kani proves the leaf, `check_segment_bounds`, and the whole-parse
//! totality proof did not return (notes/verification.md). This reaches the unproved shell: the
//! header-count and table-size arithmetic, the entry-point check, the overlap loop, and the
//! `segments()` iterator. `EXPECTED_MACHINE` is a compile-time constant, so a host build accepts
//! the host's machine only; `fuzz/seeds/elf_parse/` holds one seed per machine, and
//! `crates/elf/tests/fuzz_seed.rs` fails if none matches (notes/fuzzing/elf-seeds.md).

#![no_main]

use elf::Elf;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The shell's streaming note reader runs on bytes nobody has parsed as a program yet, so it is
    // fuzzed before `parse` gets a chance to refuse them (milestone 597 (a program carries its
    // manifest in an ELF note), provisional).
    if let Ok(segs) = elf::NoteSegments::from_head(data) {
        let mut search = elf::NoteSearch::new(b"nife", 1);
        for seg in segs {
            let Ok(r) = seg.range(data.len()) else { break };
            if search.segment(&data[r], seg.p_align).is_err() {
                break;
            }
        }
    }
    let Ok(elf) = Elf::parse(data) else {
        return;
    };

    // Everything the kernel's loader does with a parsed ELF, in the order it does it. Validation
    // already ran inside `parse`; this is the walk that happens afterwards, when the loader is
    // mapping and is past the point where refusing is easy.
    let _ = elf.entry();

    // The note lookup the progenitor makes for a program's manifest (milestone 597, provisional):
    // every `PT_NOTE` walked in full, on the same hostile bytes the loader accepted.
    let _ = elf.note(b"nife", 1);
    for seg in elf.segments() {
        let _ = seg.is_readable();
        let _ = seg.is_writable();
        let _ = seg.is_executable();
        // Both page sizes the tree can boot with. 4 KiB is what both ISAs use today; 16 KiB is on
        // the aarch64 roadmap and the arithmetic differs (`div_ceil` then `saturating_mul`), so
        // fuzzing only the one we ship would prove the less interesting half.
        let _ = seg.page_range(4096);
        let _ = seg.page_range(16384);
        // The tail the loader must zero: `memsz` beyond `data.len()` is `.bss`. Recomputed here
        // because a segment whose `memsz` is under its `filesz` would make this wrap, and the check
        // that forbids it is in `check_segment_bounds` rather than in this type.
        let _ = seg.memsz - seg.data.len() as u64;
    }
});
