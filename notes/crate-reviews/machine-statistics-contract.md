# Review package: `machine_statistics_contract`

*Name: provisional, minted by lane 1099-rulings-renames-package. The `machine_statistics_contract`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the packages together
outgrow the page.*

The architect reads this to clear the crate for its move to its own repository, in step 6 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as the branch leaves it. The outside review behind it is
[milestone 1099 (`machine_statistics_contract`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1099-machine-statistics-contract-outside-review.md).
Every item of that review has landed, except the work the move itself does and the architect's
clearance of this package.

## In one paragraph

The layout of a 576-byte read-only page that tells a program how much memory the machine has and
what each processor line is doing, with no system call. One header line of eight words carries the
magic, the frame size, the tick rate, the total and free frame counts and the version. Eight
processor lines of eight words follow, each with an online flag and five counters: busy ticks, idle
ticks, context switches, interrupts and runnable threads. The kernel writes the page with relaxed
atomic stores. A program reads it with relaxed atomic loads through `atomic_word`, or takes a
`Snapshot` and asks it for totals. Every reading is total over any page the reader admits: a sum
saturates and never overflows. The crate is `no_std`, depends on `address_space_layout` for its
address and on `current_cpu_contract` for the processor bound, and has `unsafe` in two entry
points, because each names a raw address. Three Kani harnesses and one host test, each with a
falsification patch, hold the reader, the totals, the builder and the line count. Every public name reads
ratified (2026-11-19 UTC). No file in the crate names nife, a path in this tree or a private
machine.

## Identity

| item | value |
|---|---|
| crate | `machine_statistics_contract`, ratified 2026-09-27 |
| version | `0.1.0`; no tag and no repository yet |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest |
| `publish` | `false`; the manifest comment says the line stays until the release that publishes |
| size | one `src/lib.rs` of 675 lines, a README of 99 lines, two license texts and four falsification patches |
| identity values | frozen (ruled 2026-11-14 UTC): `MAGIC` (`MACHSTA1`), `VERSION` 1, `INTERFACE_ID` 4. The layout is 72 little-endian `u64` words, 576 bytes: the header line, then eight processor lines. A layout break ships as a new magic and version together |
| the bound | `PROCESSOR_LINES` is 8 and the crate asserts it equals `current_cpu_contract::CPU_ID_BOUND` at compile time. A host test pins that equality, and a falsification patch breaks it |
| address | `PAGE_VIRTUAL_ADDRESS` is the fixed pair page from `address_space_layout::pair_page`, the same on all three architectures |

## Who in nife depends on it

Seven manifests depend on it by a path dependency with `version = "0.1.0"`. `kernel` is the one
writer: it builds the header and stores each counter through `atomic_word`. `components` holds four
readers: `free`, `top`, `vmstat` and `boot_self_test`. The crates `free`, `top` and `vmstat` hold
the programs' logic over a `Snapshot`. `system_initializer` maps the page and passes it to the
child that declares it. `system_tests` reads it from a running process, as a development
dependency. `grant_contract` names the crate in documentation only and has no dependency on it.

## Public API

| kind | items |
|---|---|
| constants | `PROCESSOR_LINES`, `MAGIC`, `VERSION`, `INTERFACE_ID`, `PAGE_WORDS`, `PAGE_CONTENT_BYTES`, `PAGE_VIRTUAL_ADDRESS` |
| module `word` | `LINE_WORDS`, `cpu(id)` |
| module `word::header` | `MAGIC`, `FRAME_BYTES`, `TICK_HERTZ`, `TOTAL_FRAMES`, `FREE_FRAMES`, `VERSION`: the word index of each header field |
| module `word::line` | `ONLINE`, `BUSY_TICKS`, `IDLE_TICKS`, `CONTEXT_SWITCHES`, `INTERRUPTS`, `RUNNABLE`: the word offset of each field inside one processor line |
| functions | `build_header(frame_bytes, tick_hertz)`, `unsafe atomic_word(virtual_address, index)` |
| `Cpu` | one processor line's fields: `online`, `busy_ticks`, `idle_ticks`, `context_switches`, `interrupts`, `runnable` |
| `Snapshot` | `frame_bytes`, `tick_hertz`, `total_frames`, `free_frames`, `cpus`; `from_words`, `unsafe read(virtual_address)`, `total_bytes`, `free_bytes`, `online_cpus`, `busy_ticks`, `idle_ticks`, `context_switches`, `interrupts`, `runnable` |

## Proofs, falsification patches and tests

| harness or test | what it proves | falsification patch |
|---|---|---|
| `the_reader_admits_exactly_the_pages_it_names` (Kani) | over every 576-byte page, `Snapshot::from_words` answers `Some` only for the page it names, by magic and version | the magic compare looks at the low 32 bits only; the harness turns red |
| `every_reading_is_total_over_an_admitted_page` (Kani) | every total and byte count over any admitted page computes without overflow or panic | `total_bytes` multiplies without saturating; the harness turns red |
| `a_built_header_reads_back_as_an_empty_page` (Kani) | for every frame size and tick rate, `build_header` reads back as a page of zero counters and the same frame size and tick rate | the builder writes the frame size into the tick-rate word; the harness turns red |
| `the_processor_line_count_equals_the_cpu_id_bound` (test) | `PROCESSOR_LINES` equals `CPU_ID_BOUND` | the line count becomes 9 and the compile-time assertion goes, so the test runs and turns red |

The `unsafe` surface is two raw-address entry points, `atomic_word` and `Snapshot::read`, behind
their safety contracts. No proof covers a caller's promise that the address names a mapped page,
since Kani cannot see a mapping, and the README says so. Miri runs the host tests, including one
that reads an unmapped frame through `Snapshot::read`.

Ten host tests cover the reader: the round trip through `atomic_word`, an unprepared frame, a
wrong magic and a wrong version, an offline line that counts for nothing, the totals over online
lines only, a page of maximal counters that saturates every reading, the units the header carries,
one cache line per processor line and the line count. One doctest runs the README's example. The kernel-side test
that reads the page from a running process belongs to `system_tests` and stays in nife.

## What was run

On the branch of pull request 3801, on 2026-11-19 (UTC).

| check | result |
|---|---|
| `python3 helpers/name_release.py --draft machine_statistics_contract` | every public item reads ratified: the crate on 2026-09-27 and each item on 2026-11-19 |
| `cargo test -p machine_statistics_contract` | 10 host tests and 1 doctest pass |
| `cargo clippy -p machine_statistics_contract --all-targets -- -D warnings` | no warning |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p machine_statistics_contract --no-deps` | no warning |
| `cargo build -p machine_statistics_contract --lib --target <t>` for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` | all three build |
| `cargo build -p kernel --target <t>` for the same three targets, and `cargo check -p components` for each | all build |
| the crate, `current_cpu_contract` and `address_space_layout` copied out of the tree, `cargo +1.88 test` and `cargo +1.88 build --lib` for the three targets | all build; 10 tests and 1 doctest pass on 1.88 |
| `cargo miri test -p machine_statistics_contract` | 10 tests and 1 doctest pass; Miri warns once about the integer-to-pointer cast in `atomic_word`, which is the entry point's purpose |
| `cargo kani -p machine_statistics_contract` | 3 of 3 harnesses verify |
| `script/falsifications --sweep machine_statistics_contract` | 4 swept, 0 survivors: each patch turns its harness or test red |
| `cargo test -p top -p vmstat -p free -p grant_contract -p system_initializer` | all pass |
| `cargo check -p system_tests --tests` | builds |
| `script/crate-standalone machine_statistics_contract` | fails on the two path dependencies (`current_cpu_contract`, `address_space_layout`) and on the build that follows; the names step passes |
| `script/crate-move machine_statistics_contract --dry-run --into <scratch>` | stages at the pinned template; standalone, prose caps, passive voice (after the README edit, checked by the template's own helper), history phrases, file and function length, comment blocks, spelled-out words, unit symbols, orphan modules, markdown links and falsification records pass, and it finds no mention of nife. One failure remains: `build`, the same path dependencies |
| `python3 helpers/lint_externalized_crates.py` | 0 hits for the crate |
| `script/crate-review-packet machine_statistics_contract` | the public API above, with no foreign type in a signature |

Not run, and so not claimed:

- Mutation. The census belongs in the 08:00 to 13:00 UTC window, and these checks ran outside it.
  The baseline row in `.cargo/mutants-baseline.txt` reads 33 caught, 0 missed, 0 timed out and 5
  unviable. It predates the harnesses, so it undercounts. The move carries the row into the
  repository's `.github/mutants-exempts` as a comment, and the first weekly run lists survivors.
- Loom: the crate has no model, since its one ordering argument is the writer's relaxed stores,
  which live in the kernel.
- The stranger test, `cargo-semver-checks` (a first release has no baseline), a 32-bit build and
  the template's workflows, which run in the repository.
- The kernel's QEMU boot rows and the `system_tests` run of the statistics programs. Those gates
  run in the merge queue.

## Names

Every public item carries a `Name:` block that reads ratified 2026-11-19 (UTC). The ruling of that
date set the renames: `PAGE_BYTES` to `PAGE_CONTENT_BYTES`, `PAGE_VA` to `PAGE_VIRTUAL_ADDRESS`, the
parameters `page_va` and `va` to `virtual_address`, `WORDS` to `PAGE_WORDS`, `word::LINE` to
`word::LINE_WORDS` and `counter` to `atomic_word`. `Snapshot` stays. No alias carries an old
spelling. The two submodule names `word::header` and `word::line` come from item 9 of the milestone,
which the maintainer set provisionally: they keep the header indices and the per-line offsets in
separate namespaces, and they keep `word::LINE_WORDS` and `word::cpu` where the ruled table puts
them. The architect clears the package, and that clearance covers these two module names too.

## Findings of milestone 1099, and their status

| item | status |
|---|---|
| P0-1, the names and the ratification table | landed in this branch |
| P0-2, manifest, license files and README | landed in this branch |
| item 3 and P1-4 to P1-8, P1-11, P2-14 and P2-15 (totals, the `'static` reference, the line count, the proofs, the type width and the documentation defects) | landed earlier, as the milestone's status block records |
| P1-9, header and line namespaces | landed in this branch |
| P1-10, `online_cpus` and the word core | landed |
| P2-13, the end-to-end test note | landed |
| item 12, the move tooling | not this crate's work; the move lane owns it |

## What the move still needs

| item | owner |
|---|---|
| `address_space_layout` and `current_cpu_contract` leave first (milestone 1017 step 6 follows their steps), and this crate then depends on both by `git` and `rev` with `version = "0.1.0"`. Until then the standalone build cannot resolve them | their moves, then this move |
| Add `repository` to the manifest | the move lane |
| Convert the seven manifests to `git` and `rev`, drop the workspace member and the mutation baseline row, and read `script/verify`, `helpers/interface_stability.py`, `helpers/trust_boundary_crates.tsv` and `packages/contracts.package.toml`, which name the crate | the move lane's nife-side pull request, as the dry run lists it |
| Lower or delete the crate's rows in `helpers/lint_externalized_crates.py` and `helpers/template_report_baseline.txt` | the move lane's nife-side pull request |
| Run the stranger test, the template's workflows and the first weekly mutation run | the move lane, in the new repository |

## Recommendation

Ready for review. The names are ratified, every finding of the outside review has landed or is work
the move does, and the crate's own tests, clippy, docs, Miri, Kani harnesses and falsification
patches pass. Clearing this package is the architect's act, and it is the go to create
`nifeos/machine_statistics_contract`, which then waits for the two crates it depends on to leave
first.

Ready for review.
