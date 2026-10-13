---
status: BUILT
raised: 2026-09-13
built: 2026-09-13
---
# 284. The first process is called `progenitor`, and 775 sentences still said `init`

Built 2026-09-13. Minted the same day by the maintainer out of a measurement taken
against `main`: milestone 266 wrote down the policy for this and carried out three cases of it.
*(Number provisional until the merge queue lands it.)*

## Why this exists: a clause of milestone 266's own policy, not carried out

[Milestone 266](0266-init-is-an-action-and-the-thing-is-a-process.md) renamed the first process from
`init` to `progenitor` (ratified by calef on 2026-09-08) and published the rule it applied, four
bullets, of which the second is:

> A present-tense claim about the system changes, because it is now false.

It applied that to three claims, all in `notes/`, out of 748 occurrences of the role it had
counted. A measurement against `main` (2026-09-13) found the rest. Hundreds of live sentences, in
module headers, doc comments and notes, said that `init` holds the budget, endows the clock, builds
the shell and refuses an unmeasured program, about a process that is not called `init`.

That is a defect rather than a naming decision, which is why a lane may take it. Correcting a
sentence that is false is not the same act as choosing what a thing is called; 266's own policy
already classifies it, and the name it corrects to was ratified.

## The measurement

Whole-word `init`, everywhere a developer lane may edit (the tree less `design/roadmap/` and
`design/decisions/`), on base commit `2bc68bd0` and after:

| | count |
|---|---|
| occurrences before | 1,751 |
| changed by this milestone | 775, across 98 files |
| occurrences after | 976 |

What the 976 are, classified by reading rather than by pattern:

| what | count | disposition |
|---|---|---|
| the verb: `fn init`, `mmu::init`, `timer::init`, rustdoc links to them | ~387 | correct, untouched |
| records, quotations, captured transcripts, dated bench journals | ~347 | correct, untouched |
| `fixtures/src/hello.rs`'s five role constants and the prose bound to them | ~103 | calef's, proposed below |
| paths and filenames (`notes/trusted-init.md`, `target/init-measure-<arch>.txt`) | ~52 | untouched |
| covered by a blanket note another milestone already wrote | ~82 | untouched |

And, outside a lane's reach: 360 in other milestones' roadmap blocks across 75 files, 92 in
`design/decisions/` across 31 files, and 40 in `design/init-and-granular-spawn.md`, which is
under `design/` and so is not a lane's either.

## The rule applied, so it can be disagreed with

266's four bullets, plus the two this sweep needed and 266 did not:

- A present-tense claim about the system changes. The whole subject. `crates/measured_boot`'s
  *"the hash the kernel checks init against"*, `crates/grant_plan`'s *"the name init loads it by"*,
  `SECURITY.md`'s *"a compromised init cannot break the kernel"*: all false, all fixed.
- A record of what was true on a date does not change. Milestone 22 (trusted)'s title is *trusted init*,
  so `notes/trusted-init.md` keeps its name and its H1 keeps that phrase. `notes/visionfive2.md` is
  a dated first-silicon bench journal and was left whole. `notes/stranger-test.md` records what a
  stranger found on a given day, including *"the two archives boot different binaries under the name
  `init`"*, which was true when they found it.
