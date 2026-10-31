//! **Machine-checked proof (Kani) that the run finder is first-fit**, over every sequence of up to
//! six slots and every run length up to three. Behind `#[cfg(kani)]`, so only `cargo kani`
//! compiles it.
//!
//! The finder is a small state machine, so the sequence length is the bound Kani needs to unroll;
//! the writer feeds it one slot per directory slot and the oracle tests cross real directories.
//!
//! Name: provisional.

use super::*;

fn any_kind() -> SlotKind {
    match kani::any::<u8>() % 3 {
        0 => SlotKind::Used,
        1 => SlotKind::Free,
        _ => SlotKind::End,
    }
}

/// **The run finder returns the first run of free slots, and only a run of free slots.** For any
/// sequence of used, free and end slots and any run length: when it returns a start, nothing
/// from there to the slot that completed it is used, the run is `count` slots or ends at an end
/// marker, and it begins right after a used slot or at the start. When it returns none, no end marker
/// came and every window of `count` slots holds a used one.
/// Falsification: replayable `crates/file_allocation_table/falsifications/directory_write.verification.the_run_finder_returns_the_first_run_of_free_slots.patch`
#[kani::proof]
#[kani::unwind(8)]
fn the_run_finder_returns_the_first_run_of_free_slots() {
    let count: u32 = kani::any();
    kani::assume((1..=3).contains(&count));
    let mut finder = RunFinder::new(count);
    let kinds: [SlotKind; 6] = [
        any_kind(),
        any_kind(),
        any_kind(),
        any_kind(),
        any_kind(),
        any_kind(),
    ];
    let mut found = None;
    let mut at = 0;
    while at < kinds.len() {
        if let Some(start) = finder.push(kinds[at]) {
            found = Some((start, at as u32));
            break;
        }
        at += 1;
    }
    let Some((start, done)) = found else {
        // No run: no end marker came, and every window of `count` slots holds a used one.
        assert!(!kinds.contains(&SlotKind::End));
        let mut first = 0;
        while first + count as usize <= kinds.len() {
            assert!(kinds[first..first + count as usize].contains(&SlotKind::Used));
            first += 1;
        }
        return;
    };
    assert!(start <= done);
    let mut i = start as usize;
    while i <= done as usize {
        assert_ne!(kinds[i], SlotKind::Used);
        i += 1;
    }
    if kinds[done as usize] == SlotKind::End {
        assert!(done - start < count);
    } else {
        assert_eq!(done - start + 1, count);
    }
    // First fit: the run starts right after a used slot, or at the directory's start.
    if start > 0 {
        assert_eq!(kinds[start as usize - 1], SlotKind::Used);
    }
}
