# Confinement outsider pass 8 (2026-10-13 UTC)

The eighth outsider pass at risk 7 (the confinement claim is false), milestone 913 (an eighth
outsider pass attacks the confinement claim), whose number is provisional. It is the GLM 5.3 half of
the pair calef ruled on 2026-10-13 (UTC): two passes at one pinned commit, one of them
non-Anthropic. Milestone 914 (a ninth outsider pass attacks the confinement claim) is the other.

Status: no escape, four near misses. The reviewer's table grades 25 of the 27 bootable rows
`booted`, and its own count says 27. Its blindness was broken before it read its prompt, and it
said so first. It does not count toward criterion (c).

## The ruling: the count stays 0

calef ruled on 2026-10-14 (UTC): "pass 8 does not count." Criterion (c)'s count stays 0. The facts
he had are below: the breach, the floor counted three ways, and the reproduction.

## The breach, stated first

The posture was blind, milestone 633 (an outside agent attacks the confinement claim)'s. calef
started the reviewer with "Do milestone 913" in the maintainers' workspace, not with the one
prompt the launch brief assembles in the scratch clone. Before it found its prompt, the session
read:

- milestone 913's block and milestone 914's block, both under `design/roadmap/`, which the posture
  forbids;
- `briefs/confinement-outsider-pass-launch.md`, whose name contains `outsider`, and the names in
  `briefs/`;
- `git log --oneline -5` of the workspace's main, which is history;
- `gh pr view 2035`, which is GitHub, for 824's merge commit and title.

Its report discloses the first two and says the leak was pass 7's retype near miss and the floor.
The harness log shows the other two as well. Three research subagents kept to the scratch clone.
One grep there returned a single matching line each from `design/fatal-risks/7-the-confinement-claim.md`,
`notes/confinement-outsider-pass-3.md` and milestone 715 (the spawn service holds the display grants
and the shell holds none)'s block. All three lines are about row 32's `caps` census.

### What the breach added

Most of what leaked was already in the prompt or in files the posture allows.

| Leaked | Already given legitimately |
|---|---|
| The floor: 27 of 34, rows 12 to 18 exempt, one ISA enough | The prompt's "What counts" says the same |
| Rows 1 to 3 may carry pass 7's tests when accepted in writing | The prompt names both files and the rule |
| Pass 7's near miss was the retype gap; 824 fixed it; row 3's test now holds | Row 3 of the table names milestone 824 (a retype mints no right its budget lacks) and both patches; `seventh_pass_retype_tests.rs`'s header tells the whole story |
| A Fable pass runs at the same commit | Nothing |
| The lane reruns booted rows; timeouts under load are inconclusive | The prompt's machine section says the second |
| Five commit titles on main, none about a confinement finding | Nothing |
| Pass 3 graded row 32 `read` | Nothing |

New information, then: that a concurrent pass existed, five unrelated commit titles, and one line
of pass 3's grading for row 32. None of it names a weakness.

### Row by row

- Row 3. Not attacked afresh. The reviewer accepted pass 7's test in writing and booted it on
  aarch64. The brief allows that, and calef's floor ruling on #2010 carries rows 1 to 3 that way.
  Its written acceptance names what the test attacks, and gives as its reason that the assertions
  now hold and the falsification patches replay. That is thin on "why that is enough". Its
  phrase "held assertions" matches the 913 block, but the test's own header says the same. The
  breach told the reviewer this row was a past near miss, which it could also read in that header.
  It then spent the least effort here of any row.
- Row 1. The same carry-over, accepted in writing with a reason ("the wire form of the claim").
  No row-specific leak.
- Row 32. The one row a leaked line touched. The reviewer leaned on `script/swish-check`'s
  census for the boot shell and booted only a spawned child's census. The leaked lines say the
  same census exists; they do not explain how to get past it.
- Every other row. Nothing leaked about it. The floor's knowledge could bend grading toward
  the floor on any row; the prompt carried the floor anyway.

## The floor, counted three ways

The brief: "A row counts as booted when a hostile test written for this pass ran under QEMU on at
least one ISA the row applies to." Rows 12 to 18 are exempt; the other 27 must boot.

