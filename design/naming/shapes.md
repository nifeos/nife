# Shapes

*An appendix to [`design/naming.md`](../naming.md). It holds the shapes a name takes, whatever its
words.*

## Nouns, not verbs

A crate, a program or a module is a thing, so it takes a thing's name. The call site shows why:
`line_edit::expand_output` reads as an instruction, and `line_editor::expand_output` reads as a
location. A term of art that happens to be a verb keeps its word: `bind` is Plan 9's, per
[§50 (namespace composition)](../decisions/0050-namespace-composition.md).

## Where the standard library has a name, use it

A method takes Rust's spelling and matches its meaning: `len`, `iter`, `iter_mut`, `get`,
`get_mut`, `values`, `contains`, `is_empty`, `peek`, `with`, and a `_mut` suffix for a method that
returns `&mut`. A reader who knows std then knows what the method does. A question takes the shapes
in [functions-that-answer-yes-or-no.md](functions-that-answer-yes-or-no.md).

## The rules cover parameters

The naming rules apply to a function's parameters as well as to its public items:
`data_physical_address`, and `blocks_per_<what>` for a count that names what it divides.

## Name the property, not the count

`MountedTree`, not `SecondDir`. An ordinal turns false the day the design allows a third.

## Name the hazard until the fix exists

`IMAGE_PLACEHOLDER_ROW`, not `IMAGE_ROW`. While a value stands in for a real one, the name says so,
and the name changes when the real value lands.

## A field that is an array of booleans takes the predicate form

`is_directory[i]`, not `directory[i]`. Each element answers a question.

## Names that follow a renamed thing move with it

`SecondDirGrant` follows `SecondDir`, and the `Flags` family takes `subtree_flag` along. A rename
that stops at a crate boundary leaves a stale name in the next crate.
