# `elf` against three readers: `goblin`, cole14's `elf` and `object`

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md), for the `elf`
row of [the rest of the tree](the-rest-of-the-tree.md). Name provisional.

calef asked on 2026-10-22 (UTC) for the data to decide whether `crates/elf` should keep its kernel
exemption. This page measures our crate against `goblin` 0.10.7, cole14's `elf` 0.8.0 and the read
side of `object` 0.40.0. It was measured on 2026-10-22 at base `297d2c4e1`, on patagonia (Apple M3),
with the pinned nightly of that date. No dependency was added. Every probe is generated outside the
tree by [elf-probe.sh](elf-probe.sh), and its mode names the section it feeds.

## What a candidate has to do here

Ten packages depend on `crates/elf`. Most callers have no allocator: `kernel`, `measured_boot`,
`supervision_protocol`, `system_initializer`, `uefi_loader`, `fixtures`, `system_tests`, and the
five programs in `components` that spawn. The shell has a heap, and `jig` and `fuzz` are std. The
kernel has no global allocator at all, so a reader that needs `alloc` cannot serve the main user.

Every user calls the same four things: `Elf::parse`, `segments()`, `entry()`, and `Segment`'s
`page_range` and flag tests. `system_initializer`, `jig` and `system_tests` also call `Elf::note`
for a program's manifest. The shell finds the same note without holding the whole file, through
`NoteSegments::from_head` and `NoteSearch` (`components/src/swish/running.rs`). A candidate has to
serve that streaming path as well: a header and table read from the first page, then one segment
at a time. All three can, since each exposes its note iterator over a caller's bytes.

None of the candidates enforces our loader's rules. Static `ET_EXEC` only, this build's machine, at
most 64 headers, W^X, no two segments on one page, the entry inside an executable segment, no
`p_vaddr + p_memsz` overflow, and a duplicate note refused: these stay ours whichever parser reads
the fields. The probe writes that policy once, 92 lines, and a 44- to 56-line adapter per candidate.
Our crate has about 490 lines of shipping code (tests and proofs aside). A switch would retire about
280 of them, the header, table and note reading, and keep the rest.

## Fit

Each reader does the loader's job (parse, walk the `PT_LOAD`s, find the manifest note) in a
freestanding binary with no global allocator, built for the three nife targets (`elf-probe.sh fit`).

| | links with no allocator | text + rodata, aarch64 | riscv64 | x86_64 | parsed type |
|---|---|---|---|---|---|
| ours | yes | 9,979 B | 8,772 B | 12,017 B | 48 B |
| cole14 `elf` | yes | 12,900 B | 11,282 B | 13,471 B | 120 B |
| `object`, read side | no | 9,332 B | 8,167 B | 11,535 B | 40 B |
| `goblin` | no | 26,227 B | 24,281 B | 29,151 B | 104 B plus a heap `Vec` |

The sizes include the shared policy, so they compare like with like. They are the loader's calls
alone, not a kernel build.

- `object`'s reader turns on `extern crate alloc` with `read_core`, so a binary that links it must
  name a `#[global_allocator]`. Nothing on this path allocates: the linked binary holds no
  allocation symbol. The kernel could link it only with a refusing allocator, a foot gun that turns
  any future allocation into a panic. Making `alloc` optional for `read_core` is an upstream change.
- `goblin` reads program headers only into a `Vec`. With `alloc` off it offers the ELF header as a
  cast from bytes, and that cast unwraps an alignment check: on a buffer one byte off an 8-byte
  boundary, `goblin::elf64::header::Header::from_bytes` panics with `BadAlignment`
  (`elf-probe.sh edge`). A package member or a file read into a byte array has no alignment
  guarantee.
- `goblin`'s `endian_fd` feature does not build with `elf64` alone; it needs `elf32` too.
- The other three read misaligned input correctly: all 375 images, shifted one byte, load as before.
- cole14's crate is also named `elf` on crates.io, so it can sit beside ours only under a renamed
  dependency.

## Agreement and parse time on real programs

`elf-probe.sh agree` ran on 375 images built today: 124 programs and the kernel for each of the
three targets. Each image's `e_machine` is set to the host's in memory, so all of them reach the
paths past that one-`u16` check.

