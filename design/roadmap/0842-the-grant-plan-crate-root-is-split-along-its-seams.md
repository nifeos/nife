---
status: BUILT
raised: 2026-10-08
built: 2026-10-11
milestone_dependencies: none
decision_dependencies: 266, 272
machine_requirements: none
specific_machine: none
needs_person: no
---
# 842. The grant-plan crate root is split along its seams

*(The title and slug are provisional.)*

`crates/grant_plan/src/lib.rs` is 7,080 lines at `0435a9aeb`, the largest Rust file after
`kernel/src/sched.rs`. It was 5,060 lines on 2026-09-24. §266 (a Rust source file stays under
2,000 lines) sets the ceiling and a 2026-12-31 goal of no file over 4,000. Milestone 840 (the
scheduler file is split along its seams) is the model for this block.

The cut and every module name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

**BUILT, 2026-10-11 (UTC).** The lane wrote the cut up on 2026-10-10 as §272 (the grant-plan
crate root splits into private modules), and calef ruled its three forks on 2026-10-11, each as
recommended, on #1913. `lib.rs` is 185 lines: the crate docs, the module lines, a named re-export
of every public item, and the byte helpers. The rest moved unchanged into eight private modules
(`program`, with its child file `program/manifests.rs`, `manifest`, `command`, `refusal`,
`parsing`, `planning`, `escalation`), and the tests moved beside them. The largest new file is
`planning/tests/designation.rs` at 692 lines. Every module name ships provisional until a
name-ratification pass. The one line of code that changed is `run.line` to `run.line()` in
`planning`, the public accessor, so `RunSpec`'s fields stay private as ruled.

Three places the move differs from §272's table. `MAX_POSITIONALS` went to `parsing` with
`RunSpec`, because it is the length of two of `RunSpec`'s fields. The two tests of the root's byte
helpers sit in the root's own `tests.rs`. The image tests are `image/tests.rs`, a child file, as
every other module's are. Section comments in the old test module that headed tests now in more
than one file are repeated in each.

Reuse: not applicable; this moves code and adds none. The crate already has seven submodules
(`expand`, `job_page_frame`, `job_windows`, `line`, `nav`, `spawnproto`, `word`), which is the
precedent.

## What is in the file

Measured on `0435a9aeb` from the file's own top-level items. Ranges are approximate at the edges,
where a doc comment belongs to the item below it.

| lines | count | what |
|---|---|---|
| 1 to 104 | 104 | crate docs, module declarations, imports |
| 105 to 242 | 138 | the `programs!` macro and the `const` checks it leans on |
| 243 to 561 | 319 | the `programs!` table: 22 programs, each with a wire id and a name |
| 563 to 1217 | 655 | `Prog::manifest`, one arm per program |
| 1218 to 1687 | 470 | output and input specs, flags, slot numbers, the image manifest rules |
| 1688 to 2097 | 410 | `ArgSpec` through `DirSpec`, `Manifest`, `Runtime` |
| 2098 to 2650 | 553 | `Command`, `RunSpec`, `Endowment`, the file and directory grants, `Holdings` |
| 2651 to 2950 | 300 | `Refusal` and its sentences |
| 2951 to 3353 | 403 | `tokenize`, `argv`, `parse`, the package and user verbs, `parse_run` |
| 3354 to 3933 | 580 | `plan` and its variants, stream checks, `check_chain`, `designate` |
| 3934 to 3977 | 44 | byte helpers: `trim`, `parse_u64` |
| 3978 to 4069 | 92 | the two-tier interrupt escalation policy, §24 (interrupting the foreground process) |
| 4070 to 7080 | 3,011 | `mod tests`, 119 `#[test]` functions |

The file is a real mix, not data. The program table and its manifests are 974 lines of
declaration. Tests are 43%. The rest is three different jobs: what a program may be granted,
what a line says, and whether the one can be granted from the other.

## A proposed cut

Every name here is provisional. The modules are private, and `lib.rs` re-exports their items, so
every path a caller uses today still resolves.