| Reading | Rows | Count of 27 |
|---|---|---|
| The report's own count | 1 to 11, 19 to 34 | 27 |
| The report's own table | 1 to 9, 19 to 34; rows 10 and 11 graded `host` | 25 |
| Fresh attacks only, no carry-over | 2, 4 to 9, 19 to 34 | 23 |

Rows 10 (the user-VA gate) and 11 (W^X) were attacked with new host tests in `crates/page_tables`, not
booted. They are not among the exempt rows. Row 2 is fresh: `eighth_pass_endpoint_usurper_tests`
asks for `READ` through `SEND_CAP`'s rights register, which is row 2's claim.

Three booted rows attack a proxy rather than the claim's own subject. Row 32's claim is about the
boot shell; the test spawns a child and censuses it. Row 23's claim is about the progenitor; the
test's child never held construction authority to drop. Row 31 builds the unvouched shape by hand,
not through the shell's path. Strict on row 32 alone, the fresh count is 22.

## The run

- Reviewer. GLM 5.3 (`zai-coding-plan/glm-5.3`) in opencode, one user message, no follow-up.
  Started 2026-10-13 18:59 UTC; report written 22:55 UTC.
- Commit. `643e0bef86a626d3be9e77e5369ee4c73e47d814`, the merge of #2035 that brings milestone 824
  to main, the commit 914 also attacks.
- Results. Twelve commits on the reviewer's local branch, `d2ae0acf` to `d5628b79`, each ending
  `Co-Authored-By: GLM 5.3 <noreply@z.ai>`. Nothing was pushed. Eight new modules under
  `system_tests/src/user/`, all `eighth_pass_*` (names provisional). New tests in `c_seam_tests`,
  `compositor_tests` and `net_confinement_tests`. Host tests in `paging`,
  `direct_memory_access_validator` and `component_plan`. Three hand-assembled `x86_64` stubs, a C
  underrun, and new fixture roles. No enforcement code changed: the non-test edits are constants,
  roles and visibility.
- Host. The maintainers' Mac, under other lanes' load: load averages from about 7 to 26, by the
  report's `uptime` lines. No leg timed out.

## Reproduction

Every booted row reproduced. The lane reran the reviewer's branch on 2026-10-13 and 2026-10-14
(UTC), one ISA's QEMU at a time, with `script/test --no-host --arch <isa> --test <filter>`. The
filters were `seventh_pass`, `eighth_pass`, and the three new tests in existing modules.

| Filter | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| `seventh_pass` (rows 1, 3) | 2 of 2 | 2 of 2 | 2 of 2 |
| `eighth_pass` (rows 2, 4 to 9, 19, 21 to 24, 26 to 33) | 15 of 15 | 15 of 15 | 19 of 19 |
| `a_c_underrun_below_the_grant` (row 20) | 1 of 1 | 1 of 1 | 1 of 1 |
| `a_forged_control_page_changes_nothing` (row 25) | 1 of 1 | 1 of 1 | 1 of 1 |
| `a_re_badged_socket_capability_is_refused` (row 34) | 1 of 1 | 1 of 1 | 1 of 1 |

`x86_64` runs four more tests than the others: the port family and the port leg of row 30. Its
second machine configuration has no RedoxFS disk, so the two directory tests skip there; both
passed in the first. Another lane's QEMU was running throughout, with load averages from 17 to 34.
No leg timed out, so none needed an idle rerun. Rows 1 and 3, which the reviewer booted on
aarch64 only, passed on all three ISAs.

The host rows reran green with `cargo test -p page_tables -p direct_memory_access_validator -p
component_plan -p c_seam`: rows 10 to 18, every new test.

## The claims

Grades are the reviewer's. `booted` means a hostile test written this pass ran under QEMU on the
ISAs named. `accepted` means pass 7's test, accepted in writing and booted once. `host` means a new
host test. The last column is the lane's rerun.

