---
status: BUILT
promoted_from: bold-keys-to-frontmatter
raised: 2026-10-05
built: 2026-10-07
---
# 791. Bold that a script reads: where it is tracked, and which of it should become frontmatter

Asked for by calef on 2026-10-05 (UTC): "Can we keep track of where scripts track bolded keys and
consider that for frontmatter?" Title and slug are provisional; the two frontmatter keys were ratified by calef on 2026-10-05.

Built in three pull requests: #1702 (the tracking mechanism and this proposal), #1714 (the nine
files, both readers and the `_APPENDIX` tightening) and #1821 (the rest of the verdict table
below). *The ban is not a key*, below, qualifies the inventory.

Reuse: `helpers/prose_ratchet.py` already derives the exempt set from the parsers, and
`helpers/roadmap_block.py` already reads the roadmap's frontmatter. This proposal extends both and
adds no module.

## The finding for the bold sweep

The marker exemption covers every key a live document carries today. That was checked two ways:
by reading every parser in `script/`, `helpers/`, `xtask/` and `.github/`, and by running the
ratchet's own span reader over all 1,603 tracked Markdown files and looking for key-shaped bold that
it counts. None was found. Every `**Status:`, `**Built`, `**Gate`, `**Reuse` and tag-shaped span that
is counted turned out to be ordinary prose ("**Built 2026-07-29**, both ISAs" in an old decision).

The exemption is safe by accident in one place and blind in two others, and all three are now closed
or recorded below.

- `script/roadmap` builds its `RESTATED` pattern with `+`, so derivation skipped it. It matches the
  same three shapes as three derived patterns in `helpers/roadmap_block.py`, so nothing leaked.
  Change either side and spans would have started leaking.
- A bold written `\*{2}` or `[*]{2}` is not found by derivation, and neither is a reader in shell,
  awk, jq, Rust or a workflow. None exists today.
- Derivation also picks up the bans. `script/decisions` refuses a `**Status:` line and `RESTATED`
  refuses a restated status. Exempting a ban as if it were a key is not free; see *The ban is
  not a key*.

## Inventory: every reader of a bold key

"Live docs" counts tracked Markdown carrying the shape as a line opener today. "If the bold goes"
says what the reader does when it is missing.

| Reader (file:line) | Key shape | Live docs | If the bold goes |
|---|---|---|---|
| `helpers/roadmap_block.py:163-166` (read by `script/roadmap`, `metrics`, `catch-up`, `citations`) | `**Status: WORD**`, `**Built:** date`, `**Gate: X.**` | 0 (frontmatter wins) | Nothing for live docs. History blobs that `script/metrics` and `catch-up` walk still parse, which is why the prose reader is permanent. |
| `helpers/roadmap_block.py:68-69`, `catch-up:335` | same, as the fallback | 0 | Same. |
| `helpers/roadmap_block.py:169-171`, read by `script/roadmap:1334-1473` and `helpers/coe_actions.py` | `- **Recorded.**`, `**Done.**`, `**Refused.**`, `**Decision.**`, `**Proposed.**`, `**Outstanding.**`, `**None.**`, `**Milestone N.**`; Revisit: `**Condition.**`, `**Nothing.**`, `**Unstated.**` | 415 roadmap, 1 decision, 5 notes (about 1,400 spans) | `script/lint` fails loudly: a Follow-on section must open with a tag. The COE open-action count in the metrics row also reads these. |
| `script/fatal-risks:241` | `**The experiment:**` lead-in | 8 risk files, 3 roadmap, 1 journey, 1 note | Lint fails: a risk with no experiment lead-in is refused. |
| `script/fatal-risks:249`, `script/metrics:1534` | `**Experiment status: RUN, date.**` | 9 risk files | `script/fatal-risks` fails. `script/metrics` reads `None` for the color and, if the appendix frontmatter disagrees, aborts the weekly run. |
| `script/metrics:1544` | `**Status: WORD` (retired-risk fallback) | 0 | Old history only. |
| `script/fatal-risks:257` | `^**WORD` in the running-order table | 1 (README) | Lint fails. Table rows are never counted by the ratchet, so it is in `NOT_MARKERS`. |
| `script/roadmap:259` | `**Reuse:**` (optional: plain `Reuse:` also matches) | 12 | Nothing. The bold is not needed. |
| `script/roadmap:891`, `helpers/roadmap_proposals.py:41,44`, `helpers/roadmap_migrate.py:152,153,304` | Index-row `**Built:**`; proposal `**Status: PROPOSED d.**` and `**Gate:**`; one-shot migrator | 0 live | Migrator and old-proposal readers only. |
| `script/stranger-test:369` | `**A run is due every N days.**` | 1 note | Lint fails ("exactly one must match"). |
| `script/journeys` | none: no bold marker | 0 | Nothing. A journey's status is its frontmatter's `status` key, with `retired` beside it, and `script/journeys --check` holds the file to that schema. |
| `helpers/name_provenance.py:368,473` | `**Provisional name`; a bold `**Name:` header is a stray | 9 (briefs, notes) | Removing a stray `**Name:` bold clears a defect. Removing `**Provisional name` hides an ad hoc opener the gate wants to see. |
| `script/decisions:309`, `script/roadmap:666` | bans on `**Status:` and restated statuses | 0 | Not keys. |
| `script/citations:293,331`; `script/fatal-risks:387,393` | emphasis optional around a citation or a status word | many | Nothing. The pattern tolerates bold and does not need it. |
| `helpers/prose_ratchet.py:192` | `**Milestone N.**` and `**Proposed.**` folded when comparing a promotion | same as tags | Promotion diffs read as edits. |

