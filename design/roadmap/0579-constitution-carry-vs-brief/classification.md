# Milestone 579's table, every section but the three roles

An appendix to
[`design/roadmap/0579-constitution-carry-vs-brief.md`](../0579-constitution-carry-vs-brief.md),
milestone 579 (which of the constitution must be carried, and which is a brief). It holds the
passage-by-passage verdicts for every section of `AGENTS.md` except the three roles, which are in
[roles.md](roles.md). It was moved out of the block on 2026-10-12 (UTC) under §212 (a prose budget);
the block is complete without it. Line ranges are against `AGENTS.md` as it stood when the proposal
was filed on 2026-09-23 (925 lines).

A verdict of `brief` names the provisional brief it would join; `uncertain` says why the block's two
sub-questions did not both resolve cleanly.

### Preamble and "What this project is" (lines 1-30)

| Lines | Opens with | Verdict | Why |
|---|---|---|---|
| 1-8 | *This file is `AGENTS.md`...* | carry | Identity of the file and the architect; every other row depends on a reader already having this. |
| 10-24 | *A capability microkernel for aarch64...* | carry | Project mission and calef's role (architect and reviewer). Shapes every judgment call, not one task. |
| 26-30 | *Default to autonomous execution.* | carry | Governs behavior on every task; nothing prompts a lookup of "how autonomous should I be." |

### Three principles (32-161)

| Lines | Opens with | Verdict | Why |
|---|---|---|---|
| 32-36 | *These are not aspirations.* | carry | Frames the three principles that follow; needed to read them. |
| 38-102 | Principle 1, the ranking function | carry | Prioritization ethos applied to every "what do I work on / how do I rank two milestones" judgment, which recurs constantly and unpredictably, not at one named task. |
| 104-134 | Principle 2, the method is a result | carry | Project self-understanding and the honesty-about-numbers rule it sets up; short enough that extracting it saves little and it is read alongside principle 1 and 3. |
| 136-161 | Principle 3, a newcomer must succeed alone | carry | Directly shapes documentation practice (`BUGS` sections, `EXAMPLES`) on every task that touches docs, and states this proposal's own test criterion in miniature ("a stranger, with only this repository"). |

### The mechanism ladder and two tenets (163-302)

| Lines | Opens with | Verdict | Why |
|---|---|---|---|
| 163-204 | *calef, 2026-08-04, after an evening...* the ladder | carry | Core judgment framework for how hard to make any new rule; consulted every time something new is built, which is not one task. |
| 206-250 | *We wouldn't be building this project out of convenience.* | carry | Governs every architectural recommendation; the "would I still choose this at equal cost" test must already be in head before the recommendation is made, not after. |
| 252-302 | *calef, 2026-08-05. The ladder above says how hard...* | carry | Reversibility framework (what is cheap to redo vs. what leaves the machine); applies to any decision at any moment, no single trigger. |


### A fork reaches calef with its questions already answered (624-691)

