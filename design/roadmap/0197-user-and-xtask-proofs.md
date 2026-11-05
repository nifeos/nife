---
status: BUILT
raised: 2026-08-30
built: 2026-08-31
---
# 197. `user/` and `xtask` are out of reach of the prover, for exactly the reason the kernel was

Built, the `user/` half. `xtask` is refused with an argument. The timer seam below is untouched and
still a fork. notes/user-proofs.md is the record.

## What was built, and where the block was wrong

One property, over code that lives in `components/src/printenv.rs` today, proved by two harnesses
`script/verify` runs, and falsified before it was believed. `push_never_writes_past_the_buffer_it_was_given`
says that for every starting offset in the whole of `usize` and every content, the bounded append
that renders a configuration page this program did not write stays inside its 96-byte line buffer.
It appends only at the offset it was given, and reports exactly what it wrote.
`the_buffer_can_be_filled_exactly` is the `kani::cover!` that the boundary is reached, so the
assertion is not proved by an assumption set that never gets near it. Relaxing one `<` to `<=` turns
both red. The patch is recorded (`user/falsifications/` at the time; milestone 175 (split
user/: components/ for services, fixtures/ for test and benchmark programs) moved `printenv`
into `components/`). The record now lives at `components/falsifications/`, corrected here
2026-09-23 for milestone 323 (the falsification record is incomplete in five ways, and each was
found by a different lane)). Cost: 2.4 seconds on `script/verify`'s ~650.

The premise above is half false, and that is the finding. This block argued `user/` had a better
claim on the prover than the kernel because it holds real parsers over untrusted bytes. It mostly
does not, any more: rule 7 and the host-testability discipline have already lifted the initrd, ELF,
GPT, mDNS, directory-entry, terminal-escape, shell and glob parsers into crates, every one of
which is in `script/verify`'s table today. What is left in `user/src` is overwhelmingly IO glue.
The prize this block was reaching for had largely been collected under other numbers.

A live defect fell out anyway, and it fell out before any harness ran. `rmle`'s save buffer was
`MAX_ROWS * MAX_COLS`, the size of the document's *text*, while `save` joins the rows with `\n` and
so writes up to `MAX_ROWS - 1` bytes more. A document at both bounds staged 3231 bytes into a
3200-byte buffer and panicked the editor on `^S`, and nothing had found it. It was found by writing
the property down, which is the first time the two constants were compared. The fix is the buffer
sized `MAX_ROWS * (MAX_COLS + 1)` plus a `const` assertion, not a harness: the claim is a
relationship between compile-time constants, so rung one of the ladder holds it for nothing.

Three property shapes did not work, and the numbers are the deliverable. A bound on a sum of 32
symbolic values did not finish in twenty minutes on CaDiCaL or Z3, because it is a cardinality
argument and that is what resolution-based SAT is worst at. A symbolic index into the editor's
3.5 KB document struct exhausted CBMC's memory in 3m23s. Any assertion about a value downstream of
`render`'s twenty chained divisions stopped finishing, while the same harness asserting only a
length bound returns in 0.3 seconds because `--slice-formula` throws the divisions away. A fast
harness can be evidence that the assertion asked nothing.

Mechanically it was cheaper than the kernel. No `--ignore-global-asm`, because there is no
`global_asm!` under `user/` at all (rule 1 again). Two changes: `#[cfg(not(kani))]` on the one
binary's `user_rt::panic_handler!()`, and `--bin` selection, because `cargo kani -p user` compiles
all 68 programs and Kani refuses any `#![no_std]` root that never mentions it. The `--bin` list is
derived from a grep of the tree in `script/verify` rather than written there, for the reason that
file has already recorded twice: a list one name short fails invisibly and the suite just goes green
faster.

`xtask` is refused, and the argument is in notes/user-proofs.md rather than left as a shrug. Its
front door is already open (`cargo kani -p xtask` compiles with no changes at all, measured). So the
refusal is on value. A defect there cannot reach anything that runs. It is host `std` code with
tests and a debugger, where Kani is least differentiated. Its pure logic already lives in crates the
suite proves, and its hand-written decoders exist to be a second opinion on the crates they
check. Aiming one prover at both halves narrows the independence that justifies them.

*This block carried `Gate: NONE` while it was open, and correctly: the `user/` half needed nothing
that did not exist, milestone 193 having established the mechanics. The line is gone because a BUILT
milestone cannot have one. The timer seam further down was and is a genuine fork, marked as one
there rather than gating this block.*

In brief. `script/verify`'s header names three things `cargo kani` never compiles: the kernel, the
user programs, and xtask. Milestone 193 removed the first for about ten seconds of run time. The
other two are unchanged, and `user/` has at least as good a claim on the prover as the kernel did,
because it holds real parsers over bytes this system did not produce.

## Why `user/` first

