---
status: DECIDED
raised: 2026-10-11
decided: 2026-10-11
ratified_by: calef
---

# 282. When a file is split, the parent becomes an index

Raised and decided 2026-10-11 (UTC), while calef ruled fork 1 of
[#1959](https://github.com/nifeos/nife/pull/1959), the cut of
[milestone 872 (the kernel's user-mode file is split along its seams)](../roadmap/0872-the-kernels-user-mode-file-is-split-along-its-seams.md).
Written by lane `lane/split-parent-is-an-index`. *(Section number provisional until the merge queue
lands it. The gate's file names are provisional too.)*

## The ruling

Fork 1 of #1959's proposal (the kernel's user-mode file splits into child modules of `user`) asked
whether `AddressSpace` stays in `kernel/src/user.rs` or moves to a child. The recommendation leaned
on §271 (the scheduler file splits into submodules of `sched`), which had kept `IpcTables` in its
parent.
calef declined to weight it:

> The precedent is super new so I don't know that I would weight it. Instead I think we're
> establishing the precedent. So what should it be and why?

The recommendation that came back was this section's rule, and calef answered:

> Agreed, make the parent an index

The ruling was given in the maintainer session on 2026-10-11 (UTC), and the maintainer records it
on #1959.

## The rule

When a file is split, the parent becomes an index. It holds module or crate docs, `mod` lines,
`use` and re-exports, and nothing that can grow into a feature. Every type and function lives in a
child named for it, the central one included.

The exception is a handful of small constants or helpers that every child uses and that would be
odd alone in a file. They may stay. The test is "nothing here can grow into a feature", and an
exception is marked where it sits.

## Why

1. Regrowth. These files grew because the parent was the default home for new code.
   `crates/system_initializer/src/lib.rs` was 2,805 lines on 2026-09-24 and 6,358 on 2026-10-08,
   which is more than double in two weeks. An index gives new code nowhere to land but a named
   file, so growth shows up as a named file growing.
2. A concept is found by its file name, and the `mod` list is the table of contents. §266 (a Rust
   source file stays under 2,000 lines) argued that an agent should see a whole file in one read.
   That is served by opening the file named for the thing being changed, not a parent that holds
   it beside everything else.
3. `pub(super)` on the fields that siblings reach into is an honest marker of that dependency. The
   compiler's visibility set is the same either way: a private field of `user` and a `pub(super)`
   field of `user::address_space` are visible to exactly the same modules. The keyword is where a
   reader sees the dependency, and where it can later be tightened into accessors.
4. An index rarely changes, so the hottest file stops being a merge conflict. 57 first-parent
   merges touched the system-initializer crate root in the 14 days to 2026-10-08.

## Prior art, read and mixed

Read on 2026-10-11 in the pinned nightly's `rust-src` (rustc 1.101.0-nightly, `6f5da406b`).

| module | shape |
|---|---|
| `alloc::vec`, `library/alloc/src/vec/mod.rs` | Not an index. `pub struct Vec` and its inherent `impl` blocks stay in the parent, 4,656 lines. The children are its iterators (`into_iter`, `drain`, `splice`, `extract_if`, `peek_mut`) and specialization helpers. |
| `std::collections`, `library/std/src/collections/mod.rs` | An index. 408 of its 459 lines are module docs. The rest re-exports `btree_map`, `vec_deque` and the other collections from `alloc`, declares `mod hash;`, and defines two inline modules, `hash_map` and `hash_set`, that hold only `pub use`. The map itself is `hash/map.rs`. |

The standard library does both, so it decides nothing. This tree's reasons above decide it. The
gate below reads `std::collections` as an index and holds `alloc::vec` at 1,284 code lines.

## What it does to the splits in flight

§271's `IpcTables` in `kernel/src/sched.rs` predates the rule and stays unless it regrows. The gate
holds that file at its size today, so it cannot.

§272 (the grant-plan crate root splits into private modules) kept four byte helpers in
`crates/grant_plan/src/lib.rs`. They predate the rule too, and are held the same way.

The three open split proposals each apply this rule to their own cut: the system-initializer crate
root (#1957), the swish-check harness (#1958) and the kernel's user-mode file (#1959). They carry
provisional section numbers and are not on main, so they are named here by pull request. Their
parents are all on the baseline below. A cut that moves code out of a listed parent only lowers its
number, so none of them is blocked by the gate.

## The gate

`helpers/parent_index.py`, run by `script/lint` beside the file-length and comment-block ratchets,
whose shape it copies. Its header is the manual; this is the summary.

A parent is a Rust file that declares a child out of line (`mod x;`), other than a child named
`tests` or `proofs`. That covers a `foo.rs` beside `foo/`, a `mod.rs`, and a crate root with
children. A file whose only children are its own tests has moved its tests out, not split its code.

An index holds inner docs and attributes, `mod` lines, `use` and `pub use`, `extern crate`, and
inline modules holding only those, as `std::collections` writes `hash_map`.

The exception is checked two ways:

- An item of at most 3 code lines may stay unmarked, while the parent holds at most 8 of them. A
  code line is one on which a token that is not a comment begins. An item is never charged for its
  docs, so nobody can pass by deleting one.
- Any other item stays only with a mark among its leading comments:
  `// Index exception (2026-10-11): <reason of three words or more>.` A comment that names an
  index exception in any other form fails.

Why 3 lines. Over the 57 parents in the tree on 2026-10-11, there are 1,506 items an index does
not hold. 382 of 400 constants fit in 3 code lines, as do all 5 type aliases and 35 of 44
statics. Three lines is also a function that is a signature, one expression and a closing brace,
which is a wrapper. The fourth line is the first statement that is logic. Every enum and all but 3
of 117 `impl` blocks are over it, which is right: those are features.

Why 8. Of the 8 parents that are indexes already, the most small items any holds is 6, in
`crates/file_system_contract/src/lib.rs`, cut by §274 (the filesystem-protocol crate root splits
into its module files). Eight leaves room for two more, and refuses the case that motivated a
count: the system-initializer crate root holds 82 small items, 71 of them constants, which together
are a feature. Past 8, every small item in the parent counts.

The baseline, `design/parent-index-baseline.tsv`, holds 49 of the 57 parents, one row each, at the
code lines outside the index: 31,563 in all. The rules are the file-length ratchet's:

- An unlisted parent fails at any number above zero, so a new split meets the rule the day it
  lands.
- A listed parent fails if its number grew against the merge base or exceeds its row. That is how
  the rule binds an existing parent as it is touched.
- A row only falls (`--bank`), may not be added, moves with a rename, and must go when its file is
  an index or no longer a parent.

Planting a four-line function at the end of `crates/file_system_contract/src/lib.rs`, an index, turns
`--check` red and names the function. Adding one statement to `trim` in
`crates/grant_plan/src/lib.rs`, a listed parent, turns it red twice: over its row, and grown against
the merge base.

## What else was considered

- The central type stays in the parent, §271's shape and `alloc::vec`'s. Refused for the four
  reasons above. The parent stays the default home, and it is the file that regrew.
- A mark on every item, with no size allowance. Refused: a mark on a one-line constant costs more
  to read than it says, and 382 constants would need one.
- Physical lines rather than code lines. Refused for §266's own reason: a measure that counts
  comments rewards deleting them.
- A cliff, failing all 49 parents today. Refused for §266's reason: a gate cannot split a file, so
  the tree goes red and nothing improves.
- An allowance of 5 or 10 code lines. Five admits a function with three statements of logic; ten
  admits two thirds of all functions in parents. Neither is "nothing here can grow".

## BUGS

- A mark can launder an item. Nothing stops a lane marking an existing large function to make room
  for growth elsewhere in a listed parent, and a marked item's own growth is not limited. Review is
  the defense; `--list` prints every mark, and there are none today.
- 3 and 8 are judgments read off one day's tree, not measurements of harm. If either proves wrong,
  it is a constant in the helper and an amendment here.
- An inline `mod tests { .. }` in a parent counts against it. That is the rule as written, since
  the parent is not an index while it holds tests, but it means the cheapest shrink is moving tests
  out, which leaves the code no smaller.
- A `mod` declared by a macro expansion, and items behind `include!`, are invisible. The lexer is
  adapted from this tree's other Rust checks and is not a parser.
