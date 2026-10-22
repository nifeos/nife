# Working on nife

*Two renames and one pivot sit behind the old names a reader will meet in older records:
[design/tenets/project-history.md](design/tenets/project-history.md). Every reason, measurement and
anecdote behind a rule here lives in [design/tenets/](design/tenets/), linked from the rule it
explains.*

## What this project is

A capability microkernel for aarch64, in Rust, built from the first instruction. **It is a
demonstration OS**: a verified-Rust capability microkernel that runs real workloads, built to stand
next to Linux, macOS, and seL4 on the primitives that define an OS and win where a minimal kernel
should. An architect, listed in [ARCHITECTS.md](ARCHITECTS.md), is an experienced engineer who
reviews this project's architecture and outcomes, not the line-by-line builder.

That should drive your judgment calls. A complete, correct, well-documented, benchmarked milestone
is the goal. Proceed autonomously, produce whole pieces, and let an architect steer design forks.

## How to work

Default to autonomous execution. Implement complete, correct, tested milestones; commit per proven
piece (green tests first); push after green.

## Read these when the work calls for them

This file is what every session and every lane needs on every turn. The rest of the rules are in
`notes/skills/`, one directory each. Each is an Agent Skills `SKILL.md`: an agent that reads only files follows the
link below at the moment named, and a tool that loads skills finds the same files through
`.claude/skills/` and `.agents/skills/`, which link to them. A lane brief names the ones its lane
needs.

- Before you brief, gate or merge a lane, or clean up after one, read
  [notes/skills/maintainer/SKILL.md](notes/skills/maintainer/SKILL.md). It holds the three roles
  (maintainer and steward), the top-up rule and what bounds lane count.
- Before any work in a lane, read [notes/skills/developer-lane/SKILL.md](notes/skills/developer-lane/SKILL.md).
  Its checks run in `script/lint-fleet`.
  It holds the developer role, the claim, the handoff, identified work and shared state.
- Before you choose between options, recommend one, or hold work for an architect, read
  [notes/skills/decisions/SKILL.md](notes/skills/decisions/SKILL.md). It holds "move fast on what
  can be undone", "elegance and performance beat implementation convenience", open decisions and
  `needs-architect`, and how a fork reaches an architect.
- Before you mint or change the name of a crate, program, module or public function, read
  [notes/skills/naming-authority/SKILL.md](notes/skills/naming-authority/SKILL.md).
- Before you commit or push in a worktree, read
  [notes/skills/worktree-commits/SKILL.md](notes/skills/worktree-commits/SKILL.md).
- Before you run QEMU by hand, or after a session that did, read
  [notes/skills/qemu-hygiene/SKILL.md](notes/skills/qemu-hygiene/SKILL.md).

## Three principles, and what makes each one hold

Each names a mechanism that keeps it true when nobody is watching, which is the only kind of
principle a free software project can enforce. The evidence, the failures that confirmed them and
calef's own wording are in [design/tenets/three-principles.md](design/tenets/three-principles.md).

### 1. The ranking function is the shortest path to a system a customer runs

When two milestones are both ready, the one on the customer path goes first. As of 2026-08-30 that
path is vacant, so the tie breaks toward [design/fatal-risks/README.md](design/fatal-risks/README.md), nine claims
that, if false, mean the project should stop. A real workload with a real user outranks everything
on that list the moment one exists. A first customer must be something nife can plausibly be
adequate at within a milestone or two. Do not expose nife to a second customer before there is a
package manager and a trivial install process (calef, 2026-08-30). Security, performance and naming
are on this path rather than beside it: they are what "runs it" means. A milestone off the path is
not thereby worthless, but when two compete for a lane, the tie breaks toward the thing that gets a
real workload running.

### 2. The method is a result, and it has to be recorded with its caveats

From a first commit on 2026-07-12, this tree passed two hundred thousand lines of Rust on three
architectures in under three months, with a booting kernel on real RISC-V silicon, a shell, a
filesystem, a network stack and a compositor. That order of magnitude was written 2026-09-24.

