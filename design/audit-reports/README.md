# Audit reports

*Name: ratified 2026-08-04 (calef; §75 (directories under `design/` and `notes/` carry provenance in
their own README) covers this directory). `audit-trail` was refused because
[0035-scanner-findings.md](../decisions/0035-scanner-findings.md) already uses that phrase in its
established sense, a chronological record of who did what, which is also what an operating system
means by it (Linux's `auditd`, BSD's audit subsystem); a kernel whose thesis is confinement is a
plausible future home for that feature, and this is not it. Bare `audits` was passed over because
every file in here is literally a report. Recorded here rather than in a registry, per §75: a
directory has no header file, so its provenance lives in its own README.*

One file per audit, this file as the index. An audit is a deliberate adversarial read of the tree
through one named lens, and the reason there is a directory rather than a habit is
[milestone 92](../roadmap/0092-security-audit-cadence.md): a practice that lives in someone's memory
gets skipped exactly when it matters.

Two kinds of audit share this index, because a documentation sweep asks the same scheduling
question a security audit does, "what has changed since the last one", and answers it from the same
counts. `script/audits` reads both.

## When the next audit is due

`script/audits` answers, from this file and from the tree. It never runs an audit and never edits
anything:

This is a real run, on 2026-09-17, the day milestone 311 repaired the script after five weeks in
which it could not run at all. It is kept rather than replaced with a tidier example because what it
says is the thing a reader of this file most needs to know:

```
$ script/audits
documentation  last 2026-08-17 (The ABI surface as documented, read from the wire outward: e)
  milestones built     76 -> 188  (+112, fires at 10)   <-- FIRED
  components           110 -> 155  (+45, fires at 10)   <-- FIRED
  ABI constants        50 -> 52  (+2, any change fires)   <-- FIRED
  external packages    108 -> 108  (+0, fires at 30)
  calendar             31 days since (12 weeks)
  DUE: milestones built +112 (fires at 10); components +45 (fires at 10); ABI constants +2 (fires at 1)
  ? has a subsystem been rewritten inside its existing crate since the last sweep? ...
  ? has a decision superseded a plan that a note still prescribes? ...

security      last 2026-08-17 (Newly minted authority, read adversarially: the seven ABI co)
  milestones built     76 -> 188  (+112, fires at 15)   <-- FIRED
  components           110 -> 155  (+45, fires at 8)   <-- FIRED
  ABI constants        50 -> 52  (+2, any change fires)   <-- FIRED
  external packages    108 -> 108  (+0, any change fires)
  calendar             31 days since (6 weeks)
  DUE: milestones built +112 (fires at 15); components +45 (fires at 8); ABI constants +2 (fires at 1)
  ? has a new component taken device or network authority since the last audit?
  ? has this booted on a new machine class (a board, a cloud) since the last audit?

The `?` lines are triggers nothing here can count, because they are a judgment and not a
number. If one is yes, that kind is due regardless of everything above (milestone 92, §74).

audits: 7 on record, DUE: documentation, security
Red means run the audit. Nothing here ran one, and nothing here can.
design/audit-reports/README.md says how, and which lens the last one lacked.
```

**Both kinds are overdue, and the security one is overdue by seven times its own trigger.** §74 set
the security count at 15 milestones or 8 components; the tree has built 112 milestones and 45
components since the last audit of any kind. The calendar backstop, the trigger a reader reaches for
first, is the only one that has *not* fired: 31 days against 42. That is the count triggers doing
exactly the job §74 gave them, and it is worth seeing once, because a mechanism resting on the
calendar alone would still be reporting green today.

The triggers, and the ruling behind each, are
[§74](../decisions/0074-audit-cadence.md): **event triggers first, a count second, the calendar a
backstop.** The count numbers are the interval this project chose when nobody was counting, rounded
to something a person can hold.

| Kind | Milestones | Components | ABI constants | External packages | Weeks |
|---|---|---|---|---|---|
| security | 15 | 8 | 1 | 10 | 6 |
| documentation | 10 | 10 | 1 | 30 | 12 |

