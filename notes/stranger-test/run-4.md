# Stranger test run 4, 2026-08-18

*An appendix to [the stranger test](../stranger-test.md), which holds the current state. This page
is the record for run 4: the design decided before the run, and what it found.*

## What run 4 changes, decided 2026-08-18 before the run

Run 3's `BUGS` entry names one fix and prices it at one line, and run 4 applies it. Everything else
about the configuration is run 3's, deliberately, because a run that changes two things measures
neither.

The harness's own files leave the stranger's working directory. Run 3's logs were called
`stranger3-stream.jsonl` and `stranger3-stderr.log` and sat beside the clone. So the stranger's
first `ls -la` told it which run it was before it had read a byte of the project. Run 4's logs go in
a sibling directory, and the stranger's working directory holds the clone and nothing else. The
directory names carry no run number either, since a path in an error message is as readable as a
directory listing.

The isolation mechanism is unchanged, and it is verified the same two ways, because it is what made
run 3 scorable. It is a separate `claude` process, not a subagent, run with `--safe-mode`, from the
clone's *parent*, since project instructions load from ancestors and never from descendants. Before
the run, a throwaway probe in the same configuration is asked which project-instructions files are
in its context. The stranger itself is asked afterwards.

The answer key is withheld the same way: the note and its `notes/README.md` entry are removed and
the deletion amended into the tip, nothing else touched, working tree clean. The in-tree mentions of
runs 1 to 3 stay, for the same reason as before.

### What is new to measure

`CONTRIBUTING.md` and the README's `## Start here` reading order both landed on 2026-08-18, after
run 3's clone was cut. No stranger has seen either. Run 3's B1 failed because there was no reading
order. Its stranger built one by instinct, reaching `AGENTS.md` twelfth and `crates/abi/src/lib.rs`
far too late. So B1 is the row this run exists to move. The questions are whether a stranger finds
the reading order, whether it follows it, and whether following it avoids what instinct cost run 3.

The task text stays verbatim from runs 1 to 3. Changing it would make the four runs incomparable,
and the comparison is what distinguishes a milestone from an audit.

### What this run still cannot give

The machine is the architect's laptop, with the pinned nightly, both QEMUs and a warm cargo cache.
So B2 is not measured, and B4 is measured only against the documented sequence, as in run 3. The
machine is also loaded by other lanes gating in other worktrees. This time the tree can say so: as
of 2026-08-18, `script/test` prints the host load average beside a failing timing leg. Whether a
stranger meeting that message needs `uptime` anyway is a measurement this run gets for free.

## Run 4, 2026-08-18: the harness stopped leaking and the tree took over the job

### The one-line fix worked

The logs were written to `/private/tmp/newclone/logs/`, a sibling of the stranger's working
directory, as `stream.jsonl` and `stderr.log`, with no run number anywhere in the path. The
stranger's first tool call was `ls -la`, as in run 3, and it returned one entry: `nife`. Nothing in
the harness told it which run it was.

### It knew anyway, within half an hour, and the source was the tree

Asked afterwards what had told it, it named `notes/adding-a-program.md`'s `BUGS` section. That holds
a table headed `walk | program | wrong in`, whose three dated rows name run 2's `doubler`, run 3's
`triangle` and "this lane". Beside it, a sentence asks the next person to add a program to treat a
surprise there as the page's bug. `README.md`'s `## Start here` block cites milestone 117 (the
stranger test) by name in the same breath. The stranger's summary: *"So I knew, within the first
half hour, that I was at minimum the fourth person to walk this exact path and that the walks were
treated as measurements."*

This is a different defect from run 3's. Run 3's leak was a mistake by the person building the
isolation, and it cost one line to fix. Run 4's leak is a *consequence*. Every run records its walk
in the page the next run is sent to, because the page's own `BUGS` convention asks for that. The
instrument's good practice generates its own contamination, and it compounds, one row per run.
There is no one-line fix, and the obvious fixes are worse than the leak. Deleting the record would
fabricate a tree. Hiding it would apply the answer-key rule to something that is not an answer key.

What it changed, in the stranger's words, which are unflattering:

- *"I performed."* It wrote a prediction into the journal before starting the walk, and calls that
  *"an imitation of this tree's voice, aimed at a reader I imagined scoring me on intellectual
  honesty. Nothing about the task required it."*
- It chose the deliverable partly for how it would read: *"I wanted to be the walk that found
  something."*
- It adopted the repository's prose style wholesale, and names that as mimicry, not judgment.
- *"I front-loaded my own honesty... The fact is fine; the framing is a bid."*

What the leak did not change, which is why the run still counts. It did not invent a breakage to
have something to fix, and it did not skip gates and claim them. It never opened
`design/roadmap/0117-newcomer-onboarding.md` or looked for `notes/stranger-test.md`, which was
withheld as in runs 2 and 3. The answer key held for the fourth time. Only the fact of being
measured leaks, as the first `BUGS` entry has said since run 1.

