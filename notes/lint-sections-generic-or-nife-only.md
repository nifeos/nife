# Which script/lint sections a crate repository would inherit

*Name: provisional, minted 2026-11-03 (UTC) by lane/lint-section-tagging. Naming is calef's.*

`script/lint` holds 131 sections, as `helpers/lint-no-cargo.awk` splits it. This note tags each one
by whether a repository with no roadmap, no decisions and no kernel could run it. It then names
what couples the portable sections to nife, sketches the interface a shared tool would need, and
recommends where and when to build one. No code moves here.

What drives the question: milestone 887 (crate repositories share one CI template, adapted from
nife's gates) put a gate set in `nifeos/crate-workflows`, and jh7110_entropy runs it. Milestone 756
(`process-tools`, `core-tools` and `disk-tools` are the first code to leave this repository) will
add repositories that need the same gates. Nothing builds the shared tool yet.

Measured at nife `62e40c14e`, crate-workflows `d73bd26` and jh7110_entropy `f2af359`. Seconds
come from the `lint-section-time` and `lint-section-cpu` lines of the `lint` job in merge-group run
41694693300 (2026-11-03 UTC), where `script/lint` took 313 seconds of a 379-second job.

## The answer in counts

| tag | sections | wall seconds, summed | CPU seconds, summed |
|---|---|---|---|
| generic | 12 | 14 | 17.0 |
| generic-with-config | 36 | 122 | 221.3 |
| nife-only | 83 | 173 | 300.0 |

The tags mean this:

- *generic*: the section runs unchanged on any Rust repository in git, with the usual exclusions
  (`vendor/`, `target/`, `patches/`).
- *generic-with-config*: the mechanism ports, but a second repository must supply a value nife
  hard-codes, such as a baseline path, a ceiling, a directory list or a marker word.
- *nife-only*: the section reads the kernel, the roadmap, the decisions, nife's own scripts or a
  table of nife files, and means nothing elsewhere.

Sections run side by side, so the summed wall seconds overlap and exceed no real run. Two clippy
sections (11 and 16) carry 20 of the 122 portable wall seconds; the rest of the portable set costs
seconds.

The full rows are in three appendices, one per tag:
[generic](lint-sections-generic-or-nife-only/generic.md),
[generic-with-config](lint-sections-generic-or-nife-only/generic-with-configuration.md) and
[nife-only](lint-sections-generic-or-nife-only/nife-only.md). Each row gives the section's header,
what it checks, what it reads, the config a second repository would supply, and its seconds.

## What a crate repository already inherits

The question assumes nothing has moved. That premise is false. Milestone 887 already adapted 18 of
the 48 portable sections into crate-workflows, and jh7110_entropy runs them through
`check.yml` and `proofs.yml`. It did so by copying or rewriting each one:

| nife helper | crate-workflows helper | relation |
|---|---|---|
| `helpers/empty-diff-check.sh` | `helpers/empty-diff-check.sh` | byte-identical copy |
| `helpers/scope-merge-base.sh` | `helpers/scope-merge-base.sh` | byte-identical copy |
| `helpers/job-budget.py` | `helpers/job_budget.py` | near copy, 55 lines differ |
| `helpers/kani-lint-shim/` | `helpers/kani-lint-shim/` | copy, comments diverged |
| `helpers/file_length_ratchet.py`, 383 lines | `helpers/file_length.py`, 48 lines | rewrite, no baseline |
| `helpers/comment_block_ratchet.py`, 1,217 lines | `helpers/comment_blocks.py`, 73 lines | rewrite |
| `helpers/orphan_rust.py`, 354 lines | `helpers/orphan_modules.py`, 124 lines | rewrite |
| `helpers/prose_ratchet.py`, 1,705 lines | `helpers/prose_caps.py`, 136 lines | rewrite, README and CHANGELOG only |

Four more sections live inline as shell in both trees: 44 (actions pinned to a SHA), 76 (tracked
files the tree ignores), and 127 and 128 (shellcheck and `sh -n`). The em-dash grep appears too,
but only in the template's own `script/lint`, not for callers. The remaining six of the 18 are
clippy, rustdoc, cargo-machete, markdown links, the falsification form check and the
`.kani-version` pin.

So the live question is not whether to extract. It is whether two implementations of one rule
should keep drifting apart, or become one.

A crate repository does not yet run ten portable sections. Four read Rust: 32 (unsafe fn
contracts), 40 (fence pairs), 41 (module-wide dead-code suppression) and 98 (unit symbols). Three
read workflows and forms: 78 (steps that swallow a failure), 79 (every workflow file parses) and 81
(issue forms). Three read any text: 43 (conflict markers), 96 (the history ratchet) and 99
(line-number citations). jh7110_entropy passes the greps among
them today, and nothing holds it there.

## Hidden couplings

Each of these makes a generic-looking section harder to lift than its header suggests.

1. *Two root conventions, and one fails open.* `orphan_rust.py`, `workflow_swallows.py`,
   `workflow_parses.py` and `issue_forms.py` find the tree from their own file's path.
   The others use the working directory's `git rev-parse --show-toplevel`. Run from a scratch
   clone of jh7110_entropy, `orphan_rust.py` judged nife's 1,703 files in 143 manifests and
   passed; `workflow_swallows.py` judged nife's 31 workflows. A shared tool must take the
   repository root as an input and never infer it from its own location.
2. *A ratchet that finds nothing passes.* From jh7110_entropy, `prose_ratchet.py --check`
   reported "0 documents" and exited 0, because `PROSE_DIRS` names `design/` and `notes/`.
   `lint_dtb_matches_dts.py` takes the opposite stance and fails on zero pairs, as does
   `dead_code_suppression.py` (exit 3) on zero files. One rule must hold across the set: a section
   that judges nothing fails unless the repository's config says so.
3. *Baselines under `design/`.* Six ratchets keep their lists there: `file-length-baseline.tsv`,
   `comment-block-baseline.tsv`, `lint-baseline.tsv`, `parent-index-baseline.tsv`,
   `prose-baseline.tsv` and `passive-baseline.tsv`. A missing baseline fails the file-length and
   comment-block ratchets outright. A crate repository has no `design/` and starts under every
   cap, so a missing baseline should mean "empty".
4. *Ceilings that count nife.* `line_citations.py`'s `CODE_CEILING` (712),
   `history_ratchet.py`'s `CEILING` (42) and `observed_failing.py`'s `CUTOFF_COMMIT` (a nife
   commit) all hold nife's history. `unit_symbols.py` and `cap_abbreviation.py` carry `KEPT`
   tables of nife file paths.
5. *Constants copied by hand.* The 2,000-line cap lives as `CEILING` in `file_length_ratchet.py`
   and as `DEFAULT_LIMIT` in crate-workflows' `file_length.py`. The 40-line comment cap and the
   prose limits (3,000 words, median 20, longest 40) also appear in both repositories. Nothing
   checks that the copies agree.
6. *A generic helper imports a nife-only one.* `unit_symbols.py` imports `cap_abbreviation.py` as
   its lexer, and `name_checks.py` (nife-only) imports it too. `prose_ratchet.py` imports
   `roadmap_block.py`, the roadmap parser that `script/roadmap`, `script/citations` and
   `script/journeys` share. `exception_markers.py` imports three ratchets at once. Extracting the
   prose or unit checks drags these along unless the lexer and the block parser split out first.
7. *A section's header is not its content.* The `shellcheck` section holds only comments; its
   commands run under `spelling`, so that section's 10 seconds include shellcheck. Section 2 runs
   the lint runner's own selftests. Section 106 also runs the `interface_stability.py` and
   `mutation_inflow.py` selftests, and section 120 runs three timing selftests. A tool that
   selects by section would select the wrong code.
8. *The clippy ratchet hard-codes nife.* `lint_ratchet.py`'s `CONFIGURATIONS` names `kernel`,
   `components` and the bare-metal triples. It reads diagnostics that `clippy_record.py` saved
   under `target/lint-clippy`, and its section waits for nine clippy lanes.
9. *Sections find helpers relative to the script.* Many write `"$(dirname "$0")/../helpers"`, and
   the preamble runs `cd "$(dirname "$0")/.."`. Once the tool and the repository are two
   checkouts, every such line has to change.
10. *The runner is portable and the most valuable piece.* `helpers/lint-sections.sh`,
    `helpers/lint-no-cargo.awk` and their markers (`# lint-quick`, `# lint-lane:`,
    `# lint-waits-for:`) run every section, keep going past a failure and time each one. The
    pre-push hook's skip marker keys on `script/lint` itself.
11. *Tools the job installs.* The lint job installs `shellcheck`, `dtc`, `typos`, `cargo-machete`
    and ruby (for `workflow_parses.py`), and the preamble demands Python 3.11 for `tomllib`. A
    shared tool inherits that list.

## BUGS

Two defects turned up on the way. Neither changes the recommendation.

- `script/lint --list` is not a flag. The preamble matches only the modes it knows and falls
  through to the full run, so `--list` started every cargo section. An unknown flag should exit 2.
- The crate-workflows README's gate table says `typos` runs in `check / standalone`.
  `check.yml` on its main has no `typos` step, so jh7110_entropy runs no spelling check.

## The interface a shared tool needs

- *Inputs.* The repository root (the working directory), the tool's own checkout, and a base ref
  for the commit-range sections (default `origin/main`).
- *One config file* at the repository root, `lint.toml` (provisional). It lists the enabled
  sections and, per section, its values: exclusions, baseline path, ceiling, marker word, script
  globs, prose paths.
- *Selection by explicit list.* A repository enables sections by id; nothing runs by default,
  because of coupling 2. `--quick` and `--list` read the same list.
- *Exit codes.* 0 when every enabled section passes; 1 when one finds something or judges nothing
  unexpectedly; 2 for a usage or config error, such as an unknown flag or section id.
- *Output.* The `==> ` header and the `lint-section-time:` and `lint-section-cpu:` lines nife's
  runner prints, so `script/ci-log-baseline` and the timing table keep working.
- *Versions.* A caller pins the tool by commit SHA and moves with Dependabot, as it already pins
  `check.yml`. The template's lag check compares that pin with the newest release tag.
  jh7110_entropy pins `55978d1`, sixteen commits past `v7` and on no tag. The lag check passes it,
  since the pin descends from the newest release, and a shared lint pin would inherit that.
- *nife as a caller.* nife runs the shared sections from a pinned checkout, like any caller, and
  keeps its 83 nife-only sections in `script/lint`. One source per rule is the point.

## Where it belongs

crate-workflows is the better home, for now.

- Callers already check it out at `.crate-workflows` in every job, and its lag check, release tags
  and Dependabot flow exist and work. A new repository would give each crate a second pin and a
  second lag check.
- The copies and rewrites already sit there, so converging them is one repository's change.
- Against it: its README calls it "one CI template for every crate repository", and nife and
  basalt are not crates. nife would pin a repository whose releases also carry the publish path.

A separate repository wins once a caller that is not a Rust crate needs the set, or once the
template's release cadence starts holding back a lint fix. Neither holds today. Keeping the tool
in one directory of crate-workflows (`lint/`, provisional) makes that later move a directory move.

## Prior art

These four come from memory, not from reading their sources this session.

- *Super-Linter* bundles many third-party linters in one container; a repository picks them with
  `VALIDATE_*` variables and pins the image tag. nife's sections are project rules, not
  third-party linters, so only the selection model carries over.
- *pre-commit* lists hook repositories in `.pre-commit-config.yaml`, each at a pinned `rev`,
  with hook ids that select checks; `pre-commit autoupdate` moves the pins. This is the closest
  match to the interface above.
- *Rust's tidy* (`src/tools/tidy`) runs rust-lang/rust's repository rules as one tool, and other
  rust-lang repositories do not share it. That matches nife today.
- *Kubernetes `hack/verify-*.sh`* runs one script per check under `make verify`. Its sub-projects
  each carry their own `hack/`, with shared pieces gathered over time in `kubernetes/repo-infra`.
  The drift between copies is the cost nife has started to pay.

## Example: jh7110_entropy running the shared set

The crate keeps its short `ci.yml`. The template's `standalone` job gains one step,
`python3 .crate-workflows/lint/run --config lint.toml`, and the crate adds this file:

```toml
# lint.toml at the jh7110_entropy root; ids and keys provisional.
enable = [
  "conflict-markers", "actions-pinned", "tracked-ignored", "unsafe-fn-contracts",
  "dead-code-suppression", "workflow-swallows", "workflow-parses", "orphan-rust",
  "file-length", "comment-blocks", "prose", "em-dashes", "unit-symbols",
  "line-citations", "history", "typos", "shell",
]

[prose]
paths = ["README.md", "CHANGELOG.md", "docs/"]

[file-length]
ceiling = 2000   # no baseline file: the crate starts under the cap
```

Those 17 sections cost 39 wall seconds summed in nife's CI over the whole nife tree. jh7110_entropy
has two Rust files, seven workflows and eight Markdown files, so the run should take a few seconds.
Its first run would add nine checks it lacks today, plus the `typos` check its template's README
promises. It would also replace four rewrites (orphan modules, file length, comment blocks, prose
caps) with nife's helpers.

## Recommendation

Extract after the first split settles, and start the convergence now. Milestone 756 has not
started, and the shapes of `process-tools`, `core-tools` and `disk-tools` will show which config
keys a second kind of caller needs. Building the full tool before then guesses at that interface.
But the copies in crate-workflows already drift, and each new crate repository inherits the
drift.

The first step is small: fix couplings 1 and 2 in nife's helpers. Every portable helper takes the
repository root from the working directory's git top level, and every one fails when it judges
nothing. That costs no new repository and no interface decision, and it lets crate-workflows call
nife's helpers instead of its rewrites.

## Proposed milestone (number for the integrator)

*Portable lint helpers judge the repository they run in.* Every helper behind a generic or
generic-with-config section reads the root from the working directory's git top level. It treats a
missing baseline as empty and takes its ceiling and exclusions as arguments, with nife's values as
defaults. It fails on an empty subject unless told otherwise. A selftest runs each helper from a
scratch repository and proves it judges that repository and not nife. `cap_abbreviation.py`'s
lexer and `roadmap_block.py`'s block parser split into modules a generic helper may import. What
drives it: crate-workflows can then call these helpers at a pinned nife commit instead of carrying
its four rewrites. The lane that takes it also makes an unknown `script/lint` flag exit 2, and
hands the `typos` gap to crate-workflows' own queue.