Read a cell as "fires when this many have been added since the last audit of this kind". So `1`
means any change at all fires, which is how the one event trigger a script can count is expressed: a
new syscall method shows up as a new ABI constant. `15` and `8` are the count trigger of §74 (audits
run on change, not on the calendar). `6` is the calendar backstop, and its job is the opposite of
what a reader assumes: it does not catch a busy period, because the count catches that sooner. It
catches a **quiet** one, a tree that sits untouched while the field's threat model moves anyway,
which no measure of this project's own change can see.

**External packages fire at 10 for security, a count rather than an event** (calef, 2026-10-20 UTC).
Every new package already passes an architect's §46 (thin primitives or whole subsystems) ruling
one at a time, so an audit on each one repeats that ruling. A dependency also rarely arrives alone:
across the merges that added packages between 2026-08-01 and 2026-10-20, the median one brought 6
with its transitive graph. At 1, or at 5, a single ruling of median size fires the audit by itself.
At 10 it takes about two of them, which at the measured rate of roughly 5 packages a week is about
two weeks of dependency change. One subsystem large enough to clear 10 alone (the crypto provider
brought 64) is the case that ought to.

**The documentation row is not the security row with different numbers, and two of its cells say so
out loud** (milestone 93, 2026-08-16). Every milestone edits prose, so the count trigger is slightly
tighter at 10; a doc correction is also cheaper to make than a security finding is to disposition,
which is what makes a tighter number affordable rather than fatiguing. The other two are the honest
ones:

- **External packages at 30, which is effectively off.** A documentation sweep does not care that a
  transitive lockfile entry moved; it cares that a dependency *class* arrived, and nothing counts
  classes. Thirty is a batch size that means the graph really moved. The cell exists because the
  table has a column, and pretending it is a real trigger would be worse than saying this.
- **The calendar at 12 weeks rather than 6, because the backstop's job inverts here.** Security's
  calendar catches a quiet tree, since the field's threat model moves whether this repository does or
  not. Documentation rots *because the tree changed*, which the counts already see, so a quiet tree
  does not rot its docs. The one thing the counts cannot see is a substantial rewrite inside an
  existing crate, and `script/audits --worklist` catches that far better than any calendar: it ranks
  documents by how much of the code they cite has moved since they were last edited.

**Two of milestone 92's four event triggers are not in that table, and cannot be.** "A new component
holding device or network authority" and "first boot on a new machine class" are judgments about what
a component *does* and about hardware that may not be in the tree at all. Nothing counts them, so
`script/audits` prints them as a question rather than pretending to answer it. The count triggers are
the backstop for exactly this: a new network-facing parser is also a new component, so the batch
trigger reaches the same place, later. Later is the wrong word for an attack surface, which is why
the judgment stays a human's and why it is printed on every run rather than filed somewhere.

**Red means "run the audit", never "an automation ran it for you".** That is why the overdue check
lives in a scheduled workflow (`.github/workflows/audit-cadence.yml`) and not in the gate every pull
request runs. An audit coming due is not a defect in the commit that happened to trigger the count,
and blocking every unrelated merge behind a review nobody can hurry is the wall this mechanism was
written not to build. It is the same split, and the same reason, as `toolchain-drift.yml`.

`script/lint` does run `script/audits --check`, which is the *structural* half: the two tables below
describe the same audits, every report link resolves, every kind has a cadence row, the dispositions
parse. That can fail a pull request, because a malformed index is a defect in the commit that
malformed it.

## The audits

