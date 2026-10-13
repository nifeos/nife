# Milestone 40 appendix: the build record, 2026-08-18 to 2026-08-22

An appendix to [milestone 40 (documentation as a system service)](../0040-documentation-service.md),
under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main
page is complete without it. This is the block's build record after phase 2: three defects in the
record, `script/apropos`, the tail-output fork worked through and built, and ranking by length. It
keeps the names the tree used then (`manual` for `crates/documentation`).

## 2026-08-18: the record was the defect three times over, and all three are fixed

*In the renderer.* A fenced code block opened inside a block quote never closed. The closing test
ran against the raw line, so a quoted closing fence never matched its own opener, and every line to
the end of the document rendered as quoted code. It survived for two reasons. The corpus test cannot
see this class of failure at all (verbatim output loses no characters, and the check is a
subsequence). And the one symptom it *could* see had been quarantined behind an honest `BUGS`
comment that refused to widen the filter until somebody answered the question. Answering it found
the bug. That refusal is the mechanism working, and it is worth reading before the next one is
waved through.

*In the prose.* Three of notes/manual.md's `BUGS` entries described a system that no longer existed:

- `doc <page>` was said to deadlock. It is refused, by `grant_plan::check_chain`, with a sentence
  that names the fix.
- `doc <page> | wc` was said to render an empty stream. The head stage's input comes off the plan
  now.
- `MAX_TEXT_CHUNKS = 32` was said to truncate a page to 512 bytes. It is `MAX_OUTPUT_CHUNKS = 4096`.

Two of the three were closed by other lanes and nobody came back. The correction is three lines in
`script/swish-check` rather than three paragraphs, because this is the milestone least allowed to
describe a system that is not there.

*In the index.* `normalize` folds a query by dropping every non-alphanumeric byte, so a reader who
types `line_editor` looks up `lineeditor`. `tokens` split the *text* on that byte, so the builder
only ever wrote `line` and `editor`, and the term the query asked for was one no page could ever
have. In a repository whose prose is full of `snake_case` identifiers that is most of what anyone
would search for: `apropos fs_proto` answered "nothing says that" while dozens of pages said it. The
test that should have caught it asserted the property in its own first comment and then checked a
word with no underscore in it, which is the fence's shape again. Fixed in the writer, so
`apropos editor` keeps working and `apropos line_editor` starts. Narrowing the reader instead would
have made it silently search for `line`, which is worse than no answer.

## `script/apropos`, milestone 117's finding rather than a nicety

Milestone 117 (the stranger test) is three stranger runs. They have measured what a newcomer
cannot reach by following this tree: `notes/net.md`, `notes/capabilities.md`, any
`design/decisions/` file, and `crates/abi/src/lib.rs`, which is four syscall numbers and the whole design on one screen. None of them is hidden; nothing a
person would type led to them. This milestone already owned the machinery, pointed at the six pages
a guest can hold. So it is now also pointed at the 512 pages a checkout holds. It uses the same
`manual::index::build` and `manual::index::search`, one shard per part of the tree, and a result
that names a repository path rather than a store location. Crate and program module headers are
indexed as pages, which is what makes `crates/abi/src/lib.rs` findable at all. The document a reader
wants about the ABI *is* that file's header, and no markdown page was ever going to fix it.

## The one fork

What remained after phase 2 was one fork rather than a list. There was still no line a person
could type that rendered a page at the prompt, and the reason was not the shell's scheduling. A
process has one wait point, so a shell that feeds a chain cannot also read from it, and no
interleaving fixes that. What fixes it is somewhere for the viewer's output to go that is not the
shell. The tree already had that thing: `terminal_sink_caretaker`, the terminal's own sink adapter,
where a declared second stream goes by default.

