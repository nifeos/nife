---
status: NOT-STARTED
raised: 2026-10-11
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 883. The three longest scripts are split, and a ruling says whether scripts have a ceiling

*(The title and slug are provisional.)*

Three files under `script/` are over 2,000 lines at `396187b0b`. `script/lint` is 4,006,
`script/metrics` is 3,297 and `script/falsifications` is 2,305. No other file under `script/` or `helpers/` is over 2,000; the next is `script/roadmap`
at 1,819.

§266 (a Rust source file stays under 2,000 lines) does not reach them. Its scope is `.rs` files,
and `helpers/file_length_ratchet.py` selects them by suffix. Whether a rule should reach scripts is
the `unwritten` decision dependency, as it was for milestone 840 (the scheduler file is split
along its seams) before §266 existed.

The cut, every file name below, and that rule are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

Reuse: not applicable; this moves code and adds none. The precedent is `helpers/`, which already
holds the Python behind 14 of `script/lint`'s sections and nine modules `script/metrics` imports.

## What is in each script

All three are mostly Python. Lint holds 17 Python heredocs; the other two are one heredoc each
behind a shell header.

| script | lines | Python in heredocs | comment lines | blank |
|---|---|---|---|---|
| `script/lint` | 4,006 | 1,703 (43%), in 17 heredocs | 1,818 | 265 |
| `script/metrics` | 3,297 | 3,122 (95%), in one | 770 | 346 |
| `script/falsifications` | 2,305 | 1,995 (87%), in one | 523 | 196 |

Comment lines count `#` lines of both languages, not docstrings.

### `script/lint`

The unit is a section: a column-0 `echo "==> ` line up to the next one. There are 86. Under
`--no-cargo`, the pre-push hook's mode, `helpers/lint-no-cargo.awk` reads this file, keeps 65 and
drops the 21 that invoke cargo. Ranges are approximate, since a check's comment sits above its
`echo`.

| lines | count | what |
|---|---|---|
| 1 to 124 | 124 | header, the `--baselines` and `--no-cargo` modes, the two hook contracts |
| 125 to 423 | 299 | 14 clippy passes, all cargo |
| 424 to 869 | 446 | kernel source rules: unsafe contracts, arch rules, CFI, fences, dead code |
| 870 to 1230 | 361 | repository text: markers, pinned actions, TODOs, records, citations, rustdoc |
| 1231 to 1544 | 314 | three `cargo metadata` cross-checks: the host pass and mutation's two scopes |
| 1545 to 1724 | 180 | script docs, the project's QEMU on PATH, icount baselines |
| 1725 to 2387 | 663 | counted claims, one heredoc of 647 lines |
| 2388 to 3093 | 706 | the numbered checks 1 to 15, and 4b, 5b and 12b |
| 3094 to 3319 | 226 | markdown links, then 14 sections that each run a helper's selftest and check |
| 3320 to 3495 | 176 | em-dashes, shellcheck, dependency direction, package boundaries |
| 3496 to 3708 | 213 | the region claim protocol, one heredoc of 200 |
| 3709 to 4006 | 298 | unused dependencies, unseen tests, the verify table, spelling, `sh -n` |

The heredocs by size: 647, 200, 104, 92, 75, 71, 65, 65, 63, 53, 50, 46, 46, 42, 40, 22 and 22.
Six of them call `["cargo", "metadata", ...]`.

Five comment blocks are over 40 lines. Lint has no `--selftest`;
`helpers/lint-no-cargo-selftest.sh` proves the partition.

### `script/metrics`

The heredoc's own banner comments give the sections.

| lines | count | what |
|---|---|---|
| 1 to 173 | 173 | shell header (one 168-line comment), usage, `exec python3 -` |
| 174 to 269 | 96 | imports of nine `helpers/` modules, paths, statuses |
| 270 to 603 | 334 | which model wrote a commit, the `MEASURES` registry, `Tree` |
| 604 to 904 | 301 | measures: milestones, built and merged per week, models per week |
| 905 to 1180 | 276 | cost inputs: ledger, cash, queue, verify and CI times, effort |
| 1181 to 1838 | 658 | measures: decisions, names, lines, file size, bugs, unsafe, prose, fatal risks |
| 1839 to 1942 | 104 | weeks, the commit for each, CSV read and write |
| 1943 to 2546 | 604 | the SVG charts |
| 2547 to 2889 | 343 | week notes, derived columns, drawing |
| 2890 to 3119 | 230 | the table printer and the selftest |
| 3120 to 3297 | 178 | `main` |