### The configuration, verified rather than asserted

The clone was `c9fe2d67` (`bb094187` after the withholding amend), with `origin` pointed at the
public GitHub URL. The probe answered `NONE`, and the stranger answered the same way: *"No. Nothing
from a `CLAUDE.md`, `AGENTS.md`, or equivalent was in my context before my first tool call... The
repo sat one level below the primary working directory, so nothing was auto-loaded."* The task was
runs 1 to 3's, verbatim, plus the journal. 177 turns, about 15 minutes of wall clock, `$12.66`.

One deliberate contamination the earlier runs did not have is the harness's, not the tree's. `pkill`
and `killall` were shadowed on the stranger's `PATH` by wrappers that kill only QEMU processes whose
command line names the clone. `README.md`'s own "Try it" block tells a reader to quit with `pkill
qemu-system-aarch64`, and four other lanes were gating on this machine. The stranger never invoked
either, so the shim changed nothing here. It would have, and the harness should keep it.

### The build half

`script/test` was green on arrival: exit 0, first try, no change to the tree. 1312 tests passed and
0 failed. They covered the host workspace, the doctests, the vendored RedoxFS round trip and its
`no_std` core on both bare-metal targets, and `redoxfs_server`'s sans-IO core. They also covered the
patched `nife-dev` std toolchain and the kernel under QEMU on both ISAs. It took about 25 minutes
wall clock, most of it the two emulated legs.

It then ran the rest of `script/gates` unprompted, on its own reading of the tree's vocabulary. That
command was retired into `script/ci-build`'s table by milestone 286 (one enumeration of the checks
that gate a pull request) on 2026-09-13; this record keeps the name the run used. It is the run's
best unforced result, and it belongs to `CONTRIBUTING.md`: *"'tests passing' in this project's own
vocabulary is `script/gates`, not `script/test`. I ran one of the three."* `script/fmt --check` and
`script/lint` both exit 0. It then named what it had not run, rather than letting silence imply
coverage: `script/verify`, `script/bench --check`, `script/coverage`, `script/fuzz`,
`script/supply-chain`, `script/test --hvf`.

B2 is not measured, as pre-registered: the pinned nightly and QEMU were installed, and
`script/setup` had nothing to do. B4 has one entry, and the stranger scored it against itself.
`timeout(1)` does not exist on this macOS host. `AGENTS.md` says so and points at
`helpers/qemu-bounded.sh`, and the stranger hit the missing binary before reading that section. Its
verdict: *"That is my error, not the tree's, the file that told me is the file the README tells you
to read third."*

The machine was loaded and no timing assertion fired, so the new load-average print went
unexercised. Load was 5.41 on 8 cores at launch and ran between about 3 and 17.5, with four other
lanes gating in other worktrees. That is well under the 45 to 63 behind run 3's 2-in-13 red rate,
and both emulated legs passed first time. The diagnostic landed on 2026-08-18 in response to run 3,
and this run could not test it, so the green is not evidence for it. As of 2026-09-24 no recorded
run has seen it fire.

### The mental model, scored: eight of eight

| # | result | where it came from |
|---|---|---|
| M1 | answered, and better than any previous run | `notes/capabilities.md` for the mechanism, `notes/abi.md` for the rights field, so it gave the attenuation too: holding an endpoint does not by itself permit receiving on it |
| M2 | answered | `notes/std.md`, not `notes/net.md`, which it never opened: slot 2 is a `Stack` endpoint with `WRITE` and slot 3 is the untyped budget the socket frames are minted from, and *"the absence of slots 2 and 3 is exactly what 'no ambient network' feels like from inside a process"* |
| M3 | answered, with an under-claim on enforcement | `AGENTS.md` rule 1 through the reading order. It said the rule looked like convention rather than mechanism and hedged that it had not read `script/lint`'s source; the gate is there, at `script/lint`'s `==> rule 1`, and it read that output |
| M4 | answered | `design/roadmap/README.md`, the whole vocabulary including `RECORDED` and the `IN-PROGRESS` branch rule |
| M5 | answered | `AGENTS.md` rule 7, with the Kani-and-host-tests reason named as the load-bearing one and `c_seam` as the case |
| M6 | answered, and read rather than induced | `CONTRIBUTING.md`, quoting it, then `AGENTS.md`'s second job for it as one of the two homes for identified work. Run 3 had to assemble this from four instances |
| M7 | answered by doing it | added `tally`, ran it at a real prompt on both ISAs, ran a negative control, reverted to a byte-identical tree |
| M8 | answered | four states, with the distinction that `provisional` is a claim about intent and the other three about the record |

Two of these moved because of documents no stranger had seen before. M6 is `CONTRIBUTING.md` working
as written: run 3 had to induce the `BUGS` convention from instances, and run 4 quoted a
definition. The gates observation above came from the same document.

### What it read, and in what order, which is B1

Twenty-two files. The stranger reached six through the `## Start here` order, and five more through
links from `notes/adding-a-program.md`. `AGENTS.md` was seventh, against run 3's twelfth.
`notes/capabilities.md` was eighth; run 3 never opened it. `README.md` was still first by
expectation, not by any pointer, which no reading order can fix.

