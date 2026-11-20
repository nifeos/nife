---
status: BUILT
raised: 2026-09-21
built: 2026-09-21
---
# 542. Gates that stopped meaning what they measure: both inputs to an icount number recorded, and a footprint reported against its budget

*(Renumbered from 541 by the integrator on 2026-09-21: 541 had already been minted for the `map_new` window while this lane was running, which is the collision AGENTS.md predicts for anything global to the tree. Number provisional until the merge queue lands it; 524 to 540 were
claimed on in-flight branches when this was written.)*

calef, 2026-09-21: *"Do the bump-time mechanism. Do the footprint gate."* Two rulings, and they are
one milestone because they are one defect wearing two costumes. A gate whose number is measured
against a moving reference stops making a claim about the system. The icount floors move when the
compiler moves; the footprint gate's percentage moves when the binary moves. In both cases the gate
stayed green, said something, and the something was no longer about this kernel.

## Part 1: a toolchain bump cannot silently revalue the icount floors

`bench/baseline-{aarch64,riscv64,x86_64}.txt` are the tripwire's committed floors, and icount counts
guest instructions. A new nightly emits a different instruction sequence for identical source,
so every number in those files is revalued by a change no commit in the tree is responsible for.
Nothing said so and nothing checked it.

The cost was live and it was paid. On 2026-09-15 the pin went to `nightly-2026-09-15` with the
floors left un-resaved, most of the tripwire's headroom eroded, and the tree was bumped again to
`nightly-2026-09-20` before anybody acted on it. The drift has since evaporated, measured
2026-09-21: worst margin +0.02% on aarch64, +2.27% on x86_64, and riscv64's `rfence_self` is
7.49% faster than its floor. So this milestone is not repair. It is the mechanism that turns
"we got lucky" into "we would have been told."

The shape, which is the loud one rather than the automatic one. Three shapes were on the table
and calef's ruling picks the first:

- A bump pull request that re-records in the same pull request. Chosen. A human sees the delta
  at the moment it is caused, and it is attributable to the branch that caused it.
- A check that fails saying the baselines are stale for this nightly. Louder, and it fails pull
  requests that did not cause it.
- Auto-re-saving on a bump. Refused, and the refusal is written in the code at both places
  somebody would reach for it. A floor that tracks the compiler by construction moves by exactly as
  much as a nightly moved the kernel. So a nightly that genuinely made this kernel slower would
  report nothing, and catching that is most of what the tripwire is for.

What was built. Four small pieces, and the first two are what make the third unnecessary:

1. The floors carry the nightly they were read against. A `# toolchain: nightly-YYYY-MM-DD`
   line, written by `cargo xtask bench --save` from `rust-toolchain.toml`'s pin rather than from
   whichever compiler happens to be running, because a `RUSTUP_TOOLCHAIN` override in one shell is
   not a fact about the repository. This is the fact everything here turns on and nothing in the
   tree stated it.
2. `script/lint` fails when that line and the pin disagree. This is the resolution of the second
   and third shapes above rather than a compromise between them: **a branch that does not raise the
   pin cannot fail this check**, because its baseline and its pin already agree. The merge queue
   rebases it onto a `main` where they still do. The only branch it can fire on is the one that
   caused it.
3. `script/toolchain-bump` says the re-record is the operator's, prints the exact commands, and
   carries the refusal above in the place a reader meets the bump.
4. `.github/workflows/toolchain-bump.yml` says the same in the pull request it opens, including
   that its own `script/lint` will be red and that the red is a step rather than a defect. That
   workflow is what raised the pin on 2026-09-15 without touching the baselines.

**One marked exception, and it is a foot gun.** `script/toolchain-bump` sets
`NIFE_BUMP_IN_PROGRESS=1` around its own gate run, because it has just raised the pin and has not
re-recorded anything. So the check is guaranteed to fire there. Firing would only trip the
script's restore trap and un-bump a tree that is otherwise fine. Nothing in CI sets it. The
enforcement was never in that script: it is `script/lint` running unexempted on the bump's pull
request, and a green lint bought with this variable means nothing. Named provisionally.

