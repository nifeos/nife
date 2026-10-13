# The notes index, by area

[`notes/README.md`](../README.md) is the index of every note. Its lines live here, one page per
area, under the 3,000-word cap of
[§212 (a prose budget)](../../design/decisions/0212-a-prose-budget-for-every-document.md), and the
top page keeps the "Start here" list and a link to each area. This is the default siting §212
ratified: a parent-named directory beside its document.

*Name: provisional, for `notes/README/` and the area page stems inside it. calef names things;
expect this to change.*

`script/lint` reads the top page and every page here when it checks that each `notes/*.md` has a
line, so a note is indexed by a line on whichever area page fits.
