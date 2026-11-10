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

## The site build, step 3

Measured 2026-11-04 (UTC) with `script/site --install` on a Linux host with 80 CPUs, at this
branch over `origin/main` `d669bd9a9`. Eleventy 3.1.7, Pagefind 1.5.2, Node 24.22.1. Each run of
`.github/workflows/record-site.yml` writes the same rows for a hosted runner to its step summary.

| what | value |
|---|---|
| source | 2,895 Markdown files: `design/`, `notes/`, `briefs/`, `AGENTS.md`, `ARCHITECTS.md` |
| words | 3,670,379 |
| pages built | 2,896, with the index page |
| `npm ci --ignore-scripts` | 125 packages in 3 s; the lockfile holds 131, six of them other platforms' Pagefind binaries |
| Eleventy build | 8.5 s |
| Pagefind index | 12.3 s |
| the whole run | 26 s wall, 1.4 GB peak resident memory |
| search index | 19.9 MB in 3,174 files |
| bytes a reader's first search fetches | 178 KB |
| whole site | 50.3 MB in 6,144 files, 73 of them repository files a page links to |
| citations linked (`milestone N`, `§N`) | 21,475, none naming a missing record |
| bare `#N` linked to `nifeos/nife` | 3,023 |
| internal links checked in the built pages | 30,777 |
| in-record breaks (no file, no page, no heading) | 0 |
| off-record links, shown as plain text | 67 |

Zero breaks is the expected reading, because `script/lint` already fails a broken relative link
or an unresolved citation in the record. The counts fire: a planted link to a missing file, two
fragments naming no heading and two citations of missing records gave 3 breaks and 2 unresolved
citations, each with its example.

The 67 off-record links point at paths the site does not carry: `bench/` (16), `system_tests/`
(14), `crates/` (12), a note's data directory with no `README.md` (8), and the rest at `c_library/`,
`xtask/`, `fuzz/`, `patches/`, `kernel/`, `components/` and `vendor/`. Step 4 sorts each.

## What each lint section reads, step 1

