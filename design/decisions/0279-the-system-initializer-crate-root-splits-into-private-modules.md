---
status: DECIDED
raised: 2026-10-11
decided: 2026-10-11
ratified_by: calef
---

# 279. The system-initializer crate root splits into private modules

Raised 2026-10-11 (UTC) as the cut that
[milestone 843 (the system-initializer crate root is split along its seams)](../roadmap/0843-the-system-initializer-crate-root-is-split-along-its-seams.md)
says is calef's call before a line moves.

It blocked the move and nothing else. Two questions were raised, each with a recommendation, and
both are now answered. Every module name is provisional and is left to calef's naming pass, so
neither question asked about one.

## The ruling

| Question | calef's words | What it decides |
|---|---|---|
| 1. Move items alone now, while `boot` stays one body | *"A"*, 2026-10-11 (UTC), on [PR #1957](https://github.com/nifeos/nife/pull/1957) | Move now. Decomposing `boot` and `spawn_service` is the follow-on, not this milestone. |
| 2. Where the layout constants go | *"Agreed, make the parent an index."*, 2026-10-11 (UTC), on [PR #1959](https://github.com/nifeos/nife/pull/1959) | A module of their own, kept whole, and the crate root an index. |

Question 1 is the recommendation, A. The follow-on is
[the progenitor's boot is split by phase](../roadmap/0899-the-progenitors-boot-is-split-by-phase.md),
which covers `boot` and `spawn_service` with the phase boundaries measured. It matches
[the function-length ratchet proposal](../roadmap/0888-a-ratchet-on-function-length.md), which
records calef's 2026-10-09 (UTC) ruling that `kernel_main` is split by boot phase and leaves every
other long function to a split of its own: "Each other long function is its own split". That
proposal's own lane builds the ratchet; this one does not.

Question 2 was not answered on this pull request. calef set the precedent for every split on the
pull request of milestone 872 (the kernel's user-mode file is split along its seams). When a file is
split, the parent holds crate docs, `mod` lines, `use` lines and re-exports, and nothing that can
grow into a feature. Every type and function lives in a child named for its topic. The only
exception is a handful of tiny items every child uses. So the layout constants leave `lib.rs`.
Beside their users would put `boot`'s file over 2,000 lines, so they take the third alternative
below, a module of their own, kept whole as the collision map it is. The same rule moves the rest
of the crate root out, as the cut shows. A separate lane is writing the precedent up as its own
decision, with a lint.

## What the file is today

`crates/system_initializer/src/lib.rs` is 5,850 lines on main at `ae6b91e79`. The block measured
6,358 at `0435a9aeb`. Milestone 809 (the package client becomes a program) has since moved package
activation out to `activation.rs`, and milestone 812 (`std::thread::spawn` runs real threads in one
address space) moved `StdLayout` to `std_layout.rs`. Both children take `use super::*`. Measured
from the file's own items, each with its doc comment:

| lines | count | what |
|---|---|---|
| 1 to 326 | 326 | crate docs with a `no_run` example, module lines, imports |
| 327 to 588 | 262 | `BootEndowment` and `SecondDirGrant`, the public types |
| 589 to 974 | 386 | address-space layout, page budgets, each stack's constants |
| 975 to 1014 | 40 | the password helpers |
| 1015 to 2714 | 1,700 | `boot`, one function |
| 2715 to 2860 | 146 | `Caretakers`, `Channels`, `Fs` and the slot helpers |
| 2861 to 3886 | 1,026 | `spawn_service`, one function |
| 3887 to 4692 | 806 | thin shapes over the ABI, the network stack and graphical terminal builders |
| 4693 to 5248 | 556 | an image request: `SpawnEndpoint`, the executable's bytes as frames the caller owns |
| 5249 to 5706 | 458 | the file service's client windows and `FsCalls` |
| 5707 to 5850 | 144 | `announce`, `build_terminal`, measured boot, the refusal sentence, `must` and `fail` |

The public surface is eight items: `boot`, `BootEndowment`, `SecondDirGrant`, and five constants
(`CHILD_STACK_PAGES`, `GRANT_MACHINE_PAGE`, `INIT_OWN_PAGES`, `JOBS_BUDGET_PAGES`,
`IMAGE_POOL_PAGES`). Two Rust files outside the crate use them, `components/src/progenitor.rs`
and `fixtures/src/hello.rs`.

## The cut

Private modules, children of the crate root, each taking `use super::*` as `activation.rs` and
`std_layout.rs` already do. An item that moves and is reached from another child becomes
`pub(super)`, which in a child of the crate root means the crate. No path outside the crate
changes: `lib.rs` re-exports the eight public items by name. Names are provisional.

| file | holds | lines |
|---|---|---|
| `lib.rs` | crate docs, `mod` lines, `use` lines, re-exports | 350 |
| `endowment.rs` | `BootEndowment`, `SecondDirGrant` | 269 |
| `layout.rs` | the layout constants, page budgets and each stack's constants, whole | 391 |
| `boot_sequence.rs` | `boot`, the password helpers, `EndowmentSlices` | 1,755 |
| `spawn_loop.rs` | the slot helpers, `Channels`, `Fs`, `spawn_service` | 1,176 |
| `children.rs` | the builders: grant, caretaker, network stack, graphical terminal, terminal; `reclaim`, `memory_region_split` | 861 |
| `image_request.rs` | `SpawnEndpoint`, frames, staging, vouching | 572 |
| `client_windows.rs` | client windows, bound channels, `FsCalls`, `PAGE_BYTES` | 473 |
| `announcement.rs` | `announce` and `sentence`, what the progenitor says | 66 |
| `measurement.rs` | `measured` | 28 |
| `fault.rs` | `must`, `must_ok`, `fail` | 29 |
| `activation.rs`, `std_layout.rs` | unchanged | 431, 189 |

Every file is under the ceiling of §266 (a Rust source file stays under 2,000 lines).

What stayed in the crate root: the crate docs (300 lines, with the `no_run` example), the `mod` and
`use` lines, and the re-exports. Nothing else qualified for the exception. `must`, `must_ok` and
`fail` are tiny, but only `boot_sequence` and `children` call them, so they have a child of their
own. `EndowmentSlices` is a type alias only `boot` uses, so it left the layout block for `boot`'s
file.

## Question 1, as raised: is moving items alone worth taking while `boot` stays one body?

`boot` is 1,700 lines in one body, with 139 `let` bindings, 72 of them at its top level.
`spawn_service` is 1,026 lines with 93. §266's checked reason is that an agent sees a whole file in
one read. A file that holds one 1,700-line function meets the number and only partly meets the
reason: a reader editing step 4, the shell, still works with locals bound in step 1.

Both functions are growing. Measured at the first-parent commit before each date:

| date | file | `boot` | `spawn_service` |
|---|---|---|---|
| 2026-09-10 | 2,679 | 1,197 | 430 |
| 2026-09-24 | 2,812 | 1,287 | 430 |
| 2026-10-01 | 5,310 | 1,544 | 744 |
| 2026-10-08 | 6,358 | 1,691 | 905 |
| 2026-10-11 | 5,850 | 1,700 | 1,026 |

Prior art:

| where | the same question | what it did |
|---|---|---|
| milestone 840 (the scheduler file is split along its seams), §271 (the scheduler file splits into submodules of `sched`) | a split of `kernel/src/sched.rs`, 8,888 lines | moved items only, no function changed; `schedule` and the send path kept their bodies |
| milestone 842 (the grant-plan crate root is split along its seams), §272 (the grant-plan crate root splits into private modules) | a split of `crates/grant_plan/src/lib.rs`, 7,034 lines | moved items only, and named a follow-on for the program table once jig installs packages |
| §266, section 1 and BUGS | what the ceiling measures | file length; a per-function limit was named as "a different problem, and not ruled out" |
| [the function-length ratchet proposal](../roadmap/0888-a-ratchet-on-function-length.md) | `kernel_main`, the kernel's boot sequence, 977 code lines; it lists `boot` at 847 and `spawn_service` at 545 | calef ruled on 2026-10-09 (UTC) that `kernel_main` is split by boot phase, one function per phase, not kept whole under an exception |
| Linux, `init/main.c` `start_kernel` (from memory, not read) | a boot sequence | one long function, but each step a call to a named `*_init` function |
| seL4, `try_init_kernel` (from memory, not read) | a boot sequence | one function of numbered stages, each a call to a `create_*` or `init_*` helper |

calef's ruling on `kernel_main` already answers where `boot` should end up: split by phase, as
the follow-on proposes. It does not answer the order, which is what this question asks. The two
peers are the same shape, and neither says anything about whether to move first.

The alternatives:

- Move now; decompose `boot` and `spawn_service` as a named follow-on. Recommended. The move is
  mechanical and checkable: `git diff --color-moved` shows every line moved unchanged, the compiler
  checks visibility, and `script/swish-check` and `script/test` on three architectures prove the
  boot. Cost: `boot`'s file sits at about 1,760 lines, so it has about 240 lines of headroom. At
  `boot`'s September rate, about 125 lines a week, that is two weeks.
- Decompose `boot` in this milestone, before or with the move. Cost: a refactor of 72 top-level
  locals whose retype order is load-bearing: the entropy block's comment in `boot` records a slot
  collision that was found by bisection, not reasoning. Its only proof is `script/swish-check`,
  since the crate has no host tests. Mixed into a 5,850-line move, no reviewer can tell a moved
  line from a changed one. The file took 35 first-parent merges in the 14 days to 2026-10-11, so a
  long refactor lane is the one most likely to conflict.

The recommendation holds at equal cost, because the reason is reviewability and proof, not effort.
If calef says no, the file stays as it is, held by the file-length ratchet at 5,850, and the
decomposition becomes the first step instead.

The ratchet is the mechanism that keeps the follow-on from being forgotten. `boot_sequence.rs` would
be a new file, so it meets the 2,000 ceiling outright and has no baseline row; the first change
that would push it past 2,000 fails `script/lint` and has to decompose first.

## Question 2, as raised: do the layout constants stay in `lib.rs`, or move beside their users?

Lines 589 to 974: about 60 constants and one type alias. Who reads each, from a grep of the crate:

| read by | constants | lines, about |
|---|---|---|
| `boot` alone | 41, including all the console, shell, login-stack and entropy addresses | 264 |
| `spawn_service` alone | 4: the child clock, configuration and job-page addresses, the directory job region | 40 |
| the network-stack builder alone | 5 | 29 |
| the caretaker builder alone | 2 | 12 |
| more than one module | `CHILD_STACK_PAGES`, `JOB_REGION_PAGES`, `FS_CLIENT_PAGE_VA`, `VIRTIO_DMA_PHYS_OFFSET`, `INIT_OUT_VA` | 50 |

Two facts decide more than the counts do.

- Beside its user, `boot`'s share puts its file at about 2,060 lines, over the ceiling on the day it
  lands. Moving them means a third file for `boot`'s constants alone.
- The block is a map, and its correctness is that entries do not collide. The progenitor's own
  window band (`INIT_OUT_VA`, `RNG_DMA_PEEK_VA`, `PROVISION_SCRATCH_VA`, `NET_DMA_PEEK_VA`) shares
  one address space across `boot` and two builders. The shell's addresses and a child's are checked
  against each other: `SH_CLOCK_VA`'s doc reasons from `CHILD_CLOCK_VA`, which `spawn_service`
  reads. Split by user, those comparisons cross files.

Prior art:

| where | constants of the same kind | what it did |
|---|---|---|
| milestone 840, §271 | `IpcTables` and the lock's state; `PRINT_CAP` | shared state stayed in the parent; a constant with one user moved with it (`PRINT_CAP` to `canary.rs`) |
| milestone 842, §272 | region page counts, the program table's `const fn` checks | moved beside their one user (`manifest.rs`, `program.rs`) |
| this crate, today | the graphical terminal's addresses, `ACTIVATION_FS_VA`, `IMAGE_STAGING_VA` | already beside their users, outside the block |
| `crates/address_space_map` | the bands every address space shares | one crate, one map, checked by `const` assertions |
| Linux, `Documentation/arch/x86/x86_64/mm.rst` (from memory, not read) | the kernel's virtual layout | one table in one place |

The alternatives:

- Keep the block in `lib.rs`. Recommended. It widens nothing, since a private item in the crate root
  is visible to every child. The map stays one read. `lib.rs` is about 1,110 lines. The constants
  already outside the block stay where they are.
- Move each beside its user. Matches 840 and 842 for single-user constants. Costs a third file for
  `boot`'s 264 lines, `pub(super)` on the five shared constants, and the collision checks above
  spread across four files.
- A module of their own, out of the crate root. Keeps the map whole and takes `lib.rs` to about 720.
  Costs `pub(super)` on all 60 and a named re-export of the five public ones, for a crate root that
  is already under the ceiling either way.

The recommendation held at equal cost: its reasons were that it widens nothing and keeps the map
one read, not that it moves fewer lines. It is superseded by the index precedent above, which takes
the third alternative. The cost that alternative names is paid: 51 constants became `pub(super)`.

## What the move costs beyond moving code

- Visibility. Items that move and are reached from another child become `pub(super)`, which in a
  child of the crate root means the crate. Nothing outside the crate widens.
- The falsification patches. `xtask/falsifications/swish_check.swish_check_boot.patch` and
  `swish_check.swish_check_leg.patch` patch `lib.rs`; each is regenerated against `boot`'s and
  `spawn_service`'s new files and shown to fail its line again.
- Line citations. 49 citations of `system_initializer/src/lib.rs:NNNN` in 19 files are on the code
  ratchet of §277 (cite a heading or a symbol, never a line number). Each is repointed to a function
  or a heading, read at the commit that wrote it, and the ceiling is banked down.
- The ratchets. `design/file-length-baseline.tsv` drops its row. `design/comment-block-baseline.tsv`
  holds two rows for this file: the crate doc (300 lines at line 6, which stays) and a 41-line block
  inside `boot`, which moves to its new file and line.
- The `cap` ratchet counts identifiers in changed files. With `use super::*`, the move adds no import
  line that names one.
- Proof. `script/swish-check` and `script/test` on aarch64, riscv64 and x86_64, `script/lint`
  and `script/coverage`.

## What it does not decide

No public path, wire format, syscall or behavior changes under any answer. No function body
changes. The decomposition of `boot` is its own milestone, filed as 843's follow-on with measured
step boundaries.
