# Step 6 plan: the record leaves for `nifeos/record`

An appendix to milestone 1018 (the design record moves to `nifeos/record` and is published as a
website), for its step 6 and the part of step 7 that step 6 needs. Measured 2026-11-23 (UTC) at
`origin/main` `96273a1de`. This page plans; it creates no repository and removes no path.
[The inventory](step-6-inventory.tsv) lists, one row each, every file and lint section that reads
a moving path and what it becomes.

## What moves

`packages/homes.toml` names 3,380 tracked files whose home is `record`: 2,118 under `design/` and
1,262 under `notes/`. The record's own tooling goes with them, with its
history: `script/roadmap`, `script/decisions`, `script/journeys`, `script/citations`,
`helpers/roadmap_block.py`, `helpers/roadmap_proposals.py`, `helpers/coe_actions.py` and
`helpers/roadmap_migrate.py`. That makes 3,388 files. The lane skills under `notes/skills/` and the
briefs under `briefs/` stay in nife: they are agent instructions, not part of the documentation, so
nife vendors nothing and the skill symlinks stay as they are. Under `design/` and `notes/`, 167
other files stay: the nine baselines and `design/removed.tsv`, the tool data under `notes/project-metrics/`,
`notes/mutation-testing/` and `notes/kernel-dependency-unsafe/`, `notes/scripts.md`, and the notes
about one crate alone. `AGENTS.md`, `CLAUDE.md` and `ARCHITECTS.md` stay, since each repository
carries its own.

## 1. What reads a moving path

140 tracked files outside the record name a moving path in code, not only in a comment. By place:
69 under `helpers/`, 30 under `script/`, 17 under `xtask/`, 8 workflows, 4 issue and pull request templates,
6 under `site/`, 2 under `tools/`, 2 under `.cargo/`, `Cargo.toml` and `deny.toml`. 69 of them
name a record file only in a message, such as a help line or an error that cites a note; those
become `nifeos/record:<path>` in the mention sweep. 45 of `script/lint`'s 154 sections carry the
`record` or `both` tag. The 14 symlinks under `.claude/skills/` and `.agents/skills/` point at
`notes/skills/`, which stays, so they do not change. `.githooks/` reads no record path.

The readers whose behavior changes:

| reader | what it reads | what it becomes |
|---|---|---|
| `script/roadmap`, `script/decisions`, `script/journeys`, `script/citations` | blocks, decisions, journeys, citations | move with history; nife keeps the code half of `script/citations`, which checks a citation by file existence at the record pin |
| `ready-status.yml`, `script/lint-fleet` 4b (`helpers/milestone-branch-block.sh`) | the block on the lane's own branch | stay; the status flip moves to the record's pull request, provisionally under question 6 of the milestone |
| `script/claim` | no record file; help text names `notes/debt-paydown.md` | stays; the help text takes the qualified form |
| `helpers/architect-label-rules.py`, `coe-architect-label.yml` | changes under `design/decisions/`, `notes/coes/` | the decision rule fires in the record; nife keeps the code rules |
| `helpers/prose_only.py`, `helpers/name_*`, `helpers/history_ratchet.py`, `helpers/prose_ratchet.py` | record paths as a class, the naming rules | the record copies its half; nife reads the naming rules at the record pin |
| `script/metrics`, `helpers/consumer_census.py`, `helpers/interface_stability.py` | block and decision counts | stay; read the record at its pin |
| `helpers/agents-md-split-check.py` | `notes/skills/`, `design/tenets/` | stays; reads the skills in place and the tenets at the pin |
| `xtask/src/manual.rs` and `xtask/src/disk.rs` | seven notes installed in the disk image | stay; the seven notes re-sort as package documentation that stays |
| `metrics.yml` | regenerates lines of `notes/project-metrics.md` | stays; that page re-sorts to tool data, so the bot writes one repository |
| `script/stranger-test`, `stranger-cadence.yml` | the run record under `notes/stranger-test/` | stay, since the test builds the system; the run record lands in the record by pull request |
| `record-site.yml`, `site/`, `script/site` | the whole record | leave nife in the removal pull request; step 8 builds in `nifeos/nifeos.org` |
| `script/lint` | 9 `record` sections, 36 `both` | the record's driver runs the 9 and its half of the 36; nife keeps the rest |

