# The scoped block, and what `( ... )` became

*Milestone 52, [§160 (what a subshell copies)](../../design/decisions/0160-what-a-subshell-copies.md),
under [§312 (swish has one way to do each job)](../../design/decisions/0312-swish-has-one-way-to-do-each-job.md).
`crates/swish/src/scope.rs`, `components/src/swish/scope.rs`, `Nav::return_to`. Both spellings are
provisional and go to calef's naming pass.*

## What it is

`scope { commands }` runs its commands as a group and puts back, on the way out, the part of this
shell's own state they changed. It is the one form that groups and the one that scopes. It is a
prefix word like `time`, so its tail is the whole braced block, and the connectors inside belong to
the block:

```text
$ scope { cd logs ; ls } && echo done
```

The block's status is its last command's. A brace is syntax only as a word of its own (`scope { ls }`,
not `scope {ls}`), and a block takes nothing after its closing brace but a connector.

## What is put back

The state this shell holds as values: where it stands, and the names `bind` filed. It holds no
variables or options yet, so there is nothing else; when it does, they belong in the same save and
restore. Restoring happens on every way out, because a failing command is a status and not a jump.
The position is restored by opening it again from the root of its tree, one `OPENDIR` per level
(`Nav::return_to`), with the bound names set aside for the walk so a name cannot redirect a
component. If the block removed a level of its own starting place, the shell says so, stays where the
block left it, and the block answers with a failure.

What a command did to the world stays done: a file it made, a program it ran. A subshell never undid
those either.

## What it refuses

A block that would start, stop, wait for or resume a background job is refused before anything in it
runs (`Refusal::ScopeCannotRestore`). The shell holds a job as capabilities, a region and a thread,
that leaving a block cannot give back. The message names the isolation form, `swish -c`. The check
reads the block's text, nested blocks and the `time` and `xargs` wrappers included, so nothing runs
on the way to the refusal. Blocks nest three deep (`Refusal::ScopeTooDeep`), because each level is a
frame on the shell's few pages of stack.

## `( ... )`

It parses as nothing. A line with a command, or a pipeline stage, that opens with `(` is refused
whole before any of it runs (`Refusal::GroupingParentheses`), naming `scope { ... }` and `swish -c`.
It is refused at the line, not the segment, because cutting `(a; b)` at its connector would run `b)`
alone.

## BUGS

- Only a job change is refused. A command that changes the world is not, by design.
- The shell has no variables or options, so the block saves none.
- A scoped block is not a pipeline stage and cannot be piped to or from.
- Tab completion offers `scope` but not the first word after its brace.
