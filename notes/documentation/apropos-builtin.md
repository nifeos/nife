# The guest searches with `apropos`, and it is a builtin

*Name: provisional.*

An appendix to [`notes/documentation.md`](../documentation.md), which states the conclusion.

Phase 2's other half, and the decision in it is *where the search runs* rather than how.

A builtin, not a program, which is exactly the argument `ls` already carries in this shell: a
listing program would have to hold the power to read everything it lists. Search is an enumeration,
so a searching program would have to be handed a capability to the whole documentation store in
order to read every shard in it. That is a new principal holding more than the answer needs, for a
command that moves no authority whatsoever. The shell already holds enumeration over what it can
see, so `apropos` is the shell reading a file it could already read.

What comes back is names, never capabilities. A result is a store location a person can type:

```text
$ apropos capability
    32  doc/swish/pipes.md            Pipes and redirection: `>`, `<` and `|` are one
    11  doc/kernel/ipc-naming.md      Who does IPC name?
$ wc doc/kernel/ipc-naming.md
  163 1556 9503
```

(Those three numbers move whenever that note is edited. What does not move is that the name the
search printed resolved, which is the claim.)

The second line is where a capability moves, and it moves because a person typed a name. So search
cannot widen what its caller could already reach, and `mdr notes/ipc-naming.md` granting exactly one
readable file survives having a search in front of it. A search *program* would have moved the
authority one line earlier and silently.

The split follows the tree's usual one. The reading is `documentation::index::search`, and it is the
single point at which the writer and the reader are proved to agree. `cargo xtask manual capability`
on the host and `apropos capability` at the prompt call that same function, over the same bytes,
through the same one-page-at-a-time `Pages`. The rendering is `swish::write_apropos`, host-tested
with the rest of what the prompt says. What is left in `components/src/swish/prompt.rs` is four
filesystem requests and a 4 KiB page buffer.