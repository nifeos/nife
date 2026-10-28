---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-28
ratified_by: calef
---

# 189. Which of two definitions `caretaker` carries, and what the translating shape is called

Ruled 2026-10-28 (UTC): B, `caretaker` means translating: taking care of something, not preserving
the same protocol; the 2026-08-03 ratification of `terminal_sink_caretaker` stands. `file_sink`
→ `file_sink_caretaker`; calef accepts the `fs_file_caretaker` collision as the cost of consistency.
Caretaker names say what they hold, then what they hand out. Unblocks milestone 413 and `file_sink`'s
ratification.

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote
down)'s slice-c lane. That lane found milestone 413 (what a `caretaker` is, when the thing it holds
is not the thing it hands out)'s `DECISION` gate naming no section. Filed 2026-09-14 by the
milestone 292 (`fixtures/src/sink.rs` is three programs) lane while naming the program that puts a
file behind a byte sink. That program had been `ROLE_FILE` inside a three-role binary and so had
never needed a name of its own. *(Section number provisional until the merge queue lands it.)*

## What is being decided

Three things, in order, because the second and third follow from the first:

1. Which definition `caretaker` carries, narrowing or translating.
2. Whether `file_sink` should be `file_sink_caretaker`, and whether the `fs_file_caretaker`
   collision is tolerable if it should.
3. Whether a caretaker's name says what it holds or what it hands out.

## Is the premise true

Checked 2026-09-19 in this worktree. The tree carries both definitions, in four files:
`components/src/fs_file_caretaker.rs`, `fs_subtree_caretaker.rs` and `fs_nameset_caretaker.rs`
narrow; `components/src/terminal_sink_caretaker.rs` translates.

And `script/names file_sink` still routes the inconsistency here, in its own closing sentence:
*"That the tree spells one translating adapter `terminal_sink_caretaker` and this one not is a real
inconsistency in what `caretaker` means. It is calef's to settle rather than this file's, and it has
a home: milestone 413."*

## The two definitions, both load-bearing

A, narrowing. A caretaker serves *the same protocol its client speaks*, and what it takes away
is authority rather than vocabulary. `fixtures/src/sink.rs` stated it in as many words before
milestone 292 removed the file:

> The `fs_file_caretaker` shape, one contract further out. `fs_file_caretaker` is a caretaker
> because it "serves the same `filesystem_protocol` protocol its own client speaks"; this one serves
> a *different* and much smaller protocol than it speaks, and that asymmetry is the point.

B, translating. `terminal_sink_caretaker` holds a terminal endpoint that carries `OP_READLINE`
and hands out a byte sink that cannot read. It speaks `line_editor::proto` on one side and
`byte_sink_protocol` on the other, which is definition A's explicit counter-example, and
`kernel/src/user/sink_tests.rs` says so while calling it a caretaker.

The asymmetry that makes this calef's rather than a reading: calef ratified
`terminal_sink_caretaker` over `terminal_sink` on 2026-08-03, so definition B has a ruling behind
it and definition A has only prose. The prose is the more precise of the two.

## What this tree already does in the analogous case

Nothing here has decided the word, and the three sections that touch it decide other things,
which is worth saying so a reader does not stop at one of them:

- §92 decides a caretaker's *lifetime* (it is supervised by the client it serves, so §40's
  subtree death collects it). Silent on what the word means.
- §56 decides that the filesystem contract describes its own verbs, so a caretaker's dispatch is
  a table lookup and a caretaker is written once. That is definition A's world and assumes it rather
  than choosing it.
- §106 (take the terminal_sink_caretaker narrowing) takes the `terminal_sink_caretaker` narrowing as a behavior, not as a name.
- `design/naming.md` carries `terminal_sink_caretaker` in its refusals table, recording that
  *"`sink` names what it hands out, `caretaker` names what it is"*, which is an answer to question 3
  for that one name rather than a rule.

**And `adapter` is the word the tree already reaches for when it is not naming a file.** Both
`kernel/src/user/sink_tests.rs` and `notes/sink-protocol.md` use it in prose for the translating
shape. It is available: nothing in `components/` or `crates/` is named for it.

## Why milestone 292 could not settle it

The lane needed a name for the file-behind-a-sink program, and `file_sink_caretaker` was the
consistency answer: the same adapter as `terminal_sink_caretaker`, for a different backend. It was
refused on collision, because `fs_file_caretaker` already exists and is a different thing, and two
names a reader must hold apart cost more than the consistency buys. That is a local answer to a
global question, which is why it left this behind rather than closing it. The program shipped as
`file_sink`, provisional.

## What each answer costs

| | what changes | cost |
|---|---|---|
| **A wins** (narrowing) | `terminal_sink_caretaker` is misnamed and a second word is owed for the translating shape | one ratified name is overturned, which is the more expensive direction; `adapter` is available and already used in prose for exactly this |
| **B wins** (translating) | `file_sink` becomes `file_sink_caretaker` and the `fs_file_caretaker` collision is a cost accepted | the ratified name stands; two names a reader must hold apart, which is what 292 refused |
| **Both** (the word covers holding-and-handing-out generally, narrowing or not) | nothing renames | the word stops predicting anything, which is the failure a name is supposed to prevent |

## Recommendation

None on question 1, and deliberately: it is a name, one of the two answers overturns a
ratification calef made himself, and AGENTS.md's rule is that an irreversible fork arrives with
options rather than a winner.

What this section does recommend is answering question 3 explicitly whichever way 1 goes. That is
because it is the cheap half and it is what makes the next adapter's name mechanical rather than
copied from whichever neighbor the lane happened to read. `terminal_sink_caretaker` says what it
holds and then what it hands out, in that order, and reads well; `fs_file_caretaker` says only what
it holds. Those two are consistent only by accident today.

## How reversible, and who has acted on it

Low, and the surface is already five programs wide. `caretaker` is a word four programs carry,
`file_sink` is a fifth waiting on the answer, and every future narrowing or translating program will
reach for it. It is cheap to settle and expensive to leave: the next lane writing an adapter will
copy whichever neighbor it happened to read, and the tree will have three definitions instead of
two.

## What this does not decide

Anything about `byte_sink_protocol`, which is a wire contract named for what it carries and survived
the 2026-09-13 structural-versus-current sweep on its own terms. Nor `fs_subtree_caretaker` and
`fs_nameset_caretaker`, which are definition A under either answer.

## What is blocked until this is answered

Milestone 413, and `file_sink`'s ratification, which `script/names` reports as provisional and
routes here.
