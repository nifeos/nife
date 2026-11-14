# Done when

An appendix to [the milestone](../1105-pci-outside-review.md).

- `crates/pci/Cargo.toml` has no `path` dependency, and no `include_bytes!` reaches outside the
  crate. The five witness tests run in nife against the constants they pin.
- `script/crate-move --dry-run pci` against the newest template release stages a tree on which
  `standalone`, `falsification records`, `citations`, `readme` and the naming checks all print `ok`.
  All eight regenerated patches pass `git apply --check -p1`.
- `read_bars` returns `None` for a non-contiguous mask. It also returns `None` for a 64-bit BAR
  whose upper half is not writable above the size, and for a 64-bit type in slot 5. It returns
  `[None; 6]` for a header that is not type 0. A proof shows that every `Some(Bar)` has a
  power-of-two `size_bytes` of at least 16 and that no probe touches an offset above `BAR0 + 20`. A
  falsification patch turns that proof red.
- Every public item has a doc comment that cites a specification, not this tree. The crate carries
  `#![forbid(unsafe_code)]` and no `allow(missing_docs)`.
- The architect has ratified one spelling for each name in items 5 and 6. The rename has landed in
  the crate, the kernel and `driver_process_contract` in one pull request.
- Nine harnesses (the eight plus `the_message_signaled_interrupt_walk_terminates_on_any_device`), each with a
  falsification patch the template's `falsify` replays red.
- The package name has a ruling and the repository, when created, carries it.
