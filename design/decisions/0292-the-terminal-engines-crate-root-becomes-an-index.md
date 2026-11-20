---
status: DECIDED
raised: 2026-10-12
decided: 2026-10-12
ratified_by: calef
---

# 292. The terminal engine's crate root becomes an index

Raised 2026-10-12 (UTC) as the cut that
[milestone 874 (the terminal engine moves its palette and its tests out)](../roadmap/0874-the-terminal-engine-moves-its-palette-and-its-tests-out.md)
says is calef's call before a line moves. Written by lane
`milestone/874-the-terminal-engine-moves-its-palette-and-its-tests-out`. *(Section number
provisional until the merge queue lands it.)*

It blocked the move and nothing else.

## The ruling

B, the index, settled by precedent on 2026-10-12 at 03:08 (UTC). calef asked nothing separate on
this section. §282 (when a file is split, the parent becomes an index) postdates the block, and its
third reason had already refused the block's one argument for keeping `Attr` and `Vt` in `lib.rs`.
calef ruled the same way on [PR #1959](https://github.com/nifeos/nife/pull/1959): *"Agreed, make the
parent an index."* §279 (the system-initializer crate root splits into private modules) applied it to
a crate root, and pull requests #1967 and #1969 applied it to blocks written before §282, as this
one was. Tests sit beside their code as the 845 ruling has it, `test_support.rs` follows 842's
shape, and the names wait for calef's naming pass. The maintainer recorded the ruling on
[PR #1968](https://github.com/nifeos/nife/pull/1968#issuecomment-6117031057).

The block asked three questions. Its ruling 2, the palette's 54-line doc comment, is
answered by §267 (a comment states the constraint as it is now)'s amendment of 2026-10-11 (UTC). A
listed block that a split moves unchanged keeps its row, so the comment moves whole and nothing is
cut. Its ruling 3, the file names, is left to calef's naming pass, and every name below ships
provisional with a `Name:` line.

## What the file is today

`crates/video_terminal/src/lib.rs` is 3,242 lines on main at `17b23264e`, the count the block
measured. It is a parent under §282 (when a file is split, the parent becomes an index), because it
declares `keymap` and `script` out of line, and `design/parent-index-baseline.tsv` holds it at 2,050
code lines. Code lines by banner:

| banner | lines | code lines |
|---|---|---|
| crate docs, `pub mod keymap`, `pub mod script` | 136 | 3 |
| geometry: `MAX_COLS` to `SCROLLBACK_CELLS` | 68 | 5 |
| the palette and its compile-time gate | 188 | 91 |
| cells: the color enum, `Attr` and its flags, `Cell` | 250 | 125 |
| damage: `CellRect`, `Damage` | 125 | 67 |
| the engine: `State`, `Vt` and its `impl` | 953 | 608 |
| `pub mod status`, inline | 31 | 6 |
| `mod tests`, 47 tests | 1,489 | 1,149 |

## The question, as raised: does the crate root become an index in this milestone?

### Background

The block was minted at 00:56 (UTC) on 2026-10-11. §282 was decided later the same day. The block
splits the code at the palette only and keeps `Attr` and `Vt` in `lib.rs`, for one reason: `Vt::sgr`
reaches `Attr`'s private `with`, its two color fields and six private flag constants, so a child
module for them "would make seven items `pub(super)`". §282's third reason answers exactly that: in a
child of the crate root, `pub(super)` names the same set of modules a private item of the root does,
so the keyword widens nothing the compiler enforces. It only shows the reader the dependency.

§282's gate does not force the answer either way. A listed parent may stay listed as long as its
number falls, and the block's cut lowers it. So the block's cut passes `script/lint`, and the rule's
text ("when a file is split, the parent becomes an index") says it should go further.

### Prior art

In the tree, which decides it. External peers were not read: §282 read `std::collections` (an
index) and `alloc::vec` (not one) and found the standard library does both, so another peer would
add a data point and settle nothing.

| split | when | what the parent kept |
|---|---|---|
| 840, §271 (the scheduler file splits into submodules of `sched`) | before §282 | `IpcTables`, held by the gate |
| 842, §272 (the grant-plan crate root splits into private modules) | before §282 | four byte helpers, held by the gate |
| 846, §275 (the file server core's tests move into child modules) | before §282 | `Server` and its helpers; `tests.rs` became a parent too, held at 325 |
| 843, §279 (the system-initializer crate root splits into private modules) | ruled with §282 | nothing: "the same rule moves the rest of the crate root out" |
| 872, #1959 (the kernel's user-mode file) | the ruling that made §282 | nothing: "make the parent an index" |

Every split ruled since §282 took its parent to an index, including 843, whose own proposal had
recommended keeping its layout constants in the root.

### Alternatives

- **A. The block's cut.** `palette.rs` with its tests, the other 44 tests in a child file, and
  `lib.rs` keeps geometry, cells, damage, the engine and `status`, about 1,560 lines. Its §282 row
  falls from 2,050 to about 810. No `pub(super)` is added.
- **B. The crate root becomes an index (recommended).** Every banner becomes a private child module,
  `status` becomes a file, and `lib.rs` keeps the crate docs, `mod` lines and named re-exports, so
  every public path still resolves. Its §282 row goes. Thirteen items become `pub(super)`: `Attr`'s
  three fields and `with`, the six flags, the underline and strikethrough rows, and `CellRect::cell`.
  Tests sit beside each module as `x/tests.rs`, the shape calef ruled for 845 in §274 (the
  filesystem-protocol crate root splits into its module files). The one helper two test files
  share goes to a crate-level `test_support.rs`, the shape of 842 (§272).

| file, provisional | holds | lines, as built |
|---|---|---|
| `lib.rs` | crate docs, `mod` lines, re-exports | 158 |
| `geometry.rs` | `MAX_COLS`, `MAX_ROWS`, `MAX_CELLS`, `SCROLLBACK_ROWS`, `SCROLLBACK_CELLS` | 72 |
| `palette.rs`, `palette/tests.rs` | `PALETTE`, the gate, `DEFAULT_FG`, `DEFAULT_BG`; its 3 tests and 2 tables | 196, 146 |
| `cell.rs`, `cell/tests.rs` | the color enum, `Attr`, the flags, the dim midpoint, `Cell`; 5 tests | 255, 86 |
| `damage.rs`, `damage/tests.rs` | `CellRect`, `Damage`; 2 tests | 132, 42 |
| `vt.rs`, `vt/tests.rs` | `State`, `MAX_PARAMS`, `Vt`; 37 tests and 3 helpers | 962, 1,217 |
| `status.rs` | the three report constants, unchanged | 23 |
| `test_support.rs` | `attr`, the helper the cell and engine tests share | 10 |

Considered and refused: an index whose one child `vt.rs` holds cells, damage and the engine
together, which adds no `pub(super)`. It meets the gate and misses the rule's reason, since `vt.rs`
would become the new default home at about 1,330 lines, which is the regrowth §282 exists to stop.

### Recommendation: B

1. It is the rule as written, and the block's only argument against it is the one §282 refused.
2. Regrowth. The file grew 1,206 lines in the 17 days to 2026-10-11. Under A, the parent stays the
   default home for the next rendition flag, the next color form and the next escape sequence.
3. Each concept is found by its file: a change to bold or dim opens `cell.rs`, not a 1,560-line
   root.

B costs more than A: five more files and thirteen `pub(super)` keywords. It would still be the choice
at equal cost, so the recommendation is not about effort.

The lane built B while the question waited. It found one more item than counted above: `Vt::sgr`
builds an `Attr` with struct-update syntax, which needs every field visible, `flags` included. That
keyword is in the thirteen.

## What it does not decide

No public crate API, wire format or system call changes under either answer. Every module and file name
is provisional.