`notes/untrusted-input-audit.md` already surveys that surface. A parser over attacker-supplied input
is the case bounded model checking is best at and the case where a defect is worst, which is the
combination that made `dtb::be32`'s unchecked `at + 4` worth catching.

The mechanics should be milestone 193's, and cheaply: `#[cfg(not(kani))]` on what Kani duplicates,
`--ignore-global-asm` where needed, and the stub boundary enumerated where the next author meets it.

## Two smaller things the same lane found

The timer re-arm seam, and it is a fork rather than work. Milestone 191 (did the proofs catch the
bugs?) named the milestone 6 timer drift as the sharpest counterfactual in the tree: its property is
already proved in `crates/timetable`'s `next_after`, over already-written code, and the timer does
not call it. Milestone 193's lane could not use it, because `rearm` lives in
`kernel/src/arch/{aarch64,riscv64}/timer.rs` and reads the counter through `asm!`. Lifting the
arithmetic out of the register access would fix that, and **where the seam goes is a design question**:
too high and the arch layer keeps the bug, too low and every ISA restates it.

**`crates/timetable` is in the verify table and the kernel does not depend on it**, which is the other
half of the same observation and is untouched.

## BUGS

- `xtask` was named here and not argued for, and it is now argued and refused; see above and
  notes/user-proofs.md. What would reverse the refusal is `xtask` growing logic the target then
  trusts, and the shape to watch is the measured-boot digest, which already lives in
  `crates/measured_boot` for exactly this reason.
- The run-time cost was not estimated here and is now measured: 2.4 seconds for the two
  harnesses that shipped, against `script/verify`'s ~650. The worry that "a parser over symbolic
  input can cost far more" was right in a way this block did not predict. The expensive shapes are
  not parsers but sums, symbolic indices into large structs, and values downstream of division.
  Three such properties were abandoned with the measurements recorded.
- **Two harnesses is not coverage of 68 programs.** The editor's editing operations are the richest
  untrusted-input surface left in `user/` and are out of reach as the document is laid out today.
  Moving a row's length out of the row would fix that and is a data-layout question, not a lane's.

## Follow-on

- **Milestone 212.** `script/falsifications` walked `crates/` only, so this milestone's record under
  `user/` was uncounted and milestone 193's kernel harnesses carried none at all, while the script
  printed its ratio as if it were the tree's. The walk now comes from `cargo metadata`.
- **Refused.** Proving `xtask`. Its front door is already open, measured: `cargo kani -p xtask`
  compiles with no changes. The refusal is on value. A defect there cannot reach anything that runs.
  It is host code with tests and a debugger, where Kani is least differentiated. Its pure logic
  already lives in crates the suite proves, and its hand-written decoders exist to be a second
  opinion on those crates. Aiming one prover at both halves narrows the independence that
  justifies them. What would reverse it is `xtask` growing logic the target then trusts, and the
  shape to watch is the measured-boot digest.
  This refusal is milestone 468 (design/roadmap/0468-proving-xtask.md), which carries it with the
  condition that would change it.
- **Recorded.** `design/roadmap/0197-user-and-xtask-proofs.md` BUGS: two harnesses is not coverage of
  68 programs. The editor's editing operations are the richest untrusted-input surface left in
  `user/` and are out of reach as the document is laid out today. Moving a row's length out of the
  row would fix that, and it is a data-layout question rather than a lane's.
- **Recorded.** `notes/user-proofs.md` holds the three property shapes measured and abandoned rather
  than left as folklore. A bound on a sum of 32 symbolic values that did not finish in twenty
  minutes. A symbolic index into the editor's document struct that exhausted CBMC's memory in 3m23s.
  And anything downstream of twenty chained divisions. It is where the warning lives that a fast
  harness can be evidence the assertion asked nothing.
- **Milestone 360.** `design/roadmap/0360-timer-rearm-seam.md`, lift the timer re-arm arithmetic out
  of the register access so `crates/timetable`'s already proved `next_after` is what the timer
  actually calls. Where the seam goes is an architect's: too high and the arch layer keeps the
  milestone 6 (threads, the context switch, and preemption) drift bug, too low and every ISA
  restates it. Until it moves, the tree's sharpest counterfactual is a property proved over code
  that nothing runs.

## Index row

The `user/` half is built (2026-08-31): `user` is in `script/verify`'s table, with the binaries
carrying harnesses derived from the tree rather than listed, and two falsified properties over `printenv`'s bounded append cost 2.4 seconds. **The block's premise was half wrong and the lane
says so**: rule 7 has already lifted almost every parser out of `user/` into a crate the suite
proves. The value arrived anyway, from stating a property rather than from running one, as a live
buffer defect in `rmle`. `xtask` is **refused, with the argument written down**. The timer re-arm
seam is still a design fork and is untouched, as is `crates/timetable`'s row.
