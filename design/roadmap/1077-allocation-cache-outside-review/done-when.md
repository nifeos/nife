# Done when

An appendix to [the milestone](../1077-allocation-cache-outside-review.md).

- `standalone.py --manifest-dir .` from the template at its pinned revision exits 0 on the crate as staged by `script/crate-move --dry-run`. It reports no `--allow-unpublished` finding other than the
  `publish = false` line itself.
- The crate's documentation and manifest name no consumer, no tree path, no milestone and no person. `grep -n -i 'nife\|std\|xtask\|notes/' src/lib.rs Cargo.toml README.md` prints nothing
  but the `#![no_std]` attribute and the word `std` where it names the Rust library in general.
- A workspace member of nife depends on the crate by `git` and `rev`. `cargo tree --locked -i allocation_cache` prints that source. `cargo xtask std-src` exits 0 with the member list no
  longer naming `crates/allocation_cache`.
- `class(usize::MAX, 8)` is `None` in release and debug, with a test. `class_bytes` cannot overflow
  on any value its argument type admits.
- A test reads an all-zero `Cache` as empty, and `Cache`'s documentation says so.
- `Cache`'s documentation states the owner's three obligations, and `put`, `spill` and `drain`
  point to it.
- The architect has ruled Decisions 1 to 3. `script/names allocation_cache` prints every name ratified before the move. The renames land in one pull request with the consumer's `use`
  sites.
- Three Kani harnesses with falsification patches verify, and `proofs / falsify` replays each patch red. Alternatively, the block records that the proofs wait, and the move lane has seen the `proofs` workflow
  green on an empty harness set (item 11).
- `const _: () = assert!(align_of::<Free>() <= GRID_BYTES)` sits beside the size assertion.
- `cargo test`, `cargo clippy --all-targets -D warnings`, `cargo doc` with broken links denied,
  Miri on the unit tests, and `cargo mutants` with none missed are green at the commit the move cites. They run on stable and on 1.88, for the host and the three bare-metal targets.