**A number here changes at the pace of a decision, not at the pace of a commit** (calef,
2026-09-24). Counts live in `notes/project-metrics.md`, generated weekly so they cannot rot; read
them in code lines, since this tree comments heavily. (The 2026-08-05 and 2026-08-30 figures are in
git.)

That is not a normal rate for one architect, and the reason is that the work is done by many agents
in parallel lanes with one person reviewing architecture and outcomes. **The demonstrator is
therefore two claims, not one**: that a capability microkernel can run real workloads, and that a
system of this size can be built this way at all. The second is at least as interesting to a
stranger, and `notes/how-this-is-built.md` is where the tree now states it.

**It has to be recorded the way everything else here is recorded, with the caveats attached**, or it
is marketing:

- The figures there are **size and rate, not quality.** A built milestone is a block marked BUILT,
  and §76 (what catches a milestone status that is wrong in both places) records a sweep that found
  nine misrecorded. Take the count as a scale, never as a claim about correctness.
- **What makes it work is not speed.** It is the gates, the proofs, the honest `BUGS` sections and
  the review discipline. The same method without them produces a great deal of code that nobody can
  trust, faster. Every failure recorded in this file is evidence for that.
- **The bottleneck moves, and pretending otherwise wastes the method.** On 2026-08-04 the constraint
  stopped being how fast lanes could produce and became how fast one merge queue could land, and
  eleven lanes made that worse rather than better.

### 3. A newcomer must be able to succeed without asking anyone

Documentation is task-oriented and in-tree, with real `EXAMPLES` and an honest `BUGS` section beside
the feature rather than in a tracker (see
[design/tenets/documentation-standard.md](design/tenets/documentation-standard.md)). Every decision
gets a written reason in `design/decisions/`, including the decisions that were refused. A name is a
claim and the reader meets it first, so an unratified name is a worklist item and never a blocker.
Anything that only works because someone knows it is a defect.

The test: could a competent stranger, with only this repository, get to a passing build and a
correct mental model without opening a chat window? Where the answer is no, that is a bug in the
tree and not in the stranger.

## Nobody remembers, so build the mechanism that does not need them to

Design for coordinating many, not for one attentive person (calef, 2026-08-04). Eleven lanes, a
conversation in progress and a queue draining in the background are this project's normal condition,
not its worst case. The evening that produced this, and the worked example on each rung, are in
[design/tenets/mechanisms-not-memory.md](design/tenets/mechanisms-not-memory.md).

The ladder, strongest first. When something must not go wrong, reach for the highest rung that fits:

1. Make the wrong state unrepresentable. A required struct field with no default is the strongest
   form there is, because the mechanism is the compiler and the exception surface is zero.
2. A gate that fails loudly, in `script/lint` or CI. Weaker, because somebody has to write it and it
   can be wrong about the tree, but it fires without being remembered.
3. A written record at the thing itself: provenance beside the name, not in a registry. It does not
   fire on its own, but the next person to touch that code is already reading it.
4. A note, a report, or a comment on a pull request. This is the floor.

"Somebody will notice" is not a mechanism. It is rung zero and it belongs on no list.

An exception is allowed and must say so. When the higher rung costs more than the failure does,
taking the lower one can be right. Write down that it is an exception and that it is a foot gun, in
the place a reader meets it. An unmarked exception reads as a design, and the next person extends
it.

The tell that you are on too low a rung: a fact that exists only at a call site or in a report, with
no artifact anyone can read. When you notice it, move up a rung.

## We are all owners: see a problem, drive it to an owner, and if none, own it

Once you have noticed a problem, it is yours to route, not to leave for whoever's pull request it
lands on. Noticing is not owning until the problem has an owner.

And owning is not recording. Apply *move fast on what can be undone*'s test to the fix: cheap and
reversible, fix it now and the record is a byproduct; on that tenet's irreversible list, write it up
and stop. A `BUGS` entry is the right answer to the second case and an evasion in the first. Acting
on what you half understand is worse than reporting it, so a refusal carrying its reason is an
action.

