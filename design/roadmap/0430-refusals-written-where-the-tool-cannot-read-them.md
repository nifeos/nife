---
status: BUILT
raised: 2026-09-18
built: 2026-09-18
promoted_from: refusals-written-where-the-tool-cannot-read-them
---
# 430. Refusals written where `script/names` cannot read them

Premise re-checked 2026-09-19: it had already been answered, which is why this block is `BUILT`.
The pass found the two exhibits it opens with, `screen_console`'s four refusals and `board_console`'s three,
visible to `script/names --refused` without either file's `Name:` block having moved. What closed it
is recorded below, under "What closed it, and it was the same day", and the text above is kept as
the account of the defect rather than rewritten.

## In brief

`script/names`' own header states the rule: "A block runs from its `Name:` line to the next empty
comment line, so it may wrap." So a refusal written after a blank `//!` line is in the file, is
read by a human, and is invisible to the tool. Milestone 115's whole purpose is that a refused
name is findable by the person about to propose it again, and a refusal the tool cannot see does not
serve that purpose.

**Verified, not inferred:**

- `crates/screen_console` records four refusals (`framebuffer_console`, `pixel_console`,
  `early_console`, `video_terminal`) in paragraphs after a blank `//!` line.
  `script/names --refused | grep -c screen_console` answers 0.
- On 2026-09-18 a maintainer edit to `crates/capability_demo_protocol` moved three previously
  visible refusals into a later paragraph. The tree-wide count fell from 171 to 168, exactly three,
  which is how the problem was found at all. (That crate is `crates/capability_witness_contract`
  now, renamed later the same day; the old name stands here and in Scope below because both
  passages are dated accounts of measurements taken under it.)
- Restoring those three and pulling seventeen more of that day's rulings into the parsed paragraph
  took the count from 168 to 188.

**This was found by arithmetic, not by a gate**, and only because the number happened to move in the
wrong direction in a session that was watching it. Nothing would have caught the same mistake in a
lane's commit.

## Why it matters more than a count

The failure is silent in the direction that hurts. A block with hidden refusals still looks
complete to a reader opening the file, and `script/names <name>` still answers for the name itself,
so nothing about the record looks wrong. What breaks is precisely the query milestone 115 exists to
serve: *has this name been refused before?* AGENTS.md's worked example for that is milestone 63
having already refused `system_builder` for a reason nobody could find.

There is a second, sharper case: a name that is refused and still live. `script/names` reports
those as a NOTE, which is how a reader learns that `pci` is refused somewhere and used anyway, or
that `dtb` is queued for a rename. A hidden refusal drops the name out of that list, so the queue of
outstanding renames silently under-reports.

## The options

1. Fix the blocks, leave the parser. Put every refusal in the first paragraph. It is what the
   header already documents, and the 2026-09-18 pass did this for ten blocks. Cheapest, and it keeps
   the parser's simple rule. Costs: nothing prevents the next block from doing it again, and long
   first paragraphs are harder to read than a paragraph per argument.
2. Widen the parser to read to the end of the doc comment rather than to the first blank `//!`
   line. Fixes every existing block at once with no sweep. Costs: the blank-line rule exists so a
   `Name:` block does not swallow unrelated prose that follows it, and several files put ordinary
   documentation after the name block. This needs checking before it is chosen, not assumed.
3. A `script/lint` check, and this is the one that holds either way: fail when the word
   `Refused` appears in a file outside the block `script/names` parses. It is a mechanical
   question with a mechanical answer, it is rung two where the header's sentence is rung four, and
   it would have caught this on the commit that introduced it.

Recommendation: 3, plus whichever of 1 or 2 the check's first run argues for. The gate is the
part that matters, because the bug is not that ten blocks were wrong; it is that nothing noticed for
as long as the convention has existed. Option 2 is attractive and may well be right, but it is a
change to how every name block is read and it needs the survey in option 3's first run to price it.

## Scope