Two findings change the order of work, and a third holds already.

- `xtask/src/manual.rs` bundles `notes/documentation.md`, `pipes.md`, `line-discipline.md`,
  `ipc-naming.md`, `stack.md`, `capabilities.md` and `glob.md` into the image, and
  `xtask/src/disk.rs` calls it. The image build breaks the day those notes leave. They are the
  documentation of the programs that ship them, which §315 (a crate's documentation leaves with it) keeps with the code, so a
  `homes.toml` pull request re-sorts them before the cut.
- Baselines that stay with code hold rows for record files: 2,318 of 2,329 lines of
  `design/passive-baseline.tsv`, 297 of 9,711 of `design/module-block-baseline.txt`, 13 of 349 of
  `design/naming-rule-baseline.txt`, 14 of 31 of `design/removed.tsv`, and one line each in
  `.cargo/mutants-baseline.txt` and `helpers/kani-peaks.tsv`. The record's lint pull request
  writes those rows to the record's own baselines, and the removal pull request drops them from
  nife's.
- A qualified path resolves at the nife commit basalt pins, `edcbb2209` of 2026-11-23. Every one
  of the 319 rewritten link targets exists there, so the rewrite needs no pin bump first.

No code includes a record file with `include_str!`.

## 2. The extraction

The tool is `git filter-repo` 2.47.0, sha256
`60f7b584d1a8295b329e365af3046489a06d62ab22dd882ce54d53dee656d81f`, installed in a virtual
environment. Two path files come from `packages/homes.toml`, through the helper that pull request
2 adds. `keep` lists the 11 prefixes whose home is the record, the eight tool files and
`DECISIONS.md`, which held the decisions before the split into one file each. `drop` lists the 37
prefixes under the record's directories whose home is elsewhere, `notes/skills/` among them. A
third input maps every address calef has committed under to his GitHub no-reply address. It lists
the old addresses, so it lives off-tree with the maintainers, and the extraction writes the
mailmap from it into a temporary file that it deletes after the run. The sequence, from a clone
of `nifeos/nife` and the full hash of the anchor commit:

```sh
git clone --no-local --no-checkout "$NIFE" record
cd record
git fetch origin "$ANCHOR"
git update-ref refs/heads/main "$ANCHOR"
git symbolic-ref HEAD refs/heads/main
git remote remove origin
git for-each-ref --format='%(refname)' | grep -vx refs/heads/main | xargs -r -n1 git update-ref -d
git filter-repo --force --paths-from-file keep
git filter-repo --force --invert-paths --paths-from-file drop
git filter-repo --force --prune-empty always --prune-degenerate always \
  --mailmap "$MAILMAP" --refs refs/heads/main
rm "$MAILMAP"
git reset --hard
git gc --prune=now --aggressive
```

Every ref but `main` goes before the first pass, the `fuzz-*` tags among them, so no tag or
branch of nife reaches the new repository. The third pass drops the commits left empty: the claim
commits, which were empty in nife, and the merges left with one parent, so the record holds only
commits that changed it (decided 2026-11-23). The same pass rewrites calef's author and committer
addresses to `476570+calef@users.noreply.github.com` (decided 2026-11-23). The bot addresses and
`noreply@anthropic.com` keep their form. Commit messages keep their text, and no `Co-Authored-By`
or `Signed-off-by` trailer in the extracted history carries an old address.

The dry run, under the session scratchpad:

| measure | nife at the anchor | the extracted record |
|---|---|---|
| tracked files | | 3,388 |
| commits | 23,112 | 13,469 |
| of them merges | 6,527 | 4,676 |
| pack | 115 MiB | 39.6 MiB after `gc --aggressive` |

The file list under `design/` and `notes/` matches the 3,380 files `homes.toml` sends to the
record, one for one, and every file holds the same bytes as in nife at the anchor. After the
mapping, calef's commits carry only the no-reply address. Each pass takes about 20 seconds, and the
whole sequence about 95.

History survives. For five records, `git log --follow` gives the same commit count in the dry run
as in nife, and `git blame` gives the same author and date on every line:

| record | commits |
|---|---|
| `design/decisions/0201-one-roadmap-until-a-citation-has-to-cross.md` | 10 |
| `design/roadmap/1018-the-design-record-moves-and-is-published.md` | 25 |
| `design/naming.md`, across its move from `notes/` | 151 |
| `notes/crate-reviews/moho-abi.md`, across a rename | 7 |
| `design/tenets/three-principles.md` | 7 |

One rename crosses between the record's directories, `notes/naming.md` to `design/naming.md`, and
`--follow` crosses it. No record file arrived from a path outside them.

The extraction is reproducible. Run twice at the anchor, it gives the same head commit,
`56cd76e76`. A run at an earlier commit gives an ancestor of a later run's head, so a later run
extends an earlier one. Anyone holding nife, the path files, the address map and the tool version
can check the record's history. The first push fixes the path files and the map, since a
change to either changes every hash.

## 3. The pull requests, in order

Each lands alone with green checks. Until the removal pull request, every record file still lives
in nife and every lane works as today.

1. basalt: done. Its nife pin, `edcbb2209`, holds every cut link target.
2. nife: re-sort in `homes.toml` the seven bundled notes, `notes/project-metrics.md`,
   `notes/skills/` and `briefs/` to stay, and add `helpers/record_paths.py`, which writes `keep` and `drop` from `homes.toml` and checks
   that they cover the record exactly.
3. nife: rewrite 319 of the 351 cut links (section 4), and extend the qualified-path check from
   roadmap blocks to all record Markdown.
4. nife: add the record pin. `helpers/record_root.py` answers where the record is: the work
   tree while the record lives here, and later a fetched tree at the commit basalt's `[record]`
   table names. Every reader in section 1 that stays switches to it. A no-op today.
5. nife, several pull requests: rewrite code mentions of record paths to
   `nifeos/record:<path>`, 4,468 exact mentions and 307 directory mentions in 1,267 files. Logs,
   patches and `vendor/` keep their text, since each records what it was. One pull request per
   top-level directory keeps each reviewable.
6. nife: freeze. A lint section refuses any change to a moving path, except in the removal
   pull request. Lanes with record edits keep their drafts and land them in the record afterward.
7. nifeos/record: create it, public, and push `main` from the extraction at the frozen tip.
   Set the ruleset for `main` before the first push, so nothing lands unchecked.
8. nifeos/record: its own `AGENTS.md`, which points at the lane skills as
   `nifeos/nife:notes/skills/<name>/SKILL.md`, and a copy of `script/claim` with a check that keeps
   it identical to nife's copy (decided provisionally 2026-11-23, until the fleet repository the
   tooling ruling names exists). A lint driver runs the 9 record sections and the record halves of the 36, against the record
   rows of the baselines, and a CI workflow runs it on pull requests and the merge queue. This is
   the minimum of step 7.
