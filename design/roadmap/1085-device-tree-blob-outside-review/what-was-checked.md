# What was checked, and how

An appendix to [the milestone](../1085-device-tree-blob-outside-review.md).

Verified firsthand, on a fresh shallow clone at the head of `main`, with the pinned nightly:

- `cargo test -p device_tree_blob`: the hostile suite (32 tests), the two QEMU fixture suites (17
  and 10 tests) and one documentation test pass. `cargo clippy --all-targets` and `cargo doc` with
  warnings denied are clean.
- A probe crate outside the tree, depending on the crate by path, drove hand-built blobs through
  `node_reg`, `node_prop`, `reserved_memory_regions`, `memory_regions`, `initial_ramdisk` and
  `boot_arguments`. Its results are the evidence cited at P1-1, P1-2, P1-4, P2-2, P2-3 and P2-10.
- The review ran the template's helpers over the crate as `check / standalone` and the naming gates
  would run them. `standalone.py` found twelve problems, listed at P0-1 and P0-2. `spelled_out.py`
  found 28, `british_spelling.py` one, and `unit_symbols.py`, `question_shape.py`,
  `function_length.py` and `file_length.py` none. `unsafe_scan.py` found three uses of `unsafe`, so the opt-in draws a refusal. `abbreviations.py` reported 37 components (report only).
- The four Kani harnesses and their four falsification patches, read; the patches' form matches
  what `falsify.py --check` wants. Not replayed.
- The mutation baseline in `.cargo/mutants-baseline.txt`: 389 caught, 1 missed, 29 timed out, 25
  unviable. No census ran. The identity of the missed mutant stays unverified.
- Prior art: Linux `drivers/of/fdt.c` (`early_init_dt_scan_memory`,
  `of_fdt_device_is_available`), libfdt's `libfdt.h` (`FDT_MAX_NCELLS`, the error codes,
  `fdt_check_full`) and the `fdt` crate's documentation, each fetched and read. The tree's own
  comparison of this crate against `dtoolkit` got a read, not a rerun.

Inferred from reading, not run: the behavior on a target whose `usize` is 32 bits wide (P1-3). This host has no such target. Not run: a mutation census (outside its window), the fuzz target, a
Kani run, a build for the three bare-metal targets, and any step of the release path.
