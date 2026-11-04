# Documentation sweep, 2026-09-29: names and numbers a reader would act on, in the notes the window moved

*Documentation audit, run by an audit lane on 2026-09-29 at base `59a43eacc`. The procedure is
[notes/documentation-audit.md](../../notes/documentation-audit.md); the index is
[README.md](README.md). The file name is provisional, like every audit report's.*

**Kind:** documentation. **Lens:** the staleness worklist's top three, read for claims a reader
would act on, plus the inbound side of the six notes this window split, plus the window's two new
notes read against the kernel that landed beside them. **Findings:** fixed 6, minted 1, accepted 1.

The sweep found six stale claims and corrected every one. The seventh finding is a fence inventory
whose count rotted three ways at once. The eighth is the reason five of the corrections nearly
could not land: the prose ratchet's bold touch rule prices a one-line correction and a full
de-bold rewrite identically, and 864 documents stand on the wrong side of that price.

## Why this lens

`script/audits` fired every count trigger for this kind: milestones 252 to 276 (fires at 10),
components 169 to 184 (fires at 10), ABI constants 56 to 68 (fires at 1). The calendar had not
fired, which is the counts doing their job.

The worklist ranked `notes/shared-page-audit.md` first (23 of 25 cited files moved, 78 commits
since its last edit). Second came `notes/architecture-list-sweep.md` (22 of 26, 103 commits) and
third `notes/memory-ordering.md` (16 of 19, 118 commits). The window argues the same three. It
rewrote two subsystems inside their crates, which no count sees, and the worklist caught both.
`kernel/src/sched.rs` took +1,112 changed lines absorbing notifications and timers.
`kernel/src/user.rs` lost 1,168 lines to 476 new ones when milestone 609 (the system tests leave
the kernel crate) moved the suite to `system_tests/`.

The window also split six notes into main pages and appendices under §212 (a prose budget: 3,000 words of
main body): `net`, `stack`, `std`, `pipes`, `x86-port` and `model-comparison`, 48 new appendix
files and +6,621 lines. The worklist is blind to a moved document, its own recorded BUGS says, so
the splits' inbound pointers were swept by hand, the method the 2026-09-24 sweep left behind. The
window's readers also meet two brand-new notes, `notes/notification-objects.md` and
`notes/timer.md`, and both were read against the code, not the lane that wrote them.

### The two uncountable triggers, answered

Has a subsystem been rewritten inside its existing crate? Yes, the two above, and the worklist
found both, which is the answer its design hopes for.

Has a decision superseded a plan a note still prescribes? The window's amendments, §101
(notification objects: async multiplexing without wait-any) twice on 2026-09-26 and §147 (a timer a
userspace service cannot hold) on 2026-09-27, were checked against their notes, which record them.
One stale prescription was found, and it is finding 3.

## What came back clean

The six splits' inbound side, otherwise. Every quoted section a citation names still stands in the
main page it names: "The inbound half" (`notes/net.md`), "What still ends a nife process"
(`notes/std.md`, cited from three files), "What had to change above `arch/`"
(`notes/x86-port.md`), "One wait point" and "Buffering: measured" (both `notes/pipes.md`). No anchored link anywhere points into the six. The split lanes swept their
own inbound sides, which is what the 2026-09-24 sweep asked the next split to do.

`notes/timer.md` checked out on every number: the tick is 100 Hz (`TICK_HZ` in `kernel/src/arch/aarch64/timer.rs`),
`counter_ticks_for` rounds up (in `crates/abi/src/timer.rs`), `CANCEL` answers a `bool`, and
`SYS_EXIT` exists (`dispatch` in `kernel/src/syscall.rs`). `notes/notification-objects.md` semantics matched
the kernel at every claim this sweep checked, which is also why the security audit of the same
session accepted its recordings.

## Findings

### 1. FIXED: `icount`'s x86_64 refusal named a reason that stopped being true

The comment in `icount` in `xtask/src/icount.rs` said the instrument's boot needs a userspace the port cannot build.
Milestone 161 (the x86-64 kernel port) built that userspace; the real reason, recorded at
`bench_x86`'s doc, is the LAPIC timer's periodic reload. The sweep note
(its row in `notes/architecture-list-sweep.md`, line 216) asked for this one-line correction on
2026-09-23. Two
commits have since touched the file, and nobody made it. The comment names the real reason again.

### 2. FIXED: a call-site count that never matched the tree

`notes/notification-objects.md`, under "Two checks the maintainer asked for (2026-09-26)", said
`bind_irq` has fifteen call sites (the same paragraph now carries the correction's date). Counted at the note's
own creation commit and again today: thirteen wiring sites plus the soak's loop. The count is now
the true one, with the correction dated beside it.

### 3. FIXED: the shared-page audit said the frame race was unreachable, and the 205 prompt opened it

`notes/shared-page-audit.md` finding 1(d) said "Reachable? Not today", on the argument that the
shell blocks on the child for the child's whole life. Milestone 205 (how a foreign program is told
what to do) put a nameset caretaker on that frame at a prompt that drains redirected jobs. Its
own block recorded that this note was left alone because a correction trips the prose ratchet. The correction has landed, beside a marked writing-standards exception, so the
security-relevant claim no longer waits on a rewrite nobody scheduled. The false claim was the
security audit's premise check, and that audit verifies the race itself.

