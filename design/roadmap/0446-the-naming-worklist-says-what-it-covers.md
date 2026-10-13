---
status: BUILT
raised: 2026-09-20
built: 2026-09-20
---
# 446. The naming worklist says what it covers, and stops saying what it used to

This milestone exists because of a false premise, and the premise is the interesting part.
A lane on milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets)
reported that `cryptography_provider`, `cryptography_exerciser` and `script/crypto-probes` were
invisible to `script/names --unratified` because the tool "cannot see
root-level workspaces or `helpers/`". calef asked for a lane to close that hole. The maintainer ran
the tool first: on 442's own branch the worklist lists all three, and on `main`
`script/names entropy_backend` prints that package's full ratified provenance with its three
refusals. The hole had been closed a month earlier and the tree was still describing it.

That is the defect `AGENTS.md` cares about most in documentation, said from the other side. An
honest `BUGS` section is the mechanism that makes a newcomer trust the docs; a `BUGS` section
claiming a limitation the code does not have spends that trust just as fast as hiding one, and
costs more, because it sends somebody to build a thing that already exists.

## What was false, and what is true

`script/names` has a fourth kind, `package`, so `script/names std_exerciser` no longer answers
*"neither a name in the tree nor a recorded refusal"*. It is discovered rather than listed: the tool
walks for `Cargo.toml` outside `crates/`, so a package arriving tomorrow is covered without anyone
editing the script. `kernel`, `xtask`, `redoxfs_server`, `tools/redoxfs_host`, `components`,
`fixtures`, `std_exerciser` and `entropy_backend` are all in the table today, which `script/names
--check` counts as 10 packages of 229 names.

Four places still told the old story, and one told a different stale story in the same direction:

| Where | What it claimed | What is true |
|---|---|---|
| `entropy_backend/src/lib.rs` `BUGS` | the tool "derives its table from four locations", a root-level workspace is none of them, and "the naming worklist will never list this crate as unratified" | it is listed, and this package is the case that motivated the fix |
| `script/names`' own scope comment | `kernel`, `xtask`, `redoxfs_server`, `tools/` and `helpers/` are "out of scope on purpose" | four of those five are covered; only `helpers/` is out |
| `notes/scripts.md`'s row for the script | the block is carried by "every crate, program and `script/` entry point" | and by every Cargo package, in its `Cargo.toml` |
| `notes/scripts.md`, same row | "Each block is `ratified`, `recorded` or `unrecorded`" | four words since §89 (`provisional` becomes the fourth provenance state) added `provisional`, which is the largest state today at 48 of 229 |
| `design/naming.md`, *The three states* | "54 of 126 names are unratified today" | 76 of 229 on 2026-09-20; the ratio held while the tree nearly doubled |

`std_exerciser/Cargo.toml` describes the same blind spot in the past tense, as the package it was
found through, and was deliberately left alone. `helpers/name_provenance.py` and
`helpers/roadmap_proposals.py` each say they are out of the worklist's scope, which is still true,
and each says so while carrying its own `Name:` paragraph as the record instead, which is the
convention working.

## `helpers/` stays out, and here is the price of the alternative

The one genuinely uncovered surface besides types is `helpers/`, the helper drawer, as distinct
from `script/`'s entry points. It was priced rather than assumed, and refused. The numbers:

- 17 files in `helpers/`, plus the `kani-lint-shim/` directory, which is Rust source rather
  than a script.
- 9 of the 17 already carry a `Name:` paragraph that no gate asked them for. So the question is
  not whether a helper may argue its own name, since more than half already do, but whether the
  worklist should **enumerate** them.
- Enumerating costs **about 15 rows on a worklist 76 deep**, a fifth again of the only queue in
  this tree whose sole consumer is an architect's attention.
- Zero of the 9 paragraphs records a refusal. The claim of milestone 115 (the names that were
  ratified, and the ones that were refused) is that the refusals are the valuable half, and today
  that half is empty here, so leaving `helpers/` out loses nothing that mechanism was built to
  keep. That is the number to re-measure if this is revisited.
- The 8 without a paragraph are the machine-invoked ones: `qemu-runner-aarch64.sh`,
  `qemu-runner-riscv64.sh`, `qemu-runner-x86_64.sh`, `qemu-bounded.sh`,
  `qemu-bounded-selftest.sh`, `memory-bounded-runner.sh`, `build-ripgrep.sh` and `rust_source.py`.
  A gate demanding blocks would mostly manufacture rulings on names nobody types.

The deciding argument is the worklist's own ordering. `--unratified` prints its rule at the top:
a program is typed at the prompt, a crate is what a newcomer greps, a `script/` entry point is typed
by whoever works on the tree. `design/naming/programs-scripts-and-directories.md`'s **Scripts**
section defines `helpers/` as the drawer that is called by other scripts and by `xtask`, **not by
people**. Enumerating it would add a tier below the bottom tier of a list whose entire ordering is
exposure, which is not a coverage improvement but a dilution of the ranking. If it is ever built, it
sorts last, after `script/`, for the same reason.

