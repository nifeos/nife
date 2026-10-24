# What the corpus test caught, and what it cannot see

*Name: provisional.*

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