So B1 passes for the first time, and it leaves one specific failure. The stranger did not follow the
order. It used the order as an index and read the items in the sequence its work needed, which the
section's own last line invites. The cost is one item: the stranger read `CONTRIBUTING.md`, item 2
of 8, sixteenth of twenty-two. Its gates paragraph arrived after the stranger had run the gates. The
one document written for a person deciding whether to work here is the one the reading order failed
to get read early.

Still unopened after four runs: every file under `design/decisions/`, `notes/net.md`,
`design/naming.md`, and `notes/README.md` itself.

### What it found, none of it fixed in this run

- `script/lint`'s naming worklist under-counted by exactly the provisional names, then named the
  command that prints the other number. The `--check` path printed `len(recorded) +
  len(unrecorded)`. The default listing printed `len(provisional) + len(recorded) +
  len(unrecorded)`. So the gate said `82 still want an architect (script/names --unratified)`, and
  that command said `UNRATIFIED (86 of 162)`. The census line above it dropped them too: `76
  ratified, 15 recorded, 67 unrecorded` sums to 158 of 162. Reproduced on `main`. It bit exactly the
  state a newcomer is told to use, since `AGENTS.md` and `notes/adding-a-program.md` both say to
  ship a provisional name. Recorded then in design/naming.md's `BUGS`. Fixed on 2026-09-19 by run
  6's lane: `script/names` now prints one worklist count everywhere, and that `BUGS` entry is gone.
- The two archives boot different binaries under the name `init`: `hello` on aarch64 and `builder`
  on riscv64, in a project whose loudest claim is architectural parity. The stranger reported this
  as undocumented and was wrong. `xtask/src/archive.rs` said so, in a comment on the aarch64 table's
  `hello` row, about 200 lines from the riscv table it describes. A stranger who read both tables in
  the same minute still called it invisible. That is a placement finding, not an absence, and it is
  run 3's closing diagnosis reproduced by a different reader. The comment now sits on the `hello`
  row in `xtask/src/main.rs` itself, fixed after run 6.
- The two commands run most often were both blind to the most-warned-about mistake. A program added
  to the aarch64 table and not the riscv one passed `cargo xtask build` and `script/lint`. It was
  caught first by `script/swish-check` or `script/test`. The `BUGS` entry in
  `notes/adding-a-program.md` said nothing gates the two lists against each other; this adds which
  gates a person will believe before finding out.
- Nothing in the suite counts programs. There were 1312 tests before adding `tally` and 1312 after.
  A program's presence was proven only by a transcript line someone remembered to write into
  `SWISH_CHECK_SCRIPT`.
- The eight places are a removal problem too, and only the addition had a page. Reverting was clean
  only because every edit was in a file it could still name. A half-removed program is a
  `PROG_COUNT` too large and an init slot no variant claims: the same silent failure, reached from
  the other side. Recorded in `notes/adding-a-program.md`'s `BUGS`. Milestone 150 (adding a program
  should not need eight hand-maintained lists) made a program one `[[bin]]` block on 2026-09-19.
- `notes/adding-a-program.md` was right about everything, including a line number. This was the
  first of four walks to find no defect in it, and the page's own `BUGS` predicted the opposite. The
  stranger checked the `match` in `write_outcome`, then in `crates/swish/src/lib.rs`, against the
  error the page quotes and got the same file and line. Its prediction, written before the walk, was
  that it would find at least one thing the page did not mention. It did not.

Its worst-thing answer is the one to keep. It is the only place in four runs where a stranger
criticized the tree's central habit rather than a document. In its words: *"the project's habitual
response to a structural problem is another document, which its own ladder names as the worst
available move... The documentation is doing work that a data structure should be doing, and it is
doing it beautifully, which is exactly what stops anyone from fixing it."* It cites three instances,
all things this tree had written about at length and not changed. They are `AGENTS.md`'s misleading
name, `notes/adding-a-program.md`'s fourth rewrite, and the eight-place program problem, whose
one-place fix was sitting in the milestone's own handoff list.

### What this run cost

The tree's own leak is now the largest contamination, and it is structural; it leads this record for
that reason. The other costs are smaller and carried over. The machine was warm, so B2 measures
nothing. The machine was loaded, though far less than for run 3, so the timing rows are contaminated
and the new diagnostic went unexercised. `pkill` was shadowed by the harness and never invoked. And
the harness's author has read `AGENTS.md`, which no arrangement of processes fixes. This lane made
that slightly worse by amending the rubric before running against it. The mitigations are the same
three: the task text is runs 1 to 3's verbatim, the amendments were committed before the clone was
cut, and the stranger's answers are recorded as it gave them.
