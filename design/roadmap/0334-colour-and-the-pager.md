---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: 302
machine_requirements: none
specific_machine: none
needs_person: no
---
# 334. Color and the pager: the spawn protocol's other two thirds

Premise re-checked 2026-09-19 and it holds. DECISIONS §106 is still the only narrowing that shipped, `components/src/mdr.rs`'s
module doc still says "No pager" for want of one line of input without the keyboard, and nothing
in the spawn wiring tells a stage it ends at a real screen. No program in this tree colors its
output.

Both halves widen a protocol two programs agree on, which is the same shape as
[DECISIONS §106 (the terminal_sink_caretaker narrowing)](../decisions/0106-tail-output-narrowing.md)
itself and the same reason §106 was calef's. The fork is decided:
[§302 (the screen fact, and one line of input)](../decisions/0302-the-screen-fact-and-one-line-of-input.md),
2026-10-15. It rules the screen fact is the presence of a `WRITE`-only terminal capability, and one
line of input is a one-shot badge served `OPERATION_READLINE` exactly once. Both commitments sit
on wire two programs agree on, so they cannot be un-shipped once a program is written against
them.

**In brief.** DECISIONS §106 narrowed the spawn protocol for a tail stage's *primary output*, which
is what lets `doc <page>` render at the prompt with no `| wc` in front of it. It built the narrowest
slice that unblocked that one command. The two untaken thirds of the original scope are decided by
[§302 (the screen fact, and one line of input)](../decisions/0302-the-screen-fact-and-one-line-of-input.md).
The screen fact is the presence of a `WRITE`-only terminal capability: the honest replacement for
`isatty`, and what color needs. One line of *input* without the keyboard is a one-shot badge, what
a pager needs. This milestone builds both.

## Why this matters

The first half is what stops this system growing a dishonest `isatty`. Unix programs decide about
color by asking the kernel what their file descriptor is attached to, which is ambient authority
answering a question about presentation. A capability system says it properly, as the presence of
a `WRITE`-only terminal capability ([§302 (the screen fact, and one line of
input)](../decisions/0302-the-screen-fact-and-one-line-of-input.md)). Every program that wants
to color output will need that grant. Until it ships,
the choices are no color anywhere or a program guessing, and a guess here becomes a convention
before anybody decides it is one.

The second half blocks the pager outright. A pager needs one line of input at a time and must not
hold the keyboard, because holding the keyboard is exactly the authority a confined viewer should
not have. There is no way to express that today, so `mdr` renders and cannot page. Milestone 40's
own refusal of `ratatui` said the tree needs its terminal contract first; this is a piece of that
contract.

Left alone, this is a decision nobody meets until they try to write the sixth program that wants a
screen, and by then there will be a habit rather than a protocol.

## Index row

DECISIONS §106 narrowed the spawn protocol for a tail stage's primary output, the narrowest slice
that let `doc <page>` render at the prompt. The two untaken thirds are decided by
[§302 (the screen fact, and one line of input)](../decisions/0302-the-screen-fact-and-one-line-of-input.md).
The screen fact is the presence of a `WRITE`-only terminal capability, the capability-shaped
replacement for `isatty`, and it is what color needs. One line of input without the keyboard is a
one-shot badge served `OPERATION_READLINE` exactly once, and it is what a pager needs. Both widen a
protocol two programs agree on, so an architect ruled both.