Not readers of a document: `**Lane:**` in a pull request body is written by `script/claim` and
`helpers/ci-failing.sh` and read by no script. `script/vendor-watch` and `xtask/src/restamp.rs` write
bold into their own reports. The Rust crates that mention `"**"` are glob code. No reader exists in
`xtask/`, `.github/`, shell, awk or jq.

## The tracking mechanism, built in this lane

The ladder asks for the highest rung that fits. Rung 1 (make the wrong state unrepresentable) would be
a single registry both parsers and the ratchet import. `prose_ratchet.py`'s header already records why
that was refused on 2026-09-26: the parsers disagree about their own markers, and sharing a definition
means either changing what eight gates accept or re-homing each variant verbatim, which shares
nothing. That reasoning still holds, so derivation stays. The cheap, clearly right addition is rung 2:

- `opaque_readers` finds the readers derivation cannot see. By AST it finds any `re.*` call whose
  pattern mentions two literal stars but is not a bare literal. By text it finds the same in every
  non-Python file under `script/`, `helpers/`, `xtask/` and `.github/`.
- `selftest`, which `script/lint` runs, fails on any such reader not named in `OPAQUE_OK` with the
  reason its spans are already exempt. Today that is one entry, `RESTATED`. A new parser written in
  an unseen style now fails the gate on the day it lands rather than leaking quietly.
- `python3 helpers/prose_ratchet.py --keys FILE...` lists the bold spans in a document that a script
  reads, with the parser files that claim each. It is the answer to "is this bold a key" for someone
  cutting bold from a document, and it reads spans the way the counter does, so a listed span is
  exactly an exempted one.

What it does not do: it cannot tell whether a pattern is a key or a ban (above), and a reader that
lives in a language it does not scan, or outside the four roots, is still invisible. The first is
recorded here; the second has no instance today.

## Which keys should move to frontmatter

