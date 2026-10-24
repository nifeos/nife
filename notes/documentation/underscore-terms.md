# The writer and the reader must agree on what a word is, and did not

*Name: provisional.*

An appendix to [`notes/documentation.md`](../documentation.md), which states the conclusion.

`documentation::index::normalize` folds a query by dropping every byte that is not a letter or a digit, so
a reader who types `line_editor` looks up `lineeditor`. `documentation::index::tokens` split the *text* on
that same byte, so the builder only ever wrote `line` and `editor`. The term the query asks for
was one no page could ever have. In a repository whose prose is full of `snake_case` identifiers,
that is most of what anybody would search for: `apropos fs_proto` and `apropos grant_plan` both
answered "nothing says that" while dozens of pages said exactly that.

The test that should have caught it asserted the property in its own first comment, *"the builder's
tokeniser and the reader's query normalizer must agree, byte for byte, on what a term is"*, and
then checked a word with no underscore in it. Same shape as the fence: a claim in prose, a weaker
thing checked.

The fix is in the writer rather than the reader, and that is the choice worth recording.
`line_editor` now yields three terms, `line`, `editor` and `lineeditor`, so `apropos editor`
keeps working and `apropos line_editor` starts. Narrowing `normalize` to stop at the underscore
would have been smaller and would have made `apropos line_editor` silently search for `line`, which
is a worse answer than no answer.

Only the underscore joins. It is unambiguous in this tree and the hyphen is not: joining across
`-` would manufacture a term out of every hyphenated phrase in the corpus, and `notes/glob-grant.md`
is a filename rather than a word. The renderer narrowed emphasis on exactly this reasoning and
records it in its own `BUGS`.