### `script/falsifications`

| lines | count | what |
|---|---|---|
| 1 to 308 | 308 | shell header (one 295-line comment), `. helpers/qemu-path.sh`, `exec python3 -` |
| 309 to 393 | 85 | imports, states, regular expressions |
| 394 to 702 | 309 | package discovery, harness and kernel-test parsing |
| 703 to 1033 | 331 | the record classes: Kani, kernel test, swish check, host test |
| 1034 to 1265 | 232 | the new-test ratchet and the selftest |
| 1266 to 1604 | 339 | `--check`, the table, the worklist |
| 1605 to 1944 | 340 | bounded replay and the sweep |
| 1945 to 2188 | 244 | affected packages, shard packing, record times |
| 2189 to 2305 | 117 | `main` |

The Python in all three already imports `helpers/rust_source.py`. `xtask/src/` holds none of this
logic: it builds, boots and benchmarks.

## The forks

### (a) Sourced shell parts behind the front door

`script/lint` keeps its header and modes and sources parts with `.`. `$0` still names the front
door inside a part, so the many `$(dirname "$0")` paths hold. Only lint is shell; for the other
two this fork does not apply.

The costs, measured. The awk partition reads one file, so it must read the parts in order. A part
under a directory in `script/` breaks shellcheck, which fails on a directory argument (tried). A part
in a `helpers/` subdirectory escapes the `helpers/*.sh` globs of shellcheck, `sh -n` and the
fixed-`/tmp` check. A flat `helpers/*.sh` part needs a shebang or shell directive (shellcheck SC2148,
tried), and its four reads of `$lint_root` draw SC2154 at the gate's severity (tried).

### (b) Python moves into modules under `helpers/`

Each heredoc becomes a module with its own `--selftest`, and the front door calls it. This is what
the tree already does. Lint has 39 lines that call a `helpers/*.py` file, and about 45 that run a
selftest. `script/roadmap` execs `helpers/roadmap_migrate.py` for one mode.

The reason is not length alone. A heredoc cannot be imported or selftested, and lint says so at
line 1775: two checks hold one exemption list twice because "there is no scope they could share".
Milestone 236 (three derivations are copied between scripts, and nothing notices when they drift)
is the same defect between scripts.

The costs. Six lint heredocs call cargo; moved verbatim, the awk no longer sees it, and the pre-push
hook would run cargo. So cargo stays in the shell: the section runs `cargo metadata` into a file
and the module reads it. A pipe would hide cargo's exit status under `set -e`. Splitting one large
program across modules turns globals into imports: `root` appears 64 times in falsifications.

### (c) Logic moves into Rust in `xtask`

Refused for lint. Milestone 729 (the pre-push hook runs what fits in seconds) made `--no-cargo`
the hook's mode, and `cargo xtask` is cargo. Refused for metrics, whose header says nothing in it
may build. Possible for falsifications, which already runs `cargo kani` and `cargo xtask test`.

It is not recommended there either. Its parser shares `comment_only_change` and
`DOC_BLIND_PROC_MACROS` with lint through `helpers/rust_source.py`. A Rust copy would be a second
lexer, which is what milestone 236 removed. The precedent for (c) is ten scripts that exec
`cargo xtask`, all of which build or boot. xtask's cold build time was not measured here.

### (d) Whether a rule gives scripts a ceiling

| option | what it means | cost |
|---|---|---|
| (i) amend §266 to cover scripts | same measure, same 2,000, same ratchet | two functions and the selftest |
| (ii) a new section | its own ceiling, say 1,500 | the same code, and a second rule |
| (iii) no rule | split these three once | none now; growth is caught only by notice |

The widening is small. `file_length_ratchet.py` selects files in `tracked_rust` and `renamed_rust`
(lines 116 to 140). A script predicate would be every file directly under `script/`, plus
`helpers/` files ending `.py`, `.sh`, `.awk` or `.jq`. That leaves out the 1,786-line JSON fixture.
The dashboard's file-size series reads Rust only and can stay so.

