---
status: DECIDED
raised: 2026-10-26
decided: 2026-10-26
ratified_by: calef
---

# 311. A contract between programs is code, not prose

Ruled 2026-10-26 (UTC), tree-wide. It sets where the contract pile of
[§310 (read a note against its code)](0310-read-a-note-against-its-code.md) goes, and it
binds every other place the tree writes prose as well.

## The rule

An agreement between two programs is code that both sides use, so the compiler or a test holds
both sides to it. The forms that serve:

- a type, or a constant both sides import;
- an ordered sequence, or a function that encodes the order;
- a const assertion that ties two values together;
- one encoder and one decoder, shared by the side that writes and the side that reads.

Prose may say why the contract takes the shape it has. Prose is never the only statement of a
contract, and never a second copy of what the code says.

## Scope

Every place the tree writes prose: notes, rustdoc, comments and design text. A wire layout, a slot
number, a role word, a report word and an order two programs follow are all contracts.

## A contract held only in prose is a finding

A lane that finds a contract stated only in prose, or only implicit in one side's code, records a
finding. A contract crate's docs count as prose here: they do not make a contract code. The finding
goes to a proposed milestone for an architect, with a sketch of the code that would state it.

A reading lane does not write contract code itself, because contract code is new public surface.
It leaves the passage as it found it, so the agreement keeps its one statement until the code
lands.

## The evidence

The two pilots of §310 found the case this rule exists for.

- In `notes/system-initializer` (#2778), most of the narration cuts that the review had to restore
  turned out to be contracts. One was the spawn request's order, which `grant_plan::spawnproto`
  holds. The note was a second statement of it, and the reader of the note needed it to act.
- In `notes/e1000e` (#2782), one contract was code already: the role word's layout is
  `Handoff::pack` and `unpack`, so its prose copy went. The reading found 7 more between programs
  that no shared code states whole, among them `net_stack`'s spawn slots 0 to 6 and its report word.
  The kernel spells those slots as integer literals, and `net_stack` spells them as its own
  constants. Every receiver reads the report word as a lease, and only the sender names the
  `BAD_HANDOFF` sentinel, so a bad handoff reads as an address. That pull request lists each with
  a code sketch.

## How it relates to other sections

- [§267 (a comment states the constraint as it is
  now)](0267-a-comment-states-the-constraint-as-it-is-now.md) sends semantics, wire layouts
  and rulings to `design/decisions/`. For an agreement between programs, this section narrows
  that: the code states the layout, and a decision section holds the ruling and its reason.
  §267's own rule that "the caller must..." becomes an assertion or a type is the case inside
  one program; this section is the case between two.
- [§213 (writing standards)](0213-writing-standards.md)'s rule 4, state a finding once, applies
  across code and prose here: what the code states, prose does not state again.

## Enforcement

Review. Each reading under §310 asks of every contract passage whether code both sides use holds
it, and lists each one that is not as a proposed milestone with its sketch. No check can tell a
contract from orientation in prose, so this section proposes none.