| Date | Kind | Lens | Findings | Report |
|---|---|---|---|---|
| 2026-07-15 | security | The whole kernel read cold, four reviewers, one dimension each, after milestone 11 | fixed 4, minted 0, accepted 0 | [A security audit](../../notes/security.md) |
| 2026-07-29 | security | The hand-written architecture assembly, for state staged in single-copy hardware registers across more than one instruction | fixed 2, minted 0, accepted 1 | [Auditing the hand-written architecture assembly](../../notes/arch-audit.md) |
| 2026-08-04 | security | Time of check to time of use across every page shared by two address spaces | fixed 5, minted 1, accepted 1 | [Auditing the shared pages](../../notes/shared-page-audit.md) |
| 2026-08-15 | security | Untrusted counterparty input: a value a hostile counterparty supplies in one message or completion | fixed 0, minted 0, accepted 1 | [Auditing untrusted counterparty input](../../notes/untrusted-input-audit.md) |
| 2026-08-16 | documentation | Docs versus reality, scoped by the staleness worklist, read for names and numbers a reader would act on | fixed 4, minted 1, accepted 2 | [Names and numbers the tree moved past](2026-08-16-docs-versus-reality.md) |
| 2026-08-17 | documentation | The ABI surface as documented, read from the wire outward: every constant two programs must agree on, checked against what the prose says the surface is | fixed 5, minted 1, accepted 2 | [The ABI surface as documented](2026-08-17-abi-surface-as-documented.md) |
| 2026-08-17 | security | Newly minted authority, read adversarially: the seven ABI constants and the new right that landed overnight, not the tree at large | fixed 2, minted 1, accepted 3 | [The authority that was minted overnight](2026-08-17-newly-minted-authority.md) |
| 2026-09-17 | security | Userspace confinement, read adversarially: the device and port authority minted since the last audit, the claims milestone 307 marked unreachable, and the two machine classes (radon, xenon) that booted real silicon in the window | fixed 3, minted 3, accepted 1 | [Userspace confinement](2026-09-17-userspace-confinement.md) |
| 2026-09-24 | documentation | Split documents read from the inbound side: every citation into the day's eight split documents, every anchor, every appendix's parent link, and every phrase quoted after a path | fixed 4, minted 1, accepted 3 | [Split documents, read from the inbound side](2026-09-24-split-documents-read-from-the-inbound-side.md) |
| 2026-09-24 | security | New trust boundaries, read where the week's change concentrated: the automation that merges, the bytes a file supplies, and the state a core carries for a thread | fixed 5, minted 1, accepted 5 | [New trust boundaries](2026-09-24-new-trust-boundaries.md) |
| 2026-09-29 | documentation | Names and numbers a reader would act on, in the worklist's top three, the inbound side of the six notes the window split, and the window's two new notes read against the kernel | fixed 6, minted 1, accepted 1 | [Names and numbers the window moved past](2026-09-29-names-and-numbers.md) |
| 2026-09-29 | security | The syscall surface as a whole, read as a confined process trying to widen: the two object types, the rendezvous method and the wake tag the window added, line by line, beside the older seams | fixed 0, minted 0, accepted 3 | [The syscall surface as a whole](2026-09-29-syscall-surface-whole.md) |
| 2026-10-03 | security | The window's delta, read as a confined process trying to widen: the eight ABI constants `MemoryRegion::USAGE` added, the capability table's growth to 32, the plain-`SEND` badge, and the thirteen components, each for what it holds against what it uses | fixed 5, minted 4, accepted 6 | [Eight constants and thirteen components](2026-10-03-eight-constants-and-thirteen-components.md) |
| 2026-10-15 | documentation | The staleness worklist's top six, read for what a reader would act on: a path to type, a count to trust, a tense that names the present, and a named lane that already built the thing a note still argues for | fixed 9, minted 0, accepted 0 | [The worklist's top six](2026-10-15-the-worklists-top-six.md) |
| 2026-10-19 | documentation | Decisions read forward: the ten sections minted since the last sweep, §300 (the paused state) to §309 (a supervised job's `ThreadControlBlock` goes to the shell that asked for it), each read for what it closed, against the prose that still argued the closed side | fixed 11, minted 2, accepted 4 | [Decisions read forward](2026-10-19-decisions-read-forward.md) |
| 2026-10-19 | security | The supply chain, read from the package inward: the nine external packages the window added and the gate that should see every graph they enter, then the path by which bytes this tree did not build become authority on the target | fixed 3, minted 0, accepted 2 | [The supply chain, read from the package inward](2026-10-19-supply-chain-read-from-the-package-inward.md) |
| 2026-10-20 | security | The window the supply-chain audit left unread, as a confined process trying to widen: the device authority minted since 2026-10-03, the ABI names added since then followed into their handlers, and the atlas's six host packages read inward to the tables the target draws | fixed 4, minted 3, accepted 2 | [The window the supply-chain audit left unread](2026-10-20-the-window-the-supply-chain-audit-left-unread.md) |

## What the tree looked like when each ran

The tripwire's half of the index. It is a second table rather than four more columns on the first
one because the two have different readers: a person wants the lens and the findings, and
`script/audits` wants the counts. The `--check` gate insists the two tables list exactly the same
audits, which is what stops them drifting apart.

Every number is **counted, not remembered**, by `script/audits --baseline` at the commit that landed
the report. Milestones built is the `BUILT` rows of
`script/roadmap --index`, which assembles them from the blocks in
[design/roadmap/](../roadmap/); components is `crates/*/` plus `[[bin]]` targets in
`components/Cargo.toml` **and** `fixtures/Cargo.toml`; ABI constants is the `pub const NAME: u64`
surface of `crates/abi`; external
packages is the distinct registry packages across every committed lockfile but the vendored one.

**Both userspace packages, and the reason is this table rather than the word "component"**
(milestone 311). Every row below was counted when `user/` was one directory; milestone 175 split it
into `components/` and `fixtures/` on 2026-09-13 and moved no program across the boundary (72
`[[bin]]` targets before, 73 after). Counting only `components/` would read 114 against a baseline of
110 that meant 155, under-reporting the trigger by forty, which is the one direction an audit signal
must not err in. So the count spans both, and it stays continuous across the split.

| Date | Kind | Milestones built | Components | ABI constants | External packages |
|---|---|---|---|---|---|
| 2026-07-15 | security | - | 10 | 9 | 3 |
| 2026-07-29 | security | - | 43 | 42 | 20 |
| 2026-08-04 | security | 55 | 95 | 43 | 87 |
| 2026-08-15 | security | 71 | 104 | 43 | 108 |
| 2026-08-16 | documentation | 73 | 108 | 43 | 108 |
| 2026-08-17 | documentation | 76 | 110 | 50 | 108 |
| 2026-08-17 | security | 76 | 110 | 50 | 108 |
| 2026-09-17 | security | 190 | 155 | 52 | 108 |
| 2026-09-24 | documentation | 252 | 169 | 56 | 172 |
| 2026-09-24 | security | 251 | 169 | 56 | 172 |
| 2026-09-29 | documentation | 276 | 184 | 68 | 181 |
| 2026-09-29 | security | 276 | 184 | 68 | 181 |
| 2026-10-03 | security | 288 | 197 | 76 | 181 |
| 2026-10-15 | documentation | 418 | 222 | 95 | 187 |
| 2026-10-19 | documentation | 466 | 235 | 97 | 190 |
| 2026-10-19 | security | 455 | 234 | 97 | 190 |
| 2026-10-20 | security | 469 | 236 | 97 | 196 |

The two `-` cells are honest rather than lazy: the roadmap was a single file with no status column
until milestone 76 (split the roadmap: `design/roadmap/README.md` as index, one file per milestone) split it into `design/roadmap/` on 2026-08-03, and milestones 1 to 11 were
backfilled the same day, so
there is no contemporaneous count to take. `script/audits` refuses to compute a delta from a `-` and
says which trigger it therefore cannot evaluate. It only ever reads the newest row per kind, so the
gap costs nothing today.

**One number here disagrees with §74 by one.** That entry says the shared-page pass landed with 54
milestones built; counted from the roadmap index at the commit that added the report, it is 55. Both
were counted honestly at slightly different commits, which is the same class of thing CLAUDE.md
records about the Kani harness count. The method above is stated so the number is re-derivable, which
matters more than which of the two is right.

## Where the reports live, and why the four security reports are not in this directory

**New audit reports land here**, and the first one to do it is
[the 2026-08-16 documentation sweep](2026-08-16-docs-versus-reality.md), which also set the file
name: the audit's date, then its lens, so the directory sorts chronologically and the index's `Date`
column is the filename's first field. The four security reports predate this directory and stay in `notes/`,
linked rather than moved, and the reason is a measurement rather than a preference: those four files
are referenced **85 times from 27 files**, including kernel source comments
(`kernel/src/arch/riscv64/trap.s`, `kernel/src/drivers/plic.rs`, `kernel/src/sched.rs`), a dozen
notes, six user programs, `SECURITY.md`, `README.md`, and four files under `design/` that a lane may
not edit at all. Moving them means rewriting all of that by hand or by a blind `sed`, and a blind
`sed` is the specific mechanism that destroyed a naming refusal in this tree once already.

The cost of not moving them is one hop for a reader, and it is the smaller cost. The index carries
the date, the lens, the dispositions and the link, which is everything the mechanism needs and most
of what a reader wants before deciding to open a 600-line report.

`notes/redoxfs-audit.md` is deliberately **not** in the index. It carries the word and it is a
different act: it costed a port by building the crate, and it did not look for vulnerabilities. An
index that took every file called an audit would report a coverage it does not have.

## Running one

1. **Pick the lens the last audit lacked.** That is milestone 43's insight and it is the whole
   reason the `Lens` column exists: read the rows above and ask what they did not look at.
   Candidates not yet taken: the compiler (the nightly pin, `patches/std-nife` under
   `-Zbuild-std`, and the workflow that bumps the pin), the graphical drivers' parsers (milestone
   600 (the graphical terminal stack is built in userspace)), and the device authority radon's
   peripherals took.

   **For a `documentation` sweep the lens question has a starting answer rather than a blank**, and
   it is `script/audits --worklist`: it ranks every document by how many of the files it cites have
   moved since the document itself was last edited. Read
   [notes/documentation-audit.md](../../notes/documentation-audit.md) first; it is the procedure, and
   it says what counts as a finding, where the boundary with milestones 125 and 117 falls, and what
   the ranking cannot see.
