---
status: BUILT
raised: 2026-09-12
built: 2026-10-16
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 393. Four files each record which crates cannot compile for the host, and one of them derives it

Premise re-read against the tree on 2026-09-19 and still true, all
four still present. `script/lint` carries `--exclude user_mode_runtime` and its siblings by hand
in two clippy invocations (lines 41 and 120) *and* derives the same set for its own gate (line 812,
"the host pass excludes exactly the bare-metal crates"). `xtask/src/main.rs` carries the list twice
(lines 5133 and 5789). `script/coverage` carries it twice (lines 70 and 100). And
`.cargo/mutants.toml` still says in its own head comment that its list "deliberately mirrors
script/coverage's exclusions". One place derives it and the rest restate it, which is the whole
block.

`script/lint` already derives the set for its own check, so the hard half exists;
what is missing is the other three consuming that derivation instead of restating it.

In brief. A crate that reaches `crates/user_mode_runtime` cannot compile for the host, because `svc` and
`ecall` from EL0 on a machine with no nife kernel under it are a fault. Four separate places record
which crates those are: `script/lint`'s two clippy invocations, `xtask`'s `test`, `script/coverage`'s
exclusions, and `.cargo/mutants.toml`'s. `script/lint` asks cargo. The other three carry a list.

## Why this matters

This has already broken once, and the way it broke is the argument. The exclusion set went wrong on
**2026-08-03 and stayed wrong until 2026-08-14**, invisible for eleven days, because CI had moved to
an aarch64 runner the same day, where the EL0 assembly compiles by accident. `script/lint`'s own
header records that, and the derived check it now runs is the repair.

Milestone 244 (the largest crate in the tree is proved by nothing a mutation can reach) then found
the repair had covered **two consumers, not four**. The two it had
missed are the two that publish a number: coverage and mutation. `.cargo/mutants.toml` says in its
own head comment that its list "deliberately mirrors script/coverage's exclusions" and asks the next
person to keep the two in step. That is rung four of the ladder, in a file whose output is quoted in
a fatal risk's verdict.

Milestone 278 moves two modules out of `user_mode_runtime` into a crate that *can* compile for the host, which
means editing all four lists in one change. A fifth list would be added by the next milestone that
splits anything.

## The shape

`script/lint` already runs the derivation: ask cargo which workspace members reach `user_mode_runtime`, and
check every consumer excludes every one. Two ways to close the class, and choosing is the work:

- Emit the set. One command prints the exclusion list, and `script/coverage`, `xtask test` and
  the mutants config consume it rather than restate it. `.cargo/mutants.toml` is static TOML, which
  is the awkward one and may need generating or a `--exclude` passed at the call site instead.
- Keep the lists and keep the check. Weaker, and it is roughly today's state with the gate
  widened to four consumers, which milestone 244 already did. The remaining defect is that four
  files still have to agree; the gate only tells you after they stop.

The first is the higher rung. The second is honest about cost and should say so out loud if chosen.

## What this is not

It is not a change to the exclusion itself. `user_mode_runtime` is correctly excluded and stays excluded;
milestone 278's argument is that the *boundary* is in the wrong place, not that the rule is wrong.

## BUGS

- **The `.cargo/mutants.toml` half chose the second shape, landed 2026-10-16 (UTC)**: the derived
  exclusions moved to the `script/mutation` call site, which passes the set to cargo-mutants on the
  command line, and the file stopped being the record. Generating a checked-in file was refused on
  the tree's recorded opinion about generated files, and the question this bullet asked with it.

## Follow-on

- **Recorded.** One consequence lives beside the number it moves: the weekly coverage column is
  taken over the whole lcov, unfiltered (`notes/project-metrics/coverage.csv`, computed by
  `script/metrics --coverage-from`), and the run now excludes `virtio`, which the derivation names
  and the hand-written run list never had. The next recompute may step that aggregate for the right
  reason; the floor's view is unchanged, because `virtio` was already exempt there.

## Index row

A crate that reaches `crates/user_mode_runtime` cannot compile for the host, because `svc` and
`ecall` from EL0 on a machine with no nife kernel under it are a fault. Four files record which
crates those are: `script/lint`'s two clippy invocations, `xtask`'s `test`, `script/coverage`'s
exclusions and `.cargo/mutants.toml`'s. `script/lint` asks cargo and the other three carry a list.
The set went wrong on 2026-08-03 and stayed wrong eleven days, because CI had moved to an aarch64
runner where the EL0 assembly compiles by accident. The repair covered two consumers, and the two it
missed publish a number: coverage and mutation. The derivation landed on 2026-10-16: every consumer
now emits the set or consumes it, and `.cargo/mutants.toml` stopped being the record when its
exclusions moved to the `script/mutation` call site.
