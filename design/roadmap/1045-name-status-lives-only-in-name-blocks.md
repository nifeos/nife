---
status: BUILT
promoted_from: name-status-lives-only-in-name-blocks
raised: 2026-11-05
built: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1045. A name's status lives only in its own Name: block

Raised by lane/naming-rules-by-topic on 2026-11-05 (UTC), from the feasibility study its brief
asked for.

Reuse: `helpers/name_provenance.py`'s block parse, `helpers/name_rules.py`'s Rule 1 computation,
and `helpers/name_release.py --draft`, which already derives a crate's worklist from the blocks.

## Background

A name records its status in its own `Name:` block, and `script/names` computes the table from the
blocks on demand ([design/naming/provenance.md](../naming/provenance.md)). Several files still
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

## What exists

1. Rules 2 and 3 are in the blocks. Each covered fixture and appendix document carries
   `Name: ratified 2026-10-17 (UTC, convention-rulings Rule 2)` or the Rule 3 paragraph, and
   `helpers/name_rules.py` reads them there; the two lists and their selftests no longer exist, and
   `script/names | tail -3` prints the same ratified count as before. Rule 1 stays a rule that
   `name_rules.py` computes.
2. The journey note no longer exists, and `script/crate-standalone`'s step 0 message points at
   `name_release.py --draft`.
3. `name_checks.py` reads a `Kept:` or `Ruled:` clause from a name's own block. The one entry whose
   name has a block moved into it. The other 25 `KEPT` and `RULED` entries name Rust items with no
   block, and the cap and unit tables name none with one, so they stay with their reasons; a block
   added to one would change the counts `script/names` prints.
4. The two baselines stay. They hold no status, only the debt the ratchets drive down.

## Done when

All four hold, as above. Nothing is left over: the entries that stay are the ones that cannot carry
a block, which is what step 3 kept the tables for.

## What drives it

The direction that a name's status lives in the documents with the name, and the merge conflicts
every naming pull request meets on the shared lists.

## Follow-on

- **Done.** The 25 `KEPT` and `RULED` entries in `helpers/name_checks.py` that name Rust items with
  no block stay by design, each with its reason, carried by step 3 of the work above (#3485).

## Index row

Several files hold a name's status beside the blocks that already carry it, and every naming pull request edits them. Proposed: derive each from the blocks and keep only the two baselines.