## Measure first, then decide

When the problem is not understood, measuring is the action: name the question the data will
answer, and decide the remediation separately, with the data in hand
([design/tenets/measure-first.md](design/tenets/measure-first.md)).

## Pull requests and comments an agent writes

- Every pull request and comment an agent writes opens by saying so. One line, first thing in the
  body: `**Lane:** <branch or milestone>, written by an agent; the author GitHub shows is the person
  running it.` It names no one because anybody may run their agents on this file (calef,
  2026-10-20 UTC). A workflow on the project's App says `nife-smelter[bot]` is the author instead.

## Measuring, pushing back, and correcting the record

Benchmarks and cross-OS comparisons are first-class. Measure, do not argue. State what each number
means and where it is not apples-to-apples: the map "tie" (zeroing-bound) and the spawn "lighter
object than a Unix process" caveats are the standard. An honest tie or loss recorded plainly is
worth more than an overclaimed win, and it is what makes the wins credible.

Push back when an architect is wrong, with a technical reason, and don't cave to be agreeable. Do
not manufacture disagreement to seem rigorous either.

Correct yourself loudly. The machine overrules the documentation, and it overrules you; when it
does, fix the record on purpose rather than quietly patching over it.

Explain on request, however basic. Autonomous by default does not mean opaque: if an architect asks
"what is a register?" or "why does `destroy` avoid `SCHED`?", answer properly, from the ground up,
and write it down. The anecdotes behind these four are in
[design/tenets/working-with-calef.md](design/tenets/working-with-calef.md).

## The rules that hold the codebase together

These come from `design/decisions/`. They are cheap to follow and expensive to retrofit. What each
one buys is in [design/tenets/codebase-rules.md](design/tenets/codebase-rules.md).

1. All architecture-specific code lives under `kernel/src/arch/`. Assembly, `asm!`, system
   registers, CPU-specific behaviour. If you're writing `asm!` outside `arch/`, that is the bug.
2. A driver never reaches into a kernel global. It gets what it needs passed in (a base address,
   later a DMA allocator, later an interrupt registration).
3. The syscall surface stays narrow and explicit. It is a boundary, not a habit.
4. Assume weak memory ordering. We're on ARM, which is the weak one, and that's a gift: don't
   squander it.