Frontmatter here is the format of milestone 582 (a decision's status becomes a field, and the index becomes generated), kept by milestone 596 (the roadmap blocks get frontmatter too): `---`, flat `key: value` lines, `---`. No lists, no
nesting, and a value may not contain `: ` or ` #`. That shape decides most of this.

| Key | Verdict | Why |
|---|---|---|
| Status, Built, Gate | Done | Moved by §207 (the roadmap is a graph, and the block says so in fields a script can walk) and milestones 582 and 596. Every roadmap block carries frontmatter. An appendix carries none, such as the seam-naming appendix of milestone 20 (a portable HAL, proven on a second architecture) at `0020-portable-hal/name-the-seams.md`. |
| Follow-on and Revisit tags | Stay bold | Many per document (about 1,400 spans over 415 files), each opening a bullet whose prose is the content. Flat frontmatter cannot hold a list, and a tag a reader needs inline is structure. |
| `The experiment:` lead-in | Stay bold | A reader needs it inline; it introduces a paragraph. |
| `Experiment status: WORD, date` | Move | It is metadata about the document and the nine documents are the whole population. The metrics workflow already prefers frontmatter for the color and cross-checks it against this line. |
| `Reuse:` | Drop the bold | The parser accepts it plain. Done 2026-10-07, by which time the template had spread it to 35 files. |
| `**A run is due every N days.**` | Stay | One sentence a reader acts on. |
| `**RETIRED` | Gone | A journey's retirement is `status: RETIRED` and its `retired` date in frontmatter; no script reads a bold marker. |
| `**Lane:**` | Stay | PR bodies, not documents. |

The one migration worth doing is the nine experiment-status lines. The proposed keys, provisional:
`experiment_status: RUN` (one of `RUN`, `NOT-RUN`, `CANNOT-RUN`) and `experiment_run: 2026-08-31` (empty
or absent unless the status is `RUN`). The sentence after the status word stays as ordinary prose and
becomes the first line of the section.

One snag, found while reading `script/metrics`. `_APPENDIX` matches every `design/fatal-risks/*.md`
that is lowercase, and the nine new risk files (`1-only-software-...md`) match it. The nine appendices
already carry `risk:` and `color:`. If a risk file gains `risk:` as well, `fatal_risks_colors` keeps
whichever it reads last, silently. So the risk files take no `risk:` or `color:` key, or `_APPENDIX` is
tightened to exclude `^\d+-` first. The second is a one-line change and is part of this migration.

## The seven questions

1. Considered and refused. A shared registry: refused on 2026-09-26 and again here, for the
   disagreement reason above. Moving every tag to frontmatter: refused, flat format has no list.
   Leaving the exemption alone: refused, because the blind spots were found by hand in one pass, and
   the next parser will be written by an agent who has not read this. Stripping the bold and teaching
   every reader a plain form: refused for the tags, since the bold is what makes a tag distinguishable
   from a sentence that happens to start with "Recorded".
2. What the tree does in the analogous case. Decisions (582) and roadmap blocks (596) moved their
   status, dates and dependencies to frontmatter, kept the prose reader for history, and left the
   Follow-on tags bold. The fatal-risks appendices took `color:` and `updated:` the same way. This
   proposal follows that line exactly.
3. Prior art outside the tree. Static-site generators (Jekyll, Hugo) and note tools (Obsidian) keep
   per-document metadata in frontmatter and leave inline structure in Markdown. That is from memory,
   not read here, and it agrees with the tree's own practice. Not verified.
4. The premise. "Scripts track bold keys" is true, and "this blocks the sweep" is false today: the
   count above shows no counted span is a key. The hazard is future parsers, which is why the
   mechanism is the main deliverable.
5. Cost, by count. Tracking: one function pair, one dict entry and one flag, about 165 lines in
   `prose_ratchet.py`, and it runs inside the existing selftest. Migration: 9 risk files, two readers
   (`script/fatal-risks`, `script/metrics`, each keeping its prose form for history) and one
   vocabulary section in the README. The tag migration would touch 415 files and was refused.
6. Reversibility. Tracking is code and fully reversible. The frontmatter keys are a format two
   programs agree on, which this tree treats as the expensive kind. The prose reader stays, so a key
   rename or a revert reads the same history. Who has acted on it: `script/metrics` and the weekly
   chart read the appendices' `color:`, and nothing yet reads `experiment_status`.
7. Same cost, same choice? Yes for the tracking, the migration of the status line, and the refusal for
   tags. None of the recommendations rests on effort. The tag refusal rests on the flat format and
   would hold if the work were free.

What cannot be answered here: whether any prose in `notes/` carries a key shape a future parser will
want. That is exactly what `OPAQUE_OK` and `--keys` are for, and the answer arrives with the parser.

## The ruling, and what each part became

calef ruled on 2026-10-05 (UTC): "the nine Experiment status: lines should move into frontmatter".
#1714 did it. `experiment_status` and `experiment_run` are ratified names, and
`design/fatal-risks/README.md` holds their vocabulary.

The rest of the verdict table needed no ruling and landed in #1821:

- `Reuse:` is written plain. notes/roadmap.md's template and `script/roadmap`'s fix message both
  wrote `**Reuse:**`, which is why the count grew from 12 to 35 in two days. Both now write it plain,
  32 files lost the bold, and `NOT_MARKERS` names the Reuse pattern, so a bold one is counted as
  emphasis rather than exempted. The reader still accepts both forms, so no open pull request breaks.
- `selftest` now fails on a `NOT_MARKERS` or `OPAQUE_OK` entry that names a pattern no parser
  carries, so the two registries cannot go stale quietly.

## The ban is not a key

`script/decisions`' `^\*\*Status:` is named in `NOT_MARKERS`, so a bold `**Status:` line elsewhere
counts as emphasis under §213 (writing standards). The free-form `**Status:` lines in notes and
design documents, which no script reads, are plain (see Follow-on).

## Follow-on

- **Done.** The tracking mechanism, by #1702; the nine experiment-status lines, by #1714; the plain
  `Reuse:` line and the registry staleness check, by #1821.
- **Done.** The line-opening `**Status:` spans are plain, except nine that sit inside code fences as quotations of the old spellings
  (eight in `notes/roadmap.md`, one in milestone 253 (a status that can only become wrong after the
  merge)) and stay as written. `('script/decisions', r'^\*\*Status:')` joined `NOT_MARKERS`, so the ban
  is not a key. `script/roadmap`'s `RESTATED` did not fire on the plain form.
- **Recorded.** The two blocks still carrying a bold `Reuse:` are in this block's `BUGS`.
- **Refused.** Moving the Follow-on and Revisit tags to frontmatter: a flat format holds no list.

## BUGS

- Milestones 796 (pin the hot trap path's placement) and 800 (a non-Anthropic model attacks the
  confinement claim) still carry `**Reuse:**`. Their lanes had the blocks open on 2026-10-07. It
  is counted as one bold span and both are under the limit.
- A reader in a language `opaque_readers` does not scan, or outside its four roots, is still
  invisible. None exists today.

## Index row

A script reads bold keys in prose, and the ratchet exempts them by reading the parsers. The block records where each is read, the nine experiment-status lines became frontmatter, and the ratchet now counts bold no parser needs; so a fact a script reads is a key a parser checks, not bold a regex finds.