Putting it in a tail stage's *output* slot was a spawn-protocol decision, the same decision the
pager and the color bit still want. notes/pipes.md had been holding it open since milestone 50
(pipes and redirection): *"a shell that wanted a program to print straight to the screen rather than
through its own result endpoint could hand it over, and would lose the ability to redirect that
program at all."* DECISIONS §106 (take the `terminal_sink_caretaker` narrowing) took that fork, and
it is built; see below. There is no graphical viewer half (calef: *"I don't need a graphical
viewer."*).

## 2026-08-22: the fork is worked through, not just named

DECISIONS §101 (notification objects), decided 2026-08-20, ratified the *direction*. The
`terminal_sink_caretaker` narrowing is "the right short-term move" and stays valid as the permanent
shape even once the notification object lands. But §101 explicitly left milestone 40's own fork
undecided: "That is milestone 40's fork, and this decision does not take it."

notes/tail-output-narrowing.md answers CLAUDE.md's six questions against it. It confirms the premise
against `grant_plan::check_chain` rather than trusting this block's own framing. It prices the two
refused alternatives (a pull-based source, a buffering stage) against measured numbers already in
notes/pipes.md. And it finds two things the tree did not have. The shell can reuse DECISIONS §26
(the fault endpoint), already built, as its completion signal instead of reading the child's bytes.
And moving output off the shell's own read loop opens a narrower race than notes/manual.md named,
between a child's exit and its own trailing delivery through the caretaker.

Decided (§106, 2026-08-22): take it. Checking `SINK_BIT`'s existing contract
(`crates/grant_plan/src/spawnproto.rs`) found the fork's own write-up had overpriced it. The child's
output slot is already opaque to the program ("the shell delegates an endpoint and init puts it
where the result endpoint would have gone, so the child writes to a pipe or a file sink without
knowing which"). So no program's manifest changes, only shell-and-init default-routing logic.

Built, the same day. An unredirected tail stage whose program declares
`InputSpec::Required { writes_while_reading: true }` (`doc` is the only one today) is delegated to
`terminal_sink_caretaker` by default. `grant_plan::check_chain` no longer refuses that shape, only
the redirected one (`doc <page> > out.txt`, still the shell's under DECISIONS §55 (the file behind a `>` is the shell itself)). The shell's completion
signal for a narrowed child is DECISIONS §26's kernel exit-delivery, on a fresh endpoint it mints and
delegates, exactly the reuse notes/tail-output-narrowing.md found. Verified at the real prompt, both
architectures (`script/swish-check`, `NIFE_SHOW_TRANSCRIPT=1`):

```text
$ doc gate.txt | wc
  1 4 26
$ doc gate.txt
  hello world hello world
```

The first line is the control this milestone has carried since phase 1 (the barrier `wc` makes the
render countable). The second is the line nobody could type before that day, rendering with no
`| wc` in front of it. The caretaker-hop display race notes/tail-output-narrowing.md named is carried
as a documented, accepted interim rather than fixed here; see the main page's `BUGS` and milestone
151. This is also the milestone's own headline demonstration, and the whole of phase 3.

## 2026-08-22, later the same day: ranking divides by length

`crates/manual/src/index.rs`'s own `BUGS` had named this since phase 2 landed: "ranking is
occurrence count and nothing else, so a long page that mentions a word in passing can outrank a
short page about it," priced at "one division" needing "the page's length, which the layout does not
store." That price was accurate. A page record grew a `tokens` field (four of the six bytes
`PAGE_REC` already held spare, no format growth), and `index::VERSION` moved to 2.
`manual::index::Ranked::offer` now ranks by `count / tokens` (fixed-point, one division) rather than
raw `count`. `Found::count` still reports the raw occurrence count in the printed answer; only the
merge order changed. It is pure crate logic with no spawn-protocol or syscall-surface change, unlike
the two items above it. See notes/manual.md's `BUGS` and "Where this goes next" for detail, and
`crates/manual`'s own test, `ranking_divides_by_page_length`. It is a quality improvement inside
phase 2.