2. **Say what you did not look at.** Every report on record has a "what was deliberately not
   examined" section, and it is the section an outside reviewer uses first.
3. **Every finding ends in exactly one of three states**, and the report is not done until each has
   one:
   - **fixed**, in the audit lane itself, for anything trivial enough to fix while you are there;
   - **minted as a milestone**, where the report proposes it and the integrator mints the number at
     merge, with severity and rationale in the block. That is how milestone 90 (a guard page under the per-CPU secondary stacks) was born from
     milestone 84 (stack high-water)'s finding. **A minted finding that bears on a fatal risk** (for a security audit,
     usually risk 7, confinement) is also cited from that risk's entry in
     [design/fatal-risks/](../fatal-risks/README.md). The integrator adds the citation at merge,
     because a lane does not edit that directory. Fatal risks are what the project launches from
     (calef, 2026-10-03), and `script/fatal-risks` checks that each cited path exists and that no
     cited proposal changed after the entry's last stated date; a finding left only in the
     proposals directory is never looked at again;
   - **recorded-accepted**, with the reason, in the report *and* in the affected doc's `BUGS`
     section wherever a reader would meet the risk.

   **"Noted" is not a state.** DECISIONS §35 already names the failure mode: a finding nobody
   dispositions is wallpaper, and a security label does not change that. The `Findings` cell counts
   them as `fixed N, minted N, accepted N`, and `script/audits --check` fails a row that does not.
   The cell says `minted` rather than `milestone` because `script/roadmap` validates the phrase
   "milestone <number>" anywhere in the tree as a citation, so a findings cell with a count of zero
   read as a citation to milestone zero and failed the build on this milestone's first full lint
   run.