5. Architectural parity is a gate, not an aspiration: DECISIONS §19 (architectural parity is a
   tenet). The targets are aarch64, riscv64, and x86_64. riscv64 (radon) and x86_64 (xenon) boot
   on bare metal; aarch64 has run only under HVF on patagonia until argon, a TX1, arrives
   (corrected 2026-10-06, #1739). A kernel capability ships on every supported architecture, proven by the same suite, or
   a scope note records the gap and the plan. If a feature works on one ISA and silently not
   another, that is the bug.
6. Taking a dependency is a decision, not a convenience (§46). Write the kernel and the crates
   Kani proves, because a model checker needs code we can restructure. Everywhere else, take or
   adapt existing code first (calef, 2026-10-04): writing needs a recorded reason in the block's
   `Reuse:` line, and an architect still rules on each dependency. Vendor only what needs a patch.
7. Anything two binaries must agree on is a crate, never a `#[path]` module (calef, 2026-08-01). If
   a constant, an opcode, a layout, or an error code is shared by more than one program, it goes in
   `crates/` and is depended on. `#[path = "x.rs"] mod x;` is not an option, and `script/lint` check
   5 counts consumers per `#[path]` target and fails at two.

Rules 2, 3 and 7 are what keep the microkernel option open. We are deliberately not speculatively
trait-ifying every subsystem, because that builds the wrong abstraction before the requirements are
known.

## The syscall surface is a boundary, not a habit

Milestone 7's process-model question is decided: capabilities, an `svc` + `x8` ABI with a narrow,
explicit surface (DECISIONS §10, §16). New methods are fine within the established capability model
(object revocation added `Untyped::SPLIT` and `DESTROY` this way); record each new method's
semantics in `design/decisions/`, not just in code. A method that does not fit the model, or a
brand-new syscall number, is a design fork, raise it before building it.

## Testing

`script/test` boots the kernel under QEMU and reports pass/fail via semihosting.
[notes/scripts.md](notes/scripts.md) has the `script/*` front door and what `cargo xtask` exposes
beneath it.

Tests should prove something specific that nothing else would have done for us. The boot self-tests
in `kernel/src/lib.rs` are the model: `.bss` was zeroed (nobody else would have), `sp` is 16-byte aligned (a
bug here is a mystery crash), we're at EL1 (we are where we think we are). Don't add filler tests.

Pure logic (allocator algorithms, page-table math, scheduling policy, filesystem parsing) belongs in
crates that compile for the host, so most tests run in milliseconds without an emulator.

## Every date in this tree is UTC

calef, 2026-09-13, closing a gap that had been open since the first commit. Provenance blocks,
roadmap rows, `design/decisions/` sections, notes and commit messages all carry dates, and
`script/names` fails a ratification that lacks one, yet nothing said what zone any of them meant.
The agents writing most of them run UTC and an architect need not, so a ruling made in calef's
evening was already being filed under the next day. **Write UTC.** Where a date is load-bearing and
the hour is near midnight, put the time in the record too, because a reader cannot recover it later.

## Comments

Comments follow Linux's convention in all first-party Rust (calef, 2026-10-22 UTC: "Consider 929
ruled."). [§267 (a comment states the constraint as it is now)](design/decisions/0267-a-comment-states-the-constraint-as-it-is-now.md)
holds the rule and its stages.

- An item's doc comment opens with a summary sentence, then its arguments, what it holds and what
  it returns or refuses, and links the note that carries its design (`See notes/stack.md`). A
  `Context:` line appears only on code that holds locks or runs in interrupt context.
- A comment inside a function body states a constraint the code can't show: *why* `sp` must be set
  before the first `bl`, a `// SAFETY:` argument, a memory ordering and what it pairs with.
  Narrative and worked examples go in the doc comment or the note.
- "The caller must..." becomes an assertion or a type where one fits.
- History goes in the commit message. Do not write comments that restate the next line.
- A `Name:` provenance block stays as [design/naming.md](design/naming.md) shapes it, refusals and
  dates included (calef, 2026-10-22 UTC).

## Style

These rules bind every note, which is prose an architect rereads for months:

- No em-dashes. Use commas, periods, semicolons, or parentheses.
- No "delve", "comprehensive", "landscape", "moreover", "furthermore", "notably", "it's worth
  noting", "straightforward".
- No sycophantic openers, no filler conclusions that restate what was just said.
- State what is in force now. How it got that way (a replaced ruling, a status timeline, "was X,
  now Y", "moved here from", which lane did what when) goes in the commit message and the pull
  request body, where git keeps it (calef, 2026-10-13 UTC).
  [§298 (a document states what is in force now)](design/decisions/0298-a-document-states-what-is-in-force-now.md) lists what stays, and
  `helpers/history_ratchet.py` holds the phrases to a ratchet.
- Plain, direct language. Vary sentence length. Write like a person. The numbers are [§213 (writing standards)](design/decisions/0213-writing-standards.md), and length is [§212 (a prose budget)](design/decisions/0212-a-prose-budget-for-every-document.md). Spelling is American and measurements metric (calef, 2026-10-06).

## Environment

- macOS on Apple Silicon (itself aarch64, which is a nice coincidence: kernel assembly is the same
  ISA the laptop runs)
- QEMU via Homebrew: `qemu-system-aarch64`, `qemu-system-riscv64`, `qemu-system-x86_64`
- Rust nightly, pinned in `rust-toolchain.toml` (needed for `custom_test_frameworks`)
- Targets: `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`, `x86_64-unknown-none`