| module, provisional | from the table | about |
|---|---|---|
| `lib.rs` | 1 to 104, byte helpers, re-exports | 200 |
| `program` | 105 to 1217 | 1,110 |
| `manifest` | 1218 to 2097 | 880 |
| `command` | 2098 to 2650 | 550 |
| `refusal` | 2651 to 2950 | 300 |
| `parsing` | 2951 to 3353 | 400 |
| `planning` | 3354 to 3933 | 580 |
| `escalation` | 3978 to 4069 | 90 |

`parsing` and `planning` are not `parse` and `plan` on purpose. Those are the crate's two public
functions, and a module of the same name beside each would read as one thing.

The tests do not fit beside their code the way 840's do. About 2,000 of the 3,011 lines test
planning: the operators, `2>`, the draining lane, `rm`, globbing, `bind` and two directories.
`planning.rs` with those inline would be 2,600 lines. So the tests go in a `tests` file per module,
`planning/tests.rs` and its siblings, declared `#[cfg(test)] mod tests;`.

## What an architect has to rule

1. Private modules with re-exports, or public modules. The recommendation is private: the crate's
   public API does not change at all. Public modules would add a second path to every item, such
   as `grant_plan::manifest::Manifest`. That is a public crate API change and a fork in its own
   right. The reason holds at equal cost, so it is not about effort.
2. Whether the program table and the manifests stay together. They are one edit when a program is
   added (`notes/adding-a-program.md`), which is the case for one module.
3. Every module name in the second table.

If the answer is "do not split", the file keeps growing. It grew 2,020 lines in 14 days.

## What moving the code breaks

Found with `grep` at `0435a9aeb`. A lane has to carry each one.

- 78 Rust files outside the crate name `grant_plan`. With private modules and re-exports, none of
  them changes.
- The doc comments inside `programs!` link `parse`, `spawnproto` and other items by bare name.
  An intra-doc link resolves from the module that holds it, so each needs a `crate::` path once the macro
  moves.
- The wire ids in the table are a format the shell and the progenitor agree on. Moving the table
  moves them unchanged; `the_wire_ids_already_shipped_never_move` is the test that says so. The
  move must not reorder rows.
- `notes/adding-a-program.md` tells a newcomer to add a row in `programs!` in this file. It and 14
  other notes and design files name the path, and 6 cite it as `lib.rs:NNNN`.
- The one falsification patch under `crates/grant_plan/falsifications/` targets `job_windows.rs`,
  not this file.
- No Kani harness is in this file, and the crate is not in `kani-reach.yml`'s list.
- 37 first-parent merges in the 14 days to 2026-10-08 touched it. Milestone 365 (`xtask/src/main.rs`
  is 6,785 lines with no module structure) found that a split of a hot file is a timing problem.
  Take it when few lanes are open in it.

## Done when

1. `crates/grant_plan/src/lib.rs` is under 2,000 lines, and no new file is over 2,000. Under
   4,000 is an acceptable first step if the ruling splits the work.
2. No Rust file outside `crates/grant_plan` changes.
3. The crate's 119 host tests pass with no test body changed, and so do `script/test` on aarch64,
   riscv64 and x86_64 and `script/swish-check`.
4. `cargo doc` for the crate reports no broken intra-doc link.
5. Each new module carries a `//! Name:` block marked provisional.

## Follow-on

- **Decision.** Every module name ships provisional. calef deferred them on 2026-10-11 (UTC) to a
  name-ratification pass, and
  `design/decisions/0272-the-grant-plan-crate-root-splits-into-private-modules.md` records it.
- **Decision.** calef expects the program table to shrink once packages install through jig rather
  than through this list. `design/decisions/0272-the-grant-plan-crate-root-splits-into-private-modules.md`
  records it, and the header of the table's own file says so where the next person to add a row
  will read it.

## Index row

`crates/grant_plan/src/lib.rs` is 7,080 lines and grew 2,020 in two weeks. It holds the program
table, the manifests, the command line's parse and the plan that joins them, plus 3,011 lines of
tests. This splits it into private modules behind unchanged public paths. The cut and every name
are an architect's call.
