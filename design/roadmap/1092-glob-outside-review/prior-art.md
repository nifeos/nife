# Prior art

An appendix to [the milestone](../1092-glob-outside-review.md).

How other matchers answer the questions this crate answers, as the reviewer knows them. Entries
marked unverified were not checked against source during this review.

| matcher | input | `**` | escape | malformed pattern | leading dot by default | worst case |
|---|---|---|---|---|---|---|
| this crate | bytes, one name | no | `\x`, also inside `[...]` | means itself; no error type | special (`*` skips `.x`) | polynomial, bounded by `cost_bound`, proven to 3 bytes, tested at 100,000 |
| rust-lang `glob` 0.3 | `str`, paths | yes | none; `[*]` and `Pattern::escape` | `PatternError` | ordinary (`require_literal_leading_dot: false`) | iterative single-backtrack loop (unverified) |
| `globset` 0.4 (ripgrep) | bytes, paths | yes, and `{a,b}` | `\` on Unix, optional | error | ordinary | compiles to a regex automaton; linear |
| `glob-match` 0.2 | `str` | yes, `{}` and `!` | `\` | returns false | ordinary | single pass with one backtrack per `*` and one per `**` (unverified) |
| `wildmatch` 2 | `str` | no | none | n/a | ordinary | `*` and `?` only, no classes (unverified for the latest version) |
| glibc `fnmatch(3)` | bytes or multibyte | no | `\`, unless `FNM_NOESCAPE` | `[` with no `]` is literal | ordinary; `FNM_PERIOD` opts in | recursive on `*`; exponential patterns exist (unverified for current glibc) |
| musl `fnmatch` | bytes or multibyte | no | `\` | literal `[` | ordinary; `FNM_PERIOD` | linear-time: one backtrack point and a tail match from the last `*` |
| OpenBSD `fnmatch(3)` | bytes | no | `\` | literal `[` | ordinary; `FNM_PERIOD` | rewritten non-recursive after a stack-exhaustion denial of service (unverified detail) |
| Go `path.Match` | `str`, paths | no | `\` | `ErrBadPattern` | ordinary | linear per segment, one backtrack per `*` |
| Python `fnmatch` | `str` | no | none | literal | ordinary | compiled to a regex; `glob.has_magic` is the same question as this crate's |
| bash pathname expansion | bytes | with `globstar` | `\` | literal | ordinary; `dotglob` opts in; 5.2 `globskipdots` hides `.` and `..` | implementation-defined |

The algorithm here is the one Russ Cox described in "Glob Matching Can Be Simple And Fast Too"
(2017). It keeps only the most recent `*` as a backtrack point, because anything an earlier `*`
could absorb a later one can absorb instead. That article cataloged the exponential behavior of the
naive matcher in several language runtimes. The README should cite it, since the crate's record
argues the point from scratch. The article also records why the method does not extend to `**`. That
is a second reason, beside the authority argument in `notes/glob-scope.md`, that `**` belongs in a
layer above.

This crate differs from all of the above in six ways, and a user coming from them meets each. It
matches bytes, so `?` is half of `é`. It matches one name, so `/` is a byte. It has no error type,
so a typo is a file name. A leading dot is special by default, so `matches(b"*", b".config")` is
false where every other library says true. A caller can compute the cost before matching. Six
machine-checked proofs ship with the patches that make them fail. The concrete example is the one
the record gives. The same function decides `rm *.txt` and `echo *.txt`, so what the user reads matches what the command receives. No other matcher in the table decides authority, and none needed to make its worst case checkable.
