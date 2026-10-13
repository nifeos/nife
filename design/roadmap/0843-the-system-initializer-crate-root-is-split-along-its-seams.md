---
status: BUILT
raised: 2026-10-08
built: 2026-10-11
milestone_dependencies: none
decision_dependencies: 266, 279
machine_requirements: none
specific_machine: none
needs_person: no
---
# 843. The system-initializer crate root is split along its seams

*(The title and slug are provisional.)*

`crates/system_initializer/src/lib.rs` is 6,358 lines at `0435a9aeb`. It was 2,805 lines on
2026-09-24, so it more than doubled in two weeks, faster than any other file over 4,000. §266 (a
Rust source file stays under 2,000 lines) sets the ceiling and a 2026-12-31 goal of no file over
4,000. Milestone 840 (the scheduler file is split along its seams) is the model.

The cut and every module name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

The lane wrote it up on 2026-10-11 (UTC) as §279 (the system-initializer crate root splits into
private modules), with the file re-measured on main at `ae6b91e79` (5,850 lines). calef ruled the
same day: the move alone now, with `boot` decomposed as the follow-on, and the crate root an index,
by the precedent he set for every split on #1959. §279 records both. Every module name still ships
provisional.

Reuse: not applicable; this moves code and adds none.

## What is in the file

Measured on `0435a9aeb` from the file's own top-level items. Ranges are approximate at the edges.

| lines | count | what |
|---|---|---|
| 1 to 437 | 437 | crate docs with a `no_run` example, imports |
| 438 to 691 | 254 | `BootEndowment` and `SecondDirGrant`, the public types |
| 692 to 1124 | 433 | address-space layout, page budgets, each stack's constants, the password helpers |
| 1125 to 2815 | 1,691 | `boot`, one function |
| 2816 to 2980 | 165 | `Caretakers`, `Channels`, `Fs` and slot helpers |
| 2981 to 3885 | 905 | `spawn_service`, one function |
| 3886 to 4217 | 332 | thin shapes over the ABI: `StdLayout`, `build_grant`, `build_caretaker`, `reclaim` |
| 4218 to 4805 | 588 | the network stack and the graphical terminal session builders |
| 4806 to 5223 | 418 | an image request: the executable's bytes as frames the caller owns |
| 5224 to 5671 | 448 | the file service's client windows and `FsCalls` |
| 5672 to 6238 | 567 | package activation: `activate`, `edit`, `fetch`, `receive_body` |
| 6239 to 6358 | 120 | measured boot, the refusal sentence, `must` and `fail` |

There are no tests in the file and no Kani harness. The crate depends on `user_mode_runtime`'s
EL0 `asm!`, so `script/lint` excludes it from the host pass. `script/swish-check` is the gate
that runs it, by booting the real progenitor and typing at the prompt.

## The caveat: two functions, not one file

Moving whole items can meet the number. `boot` alone, in its own file with its imports, is about
1,750 lines. Every other proposed module is under 1,100.

It does not meet the reason. §266's checked reason is that an agent should see a whole file in one
read. `boot` is 1,691 lines in one body, with 139 `let` bindings, 72 of them at its top level. A
reader editing step 4, the shell, works with locals bound in step 1. Moving `boot` to its own file
changes nothing about that. `spawn_service` is the same shape at 905 lines.

Decomposing `boot` along its numbered steps is the split that would help. It is a refactor, not a
move: the locals that cross steps become a struct or arguments, and the order of retypes is
load-bearing (the comments at lines 1360 to 1420 record a slot collision found by bisection). With
no host tests, only `script/swish-check` on three architectures would prove it.

So the recommendation is two steps. This milestone moves items only. The decomposition of `boot`
is follow-on work, filed by this milestone's lane once the move has made its diff readable.

## A proposed cut

Every name here is provisional. The modules are private and `lib.rs` re-exports the seven public
items, so `components/src/progenitor.rs` and `fixtures/src/hello.rs` do not change.

| module, provisional | from the table | about |
|---|---|---|
| `lib.rs` | 1 to 1124, re-exports | 1,130 |
| `boot_sequence` | 1125 to 2815 | 1,750 |
| `spawn_loop` | 2816 to 3885 | 1,080 |
| `children` | 3886 to 4805 | 920 |
| `image_request` | 4806 to 5223 | 420 |
| `client_windows` | 5224 to 5671 | 450 |
| `activation` | 5672 to 6238 | 570 |
| `measurement` | 6239 to 6358 | 120 |

`boot_sequence` and `spawn_loop` avoid the names of the functions they hold, `boot` and
`spawn_service`, for the reason milestone 842 (the grant-plan crate root is split along its seams) gives for
`parse`.

## What an architect has to rule