All four accept the same 375 images, with the same entry, the same segments and the same manifest
note (18 images carry one). No disagreement.

Parse plus the manifest lookup, per image, best of 7 rounds of 200:

| | median | p90 | slowest | all 375 |
|---|---|---|---|---|
| ours | 147 ns | 159 ns | 425 ns | 56 µs |
| cole14 `elf` | 160 ns | 168 ns | 286 ns | 59 µs |
| `object` | 127 ns | 131 ns | 202 ns | 47 µs |
| `goblin` | 317 ns | 333 ns | 596 ns | 116 µs |

These are host figures with the machine shared. `goblin`'s figure is its lean path, the header and
`ProgramHeader::parse`; its `Elf::parse` also reads sections and symbols. A spawn costs far more
than any of these, so time decides nothing here.

## Where they disagree: crafted notes

Real images never reach the edges. These cases do (`elf-probe.sh edge`):

| case | ours | cole14 `elf` | `object` | `goblin` |
|---|---|---|---|---|
| `p_align` 1 | found | found, wrong bytes: shifted 3 | found | found |
| `p_align` 16 | refused | not found, no error | refused | refused |
| malformed note after the match | refused | found | refused | refused |
| the gABI's 8-byte padding written wrong | refused | not found, no error | refused | refused |
| last note's padding past the segment | refused | found | found | found |
| note runs past `p_filesz`, bytes in file | refused | not found, no error | refused | found, read from outside the segment |
| owner `nifeX`, no NUL | not found | not found | not found | found as `nife` |
| duplicate | refused | refused by our policy | refused by our policy | refused by our policy |

Three of these matter for a manifest. cole14's iterator ends a walk at the first malformed note and
reports nothing; that is its open issue #53, which its author says needs a breaking change. With
`p_align` 1 it pads by 1, not 4 as binutils and LLVM do, and hands back the wrong descriptor.
`goblin` bounds a walk only by where each note starts, so a note may read past its segment. It
also drops the owner's last byte without checking that it is a NUL. Only `object` refuses every
malformed case, and it is lenient on one: the last note's padding.

## Hostile input: fuzzing

One cargo-fuzz target per reader (`elf-probe.sh fuzz`). Ours is `fuzz/fuzz_targets/elf_parse.rs`
unchanged. Each candidate's target runs the loader path through our policy, then the reader's own
whole-file surface: sections, string tables, symbols, notes and the dynamic table, and `goblin`'s
`Elf::parse`. The seeds are `fuzz/seeds/elf_parse/`, six crafted note images and six real programs
that carry a manifest; the dictionary is `fuzz/dictionaries/elf_parse.dict`. The build has
AddressSanitizer and overflow checks on. Each target ran 20 minutes, one after another, in fork mode
with a 10-second timeout, a 512 MiB single-allocation limit and a 2 GiB RSS limit, so a failure is
counted rather than ending the run.

| | executions | coverage (edges) | corpus | panics | out-of-bounds | huge allocations | hangs |
|---|---|---|---|---|---|---|---|
| ours | 260 million | 238 | 130 | 0 | 0 | 0 | 0 |
| cole14 `elf` | 44 million | 947 | 519 | 0 | 0 | 0 | 0 |
| `object` | 60 million | 773 | 713 | 0 | 0 | 0 | 0 |
| `goblin` | 33 million | 1,564 | 973 | 0 | 0 | 0 | 0 |

The union of the four final corpora, 2,306 inputs, then ran once through every target: no failure.
Overflow is in the panic column, since the build traps it. Coverage counts differ because the
surfaces differ; ours is small because the crate is. Twenty minutes is a smoke test, not a campaign.
It finds the shallow bugs and says nothing about deep ones. `goblin`'s open #120 was not replayed,
because its inputs are an attachment on the issue.

## Proofs

Our eight harnesses, each property in plain words. All eight verify in this run, the slowest in
33 seconds.