The measured stakes: 9 code files in `script/` and `helpers/` are over 1,000 lines, 4 over 1,500,
and 3 over 2,000. Lint grew 386 lines in the 14 days to 2026-10-11. Metrics grew 861 and
falsifications 1,019.

## The seven questions

1. Considered: forks (a) to (d) above, and leaving the files whole. Leaving them loses because §266's
   checked reason, the 2,000-line `Read` default, does not depend on language.
2. The tree already moves Python out: 14 lint sections are a line each over a helper "whose
   header is the manual", and metrics imports nine modules.
3. Prior art outside the tree was not read for this block. Pylint's `too-many-lines` defaults to
   1,000; that is from memory, not read.
4. The premise holds for metrics and falsifications, one program each. It is weaker for lint, whose
   86 sections are independent: a lane adding a check reads its neighbors, not the whole file.
5. Costs are in the forks above and in what moving the code breaks.
6. Entirely reversible. Every caller outside the tree uses a front door, and the front doors stay.
7. Yes for (b) over (a) on the Python, and for (d)(i). Sourced parts for lint are needed only to
   meet a ceiling; that one is about the number, not effort.

## A proposed cut

Every file name here is provisional.

`script/lint`, if (d) sets 2,000 for scripts:

| file, provisional | from | about |
|---|---|---|
| `script/lint` | header, modes, every section's comment, `echo` and call | 1,440 |
| `helpers/lint-clippy.sh`, sourced | 125 to 423 | 300 |
| `helpers/lint-numbered-checks.sh`, sourced | 2388 to 3093, less its three heredocs | 560 |
| `helpers/counted_claims.py` | the 647-line heredoc | 650 |
| `helpers/region_claim_check.py` | the 200-line heredoc | 200 |
| `helpers/lint_cargo_metadata.py` | the five heredocs that read `cargo metadata` | 350 |
| `helpers/lint_kernel_source.py` | unsafe contracts, machine description, fences | 215 |
| `helpers/lint_tree_text.py` | the remaining seven heredocs | 300 |

Moving only the Python leaves lint at about 2,290, still over 2,000. The two sourced parts close
that gap, so under (d)(iii) or a higher ceiling they are dropped. One module per check would give
17 files of 22 to 650 lines; grouping by what a check reads gives five.

`script/metrics`:

| file, provisional | from | about |
|---|---|---|
| `script/metrics` | 1 to 173, ending `exec python3 helpers/...` | 175 |
| `helpers/project_metrics.py` | imports, weeks and CSV, printer, selftest, `main` | 610 |
| `helpers/project_metrics_registry.py` | 270 to 603 | 335 |
| `helpers/project_metrics_measures.py` | 604 to 904, 1181 to 1838 | 960 |
| `helpers/project_metrics_costs.py` | 905 to 1180 | 280 |
| `helpers/project_metrics_charts.py` | 1943 to 2546 | 605 |
| `helpers/project_metrics_week_notes.py` | 2547 to 2889 | 345 |

`script/falsifications`:

| file, provisional | from | about |
|---|---|---|
| `script/falsifications` | 1 to 308, ending `exec python3 helpers/...` | 310 |
| `helpers/falsification_records.py` | 309 to 1265 | 960 |
| `helpers/falsification_check.py` | 1266 to 1604 | 340 |
| `helpers/falsification_replay.py` | 1605 to 2305 | 700 |

Moving the heredoc whole would leave one module of 1,995 lines, at the edge of any ceiling, hence
three.

## What an architect has to rule

1. Fork (d): does a rule cover scripts, and at what ceiling. The recommendation is (i), amending
   §266 to cover scripts at 2,000. The `Read` window is the checked reason and it is blind to
   language. That holds at equal cost. Landing the gate first, with three rows, stops the growth
   while the splits are in flight.
2. Python into `helpers/` modules, fork (b), for all three scripts. The recommendation is yes, for
   the selftest and the shared scope a heredoc cannot have. That holds at equal cost.
3. Sourced shell parts for lint, fork (a). Recommended only if ruling 1 sets a ceiling lint would
   still exceed. That recommendation is about meeting the number, not about effort or elegance.
4. Fork (c), Rust in `xtask`. Refused for lint and metrics on the measured constraints above; not
   recommended for falsifications.
5. Every file name in the cut.

No wire format, syscall or public crate path changes under any answer here.

## What it is not

