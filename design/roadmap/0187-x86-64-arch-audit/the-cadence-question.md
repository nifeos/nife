# The cadence question, and why it is a second milestone rather than half of this one

An appendix to [milestone 187 (read the x86_64 arch tree through the lens the first arch audit
used)](../0187-x86-64-arch-audit.md). It holds the check of the sweep's second claim, that the audit
cadence cannot see an architecture, the full argument for splitting the fix into its own milestone,
and that milestone's content as proposed. The page carries the decision.

## The claim, checked

The sweep's second claim is that this gap will never surface on its own, because "the audit cadence
counts elapsed time and shipped components and has no notion of an architecture". That sentence is
in pull request #568's body and in none of the files it landed, which is the record shape
`AGENTS.md` warns about most directly: a pull request body is read while the diff is open and never
again. This block is the tracked home it did not have.

The claim was checked here against `script/audits`, `design/audit-reports/README.md` and
`.github/workflows/audit-cadence.yml`. It holds, and it is worse than stated in one respect and
better in another.

Worse: the counted triggers are measurably blind to an ISA. `script/audits` has four, DECISIONS §74
(audits run on change, not on the calendar): milestones BUILT, components (`crates/*/` plus
`[[bin]]` targets), ABI constants, and external packages. At commit `797a20ac`, which added fourteen
files under `kernel/src/arch/x86_64/` in a 2,241-line commit, every one of the four was unchanged:
built 97 to 97, components 122 to 122, ABI constants 51 to 51, external packages 108 to 108. An
architecture adds no crate, no program, no system call constant, and no dependency, so the only counter
it can ever move is `milestones built`, by one, when milestone 161 (the x86_64 kernel port)'s row flipped from PARTIAL
(2026-09-19). Against a security cadence that fires at 15, an entire instruction set is worth one
fifteenth of a trigger. The script's own BUGS predicts this in general terms ("a change that lands
as neither a milestone nor a component moves no number here") without noticing that the largest
single addition to the TCB in the project's history is that shape.

Worse still: the cadence's unit is the kind, not the lens. `notes/arch-audit.md` is on record as
a `security` audit dated 2026-07-29. Three later `security` audits have run, so `script/audits`
prints `security last 2026-08-17` and the arch-and-assembly lens's age is invisible. The `lens` cell
is free text; `--check` validates only that it is non-empty, under a message that reads "the value
of an audit is the lens the last one lacked, so a row without one cannot be read for what to do
next". That is the field holding the information, and nothing ever reads it. In `AGENTS.md`'s
ladder this is a rung-2 gate firing reliably on the wrong question, which is the shape that reports
green over a real hole.

Better: one trigger does reach it, by accident, under a question about something else. The
`security` kind carries an uncountable judgment question, printed on every run: "has this booted on
a new machine class (a board, a cloud) since the last audit?" A reader could answer yes for x86_64
under q35. But a machine class and an instruction set are different axes (a Raspberry Pi is a new
machine class on an already-audited ISA). The arch layer landed after the most recent security
audit, so the question has not yet been put to anyone. And the script's own BUGS calls this rung four
and "the weakest part of the mechanism". A mechanism that catches the right thing under the wrong
question, once, if somebody reads the output, is not a mechanism.

## The decision: split, and this milestone is the reading half

Four reasons, strongest first.

1. The gate must not be written by the lane that satisfies it. `AGENTS.md`'s first principle
says a gate can be written to pass. A single lane that both teaches the cadence about architectures
and performs the audit clearing it chooses the trigger's shape knowing what will read green when it
is done. Two lanes and two pull requests is the cheapest guard available, and it costs nothing.

2. They are different sizes by an order of magnitude, and bundling makes the small one wait.
The audit is three passes. The cadence change is a cadence row, a judgment entry, and possibly one
counted trigger: well under a lane. Bundled, the mechanism lands when the last reading pass lands.
Split, it can land tomorrow, and the tree stops being blind while the audit is still being read.

3. The cadence change has value even if this audit never happens, which is the test for whether
something is its own piece of work. The 59% growth in the audited ISAs' own trees since 2026-07-29
makes the point with no reference to x86_64 at all: a lens's coverage decays on the architectures it
*did* cover, and nothing measures that either. A tree that cannot see either kind of decay is broken
whether or not anyone reads x86_64.

4. Part of the fix is not a lane's call. The obvious shape, giving the arch-and-assembly lens its
own audit kind, means retroactively reclassifying the 2026-07-29 row in
`design/audit-reports/README.md` and changing what `script/lint` runs on every pull request. That is
a global record and a global gate, which `AGENTS.md` assigns to the integrator rather than to a
lane, and handing it to a lane mid-read is the worst moment to ask.

The one-milestone case, and why the roadmap row answers it. The case is that auditing x86_64
without fixing the blindness leaves the fourth architecture in the same position. That is an
argument that both must happen, not that they must be one milestone, and this tree's own mechanism
for "this must not be forgotten" is a tracked row rather than a bundle. Bundling is rung four in a
milestone's clothes: it relies on the reading lane remembering the second half.

Ordering: neither blocks the other. If the cadence milestone lands first the story is tidier,
because its red light is then what this milestone answers. Sequencing them strictly would delay this
one for no gain, since we already know x86_64 is unread and do not need a mechanism to tell us.

## The second milestone, proposed provisionally

calef mints the number and the title; this is the content, recorded so it has a home rather than
living in a report.

Provisional title: teach the audit cadence what an architecture is. Three questions it decides,
none of them decided here:

1. Whether the arch-and-assembly lens gets its own audit kind with its own cadence row, or
   whether the existing kinds get an architecture-shaped trigger. The first is a table row; the
   second is code in `script/audits`.
2. Whether a counted trigger can be made sensitive to the arch tree at all. Directories under
   `kernel/src/arch/` is one candidate and fires once per ISA, which is the right frequency for the
   gap found here. Lines under `kernel/src/arch/` is another and would also catch the 59% decay on
   the ISAs already read, at the cost of a threshold somebody has to choose.
3. What to do with the `lens` cell, which is the field that holds the information and the field
   nothing reads.