Ten blocks were corrected on 2026-09-18 (`nvme`, `gpt`, `dtb`, `ipc`, `asid`, `pci`, `elf`,
`nvme_server`, `address_space_builder`, `capability_demo_protocol`). `screen_console` and
`board_console` are known to still be wrong and were deliberately left, because a naming sweep on
the same evening as ten rulings is how a cheap edit destroys an expensive record. A lane should
survey the whole tree rather than trust that list.

**Both are right now, and not by being edited.** Re-checked 2026-09-19:
`script/names --refused | grep -c screen_console` answers **4**, where this section recorded 0, and
`board_console`'s three are visible too. Neither file's `Name:` block moved. Option 2 is what
changed underneath them, and the section below says so.

Two of those ten names have since been replaced by the rulings the corrected blocks recorded:
`capability_demo_protocol` is `capability_witness_protocol` and `address_space_builder` is
`address_space_witness`. The list above is an account of which blocks were edited that day, under
the names they carried then, so it keeps them.

## What closed it, and it was the same day

Commit `c1a177c`, 2026-09-18, "The `Name:` block parser stopped at the first blank line, and
that hid 91 refusals". It took options 2 and 3 together and did option 1's sweep where it was still
needed, which is the combination this block recommended without being able to say that option 2
would survive its own survey.

- Option 2, the parser. The parse reads to the end of the comment run rather than to the first
  blank `//!` line, so every block written one argument per paragraph became readable at once. The
  worry this block raised about it was measured rather than argued away. Across all 165 surfaces
  carrying a block, no status, date or well-formedness verdict moved. That mattered because
  `script/metrics` shares the parser: a change there moves a dashboard as well as a gate. A
  paragraph break now bounds a refusal clause and a slash disqualifies a refused name.
- Option 3, the gate. `script/names --check` reports a refusal recorded where the parse cannot
  read it, the sibling of its strays check, proved to fire on a stray and to ignore one the parse
  reads. Narrowing it enough to know that `Refused` as a return-value name is prose cost two passes.
  It is in `script/names` rather than `script/lint`, which is the better home: the check is about
  this parser, and it runs wherever the parser does.
- Option 1, where it was still owed. Four files that put a heading after their block were moved
  to the convention 139 of 143 already followed, and `hello.rs`'s block reference moved with the
  limitation it names.

The count: 182 tree-wide before, 273 after, and 273 on 2026-09-19. The four `screen_console`
refusals this block opened with are visible, and so are `board_console`'s three, without either
file's block being touched.

## BUGS

- A name in backticks inside a refusal's own sentence reads as refused too, which is eight of
  the 273. Ending the clause at a colon removes all eight and takes 26 real refusals with them,
  including `pci`, `elf` and `mdr`, because `Refused ``x``: why` is a form this tree uses
  constantly. Losing a real refusal is the failure the record exists to prevent and a spurious one
  is noise a reader sees through by opening the block, so the trade is deliberate.
- **The gate catches a refusal outside the block, not a block nobody wrote.** A name with no
  provenance at all is a different check (`script/names --unratified`, a worklist), and neither can
  see a refusal that was never written down anywhere.

## Follow-on

- **Done.** Options 2 and 3, and option 1 where it was still owed, all in commit `c1a177c` on
  2026-09-18. The gate was proved to fire on a stray and to ignore one the parse reads.
- **Recorded.** The eight backticked names inside refusal sentences that read as refused, in the
  `BUGS` section above, with the arithmetic that says why tightening the clause costs more than it
  buys.

## Index row

`script/names`' `Name:` block parser stopped at the first blank `//!` line, so a block written one
argument per paragraph, which is how this tree's best ones are written, put every refusal after the
first break out of reach. `crates/screen_console` recorded four and the tool saw none. That defeats
the query milestone 115 exists to serve, has this name been refused before, whose worked example is
milestone 63 having refused `system_builder` for a reason nobody could find. The failure is silent
in the direction that hurts, because the block still looks complete. Commit `c1a177c` widened the
parse to the end of the comment run, after measuring that no status, date or well-formedness verdict
moves across all 165 surfaces, since `script/metrics` shares the parser. It added the gate that
lasts: `script/names --check` reports a refusal written where the parse cannot read it. 182 refusals
were visible before and 273 after.
