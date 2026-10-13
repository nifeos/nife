# The `Untyped` rename, as built

An appendix to [milestone 158 (build the eleven kernel object and identifier
renames)](../0158-kernel-object-rename-build.md). It holds the lane's account of `Untyped` becoming
`MemoryRegion`, the last of the seven: its real surface, its scope call, the compositor check and
how it was verified.

## The surface

`Untyped` -> `MemoryRegion`, done to completion in a separate lane (milestone/158-untyped-rename).

The block's own table undersold it by roughly an order of magnitude on files touched. The strict
capitalized-token count (25 files, 60 occurrences) was the pre-rename estimate. The actual surface,
re-measured fresh, was 88 files, 620 insertions and 618 deletions. That is once the predicted-and-real
lowercase identifier family was counted the same way `Endpoint`'s and `Tcb`'s were.

The block had already flagged a crate as having "independently converged on 'region' vocabulary".
It turned out to be `crates/memory_regions`, not `crates/regions` as the block's own text said. That
crate had *already been renamed* by an earlier, unrecorded pass. So its own doc comments citing
`kernel/src/untyped.rs` and `crates/regions` were themselves stale, and they were fixed in the same
lane.

The real surface:

- an ABI module, `abi::untyped` -> `abi::memory_region`. This is the
  `MAP`/`RETYPE`/`RETYPE_OBJ`/`SPLIT`/`DESTROY` method-number namespace nearly every user program's
  syscall wrappers dispatch through;
- a `rank::UNTYPED` -> `rank::MEMORY_REGION` lock-rank constant (`kernel/src/sync.rs`). No
  `objtype::UNTYPED` exists, since untyped/region capabilities are not themselves a retype target;
- a kernel module file rename, `kernel/src/untyped.rs` -> `kernel/src/memory_region.rs`, and
  `kernel/src/user/untyped_service.rs` -> `kernel/src/user/memory_region_service.rs`;
- `untyped_cap` -> `memory_region_cap`, `untyped_cap_rights` -> `memory_region_cap_rights`, and
  `untyped_root_cap` -> `memory_region_root_cap` (`kernel/src/cap.rs`);
- `untyped_map`/`untyped_retype`/`untyped_retype_obj`/`untyped_split`/`untyped_destroy` -> their
  `memory_region_*` equivalents. These are in `kernel/src/syscall.rs`'s dispatch. They are also
  repeated local wrapper fns of the same names in `crates/system_initializer`,
  `crates/supervision_proto` (now `crates/supervision_protocol`), and several `user/src/*.rs`
  programs that each define their own tiny syscall wrapper;
- `UntypedHeap` -> `MemoryRegionHeap`, and `untyped_slot`/`UNTYPED_SLOT`/`NET_UNTYPED_SLOT` -> their
  `memory_region`-spelled equivalents (`crates/user_rt/src/heap.rs`, `crates/user_rt/src/lib.rs`,
  `patches/std-nife`'s std overlay; `crates/user_rt` is now `crates/user_mode_runtime`);
- two test names, `a_process_spends_untyped_and_the_kernel_never_allocates` and
  `a_process_runs_alloc_collections_on_its_own_untyped`.

## The scope call

It was scoped identically to `Endpoint`'s and `Tcb`'s own rung. Identifiers, module paths,
backtick-quoted doc-comment citations, and test names were renamed. Informal lowercase "untyped"
prose describing the concept in English was left alone throughout. There are hundreds of
occurrences, for example "spends its own untyped" and "an untyped budget". This matches how
lowercase "endpoint" and "tcb" stayed after those renames.

Three headings read as prose rather than identifier citation. They were not literally kept. They
were reworded to match their sibling variants' own `A`/`An` + noun heading pattern.
`kernel/src/cap.rs`'s `Object::MemoryRegion` doc heading became "A memory region". That matches
`Frame`'s "A physical page" and `Aspace`'s "An address space under construction", rather than
reading "MemoryRegion memory".

One historical citation in `crates/abi/src/lib.rs`'s own crate-naming rationale kept `Untyped`. It
names `Tcb`/`Aspace`/`Untyped` as the abbreviations a naming review "sank". The reason is the one
the `Tcb` lane kept it for: renaming it would make the sentence describe the winning name as the one
that lost.

`notes/untyped.md` became `notes/memory-regions.md`. It was rewritten throughout the same way
`notes/ipc-naming.md` was, with a provenance line naming the old identifier.

## The compositor collision, checked

§113 (eleven kernel object and identifier names move to the plain, standard term) named a compositor collision: `Untyped`/`Region` colliding with the compositor's own
damaged-screen-rectangle "region". It was checked directly. `crates/compositor/src/lib.rs` never
mentions `MemoryRegion`, so there is no adjacency for a reader to misread.

## Verified

The same four ways as `Endpoint` and `Tcb`:

- the full host test suite green, including doctests;
- both kernel targets build clean;
- `user`/`redoxfs_server`/`xtask` build clean;
- `crates/inter_process_communication` (6 harnesses) and `crates/capability` (12 harnesses) both
  re-verify under Kani post-rename.

Also `script/lint` clean, and full `script/test` green on both ISAs. The lint pass included two
`clippy::doc_markdown` fixes the rename itself triggered. `MEMORY_REGION` and `MemoryRegion` need
backticks where the old `UNTYPED`/`Untyped` spellings did not, since clippy's heuristic fires on
underscored and CamelCase tokens.