Proved firing rather than asserted, which is the difference between a gate and a claim. Four
cases, run against this tree:

| case | result |
|---|---|
| this tree, pin and stamps agreeing | passes, `3 file(s) at nightly-2026-09-20` |
| pin raised to `nightly-2026-09-28`, floors untouched | **fails**, naming both nightlies on all three files |
| a baseline with the stamp line removed | **fails**, `no '# toolchain: <channel>' line` |
| `NIFE_BUMP_IN_PROGRESS=1` | skips, saying why and where the real gate is |

And the live case exists today: `origin/toolchain/nightly-bump` (pull request #1054) raises the pin
to `nightly-2026-09-21` and changes nothing else, so once this lands, that branch's CI fails
this check with exactly the second row's message.

## Part 1b: the emulator is the other half of the same fact

An icount count is a function of two things and this milestone had made only one of them a
record. The `rfence` lane found the other while chasing a benchmark that had apparently got 7.49%
faster: **`.qemu-version` pins 11.0.2, this machine has had 11.1.1 installed since 2026-08-28, and
`script/bench` never checked.** So every committed baseline in the tree was measured against an
emulator nobody recorded and nothing verified, and `script/qemu-check`'s own warning text asserted
that the baselines "were recorded against" the pin, with nothing in the tree supporting it. That
sentence is corrected in this milestone; it was an assumption wearing a fact's clothes in the one
place a reader goes to ask the question.

**The stamp records the emulator that RAN, not the pin**, which reads as an inconsistency with the
toolchain stamp and is the argument instead. `rustup` resolves the compiler from
`rust-toolchain.toml`, so there the pin and the thing that ran are one fact by construction, and the
single escape (`RUSTUP_TOOLCHAIN`) is named in the code. Nothing resolves QEMU from
`.qemu-version`: it is a wish about the machine, and on 2026-08-28 the machine stopped granting
it. Writing the pin into a baseline would file intent under the heading of provenance, which is the
exact defect this milestone exists to close. `cargo xtask bench --save` asks the binary it is about
to run, and when the answer differs from the pin the line says both, because the disagreement is a
fact about the numbers rather than something to tidy.

**The check lives in `--check`, not in `script/lint`, and compares the stamp against the emulator in
front of it.** A static lint cannot ask this question, because there is no emulator in a repository.
Of the two available comparisons only one is honest:

| compared against | what it would do |
|---|---|
| `.qemu-version` | fails any baseline truthfully recorded off-pin, so it forbids the file from stating the truth, and still passes a comparison run on some third version |
| **the emulator this run used** | asks the only apples-to-apples question there is: is the counter reading this floor the counter that produced it |

**`unrecorded` is a truthful answer and does not fail**, the posture
milestone 115 (the names that were refused) already takes for names. Every baseline in the tree carries it
today, because nobody wrote down which QEMU produced those counts and inventing a version now would
be worse than the gap. So the check fires at full strength from the first honest `--save` and never
on a number nobody stamped. **When it does fire, the remedy is not "upgrade something"**: a
re-record and the pin have to be settled in one commit, because CI builds the pinned emulator
(`script/ci-qemu`) and will read these floors on it.

Proved firing, three cases, each a real `script/bench --check` run on this machine:

| case | result |
|---|---|
| `# qemu: unrecorded`, as every baseline stands today | passes, saying the comparison is assumed rather than known |
| stamp `11.0.2`, machine running `11.1.1` | **fails**: *"a pass here would be evidence of nothing"* |
| stamp `11.1.1`, matching the machine | passes, `emulator matches the baseline's (11.1.1)` |

Nothing was fixed by changing either version. The pin stays 11.0.2, the machine stays 11.1.1,
and what is true is written down in the baselines themselves and in `notes/benchmarks.md`. Which
version this project should run is an architect's call, and it is the more urgent half: the
divergence has already reached published figures.

## Part 2: the footprint gate measured drift when it should measure distance

`script/fastpath-footprint`'s own first line says why it exists: *"the IPC fastpath must stay small
enough to live in L1i"*, from Liedtke's argument that Mach's IPC was slow because of the cache
footprint of its hot path. **That is a physical budget, and the gate did not report against it.** It
printed `+1.4% against baseline (7028)`: drift from whatever the binary happened to be the last time
somebody re-recorded, a number that means something different after every compiler change.

The facts a reader needs were in `notes/benchmarks.md` and nowhere near the tool:

- the binding constraint is radon's SiFive U74, 32 KB L1i, which that note names as the one that
  binds among the machines this tree actually runs on;
- the tree's own stated target is 4 KiB, about an eighth of it, a fraction derived from
  Liedtke's argument rather than from roundness;
- and the fastpath is 1.16x to 2.01x that target depending on ISA and shape.

What it prints now, per architecture, with drift kept, demoted and relabelled:

```
    budget: 4096 B target, 32768 B L1i (radon's SiFive U74, the smallest we run on)
    ipc_send_recv    4734 B   1.16x target  14.4% of L1i  over 8 symbols
    ipc_call_reply   6038 B   1.47x target  18.4% of L1i  over 10 symbols  <- the shape the system runs
    ipc_fastpath     6038 B   1.47x target  18.4% of L1i  the worse of the two shapes
    system_call_entry    1914 B                  5.8% of L1i  over 5 symbols (flat, no closure)
    total            7952 B   1.94x target  24.3% of L1i  (7.77 KiB), an upper bound
    49% of the 16 KiB ceiling §144 (a delta and a ceiling) decides and does not yet enforce
    drift: ipc_send_recv +2.2% against baseline (4632), within the 5% band
```

Measured 2026-09-21 on `nightly-2026-09-20`, all three:

| | `ipc_call_reply` | x target | % of L1i | `total` | x target | % of §144's 16 KiB |
|---|---|---|---|---|---|---|
| aarch64 | 7,104 | 1.73x | 21.7% | 8,612 | 2.10x | 53% |
| riscv64 | 6,038 | 1.47x | 18.4% | 7,952 | 1.94x | 49% |
| x86_64 | **8,234** | **2.01x** | **25.1%** | **9,935** | **2.43x** | **61%** |

`system_call_entry` gets the L1i share and no target ratio, because the 4 KiB target is stated over
the IPC fastpath's instructions and the trap path is not part of what it bounds. Its bytes are
fetched on every system call, so the share is still the quantity that matters.

The 16 KiB line is the third distance and the only one the tree has already voted on. §144 (a
delta and a ceiling) decides an absolute 16 KiB per architecture on exactly this `total`, derived as
half of the 32 KB L1i. It is DECIDED and not built, so printing the fraction is what keeps the
decision visible against the number it was made about.

**Drift still fails and the budget does not.** A 5% jump inside one pull request is a mistake
somebody just made, which is what a tripwire catches well. A budget the tree is 1.5x to 2x over
would fail on the day it was written and be turned off on the next.

## What was deliberately not done

- **The footprint baselines were not re-saved and nothing was shrunk** (calef's reasoning, recorded
  in the gate's own docs). Shrinking the fastpath today would be optimizing against a target whose
  value nobody can measure, because milestone 370 (a layout control) exists precisely to say that
  the perturbation experiments cannot tell footprint from addresses. Whether 4 KiB is still right
  belongs with milestone 132 (the fast path's footprint) and milestone 188 (the IPC fastpath), and
  waits on 370.
- No kernel code was touched. The whole milestone is `script/`, one `xtask` function, one
  workflow, the baselines' header line, and notes.
- §144's delta-against-`main` and its 16 KiB ceiling were not built. The ceiling is now printed
  as a fraction, which is reporting, not enforcement.

## BUGS

- The stamp is a verification and not a recording on its first day. The three baselines' counts
  were produced on an earlier nightly and re-checked green against `nightly-2026-09-20` on
  2026-09-21; the line says so in the files themselves, and the next `--save` makes it ordinary.
- `NIFE_BUMP_IN_PROGRESS` is an exemption on a gate. Rung two with a hole in it, marked as a
  foot gun where a reader meets it, and relying on CI being the place the check is not exempt.
- The check compares a pin to a stamp, not a measurement to a measurement. A baseline re-saved
  under a `RUSTUP_TOOLCHAIN` override, on a compiler other than the pinned one, is stamped with the
  pin and passes. That is the honest boundary: it catches the case that has actually happened twice
  and cannot catch a lie told deliberately.
- `bench/fastpath-*.txt` carries no such stamp, so the footprint gate's 5% band still erodes
  under a bump with nothing to say so. Recorded in `script/fastpath-footprint`'s own `BUGS` and in
  this block's Follow-on, with the measurement that makes it urgent.

## Follow-on

- **Recorded.** The footprint baselines have the icount ones' old problem and the erosion is
  measured, not hypothetical: on `nightly-2026-09-20` every figure on every ISA sits above its
  baseline, and riscv64's `system_call_entry` is +4.7% against a 5% band. One more bump can fail
  that gate for a reason no commit is responsible for. Stamping it means re-saving it, and
  re-saving was refused for this milestone, so the first deliberate `--save` there is where the
  stamp belongs. In `script/fastpath-footprint`'s `BUGS`.
- **Recorded.** Whether the 4 KiB target is still the right number is not decided here and waits on
  milestone 370 (a layout control), with milestone 132 (the fast path's footprint) and
  milestone 188 (the IPC fastpath) owning the question.
- **Recorded.** §144 (a delta and a ceiling) remains DECIDED and not built. This milestone prints
  its ceiling as a fraction and enforces nothing, and the delta-against-`main` half is untouched.
- **Recorded.** `NIFE_BUMP_IN_PROGRESS` is a provisional name, like every name a lane coins.
- **Recorded.** The most urgent item here, and an architect's call. It is deliberately not written
  up as a `design/decisions/` section, because a lane does not mint them; the maintainer holds it.
  `.qemu-version` pins QEMU 11.0.2 and the development machine has run 11.1.1 since 2026-08-28.
  Which one this project should run is not decided by this milestone, and it is not a tidying job.
  The divergence has already reached published figures. The first deliberate `--save` of any
  baseline forces the answer, because CI builds the pinned emulator and will read that floor on it.
- **Recorded.** Every baseline's `# qemu:` line says `unrecorded`, truthfully, and will keep saying
  so until somebody re-records deliberately. Re-recording was out of scope here.

## Index row

Two gates had stopped making claims about this kernel, and one of them for two reasons. The icount floors in `bench/baseline-*.txt`
are revalued by any toolchain bump, which happened unremarked on 2026-09-15 and eroded most of the
tripwire's headroom. They now carry the nightly they were read against. `script/lint` fails when
that disagrees with the pin (which can only happen on a branch that raises it). Both
`script/toolchain-bump` and the proposing workflow say the re-record is a human's while recording
why auto-re-saving is refused. The emulator is the other half of the same fact and was in worse
shape: `.qemu-version` pins 11.0.2, this machine has run 11.1.1 since 2026-08-28, nothing checked,
and `script/qemu-check` asserted the baselines had been recorded against the pin. The baselines now
carry the emulator that ran (`unrecorded` for every existing file, which is the truth about them),
`--check` compares it against the emulator in front of it, and neither version was changed to make
anything pass. `script/fastpath-footprint` reported drift from an old binary rather
than distance to a budget. It now prints bytes against the 4 KiB target and the 32 KB L1i that
binds, plus §144's undelivered 16 KiB ceiling. Drift is demoted to the secondary check that still
fails. Nothing was shrunk and no footprint baseline was re-saved.