4. **Re-baseline the docs in the same lane.** `SECURITY.md`'s claims, the confinement scope, and the
   affected notes' `BUGS` sections are part of the audit's diff. An audit that finds the docs
   overclaiming fixes them there and then, because an overclaim in `SECURITY.md` is itself a security
   finding.
5. **Add both rows**, here and in the baseline table. `script/audits --baseline` prints the counts
   to paste.

## EXAMPLES

Is anything due, and why or why not:

```sh
script/audits
```

The tripwire's own question, as the scheduled workflow asks it. Exit 0 when nothing is due, 1 when
something is:

```sh
script/audits --due; echo "exit $?"
```

The structural gate, which is what `script/lint` runs:

```sh
script/audits --check
```

The counts for a new row, at the tree you are looking at:

```sh
script/audits --baseline
```

## BUGS

- **A mechanism guarantees that audits happen and that findings get dispositioned. It does not make
  any audit good.** The lens list above is a prompt, not a proof of coverage, and the same honest
  limit applies here as the CPU matrix records about its five models.
- **Two of the four event triggers are uncountable and are printed as a question, which is rung four
  on CLAUDE.md's ladder.** "A new component holding device or network authority" and "first boot on a
  new machine class" rely on somebody reading the output and thinking. That is a known weak spot,
  mitigated only by the count triggers reaching the same place later. The `documentation` kind has
  two of its own, and they rely on the same thing.
