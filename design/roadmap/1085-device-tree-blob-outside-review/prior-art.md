# Prior art

An appendix to [the milestone](../1085-device-tree-blob-outside-review.md).

How the crate sits against the readers a stranger already knows. Figures for `dtoolkit` are the
tree's own comparison; the review read the rest from each project's source or documentation.

| reader | validates up front | lookup surface | cell counts | size ceiling | allocation |
|---|---|---|---|---|---|
| libfdt (dtc) | `fdt_check_header`; `fdt_check_full` walks the structure block, refuses misnesting and a property after a subnode | offset-based: `fdt_next_node`, `fdt_node_offset_by_compatible`, `fdt_node_offset_by_phandle`, `fdt_get_mem_rsv`, `fdt_getprop` | `fdt_address_cells` defaults 2, `fdt_size_cells` 1; a count above `FDT_MAX_NCELLS` (4) or malformed is `FDT_ERR_BADNCELLS` | none; `totalsize` trusted after the header check | none |
| Linux `drivers/of/fdt.c` | `fdt_check_header` at `early_init_dt_verify` | `early_init_dt_scan_memory` reads only the root's children with `device_type = "memory"`, skips `status` other than `okay`, prefers `linux,usable-memory` over `reg`, drops zero sizes, clamps a block past the top of the address space | root's `#address-cells`/`#size-cells`, a `WARN` when absent; `of_read_number` keeps the low 64 bits of a wider value | per-item caps (`FDT_MAX_DEPTH` 64, command-line size); the 2 MiB `MAX_FDT_SIZE` is arm64's boot mapping, not this file's | none at boot |
| `fdt` crate 0.1.5 | header and magic at `Fdt::new`; `from_ptr` is `unsafe` | `memory()`, `chosen()`, `cpus()`, `find_node`, `find_compatible`, `find_phandle`, `all_nodes`, `memory_reservations`, `total_size` | parent's, by node | none | none, `no_std` |
| `dtoolkit` 0.4.1 | the whole blob at `Fdt::new`; refuses a property name of 32 characters or more, a property after a child, a bus with no declared cells reads the specification's defaults | root, children, properties, path, `memory()`, `reserved_memory()`, `chosen()`, `cpus()`; no search by `compatible` below one level, no phandle lookup; `children()` is quadratic in depth | specification defaults where undeclared | none in `from_raw`; a bounded constructor exists upstream | none for reading; `zerocopy` and two proc macros in the graph |
| this crate | magic, `totalsize`, `last_comp_version`, three block offsets; nothing in the structure block | eleven fixed-purpose lookups, each one flat walk of the token stream into a caller's slice, no iterator and no node handle | nearest declaring ancestor's, as Linux; a malformed declaration ignored | 2 MiB in `from_ptr` only | none, `no_std`, no dependency |

Where this crate differs: every query is one bounded linear pass with no state beyond a 16-slot
stack, so a 174,000-node chain costs about a millisecond where `dtoolkit`'s child iterator costs
minutes. It is the only one of the five that proves its leaf readers total and ships the patches
that break the proofs. And it reads what `dtc` and Linux read (long property names, a property after
a child). What it costs: no general traversal, so a caller with a twelfth question writes a twelfth
walker. A name-prefix lookup whose answer differs from its siblings' on the same shape (P1-1). And
an error type with one word for five different faults (P2-1). The concrete example is the `dtoolkit`
row of the tree's comparison: a kernel on `dtoolkit` booting a Rockchip or SpacemiT tree finds no
RAM because one property name is 32 characters long, and this crate reads it.