The refusal is recorded in `design/naming.md`'s `BUGS` section, where the scope claim lives, rather
than in this block, so that the next reader meets it at the rule rather than in a milestone they
have no reason to open.

## What the refusal still owed, and was cheap to pay

A refusal is not the same as an answer that is wrong. `script/names merge-drain` printed *"neither a
name in the tree nor a recorded refusal"* about a file whose header argues that name for four lines.
That is the confident-wrong-answer shape the `package` kind was added to fix one level along, and it
does not need coverage to fix: the tool now recognizes a `helpers/<name>.sh` or `helpers/<name>.py`,
says the name is out of the worklist's scope on purpose, cites where the reason is written, and
points at the file that holds the record. Six lines, no new rows, and the query stops lying.

## The ladder, and why rung two is not available here

These claims went stale silently because nothing compares a sentence about a tool against the tool.
`AGENTS.md`'s ladder says to reach for the highest rung that fits, so the question is whether a gate
would have caught it.

**It would not, and saying so is better than building a fragile one.** The stale text was ordinary
English prose (*"cannot see"*, *"four locations"*, *"will never list"*) inside comments and notes
that legitimately discuss the tool's coverage, including this block. A grep keyed on those phrases
would fire on every honest description of the limitation, which is `git grep -w TODO`'s 82%
false-positive rate wearing a different hat, and `script/lint` has dropped checks for exactly that
signature before.

So the move is rung three, a written record at the thing itself: the scope of the naming
worklist is now stated in exactly one place, `design/naming.md`'s `BUGS` section, and every other
mention of it (`script/names`' comment, the two `helpers/` python modules, `notes/scripts.md`)
cites that bullet instead of restating the reason. A copy that is a pointer cannot go stale in the
direction this milestone was written to fix; it can only go dangling, which the relative-link check
in `script/lint` already catches.

## BUGS

- **The pointers are prose and nothing follows them.** `script/names`' comment now says "see
  design/naming.md's BUGS" rather than naming four directories, and no check reads that bullet to
  confirm it still describes the code. This is the same limit `script/names --check` records for a
  `recorded` citation, which is checked for being present and never followed, and it is not
  closeable by a script for the same reason: a scope claim is prose and prose is checked by reading.
- **Types are still uncovered and were not priced.** `design/naming.md`'s `BUGS` names types
  alongside `helpers/` as the surfaces without blocks. This milestone measured `helpers/` because
  the request named it; a type census would be a much larger count and a different argument, since
  a type name is read by everyone writing against it, which is the opposite exposure to a helper
  script. Nobody has run that count.
- The `helpers/` refusal is keyed on a number that will move. Zero of the nine paragraphs
  records a refusal today. The first `helpers/` helper whose name is argued against a rival that
  loses puts a refusal somewhere `script/names --refused` cannot reach, and the refusal above says
  to re-measure rather than pretending the answer is permanent.

## Follow-on

- **Refused.** Enumerating `helpers/` helpers in the naming worklist, priced above at ~15 rows on a
  queue 76 deep, against zero refusals currently lost. The refusal and its numbers are in
  `design/naming.md`'s `BUGS` section, where the scope claim lives.
- **Recorded.** Types carry no provenance blocks and the surface was never counted, beside the
  `helpers/` refusal in `design/naming.md`'s `BUGS` section.
- **Recorded.** Nothing checks that a prose scope claim still matches the tool, which is why the
  claims in this milestone went stale for a month. In this block's own `BUGS` section, with the
  argument for why a grep would be worse than the note.

## Index row

A lane reported that `script/names` could not see root-level workspaces, calef asked for a lane to
fix it, and the maintainer ran the tool first: the hole had been closed a month earlier by the
`package` kind, which discovers every `Cargo.toml` outside `crates/` rather than listing
directories. What was left was a tree still describing the old shape in five places, including an
`entropy_backend` `BUGS` entry promising that "the naming worklist will never list this crate as
unratified" about a crate the worklist lists. A `BUGS` section claiming a limitation the code does
not have spends a newcomer's trust exactly as fast as hiding one, and costs more, because it sends
somebody to build a thing that exists. The genuinely uncovered surface, `helpers/`, was priced
rather than assumed: 17 files, 9 already carrying a voluntary `Name:` paragraph, zero refusals
recorded in any of them, and about 15 new rows on a worklist 76 deep whose only consumer is an
architect's attention. It is refused on the worklist's own ordering rule, since that list is sorted
by exposure and `design/naming.md` defines `helpers/` as the drawer people do not type. What the
refusal still owed was cheap: `script/names merge-drain` no longer answers "neither a name in the
tree nor a recorded refusal" about a file that argues its name at length. No gate is proposed,
because every stale phrase was ordinary English inside comments that legitimately discuss coverage,
and a grep keyed on it would reject every honest description of the limitation.
