# Documentation sweep, 2026-10-19: decisions read forward

*Documentation audit, run 2026-10-19 (UTC) at base `84232e8c9` by the lane
`audit/2026-10-19-decisions-read-forward`. The procedure is
[notes/documentation-audit.md](../../notes/documentation-audit.md); the index is
[README.md](README.md). The file name is provisional, like every audit report's.*

**Kind:** documentation. **Lens:** decisions read forward: the ten sections minted since the last
sweep, §300 to §309, each read for what it ruled and what it closed, then the tree searched for
prose that still argues the closed side. **Findings:** fixed 11, minted 2, accepted 4.

## Why this lens

Three count triggers fired against the 2026-10-15 sweep: milestones built 418 to 466 (fires at
10), components 222 to 235 (fires at 10), ABI constants 95 to 97 (fires at 1). The calendar had
not. The two new constants are `ThreadControlBlock::SUSPEND` and `RESUME`, which §300 ruled, so
the event trigger already pointed at a decision.

Every documentation sweep since 2026-08-16 but two took its scope from the staleness worklist,
which ranks a document by how much of the code it cites has moved. A decision moves no code when it
lands. A note that calls a fork open, or a block that recommends the option a ruling declined,
ranks no higher on the day the ruling merges, so the worklist cannot see this class at all. It is
also the second uncountable trigger in `script/audits` ("has a decision superseded a plan that a
note still prescribes?"), which no sweep had taken as its lens.

Three readers took the ten sections in parallel (§300 to §302 with §309; §303, §304 and §308;
§305 to §307), each searching notes, roadmap blocks, tenets, the root documents and doc comments.
Every finding below was re-read against the tree before it was fixed or routed.

### The two uncountable triggers, answered

Has a decision superseded a plan a note still prescribes? Yes, in seven of the ten sections, and
it is this report.

Has a subsystem been rewritten inside its existing crate? Not in this scope. The window's crate
renames (the `_contract` suffix) surfaced only as stale names inside a decision (finding 13).

## What came back clean

`crates/abi`'s `SUSPEND = 4` and `RESUME = 5`, `notes/suspend-and-resume.md`, and
`kernel/src/sched/suspension.rs` agree with §300. §301's claims about the exit path
(`user_mode_runtime`'s `SYS_EXIT` with a zero word, `departure.rs`) are true. §302's wiring bits
and badges exist as named. §308's supervisor, its four messages, the `terminal` package's five
members and the 64-slot table match. §305's `deny.toml` rules (`allow-org` for `nifeos`, empty
`allow-git`) match its text, and no committed manifest takes a git dependency. §307 is not built:
`nifefs` and `package_archive` still use the fixed 32-byte name, so every comment and note that
says 32 bytes is true today and was left alone. §309's `hand_to_requester` sends `WRITE` without
`GRANT`, as ruled.

## Findings

### 1. FIXED: the suspend verb was still deferred in two places

`design/open-design-ideas.md` listed `SUSPEND`/`RESUME` as deferred, with build triggers. §300
ruled them, reading the entry's "one surface with the fault endpoint" as designing the pause in
§26's frame, and milestone 48 built them. The entry is marked decided, in the shape that file
already uses for superseded ideas. Milestone 103's block called a resumable pause §24's deferral;
it now names §300's methods.

### 2. FIXED: an exit status was said to need a spawn-protocol bit

Milestone 67's block, twice, and `crates/swish/src/status.rs` said a per-program status would take
a `spawnproto` bit, a delegation position and an edit to every program. §301 carries it on
`SYS_EXIT`'s first word and forwards it in the `EVENT_EXIT` message the parent already receives.
All three now say so and name milestone 172 as the build.

### 3. FIXED: the exit-status wire was called undecided

`notes/std/caveats.md`, `notes/std.md` and milestone 64's ranked list called carrying the code "a
wire-format change" still to be decided, and milestone 172's block called the wait's sizing its own
open work. §301 decided both: the status rides the message's zero word, with no new syscall. Each
now states the ruling and that the build is 172's.

### 4. FIXED: nobody owned the one kernel change §301 names

§301 chose its option partly for needing "one kernel touch on the exit path". Milestone 172's "What
it needs" listed only the userspace loop, so the documents fixed under finding 3, which now say 172
builds the status, pointed at a block that did not claim the kernel half. It now does:
`sched::exit` forwards the word, and `user_mode_runtime` passes it.

### 5. FIXED: color and the pager still waited on a "wiring bit"

`notes/documentation.md` (twice), `notes/documentation/where-next.md`, milestone 40's block and its
build record said color and the pager were an architect's call on a wiring bit. §302 declined the
bit: the screen fact is holding a `WRITE`-only terminal capability, and one line of input is a
one-shot badge served `READLINE` once. All five name the ruling and milestone 334.

### 6. FIXED: milestone 334 quoted its exhibit with words the exhibit does not hold

334's premise said `components/src/mdr.rs` carries "No pager, and the reason is authority rather
than effort". That sentence is in `notes/documentation.md`; `mdr.rs`'s module doc says "No pager"
and gives the reason in different words. The block now paraphrases what the file says.

### 7. FIXED: §303's rule was not where §303 put it, and milestone 356 still argued the fork

§303's second ruling places the region rule "in `Retention`'s and `build_child`'s docs". Neither
carried it. `build_child` now says `build_ut` must be budget the caller owns, that building from a
retained, delegated region is forbidden, and that nothing enforces it until milestone 356's check;
`Retention` points there. Milestone 356's body still presented the three shapes as an open fork
("may be the right one") and its index row called the question an architect's. Both now state
§303's choice. Its in-brief said the TCB is inert after `START`, which §300 made false; it now says
the capability authorizes only pausing.

### 8. FIXED: milestone 376 called the device-off authority question ruled

The block said "the authority question is ruled". §304 rules only that the measurement comes first
and leaves a driver-asks capability explicitly undecided, a cost and not a refusal. The sentence
now says the question waits for the number.

### 9. FIXED: the web's root count, and a trust store recorded as unminted

§306 measured Mozilla's list at 121 roots and named milestone 387 as the "roughly 150" it corrects.
Milestones 387 (three places) and 389 now say 121, or about 120 where the figure is Debian's. 387
also still recorded the trust store as "recorded rather than minted" for want of a consumer, and
argued a store "it could only update through itself"; §306 made it a package and a capability
updated over basalt's pinned channel, which `jig`'s own channel never depends on. The dependency
census row that quotes 389 regenerates from it.

### 10. FIXED: two notes cited §305 and §307 for what they no longer say

Milestone 857's fork 2 ruled path dependencies here and crates.io after, with git dependencies
refused. §305 takes a git `rev` on `nifeos/*` for the soak between; the fork now says so.
`notes/packages/container-format-measurements.md` defined its first candidate as "NIFEPKG2, as
§307's rule 3 defines it" and said §307's step 1 had not built it. §307's rule 3 now makes a package
§197's tar; the note names NIFEPKG2 as the layout §307 weighed, and its measurements stand as taken.

### 11. FIXED: `start_child`'s BUGS said no spawn site keeps a TCB

It said every spawn site declares `Retention::Nothing`. `system_initializer`'s supervised-job
spawn declares `Retention::Requester` (§309). The entry now limits its claim to
`Retention::ThreadControlBlock`, which still has no caller.

### 12. MINTED: two supervisors reap on an event code they do not know

§300's first rule is that unknown event codes are never acted on, and the section asserts every
consumer already dispatches on `EVENT_FAULT` and `EVENT_EXIT`. `components/src/job_undertaker.rs`
collects on any event, and `components/src/sub_server_supervisor.rs` reaps on any event and reads
every non-fault code as a clean exit. Harmless while the kernel sends two codes, and exactly the
reap of a paused child the rule exists to prevent once a third arrives. What an unknown code should
do per consumer, and whether the rule becomes one shared dispatch a consumer cannot bypass, is a
design choice and not a sweep's. Proposed:
[every supervisor dispatches on the event codes it knows](../roadmap/0938-every-supervisor-dispatches-on-known-event-codes.md).

### 13. MINTED: six statements in the window's decisions that a later commit moved

A lane does not edit `design/decisions/`. §303 says `Retention` has two variants and the TCB is
inert after `START` (§309 and §300 changed both). §308 sizes against a capability peak of 34 (it is
37). §307 cites the deleted `xtask/basalt-pins.index` and two crates by their pre-rename names. §306
points at an "open question 1" that milestone 809 does not have. §304 quotes a word from a crate
with the crate in British spelling and the quote in American. §303's and §304's frontmatter dates disagree with the blocks that cite
them by a day. None changes a ruling. Proposed for the integrator:
[decision text the window's own rulings overtook](../roadmap/0937-decision-text-the-window-overtook.md).

### 14. ACCEPTED: nano's stated reason for no `^Z`

Milestone 170 says `kilo` and nano are built without suspend "since there is no job control to
suspend into". Job control exists now, but milestone 48 suspends supervised jobs only, and a
foreign program's `SIGTSTP` is not delivered (`c_library`'s BUGS). The outcome stands and is already
a BUGS entry in the package; a rewrite of the reason would need the foreign-program path decided.

### 15. ACCEPTED: prose that says names are 32 bytes

`notes/adding-a-program.md` and several comments state the 32-byte name cap. It is the tree's cap
until §307's first step lands, and that lane is the one whose diff makes them false.

### 16. ACCEPTED: milestone 205 lists exit codes as open

It lists "environment variables and exit codes for a foreign program, which §170 does not rule".
§301 rules how a status travels; what a foreign program's code means on that wire, and its mapping
to POSIX, is not in §301, so the item is still open in the sense 205 means.

### 17. ACCEPTED: "only `line_editor` swaps live"

Milestone 849's versioning appendix cites §308 for it. §308 says it of the `terminal` package, and
`display_terminal` also swaps live outside an install. Read with its citation, the sentence is
§308's scope, and it is in a proposal's appendix rather than a page a reader would act on.

## What was deliberately not examined

- Decisions before §300, including the sections amended in the window. An amendment changes
  a ruling in place, which this lens reads only for the ten new sections.
- `design/decisions/` itself as a fix target, except where finding 13 routes it.
- The staleness worklist, whose top six the previous sweep took, and the ABI's doc comments beyond
  §300's two constants.
- Whether each decision is right. A sweep reads prose against rulings and code, not rulings against
  each other.

## What converts to a gate

One class, routed rather than built: finding 12's rule wants to become a type in
`supervision_protocol`, which the proposal asks for. The rest is not mechanically checkable
without false positives. A gate that flags "open" or "undecided" near a citation of a DECIDED
section would fire on every block that explains what a ruling closed. The tree's own check,
`script/roadmap`'s decision-dependency status, already catches a block gated on an undecided
section; nothing catches prose in a block whose gate cleared. That is a limit of the mechanism,
and the reason this lens is worth taking again after every batch of rulings.

## The records this report cites

- §24 (interrupting the foreground process: two-tier, shell-held, no new kernel surface)
- §26 (the fault endpoint: thread death becomes a message a supervisor holds)
- §170 (how a foreign program is told what to do)
- §197 (a package is one archive file, named and vouched for by its recipe)
- §300 (`ThreadControlBlock::SUSPEND` and `ThreadControlBlock::RESUME`: the paused state, a TCB verb, silent)
- §301 (how a spawned child's exit status and output reach the program that spawned it)
- §302 (the screen fact and one line of input: the spawn protocol's other two thirds)
- §303 (the retention declaration says nothing about the region)
- §304 (who may turn a device off, and what the unit of reclamation is)
- §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)
- §306 (the web's authority list is a package and a capability, and `jig`'s own channel never depends on it)
- §307 (how long a name may be: one bound for programs, packages and the files they travel in)
- §308 (the installer asks the terminal to swap: who holds the swap endpoint, and how a new image travels)
- §309 (a supervised job's `ThreadControlBlock` goes to the shell that asked for it)
- milestone 40 (documentation as a system service: searchable, rendered, and installed by packages)
- milestone 48 (job control: `jobs`, `wait`, `kill`, `fg`, `bg`, and a stopped state)
- milestone 64 (enough `std` to run somebody else's crate)
- milestone 67 (`swish` the language: quoting, sequencing, and exit status)
- milestone 103 (`^C` stops spinning: the shell's interrupt watch, blocking)
- milestone 170 (`nano`: a real, full-featured screen editor)
- milestone 172 (a capability-native subprocess primitive: what `cargo`'s "spawn a helper, wait, collect its output" needs, without fork/exec)
- milestone 205 (how a foreign program is told what to do)
- milestone 334 (color and the pager: the spawn protocol's other two thirds)
- milestone 356 (retention declares the thread capability and says nothing about the region)
- milestone 376 (nothing turns a device back off)
- milestone 387 (a TLS stack, and the fork is not "OpenSSL or not" but what we are trying to prove)
- milestone 809 (the package client becomes a program, `jig`, with the verbs an index needs)
- milestone 849 (a package declares what it needs at run time, apart from what it links)
- milestone 857 (crates released on their own, and proofs offered to the projects that already exist)