| | property | function |
|---|---|---|
| P1 | one program header's bounds arithmetic never panics, for any field values and file length | `check_segment_bounds` |
| P2 | when that check passes, the segment's bytes lie inside the file | `check_segment_bounds` |
| P3 | when it passes, `p_vaddr + p_memsz` did not wrap | `check_segment_bounds` |
| P4 | a segment's page range never panics and never comes out inverted | `Segment::page_range` |
| P5 | one note's extent arithmetic never panics, for any sizes and either padding | `note_extent` |
| P6 | a note that passes lies inside its segment, in order, and moves the walk at least 12 bytes | `note_extent` |
| P7 | finding the `PT_NOTE` headers in a hostile 176-byte head never panics | `NoteSegments::from_head` |
| P8 | a walk of a hostile 40-byte segment returns only bytes inside it | `find_in_segment` |

`elf-probe.sh kani` asks the same of each candidate's nearest public function, with the same input
sizes. P5 and P6 become one walk over a symbolic 24- or 40-byte segment, and P6 and P8 merge. A
harness that ran past 15 minutes is reported as intractable for CBMC at that size.

| | cole14 `elf` | `object` | `goblin` |
|---|---|---|---|
| P1, P2: segment bytes | holds, 549 s | fails for an empty segment (below); holds with `p_filesz > 0`, 0.4 s | no checked accessor: `file_range` saturates and never panics (P1 holds), but its end can pass the file (P2 fails, any `p_offset` past the end); the caller slices |
| P3: address overflow | no check; ours stays | no check; ours stays | `vm_range` saturates instead of refusing (fails) |
| P4: page range | none; ours stays | none; ours stays | none; ours stays |
| P5: one note | holds, 6.9 s | holds, 0.6 s | intractable at 15 min |
| P6, P8: a walk | holds, 60 s | holds, 5.5 s | intractable at 15 min |
| P7: hostile head | holds, 692 s (it also reads ELF32, so the bound is 9) | holds, 4.7 s | intractable at 15 min |

`object`'s one counterexample: `read_bytes_at` returns a static empty slice for size 0 without
looking at the offset. A `PT_LOAD` with no file bytes and `p_offset` past the end is accepted; ours
refuses it as `SegmentOutOfBounds`. It reads nothing, so it is not a memory-safety bug, and our
policy layer catches it, but P2 as stated does not hold.

`goblin` is the hard one for a model checker. Its reads go through `scroll`'s generic `Pread`
traits, and CBMC did not finish a 24-byte note in 15 minutes where the other two took seconds. Its
P2 and P3 "failures" are by design: it hands back unchecked ranges and leaves bounds to the caller.

A proof upstream would have to be one of these harnesses. cole14's and `object`'s note walks already
hold as written, so a contribution there is the harness and a CI job, not a restructuring.
`goblin` would need restructuring before a harness finishes. That is the code change §46 (thin
primitives or whole subsystems) keeps the kernel's own code for.

## Cost

| | crates added to `Cargo.lock` | licenses | `unsafe` in the crate | in its dependencies |
|---|---|---|---|---|
| ours | 0 | MIT or Apache-2.0 | 0 | none |
| cole14 `elf` | 1 | MIT/Apache-2.0 | 0, `#![forbid(unsafe_code)]` | none |
| `object` | 0: `object` 0.40.0 and `memchr` are already locked, for `xtask` | Apache-2.0 or MIT; `memchr` Unlicense or MIT | 10 blocks and 9 `unsafe impl Pod`, 2 blocks under `read/` | `memchr`: 113 blocks, 218 `unsafe fn` |
| `goblin` | 4: `goblin`, `plain`, `scroll`, `scroll_derive` | MIT; `plain` MIT/Apache-2.0 | 16 blocks, 9 `unsafe fn`, 14 `unsafe impl` | `scroll` 7 blocks, `plain` 5 blocks and 11 `unsafe impl` |

Every license passes `deny.toml`. `goblin` also pulls a proc-macro chain (`syn`, `quote`,
`proc-macro2`, `unicode-ident`); all are already in the lock, `unicode-ident` under its existing
`Unicode-3.0` exception. Counts are textual (`unsafe {`, `unsafe fn`, `unsafe impl`) over each
crate's `src/`.

## Record and upkeep