1. Whether the move alone is worth taking, given that `boot` stays 1,691 lines. The
   recommendation is yes: it meets §266's 2026-12-31 goal, and the decomposition is easier to
   review once `boot` is alone in a file.
2. Whether the layout constants (692 to 1124) stay in `lib.rs` or move beside the stack that uses
   each one. Most are read by `boot` and one builder, so either is defensible.
3. Every module name in the table.

No public crate API, wire format or syscall changes under any answer here.

## What moving the code breaks

Found with `grep` at `0435a9aeb`.

- The crate root has 131 private items. Each one a moved module reaches becomes `pub(crate)`. That
  widens nothing outside the crate.
- Two falsification patches patch this file: `xtask/falsifications/swish_check.swish_check_boot.patch`
  and `swish_check.swish_check_leg.patch`. Each must be regenerated and shown to fail its line again.
  A split of `xtask/src/swish_check.rs`, filed beside this one, renames the same two patches, so
  whichever lands second carries both changes.
- 41 citations of the form `system_initializer/src/lib.rs:NNNN` go wrong at once, across 37 files
  that name the path. A function name is cheaper to keep true than a line number.
- 57 first-parent merges in the 14 days to 2026-10-08 touched it, the most of the six files over
  4,000 lines. Take it when few lanes are open in it.

## Done when

1. `crates/system_initializer/src/lib.rs` is under 2,000 lines, and no new file is over 2,000.
2. No Rust file outside the crate changes.
3. `script/swish-check` passes on aarch64, riscv64 and x86_64, and `script/test` passes on all
   three.
4. Both falsification patches apply and fail their lines.
5. Each new module carries a `//! Name:` block marked provisional.
6. The follow-on that decomposes `boot` is filed, with its measured step boundaries.

## What was built

Built 2026-10-11 (UTC) on `42e0d1db6`. `crates/system_initializer/src/lib.rs` went from 5,850 lines
to 350, an index of crate docs, `mod` and `use` lines and re-exports, so its file-length row is
gone. Every name below is provisional.

| file | lines | holds |
|---|---|---|
| `lib.rs` | 350 | crate docs, `mod` and `use` lines, the eight public items re-exported by name |
| `boot_sequence.rs` | 1,755 | `boot`, the password helpers, `EndowmentSlices` |
| `spawn_loop.rs` | 1,176 | the slot helpers, `Channels`, `Fs`, `spawn_service` |
| `children.rs` | 861 | the builders, `reclaim`, `memory_region_split` |
| `image_request.rs` | 572 | `SpawnEndpoint`, frames, staging, vouching |
| `client_windows.rs` | 473 | client windows, bound channels, `FsCalls`, `PAGE_BYTES` |
| `layout.rs` | 391 | the layout constants, page budgets and each stack's constants, whole |
| `endowment.rs` | 269 | `BootEndowment`, `SecondDirGrant` |
| `announcement.rs` | 66 | `announce`, `sentence` |
| `fault.rs` | 29 | `must`, `must_ok`, `fail` |
| `measurement.rs` | 28 | `measured` |

`activation.rs` and `std_layout.rs`, cut earlier by milestones 809 (the package client becomes a
program) and 812 (`std::thread::spawn` runs real threads in one address space), are unchanged. Each
child takes `use super::*`; an item reached across a seam became `pub(super)`, and no Rust file
outside the crate changed. One correction rode along: `measured`'s doc comment had come apart from
it and sat at the top of `build_terminal`'s, and it is back on `measured`.

The two `swish_check` falsification patches were regenerated against `spawn_loop.rs` and
`boot_sequence.rs`. The citations of `system_initializer/src/lib.rs` by line under `notes/` now name
a symbol, or for the generated untested-error-paths table, a permalink to the commit it was
generated from.

## Follow-on

- **Milestone 899.** Milestone 899 (the progenitor's boot is split by phase):
  `design/roadmap/0899-the-progenitors-boot-is-split-by-phase.md`: `boot` along its twelve measured
  phases, and `spawn_service` along its request's stages. §279 records calef's ruling that this is
  the next step.
- **Decision.** Every module name is provisional and is calef's to rule in his naming pass, per
  `design/decisions/0279-the-system-initializer-crate-root-splits-into-private-modules.md`.
- **Refused.** Repointing the 17 citations of `system_initializer/src/lib.rs` by line under
  `design/` here. A developer lane edits no record under `design/` but its own block, and §277 (cite
  a heading or a symbol, never a line number) holds them in its code ratchet. Pull request #1957
  lists each with the symbol it should name, for the maintainer's sweep.

## Index row

`crates/system_initializer/src/lib.rs` more than doubled in two weeks, to 6,358 lines. A move of
whole items gets every file under 2,000, but `boot` stays one function of 1,691 lines, and only a
refactor would change that. This milestone moves; the refactor is its named follow-on. The cut and
every name are an architect's call.