9. basalt: add a `[record]` table to `pins.toml`, pinned at the record's `main`, and a bump
    workflow on each record merge. `gate.yml` builds only `[nife]`, so the record pin builds
    nothing.
10. nife: the removal pull request. It first checks that the record's tree at basalt's pin equals
    nife's moving paths at its own base. Then it removes those paths and writes
    `packages/moved.toml` with the anchor. It drops the record lint sections and the record rows
    of the baselines, removes `record-site.yml`, `site/` and `script/site`, lifts the freeze, and
    leaves a README pointer. The 28 links from 11 skill and brief files into the record become
    URLs into `nifeos/record` in the same pull request, and the 32 chart embeds section 4 leaves
    take whatever form it settles.
11. basalt: rewrite the links in its `AGENTS.md` and `README.md` that point into nife's
    `design/`; its links into `notes/skills/` stay.

Between 7 and 10, the record exists and the freeze holds nife, so no record edit can land in
either place and vanish. The window lasts as long as pull requests 8 to 10 take, which should be
under a day. A record edit that lands in nife after the push and before the freeze would break the
equality check in 10, so the freeze lands first.

After 10, agents in a nife worktree find skills and briefs where they found them, since neither
moves. A lane started in the record reads them in nife, through the qualified paths in the
record's `AGENTS.md`.

