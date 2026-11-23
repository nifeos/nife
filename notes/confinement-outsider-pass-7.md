# Confinement outsider pass 7 (2026-10-12 UTC)

The seventh outsider pass at risk 7 (the confinement claim is false), milestone 903 (a seventh
outsider pass attacks the confinement claim), whose number is provisional. Asked for by calef on
2026-10-12 (UTC) after the sixth pass's escape restarted criterion (c)'s count at zero.

Blind, milestone 633 (an outside agent attacks the confinement claim)'s posture, not the informed
posture of passes 4 to 6. The lane spawned a fresh Claude agent on Fable and briefed it with two
things: the 34-row claims table and the source tree. The brief named no audit report and no earlier
pass note. It asked the reviewer not to open the claims file's narrative, `design/audit-reports/`,
`design/fatal-risks/`, `design/roadmap/`, any earlier outsider or falsification note, or git
history. The lane did not attack and did not hint. The counting rule, the report format and the
refusal log are milestone 800 (a non-Anthropic model attacks the confinement claim)'s.

**Status: no escape, one near miss that is already known and ruled. Three of 34 rows were booted on
all three ISAs; 31 are graded `read`. It does not count toward criterion (c).**

## The ruling: the count stays 0

calef ruled on #2010 (2026-10-12 UTC), option (b): the pass does not count toward criterion (c),
because only 3 of 34 rows were booted. The count stays at 0. The next pass's brief sets a floor of
booted rows, and `nifeos/nife:briefs/confinement-outsider-pass.md`
now holds that brief. Its floor, ruled by calef on #2010 the same day ("27 seems right"), is
every row a boot can reach: 27 of 34, since rows 12 to 18 are host proofs no confined component
drives. Rows 1 to 3 count when the next reviewer accepts or strengthens this pass's tests in
writing.

## Two attempts, the first discounted

The first reviewer read `notes/confinement-claims.md` past the end of the table, into the narrative
of earlier findings. It said so in its own report. That breaks the independence rule, so the lane
discounted the attempt. It also booted nothing new: one run of an existing test, every other row
read. The lane then spawned a second fresh reviewer, handed it a copy of the table alone, and told
it that a pass held mostly by reading is weak evidence. The table below is the second reviewer's.
Everything the first attempt found, the second found too.

## What was booted

Both tests were written by the reviewer for this pass (names provisional) and booted green on
aarch64, riscv64 and x86_64. The lane reproduced both on all three ISAs, one ISA at a time, with
`script/test --arch <isa> --test seventh_pass`.

- `seventh_pass_delegation_tests::a_delegation_cannot_widen_a_right_or_lend_a_grant_less_capability`
  (claims 1 and 2). A `SEND_CAP` whose rights register asks for `WRITE` the source lacks, and a
  `SEND_CAP` of a `GRANT`-less source narrowed to a strict subset, both through the real dispatcher.
  Both are refused `NotPermitted` and park nothing. The host proofs cover the same rule on pure
  logic; this drives it at the kernel boundary. The lane falsified it on aarch64 by dropping the
  subset check in `Delegation::derive`: the run failed, but by the watchdog, because the widened
  copy parks the test thread on a rendezvous with no receiver. The reviewer's doc comment said a
  regression would fail at an assertion; the lane corrected it.
- `seventh_pass_retype_tests::a_grant_less_budget_narrows_a_split_child_but_mints_full_rights_on_every_retype`
  (claim 3). The near miss below, extended from frames to kernel objects.

## The near miss: a `GRANT`-less budget retypes `GRANT`-bearing objects

Claim 3 as written is about `SPLIT`, and `SPLIT` holds: a `WRITE`-only budget's child stays
`WRITE`-only. `RETYPE` and `RETYPE_OBJ` mint their result with `Rights::ALL`, so the same budget
yields a page frame, rendezvous, notification or timer carrying `GRANT`, which the holder may then
lend. It is the holder's own memory, so nothing outside its grant is reached. It is the gap calef
ruled on 2026-10-07 (option (a), PR #1798) and milestone 824 (a retype mints no right its budget
lacks) fixes, NOT-STARTED. New here: `RETYPE_OBJ` has the same mint for every kernel object, not
only frames. The test asserts the gap, so it goes red when 824 lands, as the existing frame-only
characterization does.

