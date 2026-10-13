# Things this project has already gotten wrong

**Provisional name** (`notes/corrections.md`): calef names what a reader meets, and this page's
name is not ratified.

Kept on purpose, because the corrections were the most instructive part. This tree's habit is to
fix the record loudly rather than quietly: `AGENTS.md` says the machine overrules the documentation
and it overrules you, and when it does, the record gets fixed on purpose. These are the cases where
that happened, in the order they were written.

Each entry names the note that carries the full account. This page is the index of scars, not a
replacement for them.

**QEMU does not hand an ELF a device tree pointer in `x0`.** It only does that under the
Linux boot protocol, which it selects for flat arm64 `Image` files. We shipped an ELF, so it
took the bare-metal path and populated no registers. We found out by printing `x0` and
getting zero. *Since fixed*: we now emit a flat binary with a 64-byte Image header, and two
tests hold the line. See [boot-protocol.md](boot-protocol.md).

**`bl` does not push a return address onto the stack.** That's x86. On aarch64 the return
address goes into register `x30`, and the stack is where it gets *parked* when a function
needs `x30` for a call of its own. See [stack.md](stack.md).

`into_iter()` on a big array is a kernel footgun. Milestone 3 (hand out physical memory, and
detect a smashed stack) hung the machine for
150 seconds with no output. `[Option<Frame>; 1024].into_iter().flatten()` moves 16 KiB by
value, twice, onto a 64 KiB stack; `sp` walked through `.bss` and `.data` into `.text` and
the kernel executed its own overwritten code. Two of the three diagnoses along the way were
wrong. The write-up of *how it was actually found* (semihosting exit codes as bisection
markers, because `println!` runs through the `.text` you just corrupted) is the most useful
thing in [stack.md](stack.md).

## Corrections of error

§210 (a correction of error, and its action items are decisions, proposals or milestones) rules that
this tree writes COEs: what happened, a timeline with times, the impact, a root cause reached by five
whys, and action items that resolve to a decision, a proposal or a milestone. The entries above are
scars, which is a record that something healed. These are records of what was changed so it would not
happen again.

[The weekly falsification sweep replayed nothing and reported success](corrections/2026-09-23-the-sweep-that-swept-nothing.md)
(2026-09-23). A `tee` into the checkout dirtied the tree before the sweep's own dirty-tree guard read
it, and `continue-on-error` swallowed the refusal, so three scheduled runs published the refusal as
their report and went green. The fifth why reaches the habit rather than the `tee`: a check that
examined nothing looks exactly like a check that found nothing, and this was the fourth of six instances of
that shape found in one day, one of them being the workflow that was supposed to label this very
record for calef.

## Why this page exists rather than a tidy repository

A newcomer who hits a limitation the documentation named will trust the documentation. One who hits
a limitation the documentation hid will not trust anything again, and there is no relationship to
fall back on. That is `AGENTS.md`'s third principle, and this page is what it looks like applied to
the project's own history rather than to its features.
