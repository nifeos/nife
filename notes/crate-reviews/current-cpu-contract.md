# Review package: `current_cpu_contract`

*Name: provisional, minted by lane milestone/1084-review-package. The `current_cpu_contract`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the packages together
outgrow the page.*

The architect reads this to clear the crate for its move to its own repository, in step 3 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as `main` leaves it. The outside review behind it is
[milestone 1084 (`current_cpu_contract`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1084-current-cpu-contract-outside-review.md).
Every item of that review has landed, except two that the move itself does and one farm check in
nife.

## In one paragraph

The layout of a 32-byte read-only page that tells a thread which core it runs on, with no system
call. The kernel maps one such page per thread and writes the core's id into it at every
switch-in, on the destination core. The page also carries two words the kernel writes once, when
it builds the page: the thread's allowance (how many cores its process may use) and the
scheduler's tick rate. Every word reads as `None` when it is unknown, never as a fabricated zero.
The crate is `no_std`, depends on `address_space_layout` for its address, and has `unsafe` in two
entry points, because the writer and the reader each name a raw address. Two Kani harnesses prove
the readers and the builder, each with a falsification patch. Every public name stands as built
(ruled 2026-11-19 UTC). No file in the crate names nife, a path in this tree or a private machine.

## Identity

| item | value |
|---|---|
| crate | `current_cpu_contract`, ratified 2026-10-18 |
| version | `0.1.0`; no tag and no repository yet |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest |
| `publish` | `false`; the manifest comment says the line stays until the release that publishes |
| size | one `src/lib.rs` of 622 lines, a README of 91 lines, two license texts and two falsification patches |
| identity values | frozen: `MAGIC` (`CURRCPU1`), `VERSION` 1, `INTERFACE_ID` 10 and `PAGE_CONTENT_BYTES` 32. The layout is the magic at offset 0, then three little-endian `u64` words: the CPU at 8, the allowance at 16 and the tick at 24. A layout break ships as a new magic and version together. A host test pins the magic and the offsets as literals |
| the bound | `CPU_ID_BOUND` 8 is part of the identity (Decision A of milestone 1084, ruled 2026-11-12 UTC): a change to it re-mints `MAGIC` and bumps `VERSION` in the same change |
| address | `PAGE_VIRTUAL_ADDRESS` is `0x7fff_f000` on all three architectures, from `address_space_layout::CURRENT_CPU_PAGE`, the last of the 16 pages of `PROCESS_PAGES` |

## Who in nife depends on it

Each by a path dependency with `version = "0.1.0"`: `kernel` (the one writer, which calls `publish`
from the context switch and `build_page_with` when it builds a thread's page), `user_mode_runtime`
(the reader), `machine_statistics_contract` (a compile-time assertion that its processor line count
equals `CPU_ID_BOUND`), `components`, `fixtures`, and `system_tests` (development only). Fifteen
programs in `components` declare the pair of `INTERFACE_ID` and `VERSION` in their interface note.
The x86 test program in `kernel/src/user/x86_programs.rs` adds `CPU_OFFSET_BYTES` to
`PAGE_VIRTUAL_ADDRESS` for a 32-bit absolute load.

`cargo xtask std-src` copies `src/lib.rs` into the patched standard library as
`sys/pal/nife/currentcpuproto.rs`, beside a copy of `address_space_layout`, and points the
dependency's name at that copy (`xtask/src/farm/copied.rs`). It finds both files through `cargo
metadata`, so a pinned crate is read from Cargo's checkout and the move changes nothing there. The
standard library's `available_parallelism` reads the allowance from that copy.

## Public API

| kind | items |
|---|---|
| identity and layout | `MAGIC`, `VERSION`, `INTERFACE_ID`, `PAGE_CONTENT_BYTES` (32), `CPU_OFFSET_BYTES` (8, a `u64`) |
| the CPU word | `UNSCHEDULED` (`u64::MAX`), `CPU_ID_BOUND` (8) |
| where the page is | `PAGE_VIRTUAL_ADDRESS`, `THREAD_PAGE_SLOTS` (16), `thread_page_virtual_address(k) -> Option<u64>` |
| writer | `build_page() -> [u8; 32]`, `build_page_with(allowance: Option<NonZeroU64>, tick_hertz: Option<NonZeroU64>) -> [u8; 32]`, `unsafe fn publish(page_virtual_address: u64, cpu: u64)` |
| reader | `CurrentCpuPage`, `const unsafe fn new(virtual_address: u64)`, `cpu() -> Option<usize>`, `allowance() -> Option<NonZeroUsize>`, `tick_hertz() -> Option<NonZeroU64>` |

No type from another crate appears in a public signature. The private offsets
(`MAGIC_OFFSET_BYTES`, `ALLOWANCE_OFFSET_BYTES`, `TICK_HERTZ_OFFSET_BYTES`) are `usize` and follow
the family rule for page fields.

What a stranger may find surprising:

- `CurrentCpuPage` is `Copy` but deliberately neither `Send` nor `Sync`: the page answers for this
  thread, and a value carried to another thread carries a wrong answer. The raw pointer field makes
  the type neither by default. The sibling `CounterFrequencyPage` is both.
- `cpu()` answers `None` for a frame without the magic, for a thread never switched in (the word
  holds `UNSCHEDULED`), and for an id at or past `CPU_ID_BOUND`. Every `Some` it returns therefore
  indexes an array of `CPU_ID_BOUND` elements. Zero is a real CPU id, which is why the unset state
  is a sentinel and not a zeroed word.
- `allowance()` reads an allowance the target's `usize` cannot hold as `None`, never as a truncated
  count.
- `publish` makes one `Relaxed` store with no magic check, on the context-switch path. The reader
  loads with `Relaxed` too. The scheduler's release and acquire handoff of a thread to a core orders
  the migration, so the page needs no fence of its own. An answer may be stale one instruction
  later.
- In debug builds, `publish` asserts an 8-byte-aligned address, a CPU other than `UNSCHEDULED` and
  a CPU under the bound; `new` asserts the alignment. Release builds keep the single store.
- `CPU_OFFSET_BYTES` is a `u64` so that it adds to `PAGE_VIRTUAL_ADDRESS` without a cast; a Rust
  reader slicing the page writes `as usize`.
- `Debug` prints the address, not the words.

## Proofs, falsification patches and tests

| harness | what it proves | falsification patch | verifies in |
|---|---|---|---|
| `the_readers_answer_only_what_the_page_holds` | over every 32-byte frame, each reader answers `Some` exactly when the magic matches and its word is a real answer: the CPU when it is neither `UNSCHEDULED` nor at or past the bound, the allowance and the tick when nonzero | an off-by-one bound check; its harness turns red | 1.38 seconds |
| `a_built_page_reads_back_its_words` | for every pair of optional allowance and tick, the page `build_page_with` makes reads them back and reads the CPU as `None` | the writer stores the tick big-endian; its harness turns red | 0.98 seconds |

The `unsafe` surface is two raw-address entry points, `publish` and `new`, behind their safety
contracts. No proof covers a caller's promise that the address names a mapped page, since Kani
cannot see a mapping, and the README says so.

Fourteen host tests cover the round trip, CPU 0 as a real answer, a zeroed frame, a thread that
never ran, a wrong magic, a migration, every id in the bound, ids at and past the bound, the
allowance and tick and their absence, the thread page slots, the frozen magic, the layout offsets
and the page sharing the stack's last-level table. Four doctests run, the README's example among
them. The kernel-side tests that read the page from a running process belong to `system_tests` and
stay in nife.

## What was run

On the branch's base, `main` at `cd99567e5`, on 2026-11-19 (UTC).

| check | result |
|---|---|
| `script/names current_cpu_contract` | the crate name reads ratified 2026-10-18; the command lists no item block |
| `python3 helpers/name_release.py --draft current_cpu_contract` | 19 rows: 1 ratified (the crate), 10 provisional, 8 with no block |
| `cargo test -p current_cpu_contract` | 14 host tests and 4 doctests pass |
| `cargo clippy -p current_cpu_contract --all-targets -- -D warnings` | no warning |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p current_cpu_contract --no-deps` | no warning |
| `cargo build -p current_cpu_contract --lib --target <t>` for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` | all three build on the pinned nightly |
| the crate and `address_space_layout` copied out of the tree, `cargo +1.88 build --lib` for the host and the three targets, `cargo +1.88 test` | all build; 14 tests and 4 doctests pass on 1.88 |
| `cargo kani -p current_cpu_contract` (Kani 0.68.0, CBMC 6.11.0) | 2 of 2 harnesses verify; 8 seconds of wall time at a peak of 314 MiB |
| `script/falsifications --sweep current_cpu_contract` | 2 swept, 0 survivors: each patch turns its harness red |
| `script/crate-standalone current_cpu_contract` | 4 failures: `names` (18 names not ratified, first `VERSION`), `manifest` (the path dependency on `address_space_layout`), and `build` and `proofs` skipped because the copy cannot resolve that dependency |
| `script/crate-move current_cpu_contract --dry-run --into <scratch>` | stages at template `v8`; both patches apply exactly; standalone, passive voice, history phrases, file and function length, comment blocks, unit symbols, orphan modules, markdown links and falsification records pass, and it finds no mention of nife. Three failures: `build` (the same path dependency), `prose caps` (the README's median sentence is 22.5 words over a cap of 20, its longest 49 over 40) and `spelled-out words` (the two test names below) |
| `python3 helpers/template_report_gates.py --list current_cpu_contract` | two `abbreviations` findings, both the test names below; `helpers/template_report_baseline.txt` still holds a row of 6 for the crate |
| `python3 helpers/lint_externalized_crates.py` | 0 hits, under the crate's ceiling of 3; the lint asks for its waiver to be lowered or deleted |
| `script/crate-review-packet current_cpu_contract` | 2 harnesses, 2 patches, 11 `unsafe` sites (the two entry points, the reader's two loads, the writer's store and six in tests and harnesses), no foreign type in a signature |

Not run, and so not claimed:

- Miri. `cargo miri test` refuses because `cargo-miri` is not installed for the pinned nightly on
  the host that ran these checks. Milestone 1084's review ran it on an earlier nightly, and every
  test passed.
- Mutation. The census belongs in the 08:00 to 13:00 UTC window, and these checks ran outside it.
  The baseline row in `.cargo/mutants-baseline.txt`, from the census of 2026-10-19, reads 37 caught,
  0 missed, 0 timed out and 2 unviable. It predates both harnesses and the bound check, so it
  undercounts. The move carries the row into the repository's `.github/mutants-exempts` as a
  comment, and the repository's first weekly run lists survivors.
- Loom: the crate has no model, since its one ordering argument is the scheduler's handoff, which
  lives in the kernel.
- The stranger test, `cargo-semver-checks` (a first release has no baseline), a 32-bit build and
  the template's workflows, which run in the repository.

## Names

Ruled 2026-11-19 (UTC), option A of milestone 1084's name decision: every public name stands as
built. `CPU_OFFSET_BYTES` is published as `u64` and stays `u64` (ruled the same date, option A).
`CPU_ID_BOUND` keeps its spelling over `MAXIMUM_CPUS`, since it bounds ids and is not a count.

| name | kind | spelling |
|---|---|---|
| `PAGE_CONTENT_BYTES` | constant | the page-contract family pattern; the bytes are the page's content, not the frame it is mapped as |
| `PAGE_VIRTUAL_ADDRESS` | constant | the page-contract family pattern, spelled out |
| `CPU_OFFSET_BYTES` | constant | the family-wide `<FIELD>_OFFSET_BYTES` form |
| `thread_page_virtual_address` | function | spelled out |
| `build_page_with` | function | the builder that takes the two optional words |
| `tick_hertz` | method, parameter | the unit spelled out |
| `page_virtual_address` | parameter of `publish` | spelled out |
| `virtual_address` | parameter of `new` | spelled out |
| `CPU_ID_BOUND` | constant | kept |
| `MAGIC`, `VERSION`, `INTERFACE_ID`, `UNSCHEDULED`, `THREAD_PAGE_SLOTS`, `build_page`, `publish`, `CurrentCpuPage`, `new`, `cpu`, `allowance` | | keep their spellings |

No alias carries an old spelling. The `Name:` blocks in the crate source still read provisional
for ten items, `VERSION` and `INTERFACE_ID` among them. The move commit strips them under the ruling
that a moved crate keeps no provenance record, or a lane before it rewrites them to ratified so
that the standalone gate's `names` check passes in this tree.

Two private names in the tests break the spelled-out rule: the helper `Aligned::va` and a binding
`va` in `the_thread_page_slots_fill_the_process_pages_band`. The maintainer names them, and the
lane that applies item 4's last step renames them.

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair.
Linux's `rseq` appears only in the review's prior-art box, and the crate copies none of its text.

## Findings of milestone 1084, and their status

| item | status |
|---|---|
| 1, the manifest | the crate's own license, lints, `rust-version`, description and docs.rs targets are in. The move adds `repository` and turns the path dependency on `address_space_layout` into a pinned one once that crate has left |
| 2, README and license files | done |
| 3, docs that cite nife and quote the decider | done |
| 4, names | the renames are in and ruled. What remains is the move's: the `Name:` blocks and the two private test names above |
| 5, `cpu()` past the bound | done |
| 6, `allowance()` on a narrow target | done |
| 7, the reader reaches its word through an integer | done |
| 8, debug checks in the `unsafe` entry points | done |
| 9, builder and readers disagree on unknown | done |
| 10, nothing machine-checked | done: two harnesses, two patches |
| 11, a stale `BUGS` section | done |
| 12, the single-file constraint | the README and `BUGS` sentence are in. The farm check that refuses a private `mod x;` or an `include!` in a copied single-file crate is a nife lane, not part of the crate, and is not built |
| 13, `#![no_std]` | done |
| 14, the type of `CPU_OFFSET_BYTES` | ruled: stays `u64` |
| 15, the kernel's safety comment | done, in nife |
| 16, two notes that do not link | done, in nife |
| Decision A, the bound is part of the identity | ruled and built |
| Decision B, the names | ruled: all stand as built |

## What the move still needs

| item | owner |
|---|---|
| `address_space_layout` leaves first, in milestone 1017 step 3's order, and this crate then depends on it by `git` and `rev` with `version = "0.1.0"`. Until then the standalone build cannot resolve the dependency | the `address_space_layout` move, then this move |
| Strip or rewrite the ten provisional `Name:` blocks, and rename the two private `va` test names | the move lane, or a lane before it |
| Bring the README under the template's prose caps: a median sentence of 20 words and a longest of 40 | the move lane, or a lane before it |
| Add `repository` to the manifest | the move lane |
| Convert the six manifests to `git` and `rev`, drop the workspace member and the mutation baseline row, and read `.github/workflows/undefined-behavior-check.yml` and `script/verify`, which name the crate | the move lane's nife-side pull request, as the dry run lists it |
| Lower or delete the crate's rows in `helpers/lint_externalized_crates.py` and `helpers/template_report_baseline.txt` | the move lane's nife-side pull request |
| Run the stranger test, the template's workflows and the first weekly mutation run | the move lane, in the new repository |

## Recommendation

Ready for review. The names and the type of `CPU_OFFSET_BYTES` are ruled, every finding of the
outside review has landed or is work the move does, and the crate's own tests, clippy, docs, Kani
harnesses and falsification patches pass. Clearing the review, which is also the go to create
`nifeos/current_cpu_contract`, is the architect's open decision in milestone 1084.