- **Adding a kind is one row here and, if it has uncountable triggers, one entry in `script/audits`.**
  The cadence numbers are index data on purpose, so a kind's *thresholds* never require editing code;
  its judgment questions do, because a question is prose and no table cell holds one. That is one
  thing to remember at the moment a third kind is added, and remembering is rung four.
- **The disposition columns for the four retroactive rows were read out of reports written before the
  three-state rule existed.** They are a faithful summary of what each report's own summary table
  says, but no author of those four was working to this vocabulary, and "left documented" was mapped
  to `accepted` by a later reader. Audits run under the mechanism state their own dispositions.
- **The count triggers cannot see a change that lands as neither a milestone nor a component.** A
  substantial rewrite inside an existing crate moves no number in the baseline table. The calendar
  backstop is the only thing that eventually catches it, and six weeks is a long time.
- **Nothing checks that the baseline numbers were taken at the commit they claim.** They are prose
  in a table, verified by re-running `--baseline` against that commit, which nobody does
  automatically. The `--check` gate validates that the cells are integers or `-`, not that they are
  true.
- **The scheduled workflow does not report its own death, and this has now happened rather than
  being a worry.** Same limitation (dates below verified against `gh run list`) `notes/merge-queue.md` records for the merge drain: a workflow
  that is disabled, or whose schedule GitHub drops on an inactive repository, goes quiet in exactly
  the way a green run does. It also only runs from the default branch, so it cannot be exercised on
  a pull request; run `script/audits --due` by hand, or dispatch the workflow once it is on `main`.

  **This workflow has never once succeeded, and reading why is the finding** (milestone 311,
  2026-09-17, after the lane's own brief had it wrong and the logs corrected it). All five scheduled
  runs since the first on 2026-08-17 went red. It is tempting, and it was the lane's starting
  assumption, to read five red runs as five weeks of a broken tripwire. Four of them were the
  tripwire **working**:

  ```
  2026-08-17  audits: 5 on record, DUE: documentation, security   exit 1
  2026-08-24  audits: 7 on record, DUE: documentation, security   exit 1
  2026-08-31  audits: 7 on record, DUE: documentation, security   exit 1
  2026-09-07  audits: 7 on record, DUE: documentation, security   exit 1
  2026-09-14  FileNotFoundError: .../user/Cargo.toml              exit 1
  ```

  Only the last is the path defect, and it is four days old, not five weeks: milestone 175 split
  `user/` on 2026-09-13 and the very next scheduled run broke. **An audit has been overdue every
  single week since 2026-08-17, the mechanism said so on schedule every time, and no audit was run.**
  That is a month in which this directory's whole purpose was served correctly and changed nothing.

  **Why a month of correct alarms was as invisible as silence, which is the part to design against.**
  Red *is* this job's signal, by deliberate choice, so the Actions tab shows the same color whether
  the tripwire is firing or the tripwire is broken, and a reader who has learned that this job is
  "the red one" stops distinguishing them. The path defect then hid inside the alarm it replaced.
  Milestone 238's `script/cadence-check` catches the second failure by asking when a workflow last
  *succeeded*, which is the right question for a dead job and, here, the wrong one for a live one: a
  job whose healthy state is red has no green to measure staleness against. Nothing yet notices a
  cadence job that is red for a *new* reason, and nothing yet notices the thing that actually
  mattered, which is that the same red repeated four times and nobody acted. Both are open.
- **A due audit can be closed by editing this file.** Adding a row is all it takes, and nothing
  anywhere checks that a report describes work somebody did. That is not fixable by a script and it
  is worth saying out loud: the mechanism makes the audit *scheduled*, and only a person makes it
  real.
