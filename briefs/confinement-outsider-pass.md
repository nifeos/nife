# Brief: an outsider pass at the confinement claim

**Name: provisional.** The lane that runs a pass hands the reviewer the section "The reviewer's
brief" below and keeps the rest for itself.

## For the lane

- Mint the pass's block the way milestone 903 (a seventh outsider pass attacks the confinement
  claim) was minted, and claim it with `script/claim <branch> --new-work`.
- Launch the reviewer with `script/launch-pass --reviewer glm|fable --commit <sha>`. It clones the
  pinned commit, builds the one prompt from the reviewer's brief and the runner section in [the
  launch brief](confinement-outsider-pass-launch.md), and pins the model. It refuses a milestone
  number or a roadmap block as the prompt. Those hold the lane's instructions, and a reviewer
  given one follows them. The launch brief holds what the script leaves to the lane.
- Every pass is informed, milestone 800 (a non-Anthropic model attacks the confinement claim)'s
  posture: the reviewer has the whole tree and its history at the pinned commit (calef,
  2026-10-14 UTC).
- Criterion (c) needs two consecutive clean passes, and at least one must come from a
  non-Anthropic model or a human. An Anthropic pass can only be the other one.
- The launcher stops a session whose reported model differs from the pin before it sends the
  brief, and its launch record holds the model reported. Copy that record into the pass note.
- The lane briefs, collects, reproduces each claimed escape by test or boot, and records. It does
  not attack and it does not hint.
- Reproduce every booted row's test yourself, one ISA's QEMU at a time. A timeout under load is
  inconclusive, not a pass.
- Count booted rows with `script/booted-rows` over your own reproduction logs, never from the
  report's own total, and quote its output in the pass note. Below the floor, run the completion
  round below once. Still below it, the pass is recorded in full but does not count toward
  criterion (c), and the note says so.

## The completion round

A pass below the floor goes back to the same reviewer, in the same session, once (calef,
2026-10-14 UTC):

```sh
script/booted-rows --short-only --checkout <clone> --pinned <sha> --log <log>... > short-rows
script/launch-pass --complete <scratch>/launch.json --short-rows short-rows
```

- The message is the short rows and nothing else, one line each with the reason it is short
  (`row 10: host only`, `row 19: rerun of the row's cited test`). The reviewer's brief already
  tells it what to do with them. It carries no finding, no file and no hint of where to look.
- The pass counts if the recount, with the reviewer's new tests reproduced, meets the floor. An
  escape found in the round counts like any other.
- One round. A reviewer that leaves rows short twice has not attacked them, and a second round
  makes the lane its supervisor.
- A session that cannot be resumed gets no round. A new session is a new pass. The launcher
  refuses a second `--complete` on one launch record.

## The booted-row floor

**Every row a boot can reach: 28 of the 35** (calef, 2026-10-12 UTC, on #2010, at 27 of 34:
"27 seems right").
Rows 12 to 18 are host proofs of the DMA validator, the VT-d encoder and the wiring plan. No
confined component drives them at boot, so they are exempt and graded `host`. When the table
grows, the floor is every row a boot can reach.

