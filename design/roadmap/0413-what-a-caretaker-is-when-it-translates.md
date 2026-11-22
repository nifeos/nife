---
status: NOT-STARTED
raised: 2026-09-14
promoted_from: what-a-caretaker-is-when-it-translates
milestone_dependencies: none
decision_dependencies: 189
machine_requirements: none
specific_machine: none
needs_person: no
---
# 413. What a `caretaker` is, when the thing it holds is not the thing it hands out

Promoted from the proposal `what-a-caretaker-is-when-it-translates`, filed
2026-09-14 by the milestone 292 (three programs wearing one name) lane while naming the program that puts a file behind a byte sink.
That program had been `ROLE_FILE` inside a three-role binary and so had never needed a name of its own.
*(Number provisional until the merge queue lands it.)*

The decision is
§189 (which of two definitions `caretaker` carries, and what the translating shape is called), written
up 2026-09-19 by milestone 435's slice-c lane because this gate named no section. That section also
records why the three sections a reader might stop at do not answer this one. §92 (a caretaker is
supervised by the client it serves) decides a
caretaker's *lifetime*, §56 assumes definition A rather than choosing it, and §106 takes the
`terminal_sink_caretaker` narrowing as a behavior rather than as a name.
`caretaker` is a word five programs already carry and every future narrowing
program will reach for, so this is a name and names are an architect's. It is cheap to settle and
expensive to leave, because the next lane writing an adapter will copy whichever neighbor it
happened to read.

Premise re-checked 2026-09-19 and still true. The tree still carries both definitions:
`fs_file_caretaker`, `fs_subtree_caretaker` and `fs_nameset_caretaker` narrow, and
`terminal_sink_caretaker` translates. `script/names file_sink` still reports the name as
provisional, still records `file_sink_caretaker` as refused on collision, and its own closing
sentence still routes the inconsistency here. Nothing has been ratified since.

## The two definitions, both in the tree, both load-bearing

Definition A, narrowing. A caretaker serves *the same protocol its client speaks*, and what it
takes away is authority rather than vocabulary. `fixtures/src/sink.rs` stated it in as many words
before milestone 292 removed the file:

> The `fs_file_caretaker` shape, one contract further out. `fs_file_caretaker` is a caretaker
> because it "serves the same `file_system_contract` protocol its own client speaks"; this one serves a
> *different* and much smaller protocol than it speaks, and that asymmetry is the point.

`fs_file_caretaker`, `fs_subtree_caretaker` and `fs_nameset_caretaker` are all definition A. So is
`fs_file_caretaker`'s own header, which reads the word that way.

Definition B, translating. `components/src/terminal_sink_caretaker.rs` holds a terminal endpoint
that carries `OPERATION_READLINE` and hands out a byte sink that cannot read. It speaks
`line_editor::proto` on one side and `byte_sink_contract` on the other, which is definition A's
explicit counter-example, and `kernel/src/user/sink_tests.rs` says so while calling it a caretaker:

> the terminal's sink is a separate endpoint served by an adapter, which is
> `fs_file_caretaker`'s shape

calef ratified `terminal_sink_caretaker` over `terminal_sink` on 2026-08-03, so definition B has a
ruling behind it and definition A has only prose. The prose is more precise.

## Why milestone 292 could not settle it

The lane needed a name for the file-behind-a-sink program, and `file_sink_caretaker` was the
consistency answer: the same adapter as `terminal_sink_caretaker`, for a different backend. It was
refused on collision instead, because `fs_file_caretaker` exists and is a different thing,
and two names a reader must hold apart cost more than the consistency buys. That is a local answer
to a global question, which is why it leaves this behind rather than closing it.

The program shipped as `file_sink` (provisional). If definition B is the ruling, `file_sink_caretaker`
becomes the honest name and the collision is a cost to accept rather than a reason.

## What a decision needs to settle

1. **Which definition `caretaker` carries.** If it is A (narrowing, same protocol), then
   `terminal_sink_caretaker` is misnamed and there is a second word owed for the translating shape.
   `adapter` is the word both `sink_tests` and notes/sink-protocol.md already use in prose, and it
   is the word this tree reaches for when it is not naming a file.
2. **Whether `file_sink` should be `file_sink_caretaker`**, which follows from 1, and whether the
   `fs_file_caretaker` collision is tolerable if it does.
3. **Whether `caretaker` should say what it holds or what it hands out.** `terminal_sink_caretaker`
   says both, in that order, and reads well; `fs_file_caretaker` says only what it holds.

## What it does not decide

Anything about `byte_sink_contract`, which is a wire contract named for what it carries and survived the
2026-09-13 structural-versus-current sweep on its own terms. Nor about `fs_subtree_caretaker` and
`fs_nameset_caretaker`, which are definition A under either answer.

## Index row

`caretaker` is spelled two ways in this tree and both are load-bearing. Under the narrowing
definition a caretaker serves the same protocol its client speaks and takes away authority rather
than vocabulary, which is `fs_file_caretaker`, `fs_subtree_caretaker` and `fs_nameset_caretaker`.
Under the translating one it speaks one protocol and hands out another, which is
`terminal_sink_caretaker`, a name calef ratified on 2026-08-03. Milestone 292 needed the word for
the file-behind-a-sink program, refused `file_sink_caretaker` on collision with the existing and
different `fs_file_caretaker`, and shipped `file_sink` provisionally, which is a local answer to a
global question. What a decision settles is which definition the word carries, whether there is a
second word owed for the translating shape, and whether a caretaker's name says what it holds or
what it hands out.