- A quotation never moves, and where the thing quoted has since changed, a note goes beside it.
  Three cases: `components/src/builder.rs`'s self-description (quoted in two name-provenance blocks
  and in `script/names`' own header, now dated with *"then"*), the panic message `no building budget
  for init` (quoted in `notes/progenitor-and-loading.md` with a date, and in
  `kernel/src/user/disk_service.rs` with one already), and the serial line `init : measured, built,
  started` in `kernel/src/sched.rs`.
- A file a developer may not edit does not change. Counted above, not touched.
- An identifier does not change, because a name is an architect's. New here, and it is the line
  that decides most of what was left. Every `INIT_*` constant, `fn init` in a doctest, `init_bytes`,
  `INIT_STACK_PAGES` and the five `hello` roles stayed exactly as they are. Where the prose around
  one had to stop lying, it says what the thing is and leaves the identifier alone.
- A string a gate or a log parser matches does not change on its own. New here, and it cost a
  real decision. `crates/board_console` recognizes the boot-tour label `init/build  :` by substring,
  against captured VisionFive 2 logs that are evidence and cannot be re-written; so the label
  stays and the sentence after it was corrected. Runtime strings nothing parses (the `expect`s in
  `kernel/src/user.rs`, `could not spawn (init is out of memory)`, the handoff line) were changed,
  each checked against the tree first.

## What changed, by shape

- Kernel and component documentation, which is the bulk: `kernel/src/user.rs` (90),
  `crates/system_initializer/src/lib.rs` (80), `components/src/swish.rs` (48),
  `crates/grant_plan/src/spawnproto.rs` (32), `crates/grant_plan/src/lib.rs` (29),
  `kernel/src/trust.rs` (23), and forty more files with fewer.
- Four user-visible strings. `could not spawn (the progenitor is out of memory)` at the prompt
  (both copies, `components/src/swish.rs` and `crates/swish/src/lib.rs`), `nife: handing the system
  to the userspace progenitor.`, the `caps` table's spawn row, and the measured-boot refusal lines in
  `kernel/src/trust.rs`.
- Three stale claims that were wrong about more than the name.
  `components/src/builder.rs` said *"the kernel loads this program from the initrd's `init` entry"*
  and *"its archive entry is `init`, the one deliberate exception"*; both stopped being true when 266
  packed it as `builder`. `xtask/src/main.rs` said `hello` is *"packed as `init`"*. And
  `kernel/src/user.rs` cited `user/src/system_initializer.rs`, a path milestone 175 moved.
- Two present-tense descriptions of a fixed bug, in `kernel/src/user/sink_tests.rs`, moved to the
  past tense they were always describing: *"there `init` is the hello binary"* is no longer true
  and was never meant to outlive the fix it explains.
- One section left whole with a note added above it. `notes/pipes/second-stream.md`'s *"A
  correction: there are
  two inits"* is the correction it records, so it stays; a dated parenthetical now says 266 made it
  one, because a reader arriving at that heading would otherwise leave believing there are two.
- `crates/system_initializer`'s `Name:` block gained calef's refusal, which is the other half of
  this work and the reason the crate is out of scope. See below.

## `system_initializer` keeps its name, and the refusal now lives beside the name

calef, 2026-09-13, asked directly whether the crate should follow the program: *"init is the issue
not initializer."*

The argument is one asymmetry. `init` lost because it is a truncated verb where the rule asks for
a noun. `initializer` is an agent noun, the thing that initializes, which is what the rule asks
*for*: 266's own house-style list cites it beside `builder`, `spawner`, `supervisor` and
`provisioner` as evidence for the convention. And the crate is not the process; it is the
initialization, and it descends nothing, so `progenitor` fits it worse than it fits the program.

That is now a paragraph in `crates/system_initializer/src/lib.rs`'s existing `Name:` block, where
the next person to wonder will be reading, rather than only in a roadmap block. The full record is
milestone 266's `## Follow-on`, which the maintainer is handling; this lane did not touch it.

## `fixtures/src/hello.rs` still has five `init` roles, and they are an architect's

Milestone 266's title is *"`init` stops being a role."* It is still a role here, five times, and this
lane investigated rather than assumed.

What they are. `INIT = 20`, `INIT_DEV = 23`, `INIT_CONSOLE = 24`, `INIT_IRQ = 25`,
`INIT_LEAST_AUTHORITY_DEMO = 28` and `INIT_COREMARK = 29` are **six**, not five, and none of them is
the first process. They are milestone 19d/19e test roles in which the `hello` catalogue plays the
**parent**: at each one it parses an ELF out of the initrd, builds a child from its own budget, and
reports. `INIT_BOOT_ROLE` (27) was the one that meant "boot the system", and that is the one 266
moved out; what is left is the demonstration that a userspace process can load another.

**Who dispatches them.** `kernel/src/user/tests.rs`, six `#[test_case]`s, each with its **own**
duplicate constant (`INIT_IRQ_ROLE = 25`, `INIT_ROLE = 20`, and so on) passed to `spawn_progenitor`.
`kernel/src/bench.rs` and `kernel/src/main.rs` reach `hello` through `HELLO_ENTRY` for other roles.
So they are live, driven from the kernel's own suite, and not dead wiring.

**The recommendation, which is not `progenitor`.** These roles are not the progenitor, and renaming
them to it would replace one false name with another. What they have in common is that the binary is
the **parent** in each: `PARENT`, `PARENT_DEV`, `PARENT_CONSOLE`, `PARENT_IRQ`,
`PARENT_LEAST_AUTHORITY_DEMO`, `PARENT_COREMARK`, with `fn parent`, `fn parent_dev` and so on.

Refusals, because they are the valuable half:

- `PROGENITOR_*` is refused: false. The progenitor is one process on one archive entry, and
  `hello` at role 20 is neither.
- `LOADER_*` is refused: it names one of the three things the role does (parse, build, endow) and
  the roles that delegate a UART or an interrupt are not loading anything interesting.
- `BUILDER_*` is refused: `builder` is already a program in this tree with its own argument, and
  a role constant that shares that name would make two things claim one word, which is the exact
  refusal that cost `system_builder` its crate twice.