### 4. FIXED: a recorded-gap row closed a month before the note knew

The recorded-gaps table of `notes/architecture-list-sweep.md` (row at line 215) said `swish_check`
refuses `--arch x86_64`, scoped under milestone 177 (wire the graphical terminal stack into the real
interactive boot). The third leg runs (`swish_check::swish_check`'s `--arch` match, milestone 182
(x86-64's own interactive-boot entry point)), and 177 has been BUILT since 2026-09-19. The row now
closes with the strikethrough its own table uses, beside a marked exception.

### 5. FIXED: a citation quoted a section that had become an appendix

`design/roadmap/0124-a-thread-is-born-where-it-lives.md`, under "Closed: the stack was freed under
its owner (2026-08-17)" (now `design/roadmap/0124-a-thread-is-born-where-it-lives/reopened-and-closed.md`), quoted "a kernel stack freed under its
owner" as a section of `notes/stack.md`. The 2026-09-24 split moved it to
`notes/stack/kernel-stack-freed-under-its-owner.md`, and the quote stopped resolving. The pointer
names the appendix, beside a marked exception.

### 6. FIXED: a quotation credited to a file that never held it

`notes/sink-protocol.md`'s BUGS list for the terminal's sink adapter quoted "built last" as
something `notes/pipes.md` wrote down. The
phrase was born in this note itself (commit `8c279536f`, 2026-08-03) and appears in no file today.
It is credited where it lives, and the boot-order pointer names the appendix the split made.

### 7. MINTED: the fence inventory rotted three counts at once

`notes/memory-ordering.md` says thirteen fences under "The count, and how it was taken". It says
"all fourteen" in a heading over a fifteen-row table, and "Twelve sites today" under "Can pairing
be checked mechanically? Measured, not argued". `script/lint` counts 19 today. (Repointed
2026-10-11 UTC from line numbers, which #1937's split moves: it puts the first and third in
appendices with those titles.) Behind the
numbers, the fences this window's milestones added are adjudicated nowhere a reader would meet, and
an inventory whose table no longer names the population is worse than no inventory. Proposed as a
milestone, its number to be minted at merge: re-derive the count, adjudicate the sites the window
added (the clock writer, `drain_input`'s two acquires, and whatever the nineteen includes), and
correct the three stale numbers. Too large for this lane because the correction without the
adjudication would misrepresent the table as complete.

### 8. ACCEPTED: a correction pays the bold toll, and the toll was never measured until now

The touch rule (calef's 2026-09-26 ruling) asks 4 bold spans per 1,000 words of any document a
change touches. Findings 3 to 6 each met that bill at 25 to 82 spans to remove, and each paid with
a marked writing-standards exception, the line `notes/timed-wait.md` set on 2026-09-26. Measured
for the class: 864 documents stand over the density, 634 of them in this sweep's own scope,
including `notes/documentation-audit.md` itself, which cannot record a lesson without the same
marker. The bite on a document being condensed is the ruling's intent. The identical price on a
correction of a false claim is a consequence nobody had measured, and it is now recorded at the
feature, `design/roadmap/0586-a-prose-ratchet-in-lint.md`, as a design note. Whether corrections get
a carve-out, or the bold backlog gets a lane that retires these markers, is calef's call, and the
note says so.

## What was deliberately not examined

- The worklist below its top three. The 502 documents beneath them cite moved code, and reading
  them is the next sweep's scope, not this one's. The procedure's own rule: four to six documents,
  read.
- The six splits' content against their pre-split notes. Each split lane checked that itself; this
  sweep checked reachability, which is the 2026-09-24 audit's precedent.
- `design/decisions/`. Out of a lane's edits, and excluded from the worklist for that reason.
- The ABI's doc comments. The 2026-08-17 sweep's lens; the window's new surface prose was read by
  the security audit running in this same session, against the kernel.
- Counted-claim markers and structural link rot: gated, and a sweep reporting them reports the
  gates' output.

## What wants a lane of its own

- Finding 7's adjudication, the proposed milestone.
- The bold backlog of finding 8: 634 documents in this sweep's scope that cannot be corrected
  without a de-bold pass, and five markers now carrying dated debt for exactly that.
- The worklist's remainder: 502 documents with moved code under them, ranked and waiting.

## Process notes on the mechanism itself

The worklist's recorded blindness to split documents held a second time: none of the six is in its
top fifty, because each was edited the day it split. The inbound sweep by hand is now twice
proven, and it stays with whoever splits the next note. The environment-names gate exempts this
directory, so every stale name quoted in this report was checked by hand instead.
