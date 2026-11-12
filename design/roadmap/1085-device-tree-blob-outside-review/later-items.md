# P2: later

An appendix to [the milestone](../1085-device-tree-blob-outside-review.md).

## P2: land when convenient

P2-1. `Error::Truncated` names five different faults. `src/lib.rs` lines 67 to 68 documents it as "an offset
in the header points outside the blob", and it is also returned for four other faults. They are a
node name or property name with no NUL (`:1220`) and an integer property of a width other than 4 or
8 (`:1161`, probe: a two-byte `linux,initrd-start` is `Err(Truncated)`). They also include a
property value past the blob (`:501`) and the offset checks (`:170`, `:189`). libfdt separates
`TRUNCATED`, `BADOFFSET`, `BADSTRUCTURE`, `BADNCELLS` and `BADVALUE`; a caller printing the error at
boot deserves the same. Add `BadValue` (width or terminator) and `BadStructure` (a token or block
past its end, after P1-4), and implement `core::fmt::Display` and `core::error::Error` for `Error`,
which a stranger expects of a public error type. Mark the enum `#[non_exhaustive]` before the first
release, since adding a variant later is otherwise a breaking change. Effort S. Two-way now; the
enum's shape is one-way once published.

P2-2. A cell count above two reads silently as its low 64 bits. `cells` (`src/lib.rs:
1209-1215`) shifts a `u64` left by 32 for each cell and keeps whatever remains. Probe: a bus
declaring `#address-cells = 3` with `reg = <1 2 3 4>` yields `start 0x2_0000_0003, size 4`, and no
error. Linux's `of_read_number` does the same; libfdt refuses any count above `FDT_MAX_NCELLS` (4)
as `FDT_ERR_BADNCELLS`. A `Region` holds 64 bits, so a nonzero cell above the second is a value the
type cannot represent. The stricter reading is to refuse a count above four with the new error of
P2-1 and to refuse a pair whose dropped high cells are nonzero. A PCI child node, the one real user
of three address cells, is already read raw through `node_prop`. Effort S. Two-way.

P2-3. `memory_regions` keys on the node name and counts a disabled node. `src/lib.rs` line 269
matches `memory` or `memory@`; nothing reads `device_type` or `status`. Probe: a `memory@0` with
`status = "disabled"` counts beside an enabled `memory@1`. Linux reads only root children with
`device_type = "memory"`, skips any `status` other than `okay`, and prefers
`linux,usable-memory` over `reg` when present. notes/device-tree.md records the name-only choice
as a known limit. Honor `status` (a disabled bank is RAM the firmware says not to use), accept
`device_type = "memory"` beside the name, and say in the documentation that `linux,usable-memory`
is not read. Effort S. Two-way; a change on the boot path, so run the fixture suites and a QEMU
boot after it.

P2-4. The source writes one token loop nine times. `memory_regions`, `node_reg`, `node_reg_compatible`,
`prop_compatible_up`, `node_prop`, `node_prop_inherited`, `phandle_prop`, `node_props`,
`reserved_memory_regions`, `initial_ramdisk` and `boot_arguments` each open the same `loop { be32;
match token { BEGIN_NODE => cstr, align4, depth += 1 ... } }` (`src/lib.rs` line 255, `:339`, `:444`,
`:567`, `:644`, `:711`, `:786`, `:880`, `:942`, `:1031`, `:1101`). The one guard that a token
advanced the cursor is a `debug_assert!` in `boot_arguments` alone (`:1148`). The 29 timeouts in the
mutation baseline are, by shape, the mutants that stall a cursor in one of these loops. Each is a
verdict, and each is also a loop that nothing but the test harness's clock catches. A private
`tokens()` iterator yielding `BeginNode(name)`, `EndNode`, `Property { name, value }` and `End`,
with the advance guard and the P1-4 bound in one place, would carry every walker, retire several
hundred lines, and give Kani one function to prove. libfdt's `fdt_next_tag` and `dtoolkit`'s
`Fdt::events` are this shape. Effort M. Two-way.

P2-5. The proofs cover the leaves and not the arithmetic above them. `src/lib.rs` lines 1238 to 1295
proves `be32` and `be64` total, `be32` big-endian when in bounds, and `align4` correct below `1 <<
60`. Not proved: `decode_reg` writes at most `out.len()` regions and never panics for any `len` and
any cell counts. `cells` is total. `from_bytes` is total over an arbitrary 48-byte buffer. And,
after P2-4, the token iterator is total. Each is a bounded harness of the same size as the four that
exist, and each needs its falsification patch. The `align4` bound needs a statement against the real
ceiling (`MAXIMUM_TOTALSIZE` plus the header), not `1 << 60`. Effort M. Two-way.