| # | Attack | Outcome | Grade | Rerun |
|---|---|---|---|---|
| 1 | Pass 7's `SEND_CAP` widening test, accepted | held | accepted, aarch64 | yes |
| 2 | `READ` asked through the rights register; a slot past the table and `u64::MAX` | held | booted, three ISAs | yes |
| 3 | Pass 7's `SPLIT` and `RETYPE` test, accepted | held | accepted, aarch64 | yes |
| 4, 5 | Use after delete by number, double delete; a neighbor byte-identical after a delete | held | booted, three ISAs | yes |
| 6 to 9 | A full supervisor of one domain reaps and surveys another's live and dead members | held | booted, three ISAs | yes |
| 10, 11 | The user-VA gate at each format's split; W^X over every leaf constructor | held | host | yes |
| 12 to 17 | VT-d reserved bits; wrapping lengths, mid-chain indirects, out-of-ring `next`, post-validate mutation | held | host | yes |
| 18 | A four-role plan, a surplus role and a repeated provision | held; near miss 3 | host | yes |
| 19, 24 | One right dropped at a time; each shell runs the other root's script | held | booted, three ISAs | yes |
| 20 | A C underrun one page below the grant | held | booted, three ISAs | yes |
| 21 | The outlaw reads kernel text and the direct-map alias of its own page | held | booted, three ISAs (alias leg aarch64, riscv64) | yes |
| 22 | Overflowing segment arithmetic, a band-to-kernel segment, a benign-then-kernel image, W+X in OS bits | held | booted, three ISAs | yes |
| 23, 31, 32 | A child with no budget tries to build; a spawned child's whole table read back | held | booted, three ISAs | yes |
| 25 | A client forges its whole control page after `HELLO` and commits after each forge | held | booted, three ISAs | yes |
| 26 | Self-upgrade through the rights register; a badged copy tried on the server's methods | held | booted, three ISAs | yes |
| 27 to 29, 33 | A non-holder's `in`; revoke precision; one range in two slots; `READ`-only `out` | held; near misses 1 and 2 | booted, x86_64 | yes |
| 30 | `SEND_CAP` paused after its rights gate and the frame revoked; a `PortRange` in flight | held | booted, three ISAs (port leg x86_64) | yes |
| 34 | A second badge for a minted socket | held | booted, three ISAs | yes |

Hardest attacked, by the report: rows 25, 30, 21, 19 and 24, 22, and the port family. Touched
lightly: rows 1 and 3, the host rows, row 32 and row 34.

## The near misses

1. A port-range revoke matches by object equality. Revoking `(0x3F8, 2)` leaves a holder of
   `(0x3F8, 8)` able to write 0x3F8. No syscall can name a narrower range than the caller's own
   capability, so it is unreachable. It is now a `BUGS` entry at
   `sched::delete_port_range_caps_from_others`.
2. Deleting one of two identical port capabilities drops the grant. It fails closed. It is the
   `BUGS` entry already at `sched::delete_current_cap`, so it is a re-discovery from an allowed
   source.
3. `component_plan::plan` accepts two roles routed to one slot. Nothing widens past the
   declaration. Read, not booted. It is now a `BUGS` entry in `crates/component_plan`.
4. The `x86_64` direct map does not cover the kernel image. That is deliberate: a writable alias
   of `.text` would undo W^X, and `mmu::claims` says so at the skip. `phys_to_virt`'s doc now says
   it too. Not a near miss on any claim.

Surfaces with no row, from the report: `Rendezvous::BADGE` as a server forgery; `read_volatile`'s
alignment check panicking a misaligned probe; `last_user_fault` being one global slot; and a
zero-length `SENDTO` refused on an open socket, against `socket_give`'s comment. The fault-slot
finding is now a `BUGS` entry at `x86_64`'s `last_user_fault`; the `SENDTO` one goes with the tests.

## Refusal log

None, by the report. The harness log agrees: one user message, and no stop by a safety system or
classifier. The breach above is not a refusal; it is listed first instead.

## BUGS

- The report's count says 27 booted while its own table grades rows 10 and 11 `host`. Read the
  counts in this note's floor table.
- The reviewer booted rows 1 and 3 on aarch64 only; the lane's rerun covers all three ISAs. Row
  21's alias leg runs on two ISAs, and rows 27 to 29 and 33 apply to `x86_64` only.
- Row 32's test censuses a spawned child, not the boot shell.
- The session ran in the maintainers' workspace with the whole tree in reach. After the prompt,
  the harness log shows no forbidden file opened, apart from the three grep lines above. That is
  honor, not isolation.
