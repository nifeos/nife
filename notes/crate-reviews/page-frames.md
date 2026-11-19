# Review package: `page_frames`

*Name: provisional, minted by lane 1103-renames-package. The `page_frames` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to ratify the crate's public names in one pass and to clear the crate for
its move to its own repository. It follows
[milestone 1103 (what an outside maintainer would fix before 0.1.0)](../../design/roadmap/1103-page-frames-outside-review.md)
and describes the crate as `main` plus this branch leaves it.

## In one paragraph

A physical page frame allocator. It manages a bitmap of one bit per 4 KiB frame over a span of RAM,
and the caller lends the bitmap, so the crate needs no allocator of its own. Every frame starts
used, and boot code declares frames free from the memory map, so an unknown address is never
handed out. It allocates single frames and contiguous runs for direct memory access, frees them,
reports the longest free run (free frames and contiguity are different numbers) and reports counts.
The crate is `no_std` with no dependency, `#![forbid(unsafe_code)]`, and has nine Kani harnesses,
each with a falsification patch. Nothing in it points back into nife. Its names stand
provisional until the architect ratifies them from the table below.

## Identity

| item | value |
|---|---|
| crate | `page_frames`, ratified 2026-08-23 (a kernel-dependency crate naming review) |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024; the crate builds and tests on 1.87 and 1.88 outside the tree, and its manifest sets no `rust-version` yet |
| license | the manifest inherits `license` from the nife workspace; the move writes `MIT OR Apache-2.0` and the two texts |
| `publish` | `false` |
| dependencies | none |
| size | `src/lib.rs` of 661 lines (the library and its Kani harnesses) and `tests/allocator.rs` of 434 lines (25 host tests) |
| layout | no identity values: the crate is in-process logic and has no wire format, magic or interface number |

## Who in nife depends on it

Two crates, each by a path dependency with `version = "0.1.0"`: `kernel` and `system_tests`. In
`kernel`, only `kernel/src/memory.rs` uses the allocator (`PageFrameAllocator`). The other files
that mention the crate (55 in all, outside the crate) use `FRAME_BYTES` and
`PageFrame::from_physical_address`. No program in `components`, no other crate, no fuzz target
and no tool depends on it. `packages/kernel.package.toml` lists it, and the tooling holds rows for
it in `helpers/template_report_baseline.txt`, `.cargo/mutants-baseline.txt` and
`helpers/verify_times.py`.

What references back into nife from the crate: nothing. The manifest inherits `license` and
`lints` from the workspace, which the move rewrites as literals.

## The bar for leaving

The rule is that a crate leaves only if it is publishable or used by more than one repository.
Today the only consumers are the kernel and `system_tests`, both in this repository, so the second
test does not hold and the crate leaves only on the first. The case for publishable: it is a
general bitmap frame allocator with no `unsafe`, no nife vocabulary in its code, no dependency, a
borrowed bitmap, contiguous-run allocation and nine proofs. The case against: nothing outside
nife uses it yet, and a kernel-specific allocator is a small crate a stranger may prefer to
write. The architect weighs these; this package does not.

## What was run

| check | result |
|---|---|
| `cargo test -p page_frames` | 25 host tests pass; no doctest |
| `cargo clippy -p page_frames --all-targets -- -D warnings` | no warning |
| `cargo doc -p page_frames --no-deps` with `RUSTDOCFLAGS=-D warnings` | no warning |
| `cargo build -p page_frames --lib` for `aarch64-unknown-none-softfloat`, `x86_64-unknown-none`, `riscv64imac-unknown-none-elf` | passes on all three |
| a copy outside the tree, with the manifest's `license` and `lints` written out, `cargo test` on 1.87.0 and on 1.88.0 | 25 host tests pass on both |
| `cargo miri test -p page_frames` | 25 host tests pass |
| `cargo kani -p page_frames` | 9 harnesses verified, 0 failures, 1 min 51 s, 476 MB peak; the slowest is `the_used_count_is_the_number_of_set_bits` at 50 s |
| the falsification patches (`patch -p1 --dry-run -F0`) | each of the nine applies at no offset |
| `script/crate-standalone --no-kani page_frames` | seven failures, all the move lane's (see below); the build passes on the host and three targets |
| `script/crate-move --dry-run` | stages; four findings, all for the move lane. There is no README. The manifest inherits from the workspace, so `cargo generate-lockfile` fails in the copy. The licenses are absent. The scaffolded README exceeds the prose caps until it is written |
| `script/names page_frames` and `python3 helpers/name_release.py --draft page_frames` | the crate name ratified; 21 names: 1 ratified, 2 provisional with a block, 18 with no block |
| the template's naming checks (`question_shape`, `contract_names`, `spelled_out`, `unit_symbols`, `register_names`, `british_spelling`) | each judges 120 names and refuses none |
| `abbreviations` (report only) | finds one component, `allocator`, eight times, which its word list does not hold; a question for the architect |

Not run, and so not claimed: a mutation census, which runs only between 08:00 and 13:00 UTC; the
baseline row (101 caught, 3 missed, 1 timeout, 12 unviable) predates the renames, which change no
logic. `cargo-semver-checks` has no baseline for a first release. The stranger test and the
template's workflows run in the new repository.

## Public API