`script/booted-rows` reads the lines below and counts the rows of the claims table in
[`notes/confinement-claims.md`](https://github.com/nifeos/record/blob/main/notes/confinement-claims.md); the floor is every row there that is not exempt.

    exempt: 12 to 18
    carried from pass 7: 1 to 3
    x86_64 only: 27 to 29, 33

A row counts as booted when a hostile test written for this pass ran under QEMU on at least one ISA
the row applies to. A rerun of the row's own cited test does not count. One test may cover several
rows. All three ISAs are preferred. A timeout is inconclusive, not a run.

Rows 1 to 3 may be carried from pass 7. They count as booted when the reviewer reads pass 7's
attack tests and, for each, either accepts it in writing (what it attacks and why that is enough)
or strengthens it and boots the result.

## The reviewer's brief

### Context

This is the nife project's own kernel. Its maintainers authorized this pass. Every attack test runs
only under QEMU, on the maintainers' machine or a cloud runner they started, in a scratch branch.
Its purpose is to falsify nife's published confinement claims before anyone else does: defensive
verification of the project's own code.

### Your job

You are an outside security reviewer. Break a published security claim of an operating system you
did not write. Assume every claim is false and look for the case that makes it so. An escape is the
most useful thing you can find.

nife is a capability microkernel in Rust for aarch64, riscv64 and x86_64. Its confinement claim: a
confined component cannot reach an object it was not granted. The numbered claims are in the table
at `<path>`. Work in `<worktree>` only.

### What you may read

The whole tree and its history at your commit, as any attacker of a public repository has them.
Earlier passes' notes, audit reports and fixed escapes are in it. Report a re-found fixed escape
apart, as a re-discovery; only a new finding counts toward the claim.

### What counts

An attack counts when it boots. Write your own hostile program or system test, aimed at the case
you think breaks the claim, and boot it. Running a test that already exists is not an attack. Every
row except 12 to 18 must be booted for the pass to count: 28 of 35. Reading is the weakest grade.
Where a claim concerns a userspace server, build a hostile client that drives it. Before you
report, count your booted rows against the floor with `script/booted-rows` (see Mechanics). If
you fall short, the maintainers may send you its short-row lines once in this session; boot those
rows and update your report.

Rows 1 to 3 have attack tests from an earlier pass in `system_tests/src/user/`:
`seventh_pass_delegation_tests.rs` (rows 1 and 2) and `seventh_pass_retype_tests.rs` (row 3). Read
each. Either accept it in writing, or strengthen it and boot the result. Either way the row counts
as booted. A written acceptance is one line in your results file, `Accepted row 1: <what the test
attacks, and why that is enough>`, one per row.

### Mechanics

Read `notes/scripts.md` and [`notes/qemu.md`](https://github.com/nifeos/record/blob/main/notes/qemu.md). The two test files above are a working harness to copy:

- Layout: one module per attack family under `system_tests/src/user/`, declared in
  `system_tests/src/user.rs` under `#[cfg(test)]` with a doc comment saying which claims it
  attacks. Each test drives the real dispatcher through a local `call` helper over
  `system_call::invoke`.
- Names: new code spells `capability` out. Alias the kernel's `cap` names at the import, as those
  files do (`rendezvous_cap as rendezvous_capability`, a `const SEND_CAPABILITY`), because
  `script/lint` refuses new `cap` identifiers.
- Rows: each module's `//!` header carries one line naming the claims it attacks,
  `Attacks rows: 4, 5, 9` (a range is `6 to 9`). A test in a module shared with other work carries
  the line in its own `///` doc comment instead. A test with no such line counts for no row.
- Running: `script/test --arch aarch64 --test <module>`, then `riscv64`, then `x86_64`, one at a
  time and never in parallel. Keep each leg's whole output as a log under `<logs>`, with the ISA in
  its file name: `script/test ... > <logs>/<module>-aarch64.log 2>&1`.
- Counting: `script/booted-rows --checkout <worktree> --pinned <commit> --results <results path>
  --log <logs>/*.log` prints every row's grade and `booted N of 28`. A row counts when a test
  written in this pass and naming it ran to `ok` or a panic on an ISA the row applies to.

Never run `script/verify`, a mutation sweep or Kani. Interactive runs go through
`helpers/qemu-bounded.sh`. Put `/opt/homebrew/opt/python@3.14/libexec/bin` first on `PATH`. Shape
an exploit so a break ends the run at an assertion, exit or fault. When you finish,
`pgrep -l qemu` shows nothing you started.

### Escapes

Each escape is a test that is red because of it, booted to confirm and committed before anything
else. Do not write the fix. A red test that would break the default suite is opt-in and says so. A
path one bug away, or holding by accident, is a near miss.

### Commits

Commit on the current branch; do not push, rebase or branch. One purpose each. Never use
`git stash`; never edit `design/`, `AGENTS.md` or `CLAUDE.md`. Run `cargo fmt` first.

### Output

Write to `<results path>`, outside the repository:

1. One row per claim: the attack, the outcome (escape, near miss, held, or untestable with the
   reason), the grade (`booted <isas>`, `host` or `read`), and the test.
2. The rows you attacked hardest, and why you chose them. Name the rows you only touched lightly
   too, so a shallow pass shows in your own report.
3. A section per escape and near miss, with the reproduction command and the observed lines.
4. Surfaces attacked that have no row.
5. A refusal log, last. List every step you declined, and every stop by a safety system or
   classifier, including one before any tool ran. Give the claim, one sentence with no exploit
   detail, and whose refusal it was. An empty log says "none".

Write plainly, with no em-dashes and American spelling. End with counts by outcome and by grade.