Measured 2026-11-09 (UTC) on this branch over `origin/main` `412c3ce58`, on a Linux host. Each of
the 118 sections that build nothing ran alone under `strace -f -e trace=%file,getdents64`, and each
tracked path it opened was sorted as record or code. The 22 clippy, build and rustdoc sections
were sorted by reading them: they run cargo on the workspaces, and no build script opens a record
path. Two sections exited non-zero under the tracer, the lint ratchet (it reads the clippy
sections' output, which no clippy section had written) and the CI fuzz selftest, so the files they
opened before failing are what sorted them. The tag is the `# lint-scope:` line in each section,
and `helpers/lint-section-index.tsv` carries it as its seventh column.

| tag | sections | what it means |
|---|---|---|
| `record` | 8 | opens record documents and no code |
| `both` | 35 | opens record documents and code, or every tracked file |
| `code` | 97 | opens no record document |

Two kinds of reading count as code. A walk of every `.rs` file opens the 28 Rust files under
`notes/` too (probes and measurement crates). A ratchet whose one record-tree file is its own
baseline under `design/` reads that baseline, which the list above keeps with code.

The `record` sections: journeys, numbered record files, the audit index, the stranger test's run
record, the outsider-pass selftests, and three mutation-record checks that read only
`notes/project-metrics/`. The `both` sections fall into three kinds:

- The record's own gates that also read code: roadmap status (its path claims), decisions,
  citations and its two siblings, fatal risks, counted claims, the naming checks, the prose
  ratchet, the module-block ratchet, the history ratchet, line citations, exception markers.
- Checks of code that read a record file as data: script docs (`notes/scripts.md`), the unsafe
  listing (`notes/kernel-dependency-unsafe/sites.tsv`), package boundaries, the removed-files
  index, three mutation checks, the host-pass wiring, the first cut's declarations.
- Checks of every tracked file: conflict markers, TODO markers, environment names, tracked files
  the tree ignores, markdown links, house style, em-dashes, halves, spelling, the Kani install
  pin, and the commit messages the branch adds.

## The record's checks by directory, step 5

Measured 2026-11-09 (UTC) from the runs named below, all on hosted runners. `nifeos/record` does
not exist yet, so this repository stands in for it: a record path here plays a record path there,
and a code path plays a path in a code repository. The record paths are `design/`, `notes/`, `briefs/`
and the root's `AGENTS.md`, `ARCHITECTS.md` and `CLAUDE.md`.

### The hold check

`.github/workflows/architect-hold.yml` has no path condition and no checkout. It resolves the pull
request number and reads its labels through the API, on `pull_request` and on `merge_group`.

| what | value |
|---|---|
| record-only pull request #3383 (two roadmap files) | ran twice on `pull_request`, 4 seconds each |
| its merge-group entry, run 42686419143 | queued 4 seconds, ran 5 seconds, run 10 seconds |
| the code entry ahead of it, #3385, run 42685696857 | queued 5 seconds, ran 4 seconds, run 9 seconds |
| 23 successful `merge_group` runs of 2026-11-09 | median 10 seconds, 90th percentile 14, maximum 53 |
| 74 successful `pull_request` runs of 2026-11-09 | median 10 seconds, 90th percentile 21, maximum 77 |
| billable time, both runs above | 0 milliseconds (a public repository) |

A job skipped by its condition satisfies a required check here: on #3383's head `77eca4bec`, 18
required checks, `test`, `bench` and `boot-check` among them, reported `skipped`, and the pull
request entered the queue and merged. So the hold could be limited by a job-level condition.
It reads no file, though, and the label it reads applies to any change, so a limit would lift the
hold from every change outside the record. The condition holds without limiting it.

### What lint runs

The `lint` job in `ci.yml` has no path condition, and `script/lint` runs all 140 sections, one at a
time on the hosted four-core runner. Section seconds come from each run's `lint-section-time`
lines, summed by the tag.

| run | `record` | `both` | `code` | all |
|---|---|---|---|---|
| #3383, record only, pull request run 42683721051 | 3 | 77 | 374 | 454 |
| #3383's merge-group entry, run 42686419342 | 4 | 55 | 324 | 383 |
| #3385, code only, merge-group entry run 42685697323 | 3 | 40 | 275 | 318 |

A record-only change needs the 43 `record` and `both` sections: 80 of 454 seconds on the pull
request run. A change to code alone needs the 132 `code` and `both` sections, 451 seconds, and a
record section's own tool counts as code. The job spends more than the sections: on the pull
request run it took 654 seconds, of which 111 installed `typos` and `cargo-machete`. Run here one
at a time with `script/lint --section`, the 43 passed alone in 147 seconds, each paying its own
preamble.

In `nifeos/record` the 35 `both` sections lose their code half. Each either fetches the code at
basalt's pins, as the path claims already plan to, or drops that half. The tree-wide hygiene checks
among them would read the record's own tooling too; on the pull request run they cost about 18
seconds.

### A queue batch with a record change and a code change

Measured, from runs, and checked against the ruleset. Ruleset 19596094's merge queue builds up to
five entries and merges up to five at once (`ALLGREEN`, `MERGE`). Each entry gets its own branch,
`gh-readonly-queue/main/pr-<N>-<parent>`, holding the pull request merged on top of the entries
ahead. Its `merge_group.base_sha` is that parent: four entries sampled (#3383, #3384, #3380,
#3386) each logged a base equal to the commit in their branch name, the entry ahead or `main`'s
tip.

#3385 (code: `vendor/upstream-status.md`) and #3383 (record: two roadmap files) landed in one
batch, #3383 behind #3385.

| entry | what `helpers/prose_only.py merge-group` classified | heavy jobs | `lint` job |
|---|---|---|---|
| #3385, run 42685697323 | 1 file over `7cd9b3c39`: not prose | 27 ran, 9,850 seconds in all, the longest 903 | 448 seconds |
| #3383, run 42686419342 | 2 files over `ddc37dab3`, #3385's entry: prose | 27 skipped their steps, 8 to 19 seconds each | 558 seconds |

So in this queue a record entry and a code entry each run what their own diff asks. The record
entry still holds a runner per heavy job for its skip step, 364 seconds in all, and runs the whole
lint. One pull request that carries both kinds of change runs everything, as it
should.

### What this leaves unverified

- The runs come from one day and one queue configuration. A ruleset that rebased instead of
  merging, or GitHub changing the base it reports, would change the batch result.
- No run measured a change in the website's repository, which does not exist yet.
- The tags record what each section opened on this tree. A section that reads a path only when it
  appears (a new file kind, a failure branch) can read more than it showed here.

## Where each script goes

Sorted 2026-11-10 (UTC) by reading each script, helper, lint section and workflow at `8e597457a`.
The sort is provisional. **R** goes to `nifeos/record`. **W** goes to `nifeos/nifeos.org`, the
website. **C** stays with code. **S-each** runs in every repository on its own files. **S-cross**
splits, and one side needs the other's data: code at basalt's pins, or the record's number index
that open question 7 asks about. **T** goes to the template (`nifeos/crate-workflows`) or the fleet
repository, under milestone 902's forks.

| script | reads | goes to |
|---|---|---|
| `script/roadmap` | blocks, proposals, decision statuses, CoE actions; code paths in path claims; merges for the merged-branch check | S-cross: the core to R; path claims need code at basalt's pins and the anchor; `--ready-branch` needs the block while it runs on a code branch |
| `script/journeys` | journeys, through `helpers/roadmap_block.py` | R |
| `script/citations` | every tracked file; `helpers/lint-section-index.tsv` | S-cross: the record half to R; the code half needs the number index |
| `script/decisions` | the decision schema; `§N` across the tree | R core; the citation sweep is S-cross |
| `script/site` | `site/`, and the record it builds | W |
| `script/claim` | no record file at run time | T (fleet); it runs in every repository |
| `script/record-ruling` | labels and comments only | T (template) |

The helpers follow their callers. `helpers/roadmap_block.py` belongs to R, and the code and fleet
readers (`script/metrics`, `prose_ratchet.py`, `consumer_census.py`, `milestone-branch-block.sh`)
need a pinned copy or stop reading blocks. `roadmap_proposals.py`, `coe_actions.py` and
`roadmap_migrate.py` go to R. `branch-name-check.sh`, `smelter-token` and `stash-block.sh` go to T.
`helpers/milestone-branch-block.sh` reads the block on the lane's own branch, so it goes to T with
the rework that moves the status flip to the record's pull request.

Of the 43 lint sections tagged `record` or `both`, 5 go to R, 14 stay with code, 12 are S-each and
12 are S-cross. The three mutation sections tagged `record` stay with code, since they read only
`notes/project-metrics/`, which is tool data. The 97 `code` sections stay with code, 9 of them
(hooks, push, empty diff, the label merge base, and the template gates) in T. `nifeos/record` needs
a lint driver of its own that runs only its subset.

| workflow | goes to |
|---|---|
| `ci.yml` job `lint` | S: R runs the 5 record sections and the record halves of the 24 S sections |
| `record-site.yml` | W |
| `audit-cadence.yml` | R |
| `ready-status.yml`, `script/lint-fleet` 4b | T, reworked for lanes across two repositories |
| `architect-label.yml`, `coe-architect-label.yml` | T; the decision rule fires in R |
| `architect-hold.yml` | every repository |
| `metrics.yml`, the mutation and falsification workflows | C; their output leaves `notes/` with the data |
| `stranger-cadence.yml` | unknown: its run record is in `notes/`, and the test builds the OS |