## 4. The cut links

`helpers/record_links.py --list` names 351 links, from 191 record files, whose target stays. 123
go into tool data, 82 into the seven notes the image installs and 42 into notes about one crate
alone. 33 go into crates, tools and other code, 20 into root documents, 20 into `bench/`, 14 into
`system-tests/`, 12 into `briefs/` and 5 into `notes/skills/`.

The rewrite is mechanical. For each listed link, a one-time script replaces the Markdown link
with its text followed by ``(`nifeos/nife:<path>`)``, where `<path>` is the target resolved against the linking
file. When the text only repeats the target's path, file name or stem, the qualified path alone
replaces the link (provisional), which keeps a note under its word cap. A link to a directory
becomes plain text naming it, since the qualified form names files. A
target that has already left nife takes its crate's repository, `nifeos/<crate>:<path>`, which
resolves through basalt's pinned `Cargo.lock`. Anchors after `#` drop, because the form has none.

32 of the 351 are image embeds, not links: 31 in `notes/project-metrics.md` and one in
`notes/register-of-measures/which-model-wrote-it.md`, each drawing a chart from
`notes/project-metrics/`. `script/metrics` writes its week notes under each chart by matching that
embed, and text in its place would drop the chart and its notes. The embeds keep their form until
the removal pull request settles where the charts are served from (provisional). The rewrite
covers the other 319.

Three checks hold it:

- `record_links.py --list` prints only the 32 image embeds.
- `script/roadmap --check`, with the qualified-path check widened to every tracked Markdown file
  under `design/` and `notes/`, resolves each new path at basalt's pin.
- The rewrite script runs again on the result and changes nothing.

179 links from 56 staying files into the record (`README.md` 29, `notes/scripts.md` 28,
`AGENTS.md` 22, `CONTRIBUTING.md` 12) become URLs into `nifeos/record` in the removal pull request.
They are links readers follow, so a URL fits better than a qualified path.

## 5. Risks, and what cannot be undone

- A public push is permanent. Once `nifeos/record` holds history, clones, forks and caches keep
  it. Everything extracted is already public in `nifeos/nife`'s history, so the push exposes no new
  text. A scan of every blob and commit message of the extracted history for unit-specific and
  private content ran before this plan, and its results went to the maintainers off-tree.
- Commit identities. The first push fixes them. The path files, the prune choice and the address
  map all change every hash, and once a record cites a commit, changing them breaks the citation.
  Rulings settled all three before the push.
- The repository name. A ruling settles it, and a rename leaves GitHub redirects but breaks pins and
  qualified paths that spell it.
- A lost edit. A record change landing in nife between the push and the freeze would vanish at
  removal. The equality check in pull request 10 refuses that case.
- Lanes in flight. A draft holding a record edit conflicts with the removal. The freeze notice
  tells such lanes to move the edit to the record.
- The image build. It breaks if the seven bundled notes leave before their re-sort, which is
  why pull request 2 comes first.
- The removal itself. A revert in nife undoes it, and the record's history stays valid. Reverting
  after the record has moved on makes two diverging copies, so a revert also archives the record.

## 6. Questions for the architect

Rulings answer all four.

1. The lane skills stay in nife, and nife vendors nothing (decided 2026-11-23). The briefs stay
   with them (decided provisionally 2026-11-23).
2. calef's author and committer addresses map to his no-reply address in the extraction (decided
   2026-11-23).
3. The extraction prunes the commits it leaves empty (decided 2026-11-23).
4. The record carries a copy of `script/claim`, checked to stay identical to nife's copy, until the
   fleet repository exists (decided provisionally 2026-11-23).