| kind | items |
|---|---|
| constant | `FRAME_BYTES` (4096) |
| a frame | `PageFrame`: `from_physical_address`, `try_aligned`, `containing`, `physical_address` |
| counts | `Stats { frames, used_frames }` and `free_frames()` |
| the allocator | `PageFrameAllocator<'a>`: `bitmap_bytes`, `page_frames_in`, `page_frames_covering`, `new(base, total, bitmap)`, `stats`, `mark_free(start, bytes)`, `mark_used(start, bytes)`, `alloc`, `alloc_contiguous(count)`, `largest_free_run`, `free`, `is_used` |

No nife type, capability or handoff appears in a public signature.

What a stranger may find surprising:

- The allocator starts with every frame used. `mark_free` frees only whole frames inside the
  range, and `mark_used` claims every frame the range touches. An edge mistake wastes a frame
  and never hands out a held one.
- `from_physical_address` panics on an unaligned address; `try_aligned` returns `None` and
  `containing` rounds down.
- `page_frames_in` is the whole frames in a span and `page_frames_covering` rounds up.
- `free` of a frame that is already free, or outside the span, is a recorded no-op and not a panic.
  The documentation's `BUGS` section says what the allocator cannot detect.

## Proofs

Nine Kani harnesses, each with a falsification patch. They prove that an allocated frame is aligned and in range. Two allocations differ. A contiguous run was free and is now used. `mark_free` frees only whole frames inside its range, and `mark_used` claims exactly the frames it touches. The used count is the number of set bits. `index_of` inverts frame addressing, `bitmap_bytes` covers every frame and `containing` rounds down within a frame. Each patch breaks the property
it names and fails its harness.

## Names

Every public name is provisional. The rename lane applied the two rulings of 2026-11-19 UTC and
2026-11-14 UTC and the two-way renames of item 8, with no alias of an old spelling. The architect
ratifies each row, or names a change.

| item | kind | spelling and reason | proposal |
|---|---|---|---|
| `page_frames` | crate | ratified 2026-08-23 | none |
| `FRAME_BYTES` | constant | unit on the name, as `PAGE_BYTES` does elsewhere in the tree (was `FRAME_SIZE`) | ratify |
| `PageFrame` | struct | the standard term; also the kernel's capability name | ratify |
| `PageFrame::from_physical_address` | function | spelled out (was `from_addr`) | ratify |
| `PageFrame::try_aligned` | function | returns `None` on an unaligned address; provisional block from item 5 | ratify |
| `PageFrame::containing` | function | rounds down to the frame holding an address; kept | ratify |
| `PageFrame::physical_address` | function | spelled out (was `addr`) | ratify |
| `Stats` | struct | counts of frames | ratify |
| `Stats::frames`, `Stats::used_frames` | fields | the unit is the frame (were `total`, `used`) | ratify |
| `Stats::free_frames` | function | free count (was `free`) | ratify |
| `PageFrameAllocator` | struct | names what it allocates; the abbreviations helper does not hold `allocator` | ratify; add the word to the helper's list |
| `bitmap_bytes` | function | bytes of bitmap for a frame count | ratify |
| `page_frames_in` | function | whole frames in a span of bytes; parameter `span_bytes` | ratify |
| `page_frames_covering` | function | frames a span reaches, rounding up; provisional block from item 9 | ratify |
| `new` | function | construction from the base, the total and a lent bitmap | ratify |
| `stats` | function | the counts | ratify |
| `mark_free`, `mark_used` | functions | parameters `start` and `bytes` | ratify |
| `alloc`, `alloc_contiguous` | functions | `alloc` is the std spelling for allocation | ratify |
| `largest_free_run` | function | the longest run of free frames | ratify |
| `free`, `is_used` | functions | free one frame; the used state of a frame, `None` outside the span | ratify |

Private names renamed with them: `is_index_used` (was `get`, which the template's `question_shape`
gate refused), the harness locals `bitmap`, `allocator`, `first` and `second`, and the test
`an_unaligned_physical_address_is_refused_without_a_panic`.

## License

`license` is inherited from the workspace today and becomes `MIT OR Apache-2.0` in the move. The
crate adapts no source from elsewhere. The design follows the common bitmap allocator and borrows
no text.

## What still blocks the move

| item | owner |
|---|---|
| Write the crate's own README, carrying the design arguments that stay behind in nife (milestone 1103 item 2). | move lane |
| Make the manifest stand alone: literal `license`, `lints` and `rust-version`, and the two license texts (item 1). | move lane |
| Convert the two path dependencies in `kernel` and `system_tests` to `git` and `rev` with `version = "0.1.0"`. | move lane |
| Rows for the crate in `helpers/template_report_baseline.txt`, `.cargo/mutants-baseline.txt`, `helpers/verify_times.py` and `packages/kernel.package.toml`. | move lane |
| Rewrite the `Name:` blocks of the ratified names, and the two provisional blocks, to say ratified. | the lane that records the ratification |
| Items 12 and 17 of milestone 1103. | move lane |
| Run the stranger test, the template's workflows and the first weekly mutation run in the new repository. | move lane |

## Open questions

1. Whether the crate leaves at all, given that only the kernel and `system_tests` use it (see the
   bar above). Recommendation: leave it, so the proofs and the falsification patches run in a
   repository whose gates a stranger can read; the architect decides.
2. Whether the abbreviations helper's word list gains `allocator`. Recommendation: yes; it is a
   standard word and the crate's own type uses it.

## Recommendation

Ready for review.
