# The manual: the appendices to notes/documentation.md

[`notes/documentation.md`](../documentation.md) carries what the manual renders, how it is
installed, how search works, the examples, every live limitation and what comes next. These files
carry the evidence its prose budget has no room for.

- [`corpus-test.md`](corpus-test.md): what `every_character_survives` catches (a dropped image
  path, a table cell lost to an unescaped pipe), what it cannot see (a page rendered wrongly
  but completely), and why it runs 4,000 columns wide and natively only.
- [`underscore-terms.md`](underscore-terms.md): the index writer and reader disagreeing on what a
  word is, and why the fix joins only the underscore.
- [`apropos-builtin.md`](apropos-builtin.md): why search is a shell builtin, the worked example,
  and the split between the crate and the shell.
- [`repository-search.md`](repository-search.md): `script/apropos`, the stranger test's finding
  it answers, and its three deliberate choices.
- [`index-cost.md`](index-cost.md): the per-bundle table and how the index ratio moved.
- [`search-limitations.md`](search-limitations.md): the full `BUGS` entries for search, with the
  reason behind each limit.
- [`resolved-bugs.md`](resolved-bugs.md): the `BUGS` entries that are resolved, kept for their
  reasons, and what used to head "Where this goes next".
- [`narrowing-bugs.md`](narrowing-bugs.md): the two `BUGS` entries about the
  caretaker hop and a screen-narrowed child, in full.
- [`where-next.md`](where-next.md): what is left in "Where this goes next", in full, and the prior
  art.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

Each stem here is named for its content, and calef names files.
