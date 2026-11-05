---
status: PROPOSED
raised: 2026-11-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A name's status lives only in its own Name: block

Raised by lane/naming-rules-by-topic on 2026-11-05 (UTC), from a feasibility study the naming
re-cut was asked to make. The integrator mints the number at promotion.

Reuse: `helpers/name_provenance.py`'s block parse, `helpers/name_rules.py`'s Rule 1 computation,
and `helpers/name_release.py --draft`, which already derives a crate's worklist from the blocks.

## Background

A name records its status in its own `Name:` block, and `script/names` computes the table from the
blocks on demand ([design/naming/provenance.md](../../naming/provenance.md)). Several files still
hold names centrally, beside or instead of the blocks. Each one is a file every naming pull request
edits, which is the conflict shape the block exists to avoid, and each can drift from the blocks.

| Central record | Holds | Read by | Derivable from the blocks? |
|---|---|---|---|
| `design/naming/convention-rulings.md` Rule 2 | 24 fixture programs ratified as a group | `helpers/name_rules.py` (`script/names`, `script/metrics` through `name_census.py`) | yes: each fixture's module doc carries `Name: ratified 2026-10-17 (UTC, convention-rulings Rule 2)` |
| `design/naming/convention-rulings.md` Rule 3 | 114 appendix documents in 39 directories, ratified as a group | the same | yes: each document carries a `*Name: ratified 2026-10-18 ...*` paragraph, and each directory README block agrees |
| `notes/unratified-names-journey-5.md` | each journey 5 crate's unratified names, grouped for review | no script; people | yes: it is the output of `name_release.py --draft <crate>`, frozen |
| `helpers/name_checks.py` `KEPT` (24) and `RULED` (2) | names a check exempts, with the owner or the ruling | `name_checks.py` | yes: an exemption clause in the item's block (`Kept: DesignWare's PA`) that the check parses |
| `helpers/cap_abbreviation.py` and `helpers/unit_symbols.py` `KEPT` | identifiers where a short form is not ours | the two ratchets | partly: an item with a block can carry the clause; a local or a field without a block cannot |
| `design/naming-rule-baseline.txt` (374 rows) | unratified names that break a checked rule | `name_checks.py --check` | no: it grandfathers existing breaks, and a marker in each block would let a new name mark itself and pass unless the check also compares markers against the merge base |
| `design/module-block-baseline.txt` (about 5,400 names) | names with no block at all | `module_block_ratchet.py` | no, by definition; it shrinks to nothing as blocks arrive |

## The work

1. Rules 2 and 3: write the block into each covered fixture and document, make each directory
   README block agree, then drop the two lists and the `fixture-roles` and `listed-stems` paths
   from `name_rules.py` with their selftests. `script/names | tail -3` must print the same ratified
   count before and after. Rule 1 stays a rule: it is computed, and it ratifies future stems.
2. The journey note: delete it once its in-flight pull requests land, and point
   `script/crate-standalone`'s step 0 message at `name_release.py --draft`.
3. The `KEPT` and `RULED` tables: move each entry with a block into its item's block as a clause
   the check reads; keep the table only for names that cannot carry a block, each with its reason.
4. The two baselines stay. They hold no status, only the debt the ratchets drive down.

## Cost and risk

Step 1 touches about 140 files, most of them notes, and conflicts with any lane editing those
notes; it wants a quiet tree and one pull request per directory. Step 3 changes what three checks
read and needs a selftest per clause form. Neither step can lose a ratification silently, because
the ratified count is a number to compare. Step 2 waits for the journey note's pull requests (3142
to 3144, 3149, 3153, 3154, 3156, 3200).

## What drives it

The direction that a name's status lives in the documents with the name, and the merge conflicts
every naming pull request meets on the shared lists. Nothing blocks on it.
