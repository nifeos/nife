---
status: NOT-STARTED
raised: 2026-09-19
promoted_from: a-program-that-takes-an-argument-and-an-input
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 498. Whether a program may take an argument and an input stream

*(Number provisional until the merge queue lands it.)* Promoted from the
proposal `a-program-that-takes-an-argument-and-an-input`, filed 2026-09-19, on calef's instruction
of 2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own,
unedited except for this paragraph: the argument is its author's and promotion is not the moment to
improve it. Raised by milestone 150 (eight hand-maintained lists), item 3 of its design questions.
That lane was told to keep current behavior and write this up rather than decide it in code,
because it is a policy about what programs may be.

Nothing is blocked on it. Milestone 150 closed the mechanical half, so either answer
is cheap to carry out.

## In brief

The question: may a program's manifest declare `ProgramArgumentDeclaration::Required` together with
`InputDeclaration::Required`, so that `nth 21 report.txt` runs `nth` with the integer 21 and `report.txt`
streamed into it?

**Today the tree allows it and nothing uses it.** The planner composes the two by fixed position
(`an_argument_and_an_input_stream_compose_by_the_same_fixed_order` in `crates/grant_plan/src/lib.rs`
plans exactly that line and gets a clean grant). The one thing that used to break was a host test in
`crates/swish` that typed `<name> 21` for every program and so went red on this manifest shape.
Milestone 150 taught that sweep to supply every operand the manifest asks for, so **adding such a
program no longer needs an edit anywhere outside its own declaration**.

What is left is whether the combination is wanted, and that is the call this asks for.

## Why it is a question at all

Milestone 117's fifth stranger picked the combination deliberately, as the one manifest shape nothing
had used, and hit the swish sweep. `notes/adding-a-program.md` then recorded it as open because the
planner's comment ruled out the neighboring case, a file together with an input, and said
nothing about this one. The 2026-08-22 correction established by test that the two are not
analogous: `FileGrantDeclaration` and `InputDeclaration` both take a bare name, so a manifest with both would leave the
parser two indistinguishable positions. An argument is numeric-shaped and claims position 0
before the input's bare-name fallback looks at what remains.

## Options

1. Allow it, as today. Nothing to build. A program like `head 5 report.txt` or `nth 21
   report.txt` is one declaration. The cost is that the positional grammar grows a third shape
   (`arg`, `arg file`, `arg input`) before milestone 47's deferred positional-arity work has decided
   what the general grammar is, so that work inherits one more case to stay compatible with.
2. Refuse it at declaration, the way the tree treats file-plus-input. A host test in
   `grant_plan` (about twenty lines, beside the one milestone 150 added for file-plus-input) fails on
   any manifest declaring both, with the reason written where the next person meets it. The planner
   code that composes them stays, unreached. Reversal later is deleting the test.
3. Defer to milestone 47's positional arity, recording that the combination waits on that
   widening. In practice this is option 2 with a pointer, since nothing would be refused or allowed
   differently until 47 lands.

## Recommendation

Option 1, because it is reversible and it is already true: no program declares the combination,
so refusing it later costs nothing and nobody has acted on its being allowed. It is also the shape
Unix users will reach for first (`head -n 5 file` is the everyday case), and the planner already gets
it right without special-casing.

Asked the tenet's question, *would we still choose this if both options cost the same?* Yes: the two
options cost about the same (one test either way), and the recommendation rests on the grammar
already composing cleanly rather than on effort.

## If calef says no

Option 2 is one host test plus rewording the planner comment at the input operand in
`plan_against_with` from "unclaimed headroom" to "refused, and why". No shipped program changes, no
wire format moves, and `notes/adding-a-program.md`'s manifest section gains one sentence.

## What this is not

It is not the file-plus-input question. That combination has a real ambiguity, stays unsupported,
and since milestone 150 is checked by `no_program_declares_both_a_file_and_an_input` rather than
by a comment.

## Index row

The question: may a program's manifest declare `ProgramArgumentDeclaration::Required` together with
`InputDeclaration::Required`, so that `nth 21 report.txt` runs `nth` with the integer 21 and `report.txt`
streamed into it?
