# A shell at EL0: its appendices

[`notes/shell.md`](../shell.md) is the page to read: what runs, what a session looks like, and the
line between the program and the `swish` crate. This directory holds the crate's design, which the
crate's doc comments link to:

- [The `swish` crate](the-swish-crate.md): bare words, version sets, completion, what a word
  designates, and what `time` may say.
- [What `caps` prints](caps-preview.md): the reason behind each row of the preview.
- [The program](the-program.md): what the shell grants from, how a path is walked across two trees,
  and the long form of the program's `BUGS` entries, which `components/src/swish/` links to.

*Name: provisional, for the directory and the stems `the-swish-crate`, `caps-preview` and
`the-program`: the directory follows `notes/process-view/`'s shape, a page's appendices beside it.
calef names directories and files.*
