# Prior art

An appendix to [the milestone](../1094-grant-plan-outside-review.md).

How the crate sits against what a reader already knows. Download figures are recent (90 days) and
all time, fetched from crates.io during the review; everything else in the table comes from memory of
those projects' documentation rather than a re-read, and carries an unverified mark where it matters.

| what | downloads | what it does | what a user coming from it will miss here |
|---|---|---|---|
| `shlex` 2.0 | 351M / 1.08G | Splits a string into POSIX-style words, joins and quotes them; allocates `String`s | Backslash escapes, joining `a"b"` into one word, `std` and allocation. Here every token is a slice of the line and a partly quoted word is a refusal |
| `shell-words` 1.1 | 48.3M / 179M | POSIX word splitting and quoting into `Vec<String>` | The same; also `$`-free, which this crate shares |
| `clap_lex` 1.1 | 252M / 1.05G | The lexer under clap: `--long=value`, `-abc` clusters, `--` | `--long=value`, `--` as an end of options, a `-` operand. Here `--mem N` is the one long option, the planner checks a cluster against a manifest's letters, and any other `-x` is a refusal |
| `lexopt` 0.3 | 3.29M / 15.0M | Minimal pedantic argument parser over `OsString` | Long options other than one, option values as `=` |
| `pico-args` 0.5 | 26.5M / 93.5M | Small positional and option parser | Free-form options |
| POSIX `sh` grammar (recalled) | | Word splitting, quote removal, `2>` recognized only when the digit begins the token, `<<` here-documents, `$`, globbing with pass-through on no match | Here-documents, variables, `\|\|` and `&&` inside this crate (the shell splits those first), quote joining. The `2>` word-boundary rule matches POSIX |
| zsh `NOMATCH` and bash `failglob` (recalled) | | A pattern matching nothing is an error | The default here; `Refusal::NoMatch`. the crate refuses bash's default pass-through because the pattern would name a file nobody has |
| Plan 9 `rc` and `bind` (recalled, unverified) | | `rc` quotes with `'` only and never rewrites a word; `bind` names a position in a per-process namespace | `Bindings` is `bind`'s shape, four entries, no union; `..` at the holder's root draws a refusal rather than climbing |
| Shill (OSDI 2014, recalled) | | Capability contracts declared by scripts, checked before the script runs | The `Manifest` is Shill's contract shrunk to a fixed struct; `plan` is the check at the prompt |
| Plash `pola-shell` (recalled, unverified) | | A shell whose command line designates what the child may open; a filename on the line is a grant | The closest ancestor of "designation is authorization" at a prompt. Plash passed grants through a file-server proxy; here a `DirectoryGrant` names a directory plus a bounded set of names for a caretaker to serve |

Where this crate differs: it has no allocation, no `std` and no `unsafe`. Its tokens are slices of
the line. Every way a line can fail has a typed `Refusal` with a fixed sentence. A manifest
classifies positionals, rather than position or sigil. Expansion is a grant: a bounded `NameSet` the
child receives, never an argv of paths. A batch sweep resumes at a name, so the sweep covers a shrinking directory
exactly once. What it costs: a 16-token line, a four-stage pipeline, eight names of 512
bytes per grant, and eight path levels of 16-byte directory names. The shell copies values of two kilobytes
by value through its stack.

Concrete example. In bash, `rm *.txt` with nine matches expands to nine paths and `rm` opens each
with the user's whole authority. Here the same line is `Refusal::TooManyNames` and nothing runs.
`xargs rm *.txt` sweeps the match in batches of at most eight names, each batch a `DirectoryGrant`
whose caretaker serves exactly those names. The second batch resumes after the last name of the
first, so a name `rm` removed does not shift what the next batch sees
(`a_sweep_of_a_shrinking_directory_takes_every_name_once`).
