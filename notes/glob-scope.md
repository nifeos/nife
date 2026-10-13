# What glob deliberately does not match

*Name: provisional. The design argument behind `crates/glob`'s module doc, kept here because §267
(a comment states the constraint as it is now) puts the argument in a note and the constraint in
the doc.*

The matcher answers one question, does this pattern match this name, and two whole features of
other glob engines are refused here rather than deferred.

## Recursive descent is out, permanently

`**` is **out of scope for this crate, permanently, not "not yet".** Two reasons, and the second
is the real one.

- It needs a path separator, and the matcher still has no business owning one. Milestone 47
  (navigation and naming) settled the syntax on 2026-08-18, Plan 9's way. A path is resolved in
  the client, `/` is the root of the holder's own namespace, and the FS server still sees one
  component per request. The resolution lives in `grant_plan::nav` and in the `std` PAL, both of
  which walk components against capabilities. A matcher that also split paths would be a second,
  unauthorized resolver.
- `**` is not a matching feature. It is a traversal feature. `*` says "consider these bytes";
  `**` says "and also descend into that directory, and the one below it". Descending means
  opening a subdirectory, which in this system means holding a capability for it, which is
  enumeration and granting. Putting `**` in a string matcher hides an authority question inside a
  pure function, which is the exact mistake this OS exists to not make.

So the crate matches **one name**, a single path component, the thing `filesystem_protocol`
actually carries. When path syntax is settled, recursive descent lands as a traversal layer above
the crate, walking directory capabilities and calling `matches()` per component. That layer is
where `**` belongs, because that is where the authority to descend is.

The honest cost, stated where you meet it: nothing in the crate treats `/` as special. Hand
`matches()` a whole path and `*` will happily match across separators, because to the crate a `/`
is a byte like any other. That is a caller error, not a mode. The type system cannot catch it
while a name is `&[u8]`, so it is written down instead.

## Glob qualifiers are out, and the reason is authority

zsh's qualifiers are the best thing in its glob engine (`*(.)` for regular files, `*(om[1])` for
the newest, `*(Lm+1)` for over a megabyte) and none of them are here. The roadmap said to settle
this before building the matcher around them, so: settled, out, and the crate is not built around
them.

It is not squeamishness about scope. A qualifier needs type, mtime and size per candidate, so one
`enumerate` becomes N `FSTAT` calls and needs a read right beyond enumerate. That turns
`echo *(.)`, which reads like a display, into an operation that requires more authority than
listing the directory. In a capability system that is a change to what the command is, not a
feature flag. If qualifiers ever arrive they arrive as a separate, visible step over an already
enumerated set, with the extra right named, and the matcher stays a function of two byte strings.

POSIX character classes (`[[:alpha:]]`) are out for a smaller reason: the inner `[:` and `:]` are
not syntax here, so `[[:alpha:]]` parses as the class `[[:alpha:]` (members `[`, `:`, `a`, `l`,
`p`, `h`) followed by a literal `]`. That matches the two-byte names `[]`, `:]`, `a]`, `l]`, `p]` and
`h]`. Locale-dependent classes have no meaning on a system with no locale, and a wrong answer
that is quiet is worse than a missing feature, so it is written down here and pinned by a test.
