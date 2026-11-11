# The record sort: what moves to `nifeos/record` and what stays

An appendix to milestone 1018 (the design record moves to `nifeos/record` and is published as a
website), for its step 4. `packages/homes.toml` holds the sort itself, and
`python3 helpers/record_links.py` counts it and lists the links it cuts. This page says how the
lane made the sort and what the counts were on 2026-11-11 (UTC), at `20275e6d0`.

## The rule it follows

The ruling on question 4 sets the division. The record moves: `design/`, `briefs/`, the lane
skills under `notes/skills/`, and every note that is not tool data or about one crate alone. What
stays: the tool data under `notes/` that the ruling names, the eight lint and prose baselines under
`design/` and `design/removed.tsv`, and each note about one crate alone, which §315 (a crate's
documentation leaves with it) sends with that crate. Crate documentation, `BUGS` sections,
workflow data and each repository's `AGENTS.md` sit outside the record's directories already.

## How the lane judged a note

A note is about one crate alone when its subject is that crate's own design, format, invariants,
proofs or hardware facts, and it reads true to someone holding the crate's repository and nothing
else. A note about how a crate meets the rest of nife is cross-cutting and moves: a component's
wiring, a contract two programs share, a benchmark, a bench session, a confinement claim, a review
package, a decision's proposal.

Of the 1,151 Markdown notes outside tool data, 169 were candidates: a crate's name is in the
note's path, or one crate holds three quarters of its `crates/<name>` paths and backticked crate
names. The lane read each candidate's title and opening paragraph against that test. 36 notes, in 23
crates, are about one crate alone. 33 belong to crates still in this tree, and each crate's
`[[home]]` table names it, undecided until the crate's own repository is. 3 belong to crates
that have already left, `designware_mobile_storage` and `generational_table`, so their home is
that crate's repository. The sort is provisional, and a note outside the 169 can still be about
one crate alone: the census found candidates by name and by mention, not by reading every note.

The three notes about crates that have left are §315's sort arriving late. §315's Follow-on
records that `script/crate-standalone` owes the step that would have refused those extractions.

## The links a move cuts

`helpers/record_links.py` reads every inline Markdown link in a file that moves, resolves it
against the file's path, and reports each one whose target is a tracked path that stays. A
directory stays when any tracked file under it stays.

| what the links name | links |
|---|---|
| tool data under `notes/` (`scripts.md`, `mutation-testing/`, `project-metrics/`) | 122 |
| a note about one crate alone | 44 |
| `AGENTS.md`, `ARCHITECTS.md` and the other files every repository carries | 21 |
| `bench/` measurements | 19 |
| a package's files (`system-tests` 14, `kernel` 6, `contracts` 5, and 6 others) | 38 |
| a crate repository that exists (`generational_table`, `designware_mobile_storage`) | 10 |
| a directory with a file that stays | 4 |
| the build and gates, and `vendor/` documentation | 1 each |

That is 260 links from 125 record files, and `--list` prints each with its line. Each becomes a
qualified `nifeos/<repo>:<path>` or plain text in step 6, since the site's link check fails the
build on a link into nothing. A code path in backticks is not a link, and the main appendix
counts those.

The script reads inline links only. A reference-style link is not counted, and the record holds a
few.
