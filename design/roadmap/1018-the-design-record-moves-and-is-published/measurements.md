# Measurements for the design record's move

Measured 2026-11-01 (UTC) at `origin/main` `295782808`, in a lane worktree. Counts only; the
commands below reproduce each one. "The record" means `design/`, `notes/`, `briefs/`, `AGENTS.md`,
`ARCHITECTS.md`, `CLAUDE.md` and `.claude/`. "Code" means every other tracked path.

## Size

| what | count |
|---|---|
| tracked files under `design/` | 1,686 |
| tracked files under `notes/` | 1,351, of which 1,148 Markdown |
| Markdown files in roadmap, decisions and journeys | 1,584 |
| words in the record's Markdown (`wc -w`) | 3,585,633 |
| disk size | `design/` 17 MB, `notes/` 22 MB |
| image and table files in `design/` and `notes/` | 118 |

## Tools that read the record by path

Searched `script/`, `helpers/`, `xtask/`, `tools/`, `jig/` and `.github/` (442 tracked files) for
`design/`, `notes/`, `AGENTS.md` or `skills/`. A hit counts as a read when the line is not a comment
and the path sits in a string or a path expression. Joined path components (`"design"`, `"notes"`)
added no new files.

| scope | files |
|---|---|
| any record path, outside comments | 138 |
| by directory: `helpers/` 81, `script/` 29, `.github/` 14, `xtask/` 12, `tools/` 2 | |
| naming `design/roadmap` | 43 |
| naming `design/decisions` | 17 |
| naming `notes/` | 109 |
| naming `AGENTS.md` | 13 |
| naming `skills/` | 7 |
| naming `journeys` in any form | 5 |
| workflows that commit into the record (`git add` of `notes/` or `design/`) | 5 |

The heuristic over-counts a path quoted in a help message, and under-counts a path built from a
variable.

`script/lint` has 128 sections, split at each `echo "==> ` line with the mode banners left out. 56
sections name a record path or the words `roadmap`, `decisions` or `journeys` in their own body. 34
more call a script or helper from the 138 above. 38 do neither.

## Citations into the record

| where | `milestone N` | `§N` | record paths |
|---|---|---|---|
| code, all files | 7,774 in 1,772 files | 3,525 in 978 files | 1,062 in 257 files |
| Rust only | 4,813 in 1,117 files | 2,641 in 735 files | 114 in 89 files |
| the record itself | 13,559 | 8,574 | |

1,972 of 3,425 tracked code files carry at least one of the three. No code file names a journey by
number.

## Citations from the record into code

A code path here is a backticked or bare token beginning with `crates/`, `kernel/`, `components/`,
`script/`, `helpers/`, `xtask/`, `tools/`, `system_tests/`, `uefi_loader/`, `packages/`, `fuzz/`,
`jig/` or `bench/`.

| where | files naming code | occurrences |
|---|---|---|
| roadmap, including appendices | 963 of 1,203 | 7,845 |
| top-level roadmap blocks | 812 of 952 | |
| decisions | 239 | 1,119 |
| notes | 961 | |

Other references that change meaning when the record leaves this repository:

| what | count |
|---|---|
| bare `#N` pull request references in the record | 2,866 in 803 files |
| short commit hashes (9 to 12 hex digits) in the record's Markdown | 1,028, loosely matched |
| `github.com/nifeos/nife/blob` or `tree` links in the record | 36 |
| `nifeos/<repo>@<sha>` citations anywhere | 1 |
| record paths naming `crates/jh7110_entropy`, which left on 2026-10-31 | 32 |

## History

`git log --no-merges --name-only origin/main`, each commit sorted by the paths it touched.

| commits | count |
|---|---|
| non-merge, all | 11,661 |
| touching only the record | 4,134 |
| touching record and code | 2,729 |
| touching only code | 3,299 |
| touching no file (the claim commits) | 1,499 |
| first commit | 2026-07-12 |

## What reads the record and stays with code

These record-tree files hold tool data rather than prose, so they leave with the tool that reads
them: the eight lint and prose baselines under `design/` (`lint-baseline.tsv`,
`prose-baseline.tsv`, `passive-baseline.tsv`, `file-length-baseline.tsv`,
`comment-block-baseline.tsv`, `parent-index-baseline.tsv`, `module-block-baseline.txt`,
`naming-rule-baseline.txt`), and `design/removed.tsv`, which lists code paths.