Advisories: none for `goblin`, `elf`, `object`, `scroll`, `plain` or `memchr` in RUSTSEC
(`rustsec/advisory-db` read 2026-10-22), GitHub's advisory database or NVD. The nearest is
`xmas-elf`'s RUSTSEC-2025-0018, which 784 already records.

Their own fuzzing, read from each repository on 2026-10-22:

- cole14 `elf` has five cargo-fuzz targets (common, notes, stream, symbol table, symbol versions),
  and CI runs each for 60 seconds on every push and pull request.
- `goblin` has cargo-fuzz and AFL targets; CI builds them and runs none. Its oldest open ELF crash
  report (#120, out-of-memory on malformed ELF, 2019) is still open. The newest DoS report (#542,
  2026-08) is Mach-O, not ELF.
- `object` has no fuzz targets and is not in OSS-Fuzz. `gimli`, its sibling, is, and its targets
  do not reach `object`.
- None of the three has a Kani harness.

| | releases since 2023-10-22 | last release | last commit | maintainers (crates.io owners) | open issues; PRs | outside PRs |
|---|---|---|---|---|---|---|
| `goblin` | 13 | 0.10.7, 2026-05-28 | 2026-08-30 | 1 | 75; 12 | 91 opened, 62 merged (median 15 days), 19 closed, 10 open (median age 235 days) |
| cole14 `elf` | 2 | 0.8.0, 2025-05-14 | 2025-11-29 | 1 | 5; 6 | 11 opened, 5 merged (median 16 days), 2 closed, 4 open (median age 357 days) |
| `object` | 21 | 0.40.0, 2026-08-01 | 2026-10-12 | 2 | 28; 9 | 84 opened, 62 merged (median 1 day), 15 closed, 7 open (median age 18 days) |

Outside pull requests are those opened since 2024-10-22 by anyone but a crates.io owner or a bot.
GitHub's own author association will not do: it calls one of `object`'s owners a contributor.
Open issues are not labeled consistently, so no bug count is given. `goblin` has 3 open with
"panic" in the title; the others have none.

## Options

Whichever is chosen, the loader's policy stays ours, about 200 lines with the machine constants and
the error type. The choice is only who reads the bytes.

- A. Keep ours. It fits every user, with no allocator, no `unsafe` and the smallest parsed type. It
  refuses every malformed note case above, and its eight properties hold. It fuzzed clean. The cost
  is about 280 lines of parsing we own, and the `elf` name on crates.io is taken if 857 publishes it.
- B. Replace with `object` now. It is the strongest candidate: the most active upstream, no new
  crate in the lock, smaller and faster than ours, strict on malformed notes, and its note walk
  proves in seconds. Today it cannot link into the kernel without a `#[global_allocator]`. The only
  way round that is a refusing allocator, a foot gun. It also accepts an empty segment at any offset
  and lenient last-note padding, which our policy would have to keep catching.
- C. Contribute to `object`, then switch. Upstream: make `alloc` optional for `read_core` (the
  reader allocates nothing on this path), and offer the note-walk and segment harnesses with a CI
  job. Switch once a release has it. This retires our parser and keeps the policy.
- D. Contribute to cole14's `elf`, then switch. It fits today and forbids `unsafe`, and P1, P2, P5,
  P6 and P7 already hold. But its note walk swallows errors and misreads `p_align` 1, and fixing
  the first is a breaking change its author has left open since 2025-09. One release in three
  years, and outside pull requests wait a year.
- `goblin` is not an option. It needs `alloc` for anything past the header, panics on a misaligned
  header without it, reads notes outside their segment, and resisted the model checker.

Recommendation: A, and C only if calef wants the parser gone. The measured gain from switching is
small: about 280 lines, and 20 ns a spawn. Against it, the kernel's loader would take on a crate
with `unsafe` casts, three breaking releases in the last year, and no fuzzing of its own, in place
of code that is proved and strict. I would choose A at equal effort, so this is not an effort
argument. C is the right way to switch if the answer is "switch": the upstream change is small,
and the proofs we would bring are ones that already hold on its code. D loses to C on upkeep and on
the wrong-bytes case, not on effort.

Ruled: A. calef, 2026-10-22 (UTC), on #2583: "A". `crates/elf` stays ours under the kernel
exemption.