The reviewer reached this from the dispatcher and then met the in-tree record of it (the `BUGS`
entry at `kernel/src/cap.rs`'s `memory_region_cap` and the existing test's doc comment), which
milestone 633's own `BUGS` says a brief cannot remove. Scored as a re-discovery of a ruled gap.

## The claims

Grades: `booted` means a hostile test written this pass and run under QEMU on aarch64, riscv64 and
x86_64. `read` means source review only. "Cited" means the row was not attacked independently and
the verdict rests on reading plus the coverage the table already cites.

| # | Attack | Outcome | Grade |
|---|---|---|---|
| 1, 2 | `SEND_CAP` widening through the rights register; lending without `GRANT` | held | booted |
| 3 | `SPLIT` versus `RETYPE` and `RETYPE_OBJ` off a `WRITE`-only budget | `SPLIT` held; `RETYPE` near miss | booted |
| 4, 5 | Read slot lookup, `delete` and generational names | held | read |
| 6 to 9 | Read `reap_decision` ordering and the supervision set behind `REAP` and `SURVEY` | held | read |
| 10, 11 | Read the user-VA gate on every map path and the leaf flags `page_frame_map` sets | held | read |
| 12 to 17 | Cited: the VT-d encoder and DMA validator proofs; no confined driver programs a device | held | read |
| 18 | Cited: the component-plan host test | held | read |
| 19, 24 | Read `attenuate` (an AND with the parent) and the composition model | held | read |
| 20 to 23 | Cited: the C seam, kernel-address, ELF and progenitor tests | held | read |
| 25 | Cited: the compositor tests | held | read |
| 26 | Read the dispatcher: the server-side methods need `READ` or `ENUMERATE` | held | read |
| 27 to 29, 33 | Cited, x86_64 only: the port tests; read the `PortRange` rights gate | held | read |
| 30 | Read `Delegation` and `grant_derived`: source read and copy filed under one hold | held | read |
| 31, 32 | Cited: the `script/swish-check` censuses | held | read |
| 34 | Cited: the socket confinement tests | held | read |

Surfaces with no row were read, with no gap seen. `RETYPE_OBJ`'s slot floor never overwrites an
occupied slot. `PageFrame::SLICE` needs `GRANT` and copies the source's rights. `PageFrame::MAP`
makes no executable or writable-beyond-rights user page. `Rendezvous::BADGE` needs `GRANT` and sets
a badge once. `Notification::BIND` needs a TCB capability, so it reaches only a thread the caller
already controls.

Not reached: the milestone 801 (packages over the internet) package fetch, which has no row (it
waits on milestone 855 (the TLS graph enters the gated build)), and every userspace server a
hostile client could drive. A blind reviewer chose the capability core instead.

## Refusal log

1. **Setup, before any claim (attempt 1).** The reviewer's first turn, orienting in the tree, was
   stopped before any tool ran. An external safety system's; it then proceeded. Not on a shipped
   path.
2. **Setup, before any claim (attempt 2).** The same, on the second reviewer's first turn. An
   external safety system's; it restarted and completed. Not on a shipped path.
3. **Independence (attempt 1).** The reviewer read past the claims table into earlier findings. Its
   own error, reported by itself; the attempt was discounted, and attempt 2 was given the table
   alone.

No shipped-path step was declined, so the log leaves no shipped-path refusal unexamined.

## BUGS

- Thirty-one of 34 rows are graded `read`, and about half of those were not attacked independently
  at all: the verdict cites the row's own test. Pass 4 had the same shape (2 booted, 31 read). This is the
  weakest evidence a pass can give for those rows, and it is why the pass does not count.
- Both reviewers were Anthropic models, so this pass can only be criterion (c)'s Anthropic half.
- The near miss was in the source the reviewer was allowed to read, so independence did not hold
  for it.
- The delegation test fails by the watchdog rather than at its assertion when the subset check is
  dropped. Red either way. The falsification is `attested`, not a replayable patch.
