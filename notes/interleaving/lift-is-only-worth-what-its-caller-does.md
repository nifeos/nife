# A lift is only worth what its caller does

*Name: ratified 2026-10-18 (calef, "Yes") Refused `lift-and-caller`.*

An appendix to [notes/interleaving.md](../interleaving.md), which summarizes it.

Every retrofit here has the same shape: take a protocol out of the code that runs it, put it in a
crate a model checker can reach, and have the original call the crate. The entire value is the
last clause. A model that searches code the kernel no longer runs reports success forever, on a
question nobody is asking. And there is no symptom: the harness count holds, the executions stay
green, and the parent note keeps saying the protocol is modeled.

Milestone 136 gated that for `crates/memory_regions`, and found the exposure was larger than it looked.

The gap is rebuildable from the public API alone. Before the gate, this compiled, with no edit
to `crates/memory_regions` whatsoever:

```rust
pub fn destroy(region: u64) {
    if REGIONS.lock().has_children(region) { return; }
    let Some((base, size)) = REGIONS.lock().bounds(region) else { return };
    crate::revoke::revoke_region(base, size);
    for i in 0..(size / FRAME_SIZE) {
        memory::free(PageFrame::from_addr(base + i * FRAME_SIZE));
    }
}
```

That is pull request #316's double free restored: read under one hold, release, revoke, free, never
remove the slot, so two callers both pass the read and both reach the loop. `has_children` and
`bounds` are public because single callers legitimately want them, and together they are enough. The
lesson generalizes past this crate: a lifted protocol's `&self` observers are the material a second
decision path is built from, because each answers a question about state while leaving the state
addressable.

`script/lint`'s *"the region claim protocol has one decision path"* check is what holds it now, in
two halves that are each insufficient alone:

| Half | Asserts | Catches |
|---|---|---|
| the surface pin | the public items of `crates/memory_regions/src/table.rs`, **with receivers**, are exactly a pinned set | a new observer; a method removed; `claim_for_destroy` respelled `&self`, which deletes the single-borrow argument while changing no name |
| the warrant pin | each function in `kernel/src/memory_region.rs` that reaches a free loop made its entitling call first (`create`/`insert_root`, `destroy`/`claim_for_destroy`) | the rebuilt gap above, and any third path that returns region pages |

Two `compile_fail` doctests on `DestroyClaim` cover what neither half can see, since a
`#[derive(Clone)]` and a `pub` on a field change no name and no call: the claim cannot be forged
from outside the crate, and cannot be duplicated. They carry explicit error codes
(`compile_fail,E0451`), because a bare `compile_fail` passes when the snippet fails for any reason
at all, including a typo, which is how a compile-fail test rots into an assertion nobody has watched
fail.

The Kani shim of milestone 113 (the proofs' unsafe code) is not the mechanism here, though 135's own
`BUGS` section proposed it. 113 built `helpers/kani-lint-shim/` so clippy could compile code written
against Kani's intrinsics; loom needs nothing of the sort, being an ordinary dependency behind
`[target.'cfg(loom)'.dependencies]`, so the benefit costs one flag this script already passes.
Making harness code visible to the linter was a real gap, already closed. It is simply a different
gap from *does the caller still call this*, and no amount of linting a model answers the second.

The four other loom crates have the same exposure and are not gated; see BUGS.