- **Leaving them** is a live option and costs nothing mechanical. What it costs is that milestone
  266's title is not true of this file, and a newcomer reading `INIT => init(dma_phys)` beside
  `PROGENITOR_ENTRY` has two words for one idea and no way to tell they are different ideas.

**Not performed.** A role constant is a name; names are an architect's; and `AGENTS.md`'s latitude
for recommending on reversible forks does not reach a name. The prose around them was left matching
them, deliberately, so that a rename moves one set of words and not two.

## BUGS

- The `init/build  :` boot-tour label is still wrong and is left that way on purpose.
  `crates/board_console::progress` matches it by substring and its captured VisionFive 2 fixture logs
  contain it; changing the label means changing the recognizer and stranding evidence that cannot be
  re-captured without the board. The sentence after the label was corrected, so the transcript now
  reads `init/build  : the userspace builder loaded ...`, which is honest and mismatched.
- `design/init-and-granular-spawn.md` is cited from nine places and is 40 occurrences of the old
  name, including its filename. It is under `design/`, which a lane may not edit.
- 471 occurrences remain in roadmap blocks and decisions, 360 and 92 respectively plus the design
  file, unchanged since 266 recorded the same limitation. The most misleading are named in the
  hand-off below.
- Three quotations are now dated rather than live, which is the rule working and is still a cost:
  a reader who greps `no building budget for init` or `a minimal init: the system builder` finds the
  note and not the code. Each says so where it sits.
- The captured transcripts in `notes/trusted-init.md` still read `init:`, unchanged from 266's
  own `BUGS`, and so do the fenced diagrams in `notes/pipes.md` and its appendices, whose column
  alignment is the
  meaning.
- This was not gated by a boot. `script/test` and `cargo build` were run; `script/swish-check`
  needs a QEMU this container does not have, and CI is what runs it. The change is comments and
  strings, and the four strings that a person sees were each checked against every consumer in the
  tree first, but the honest statement is that no interactive boot ran here.
- **The sweep is a judgment per occurrence and 976 were left.** The classification above was made by
  reading, and a reading is wrong some of the time. The tell for a miss is a sentence in the present
  tense with `init` as its subject.

## Follow-on

- **Recorded.** The `init/build  :` label, the `design/` occurrences and the dated quotations are
  limitations beside the features they belong to: `crates/board_console/src/progress.rs` for the
  label, and this block's `BUGS` for the rest.
- **Recorded.** The 471 occurrences in `design/roadmap/` and `design/decisions/` are this block's
  own `BUGS` entry beside the count, in `design/roadmap/0284-finish-the-progenitor-rename.md`, because
  a lane may not edit either directory. They are a maintainer's sweep and were re-counted rather than inherited: 360 across 75 roadmap files, 92
  across 31 decisions, on 2026-09-13. The three that mislead most, because they read as live claims
  rather than as history: `design/decisions/0014-project-direction.md` (*"init is the privileged
  unverified component"*, in the thesis `SECURITY.md` points at), `design/decisions/0026-fault-endpoint.md`
  (26 occurrences, the measured-boot record, *"this kernel image runs exactly this init"*), and
  `design/decisions/0055-shell-holds-the-redirect.md` (*"the `fs_subtree_caretaker` init would build per
  invocation"*). `design/init-and-granular-spawn.md` is a fourth of a different kind: 40 occurrences
  and the old name in its own filename, cited from nine places.
- **Milestone 399.** The six role constants above, their functions, and the
  `kernel/src/user/tests.rs` duplicates, with `PARENT_*` recommended and three alternatives refused.
  Held out of this milestone because a name is an architect's. Numbered on 2026-09-19 by milestone
  433's drain of the pile and `SUPERSEDED` in the same act by milestone 405 (the nine `INIT` roles),
  which turns these six parents into programs with their own names; 399 keeps the refusals and 405
  carries them forward.

## Index row

Milestone 266 (one progenitor, on all three architectures) renamed the first process and published
the rule that a present-tense claim about the system changes because it is now false; it applied
that to three claims out of the 748 it counted. 1,751 whole-word `init` remained where a lane may
edit, hundreds of them live sentences about a process that has not been called that since
2026-09-08. 775 changed across 98 files, 976 left and each class named: the verb `fn init` (correct
and untouched), records, quotations, captured transcripts, `fixtures/src/hello.rs`'s six role
constants (calef's, proposed with `PARENT_*` recommended and four refusals) and 471 in files a lane
may not edit. Three claims were wrong about more than the name. calef's refusal of a
`system_initializer` rename now sits in that crate's own `Name:` block.