Not a change to what any check, measure or replay does, and not §267 (a comment states the
constraint as it is now) for scripts. That cap's ratchet is Rust-only too. Falsifications' header
is one 295-line comment and metrics' is 168, but whether the cap covers scripts is a separate
question.

## What moving the code breaks

Found with `grep` at `396187b0b`.

- Call sites keep working if each front door stays and forwards its arguments. They are
  `.githooks/pre-push` (`--no-cargo`) and `ci.yml` (`script/ci-build lint`, `script/metrics
  --selftest`). `metrics.yml` has three and `falsifications.yml` four. `verify.yml` has three.
- `helpers/lint-no-cargo.awk` reads `script/lint` only. Sourced parts must be fed to it in order,
  and `helpers/lint-no-cargo-selftest.sh` case 6 then checks the parts. Moved Python must leave
  cargo in the shell, or the six `["cargo", "metadata"` sites vanish from the partition.
- At least 54 lines in 38 files outside lint cite its checks by number: 3, 4, 4b, 4c, 5, 5b, 10, 11, 13 and
  15. The numbers live only in comments, so each moves with its comment and none is renumbered. Two
  existing defects surfaced. Checks 3, 4, 4b and 5 have no `echo` of their own and print under
  "fixed /tmp paths in scripts". Three citations name a check 4c that no longer exists.
- Six line-number citations point into these scripts, and all six already point at the wrong
  lines. One is in milestone 849 (a package declares what it needs at run time). Two are in
  milestone 791 (bold that a script reads), two in milestone 118 (CLAUDE.md has a budget), and one
  is inside lint.
- Lint's QEMU-on-PATH check reads files under `script/` only. Falsifications sources
  `helpers/qemu-path.sh` because of its `["cargo", "xtask", "test", ...]` list at line 886. Moved to
  `helpers/`, the line is no longer required and could go silently. The check must follow the
  module the front door execs.
- `helpers/prose_only.py` names `script/metrics` in `HOST_ONLY_TOOLS`, so a metrics-only change
  skips the heavy jobs. New metrics modules must join that list. A module stem is matched as a
  word in other files, so a stem like `metrics` alone would match far too much.
- Selftests: lint runs `script/falsifications --selftest` and `--count`, and `ci.yml` runs
  `script/metrics --selftest`. Both run through the front doors unchanged; new modules add their
  own `--selftest` to lint.
- Names: `script/names` requires a `# Name:` block in every file directly under `script/`, and
  lint's script-docs check requires a `notes/scripts.md` entry for each. Files under `helpers/` need
  neither, by milestone 446 (the naming worklist says what it covers), and carry a provenance
  paragraph by convention.
- No baseline TSV names these paths. The file-length and comment-block baselines are Rust-only.
- Hotness: 58 of the 460 first-parent merges in the 14 days to 2026-10-11 touched lint, 23
  touched metrics and 15 touched falsifications. Each move is one pull request, landed fast.
  Verbatim-moved lines do not count as new citations, by milestone 609 (the system tests leave the
  kernel crate)'s rule in `script/citations`.
- CodeQL analyzes Python, so about 6,800 lines become visible to it and may raise alerts. That it
  skips extensionless heredocs today is from memory, not read.

## Done when

1. The ruling on fork (d) is recorded in `design/decisions/`, and if it sets a ceiling, the gate
   holds scripts to it.
2. No Python heredoc remains in `script/lint`, `script/metrics` or `script/falsifications`.
3. Each script is under the ruled ceiling, or under 2,000 if none, and no new file is over 1,500.
4. Every caller in `.github/`, `.githooks/` and `script/` runs unchanged, and `script/lint
   --no-cargo --list` prints the same sections it did before.
5. Every numbered check keeps its number, and `helpers/prose_only.py` and the QEMU-on-PATH check
   name the new paths.
6. `script/lint`, `script/metrics --selftest` and `script/falsifications --selftest` pass, and a
   `--sweep` replays the same records.

## Index row

`script/lint`, `script/metrics` and `script/falsifications` are 4,006, 3,297 and 2,305 lines, and
most of each is Python in heredocs that cannot be imported or selftested. This moves that Python
into modules under `helpers/` behind unchanged front doors. It asks whether §266's 2,000-line
ceiling should cover scripts. The cut, the names and that rule are an architect's call.
