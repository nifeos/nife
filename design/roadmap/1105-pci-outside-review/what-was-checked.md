# What was checked, and how

An appendix to [the milestone](../1105-pci-outside-review.md).

Verified firsthand, on the clone, with every cargo run under a 12 GiB memory cap:

- `cargo test -p pci`: 29 unit tests, 5 fixture tests, 2 documentation tests, all green.
- `cargo build -p pci` for `riscv64imac-unknown-none-elf`, `aarch64-unknown-none-softfloat` and
  `x86_64-unknown-none`; `cargo clippy -p pci --all-targets -- -D warnings`; `cargo doc -p pci
  --no-deps` with `RUSTDOCFLAGS=-D warnings`. All clean on stable 1.99.1.
- All eight falsification patches apply to a fresh copy of the crate with `patch -p3` (at an
  offset, since their hunk headers are stale) and with `git apply --check -p3`. Nobody replayed them
  under Kani.
- The template's gates, run from its head over the crate's sources. `spelled_out` found 11,
  `register_names` 9, `british_spelling` 1, `unit_symbols` 0, `question_shape` 0 and `standalone`
  13. `abbreviations` listed 83 components, report only. `falsify --check` found 8 records in a form
  the template does not read.
- `script/crate-move --dry-run --no-build pci` against the template at `v7`. The staged tree fails
  `readme`, `standalone` (10 lines), `falsification records` (one regenerated patch, which
  `git apply` refuses as corrupt), `citations` (31 lines in five classes) and `mentions nife`
  (5 lines, all in scaffold files).
- Every caller in the tree: `kernel/src/pci.rs` (five `read_bars` call sites, `walk`,
  `enumerate`, both capability walks, both MSI decoders, `intx_interrupt_number`,
  `bridge_buses`), `kernel/src/memory.rs` (`mem32_window`), the three interrupt controllers
  (`MsiTarget`) and `crates/driver_process_contract/src/virtual_input_output.rs`
  (`virtio_capabilities`, `msix_capability`, `MSIX_ENTRY_BYTES`).
- The recorded mutation figures: the baseline row in `.cargo/mutants-baseline.txt` (246 caught, 11
  missed, 8 timed out, 10 unviable) and the triage page that classifies the 11 as equivalent (`|`
  for `^` on disjoint operands in `mem32_window`, `read_bars`, `ecam_offset` and `requester_id`).
  This review ran no census.
- crates.io, through its API: another author holds `pci` with a 0.0.1 release, and the
  figures in the prior-art table.
- The stabilization versions of `<[T]>::as_chunks` (1.88) and `u64::is_multiple_of` (1.87), from
  the toolchain's own library source.

Inferred from reading, not run: the behavior of `read_bars` on a non-contiguous mask (item 3), the
swizzle behind a bridge (item 10) and the cause of the corrupt regenerated patch (item 2).

Not run: Kani, cargo-mutants, a fuzz run, the template's `standalone` build step, a release dry
run, and `script/names`.