| Lines | Opens with | Verdict | Why |
|---|---|---|---|
| 624-639 | *calef, 2026-08-18, sharpening his own rule...* | carry | Sets up why the seven questions exist; the recognition that "this is a fork and needs this treatment" must be ambient. |
| 641-673 | **The seven questions, because they are stable** | brief | A provisional `raise-a-fork-for-calef.md` (name and even the directory are open, see [the five briefs](../0579-constitution-carry-vs-brief.md#the-five-new-briefs-provisional-names)). Writing up a fork for calef is a task a lane already recognizes itself as doing once the "stop and bring it to calef" criterion has fired (lines 546-549, in [roles.md](roles.md)); the seven-question checklist is exactly the kind of reference a lane would want spelled out rather than reconstructed from memory at that moment. **Flagged open question**: `briefs/README.md` currently scopes that directory to "work that recurs, whose failure modes are already known, and whose correctness a gate can check... not design, not prose a reader will trust," aimed at rented mechanical-work models. This passage is prose a reader will trust, produced by whichever agent is presenting to calef, not mechanical work for a cheap model. It may belong in `briefs/` with that scope note widened by one sentence, or it may belong somewhere else (a `design/` reference doc). That choice is calef's, not this lane's. |
| 675-691 | **Two limits, so this does not become a tax** | carry | Judgment for *whether* to spin a lane or a research pass at all; must fire before the seven questions are even reached, so it has to be ambient rather than found by consulting something after the decision to look is already made. |

### Codebase rules and naming (693-782)

| Lines | Opens with | Verdict | Why |
|---|---|---|---|
| 693-747 | **The rules that hold the codebase together**, seven numbered rules | carry | Architectural constraints (arch code location, driver isolation, system call surface, memory ordering, dependencies, the `#[path]` rule) that must already be known before a line of code is written in the wrong place; there is no "check architecture rules" task separate from writing code itself. |
| 748-772 | **calef names the crates, the programs, and the shared modules** | carry | Naming is on the file's own list of irreversible categories (the reversibility tenet, "move fast on what can be undone"); must fire the instant something new is created, which is constant and unscheduled. |
| 774-782 | *Milestone 7's process-model question is decided*, milestone 7 (user mode: EL0, capabilities, the ELF loader, and IPC) | carry | Short; defines what counts as a design fork on the system call surface, needed the moment new system call work is touched. |

### Testing, commits, dates, comments, style (783-878)

| Lines | Opens with | Verdict | Why |
|---|---|---|---|
| 783-798 | *`script/test` (a thin wrapper...)* | carry | Short definition plus the "tests should prove something specific" ethos, needed whenever any test is written. |
| 799-804 | *One purpose per commit.* | carry | Applies to every commit. |
| 805-817 | **Commit early and push, then curate before reporting.** | carry | Core practice for every lane's whole session, not one task; matches the user's own standing "push cadence" memory. |
| 818-824 | **Squash against the base commit you branched from, never against `origin/main`** | carry, landmine | Recorded scar (a lane staged four other lanes' files); must be known before *any* squash, which can happen at arbitrary points, not a scheduled task. |
| 825-836 | **`git stash` is unsafe in these worktrees** | carry, landmine | This is the passage the proposing lane's brief named directly as the example of a procedure that is really a landmine; kept carry for exactly the stated reason: no task boundary contains the risk. |
| 837-847 | **Never squash across purposes.** | carry | Short, applies whenever committing. |
| 849-857 | *calef, 2026-09-13, closing a gap...* every date is UTC | carry | Applies to every dated line written anywhere in the tree; far too broad a trigger to confine to one task. |
| 858-867 | *The kernel is commented far more heavily...* | carry | Applies whenever writing a comment. |
| 868-878 | *calef's global preferences apply...* Style | carry | Applies to all prose written in this tree; short. |

### QEMU (879-925)

| Lines | Opens with | Verdict | Why |
|---|---|---|---|
| 879-891 | **Never leave QEMU running**, `wfi` not `wfe` | carry, landmine | A halted kernel using the wrong instruction burns 99.7% of a host core; must be known before the code is written, not after the burn is noticed. |
| 892-898 | Environment facts (macOS, QEMU via Homebrew, Rust nightly, target triple) | carry | Five lines of reference facts; too small to be worth the overhead of a separate file. |
| 899-906 | **`perl -e 'alarm N; exec @ARGV'` DOES NOT WORK ON QEMU.** | carry, landmine | Leaked eleven QEMU processes burning 729% CPU in one day when this was not known ahead of time. Must be ambient. |
| 908 | **After any session that ran QEMU, check `pgrep -x qemu-system-aarch64` and clean up.** | carry | The one-line ambient trigger that must survive on its own even after the hunting-recipe passage below moves; see the caveat under [what could go wrong](../0579-constitution-carry-vs-brief.md#what-could-go-wrong). |
| 910-925 | *That check is not sufficient after you kill a harness...* the hunting recipe | brief | `hunt-leaked-qemu.md`. Once the trigger at line 908 has fired, this is a specific, bounded task (find and safely kill leaked QEMU) with a precise recipe: `lsof` the image file, walk `ps -o pid,ppid` in both directions before killing anything, kill the tree at its root. Exactly `briefs/triage-a-failing-check.md`'s shape. |
