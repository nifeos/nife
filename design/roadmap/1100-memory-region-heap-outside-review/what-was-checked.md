# What was checked, and how

An appendix to [the milestone](../1100-memory-region-heap-outside-review.md).

Verified firsthand, on a fresh shallow clone at the head of `main`:

- `cargo clippy -p memory_region_heap -- -D warnings` for `aarch64-unknown-none-softfloat`,
  `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`, `cargo doc --no-deps` with warnings
  denied, and `cargo check` on the host: all clean on the pinned nightly.
- `script/crate-standalone --no-kani memory_region_heap`: 16 failures (names, manifest, files,
  citations; the run skipped the build because three dependencies sit outside the copy).
- `script/crate-review-packet --no-rustdoc memory_region_heap`: 289 lines in one file, six public
  items, 15 `unsafe` sites, no Kani harness, no README, three nife dependents.
- The template's own helpers over `src/lib.rs` with the manifest: `spelled_out` refuses one name
  (`va`). `question_shape` refuses one (`commit_all`). `unit_symbols` and `register_names` pass.
  `unsafe_scan` finds `unsafe` in shipped source (so the weekly fuzz opt-in would refuse, which is
  correct for this crate).
- `script/names memory_region_heap` and `helpers/name_release.py --draft`: the crate name and
  `initialize` are provisional; `DEFAULT_BASE`, `MemoryRegionHeap`, `commit_all`, `committed_bytes`
  and `new` carry no `Name:` block.
- The vendored `talc` source for every call the crate makes (`claim`, `extend`, `reserved`,
  `try_allocate`, `deallocate`, `try_realloc_in_place`, `min_first_heap_size`, `CHUNK_UNIT`,
  `required_chunk_size`). The kernel's `MAP` handler (`kernel/src/syscall/memory_region.rs`). Every
  caller in the tree and the std overlay's copy of the policy. The QEMU test
  `a_process_runs_alloc_collections_on_its_own_memory_region`.
- Upstream `SFBdragon/talc` pull request 57 through the GitHub API, and download figures for the
  prior-art crates through the crates.io API.

Inferred from reading, not run: the behavior of the template's `coverage` job on a crate whose
library has `test = false` (item 4). Also inferred: the page cost of a refused `MAP` at an
already-mapped address (the kernel's own comment states it; item 7). The same goes for the size of
talc's first heap on a 64-bit target (computed by hand from `DefaultBinning`; item 9) and for
whether the user-mode panic handler allocates (item 11). Each carries an unverified mark where it
appears.

Not run: the QEMU suite, any mutation census (outside the window), Miri, Kani (there is no harness),
and the stranger test (there is no README).
