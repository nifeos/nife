# What the corpus test caught, and what it cannot see

An appendix to [`notes/documentation.md`](../documentation.md), which states the lesson.
`every_character_survives` checks that every letter and digit of the corpus reaches the rendered
output, in order.

## A dropped image destination

A link and an image both show their destination. A terminal cannot follow one or draw the other,
so the path is the only thing a reader can act on. Rendered as `[image: alt text]` alone, an image
names a picture the reader cannot find. The test is a subsequence check over the source, so a
dropped destination is what it exists to catch. notes/project-metrics.md carries an image, and
trips it if the path goes missing.

## Text lost to the grammar

In a GitHub table a `|` splits a cell even inside a code span. Escaped as `\|`, it is a pipe. Cells
past the header's count are dropped. So a page with a bare pipe in a table's code span loses text,
and the test names it. The escaped source reads the same on a terminal and on GitHub.

Two other CommonMark rules are markup rather than lost text, and the test's expectation knows them.
A character reference such as `&nbsp;` is one character. An ordered list counts up from its first
number, whatever the later items say.

## What it cannot see

A page rendered completely and wrongly passes. Say a fenced block inside a block quote never
closed: the rest of the page would render as quoted code. Verbatim output loses no characters, so a
subsequence check is blind to it. A unit test is the guard (`a_fence_inside_a_block_quote_closes`).
The lesson generalizes. A subsequence check proves nothing was dropped and nothing about what was
ruined, so a renderer wants both kinds of test.

## Why it runs wide, and natively only

The test renders at a terminal 4,000 columns wide. A real terminal truncates a table cell to its
column, and that is a layout choice the `BUGS` section records, not lost text. At 1,000 columns the
widest table in this repository still shrinks, so the width was measured rather than picked.

It is ignored under Miri. Its subject is a corpus on disk, 547 markdown files and 7.2 MB, and
reading it under the interpreter needs `-Zmiri-env-forward=CARGO_MANIFEST_DIR` and
`-Zmiri-disable-isolation` for the whole workspace run. Measured 2026-09-03 (UTC), it took 0.74
seconds natively and was still running after 12 minutes under Miri. Miri would find little here.
The crate has no `unsafe` of its own, and `pulldown-cmark` is fuzzed upstream. The unit tests in
the same file cover every construct on in-memory input and do run under Miri. That is the
`cfg(miri)` sampling convention of notes/undefined-behavior.md taken to its limit: the unit tests
are the sample, and the corpus stays native.
