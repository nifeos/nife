# Why adding a program works this way

*Name: provisional. After the parent's "Why it works this way". calef names files.*

An appendix to [`notes/adding-a-program.md`](../adding-a-program.md): the reasoning behind its steps.

Milestone 150 (adding a program should not need eight hand-maintained lists) built this. The block is
[design/roadmap/0150-program-declaration-data.md](../../design/roadmap/0150-program-declaration-data.md);
this page is the reasoning, and
[DECISIONS §158 (a program is declared once)](../../design/decisions/0158-a-program-is-declared-once.md)
is the record minted from it.

## The archive list is the `[[bin]]` blocks

A program has to be declared there for cargo to build
it, so that was always one of the places; the choice was whether the others could be derived from
it. Considered and refused:

- *A shared crate holding a `const` table of programs*, which both `xtask` and the kernel could
  read. It is still a second list beside `Cargo.toml`, only gated rather than hand-copied, so adding
  a program stays two edits. It would earn its place if the kernel needed the list at runtime, and
  it does not: everything that loads a program looks it up by name.
- *`cargo metadata` instead of reading `Cargo.toml`.* Correct by construction, but its output is
  JSON and `xtask` has no JSON parser. Hand-scanning JSON is no less fragile than hand-scanning the
  four keys this tree writes in a `[[bin]]` block, and DECISIONS §46 rules out a `serde_json` or
  `toml` dependency for one list. The scanner refuses what it does not understand instead.
- *Keeping per-architecture tables and gating them against each other.* The two tables disagreed
  about two programs when they were deleted, and neither difference was a decision. The shared
  table's own comment said not to filter by architecture. Its reason: an archive entry costs a slot and
  some bytes, nothing spawns a program by accident, and a test that cannot run somewhere `skip!()`s
  with the reason. Packing everything everywhere is that rule without a second place to break it.

## The shell's program table is one `macro_rules!` declaration

The seven-edit `Program` bookkeeping
had exactly one edit nothing caught (`PROGRAM_COUNT`), and forgetting it also disarmed the test that
would have caught two more. Considered and refused:

- *A derive crate that counts variants* (`strum` and the like). DECISIONS §46: a proc-macro
  dependency in a crate the kernel and the progenitor link, for one count, is the dependency that
  section exists to refuse.
- *A `const` table of `(Program, id, name)` beside a hand-written enum.* Still two lists: nothing
  stops a variant existing without a row, which is the original bug moved one line down.
- *Putting `manifest()` inside the declaration too*, for one edit instead of two. It would have
  worked, and it was refused because the manifest arms are the most commented code in the crate and
  `rustfmt` does not format inside a macro invocation. The compiler already demands the arm, so
  moving it buys one fewer edit that was never silent.
- *Deriving the id from declaration order.* Removing a program would renumber every later one,
  which is a wire-format change dressed as a deletion. Ids are written, `PROGRAM_COUNT` is one past the
  highest, and removal leaves a hole.

## The count gate became a set of checks rather than a number

The block asked for "a gate on
program count". A pinned total was considered and refused: it would be a hand-maintained number
that fails on every legitimate addition, which is the shape this milestone removes. What a count was
for is covered by construction (the archives and `PROGRAM_COUNT` are derived, so neither can be short
of the declaration). It is also covered by three host tests on the relationships that can still go wrong: a
spawnable program with no binary, a program the tree loads by name with no binary, and a spawnable
program no swish-check line runs.

## `swish`'s exhaustive `write_outcome` match became a wildcard

Eleven of its thirteen arms were
`=> {}`, so the compile error it raised for every new program asked a byte-stream author for a
keystroke, not a decision. `every_program_that_answers_in_words_renders_its_answer` now asks the
question it was standing in for, of exactly the programs it applies to. That is rung two in place of
rung one, deliberately, and it is the one place this milestone went down the ladder.