P2-6. The reservation block gets fewer checks than `reg`. `reserved_regions`
(`src/lib.rs` lines 208 to 225) passes `start` and `size` out without the `RegionOverflow` check `decode_reg`
applies (`:1197`), so a reservation whose `start + size` wraps reaches the kernel's reserve list
while a `reg` pair of the same shape draws a refusal. And nothing checks `off_rsvmap` for the 8-byte
alignment the specification requires. Apply the same overflow refusal; document that alignment is
not checked because the reader is byte-wise and needs none. Effort S. Two-way.

P2-7. `from_ptr` reads 40 bytes while its contract speaks of 8. `src/lib.rs` lines 133 to 142 says "we trust
the first 8 bytes before we have validated anything" and then builds a 40-byte slice. The caller's
contract (a blob valid for `'a`) covers both, since a blob is at least 40 bytes. The comment and the
code should still say the same number: read 8, check the magic and the size, then build the rest.
Also say that `ptr` needs no alignment (every read is byte-wise), which the `fdt` crate
states and a stranger will ask. Effort S. Two-way.

P2-8. `from_bytes` does not say what it trims or ignores. `src/lib.rs` lines 159 to 162. A buffer longer
than `totalsize` gets accepted and trimmed (`tests/hostile.rs` lines 283 to 290 proves it, the documentation
does not say it). `MAXIMUM_TOTALSIZE` applies to `from_ptr` only (right, and unstated). `version`,
`size_dt_strings`, `size_dt_struct` and `boot_cpuid_phys` are not read (P1-4 reads one). One
paragraph closes it. Effort S. Two-way.

P2-9. One example, and it does not run. The only `# Examples` block is on `node_props`
(`src/lib.rs` lines 852 to 861), marked `no_run`. The two QEMU fixtures under `tests/fixtures/` are real
trees the documentation could `include_bytes!` into a running doctest on `from_bytes`,
`memory_regions` and `node_reg_compatible`, and the README's `## EXAMPLES` needs the same code.
Effort S. Two-way.

P2-10. Two depth counters. Four walkers count depth in `i32` and subtract bare (`src/lib.rs:
242`, `:937`, `:1027`, `:1097`); seven count in `usize` with `saturating_sub`. Probe: three
`FDT_END_NODE` tokens before the root leave every walker answering `Ok(0)` or `Ok(None)` rather
than an error, which notes/device-tree.md records as a wart. P2-4 removes the duplication; until
then one type and one rule (refuse an unbalanced tree as `BadStructure`, as libfdt does). Effort
S. Two-way.

P2-11. `MAXIMUM_DEPTH = 16` is five local constants, and two lookups have no such limit.
`src/lib.rs` line 331, `:433`, `:559`, `:701`, `:778`. `node_prop` and `node_props` keep no stack and
answer at any depth, so a node at depth 17 exists for two lookups and not for five (recorded in the
note's BUGS). Linux's `FDT_MAX_DEPTH` is 64. One crate-level constant, raised to 64 (the stacks are
64 entries of at most 24 bytes each, on the caller's stack), documented once at the type. Effort S.
Two-way.

P2-12. A trailing partial `reg` pair drops silently. `decode_reg` (`src/lib.rs` line 1183)
stops when fewer than `pair_bytes` remain. Linux refuses a `reg` whose length is not a multiple
of the pair (`of_flat_dt_get_addr_size_prop`); `dtoolkit` refuses too; this crate's documentation
says nothing. State it at `memory_regions`, `node_reg` and `node_reg_compatible`, or refuse with
the P2-1 error. Effort S. Two-way.

P2-13. Private names that need a decoder. `acells`, `scells` (`src/lib.rs` lines 332 to 333 and
after), `nm` (`:573`, `:650`, `:717`, `:792`, `:886`), `pname` (`:606`, `:823`), `rd` (`:984`),
`dest` (`:997`), `int` (`:1157`), `cstr` (`:1218`), `off_struct`, `off_strings`, `off_rsvmap`
(`:127-129`). None is public, so none is the architect's; the template's `abbreviations.py`
reports them. Effort S. Two-way.

P2-14. `#![cfg_attr(not(test), no_std)]` where `#![no_std]` would do. `src/lib.rs` line 34. The
crate has no in-crate tests that need `std`; the integration tests link `std` on their own. Plain
`#![no_std]` matches what a stranger expects to read and removes a configuration that gets tests only one way. Effort S. Two-way.
