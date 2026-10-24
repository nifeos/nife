# What the documentation index costs, measured

*Name: provisional.*

An appendix to [`notes/documentation.md`](../documentation.md), which states the conclusion.

`cargo xtask manual` over the current bundles:

| bundle | pages | terms | postings | markdown | index | probes |
|---|---|---|---|---|---|---|
| `manual` | 1 | 1222 | 1222 | 34011 | 57344 | 5 |
| `swish` | 2 | 1820 | 2143 | 91736 | 81920 | 5 |
| `kernel` | 3 | 2461 | 3369 | 101022 | 106496 | 6 |
| `glob` | 1 | 882 | 882 | 20019 | 40960 | 4 |

246,788 bytes of markdown produce 286,720 bytes of index, which is 1.16x. That is the number worth
arguing with rather than the pleasant ones.

It was 1.56x when phase 1 measured it and 1.24x in the middle. The improvement is not an
optimization. The notes it indexes grew, and page alignment's fixed floor is a smaller share of a
bigger bundle. It went the other way on 2026-08-18, from 1.14x, and that one *is* a cost.
Underscore-joined terms are a third term for every `snake_case` identifier in the prose, which is
what makes those identifiers findable.

Two things pay for it. A term record stores its term inline in 24 bytes, so a probe is one page
read rather than two; that is most of the bulk. And page alignment puts a four-page floor (16 KiB)
under every bundle however small. So a bundle of one short page still costs 16 KiB to index.

Ranking by length (2026-08-22) cost four more bytes per page record, spent out of the six the layout
already held spare. With four pages in the shipped store that is sixteen bytes. That is
immeasurable against the four-page floor, which is why the ratio above barely moved.

The `manual` row indexes the main page, so editing it moves its own numbers. Rerun
`cargo xtask manual` for current ones; the ratio is what is stable.

The number that justifies the layout is the last column. A lookup is at most six page reads and
24 KiB of IO (`kernel`; the others take four or five), with no allocation and a 4 KiB working set.
