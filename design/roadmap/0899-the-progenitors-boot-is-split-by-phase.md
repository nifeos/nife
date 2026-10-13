---
status: BUILT
raised: 2026-10-11
built: 2026-10-12
promoted_from: the-progenitors-boot-is-split-by-phase
milestone_dependencies: 843
decision_dependencies: 266, 279
machine_requirements: none
specific_machine: none
needs_person: no
---
# 899. The progenitor's boot is split by phase

*(The title and slug are provisional.)*

It is the named follow-on of milestone 843 (the system-initializer crate root is split along its
seams).

Milestone 843 (the system-initializer crate root is split along its seams) moves
`system_initializer::boot` into a file of its own unchanged. That meets the ceiling of §266 (a Rust
source file stays under 2,000 lines), but `boot` is still one body of 1,700 lines. A reader editing
the shell's step still works with locals bound while the archive was read. calef ruled on
2026-10-09 (UTC) that the kernel's own boot sequence, `kernel_main`, is split by boot phase, one
function per phase
([the function-length ratchet proposal](0888-a-ratchet-on-function-length.md), open question 4). This is
the same split for the progenitor's half of booting.

Reuse: not applicable; this restructures code and adds none.

## The phases, measured

Measured on main at `ae6b91e79`, before 843's move, from `boot`'s top-level statements. Each phase is
named by the comment that opens it, since the move changes every line number. Counts are physical
lines, comments included. "Carried" counts the top-level bindings made in that phase and read in a
later one.

| phase | opens with | lines | carried |
|---|---|---|---|
| signature and doc | "Build the interactive system and become its spawn service" | 14 | |
| read the archive | "The archive the kernel mapped read-only" | 229 | 25 |
| entropy | "Read before anything below retypes" | 159 | 3 |
| network stack | "The network stack (milestone 590 (provisional)" | 37 | 1 |
| steps 1 and 2: console, line discipline, terminal | "The two components that have to exist before the progenitor can say anything" | 224 | 5 |
| the refusal | "Refuse the system if a required component is not the one that was measured" | 48 | 3 |
| step 3: input, the USB keyboard, the spawn channel | "3. The keystroke source" | 85 | 3 |
| step 4: the shell | "4. The shell" | 229 | 4 |
| step 5: the sink adapter | "5. The terminal's sink adapter" | 40 | 1 |
| the login stack and the run-unvouched endpoint | "Milestone 49's login stack" (milestone 49 (users and attribution)) | 358 | 2 |
| the giveaway and what the progenitor says | "Give the construction budget away" | 188 | 4 |
| step 6: the undertaker, then the spawn loop | "6. The undertaker" | 89 | |

70 top-level bindings, 51 of them read in a later phase. 25 of those 51 come from reading the
archive: each component's measured image, in one pass so a program is hashed once. That pass is
the natural first function, returning one struct of verdicts.

`spawn_service` is the same shape at 1,026 lines: 30 lines of setup, then one `loop` whose body is
about 970 lines handling one request. Its seams are the request's own stages (decode the wiring,
receive the grants and frames, carve the job's memory, build and endow the child, answer). It
belongs in the same milestone or the next.

## What makes it hard

- Retype order is load-bearing. Every object `boot` makes lands in the first free slot, and the
  entropy phase's comment records a slot collision found by bisection, not reasoning. A phase
  function must not reorder a retype across a phase boundary.
- The progenitor's table peaks inside the login phase, "one slot from the wall". Moving a
  capability's lifetime into or out of a function can move that peak.
- There are no host tests. `script/swish-check` on aarch64, riscv64 and x86_64, which boots the
  real progenitor and types at the prompt, is the only proof, with `script/test` on all three.
- Stack. The progenitor's stack is measured (`kernel/src/progenitor_stack.rs`); a phase function
  kept out of line changes frame sizes, so the headroom is re-measured.

## Done when

1. `boot` is a sequence of calls, one per phase above, and no phase function is over 300 lines.
2. The retype order is unchanged. The lane shows that by tracing the slot each object lands in, on
   one boot per architecture, before and after.
3. `script/swish-check` and `script/test` pass on aarch64, riscv64 and x86_64, and both
   `swish_check` falsification patches still fail their lines.
4. If the function-length ratchet has landed, `boot`'s row is banked down or removed.

## What was built

Built in #1993 and its follow-up, under milestone 888 (a ratchet on function length).

- `boot` is 75 code lines, a sequence of step calls. Its steps live in a new private module,
  `crates/system_initializer/src/boot_steps.rs` (name provisional), because keeping them beside
  `boot` would have taken `boot_sequence.rs` past §266's ceiling. The largest step is
  `build_shell`, at 68 code lines. Each step is `#[inline(never)]`, so its temporaries stay out of
  `boot`'s frame, which lives as long as the spawn loop. On aarch64, that frame went from 4,368 to
  3,792 bytes in release and from 14,752 to 5,520 in debug.
- `spawn_service` went from 609 code lines to 13 in #1993. A `Service` struct now serves one
  request per call, with one function per stage.
- Item 2: a temporary kernel patch, never committed, made the progenitor print the occupancy mask
  of its 64 slots after every syscall that changed it, with a running FNV-1a hash of every mask so
  far. It ran on one `script/swish-check` leg per architecture, before (`8daf695a9`) and after.
  - At the giveaway, the kernel-computed hash is identical before and after on every boot
    configuration: three on aarch64, one each on riscv64 and x86_64.
  - It stays identical well past the giveaway, to the 2,958th change on riscv64, the 2,968th on
    aarch64 and the 2,999th on x86_64.
  - The x86_64 trace rebuilds line by line from its masks, all 1,264 changes up to the giveaway,
    and is the same before and after. The other two lose lines where console output splices into
    them, so only their hashes compare.
- Item 3: `script/test` and `script/swish-check` exit 0 on aarch64, riscv64 and x86_64. The
  `swish_check_leg` patch was re-cut against the split code. It still fails on `caps` with the
  recorded slot list, and `swish_check_boot` still fails on `installed/unvouched`.
- Item 4: `boot`'s row is gone from `design/lint-baseline.tsv`. Peak progenitor stack, from
  swish-check's gauge, fell by about 9 KiB on every architecture.

## Follow-on

- **Decision.** The module name boot_steps is provisional and is calef's to rule on, with the rest
  of the names in
  `design/decisions/0279-the-system-initializer-crate-root-splits-into-private-modules.md`.

## Index row

The progenitor's `boot`, once one body of about 1,700 lines, is split into one function per boot
phase, the split calef ruled for the kernel's own `kernel_main`, with the retype order traced
unchanged on all three architectures.
