# Selectors that can select nothing

A gate that picks the things it judges with a pattern, and then judges them, has a failure mode no
other gate has: when the pattern stops matching, the loop body never runs and the gate reports
clean. The green is read as an answer. This note is the enumeration milestone 401 (a gate that selects the set it judges) asked for, the
rule that came out of it, and what each guarded selector now guarantees.

Name provisional.

## The rule

A selector whose empty result is silent needs a post-selection assertion that it selected
something. A selector whose empty result raises does not.

That split is the whole finding, and it is what keeps this from being 35 assertions. Three families
go quiet:

- A shell glob whose expansion is consumed by something tolerant.
- `git ls-files <pattern>`, which prints nothing and exits 0 for a pattern that matches nothing.
- Python `glob.glob`, `Path.glob` and `Path.rglob`, which return an empty list for a directory that
  does not exist.

One family does not: `os.listdir(d)` and `pathlib.Path(d).iterdir()` raise `FileNotFoundError` when
the directory moves. `script/citations`, `script/decisions`, `script/roadmap`, `script/names`,
`script/journeys` and `script/falsifications` all read their record directories that way, so a
rename takes them red rather than quiet. That is not luck worth relying on, but it is a real
difference and it is why those sites are listed below without a guard.

## The instance milestone 401 was filed about, and why the block's account of it is wrong

The block says `script/lint`'s contract-crate check passed by checking nothing when milestone 265 (`_proto` is a truncation)
renamed every `*_proto` crate to `*_protocol`. Replayed against the tree at `51fee8248^`, the commit
immediately before 265's sweep landed, that is false. The loop was:

```sh
for dir in "$(dirname "$0")"/../crates/*proto; do
    name="$(basename "$dir")"
    case "$name" in
        *_proto) ;;
        *) echo "lint: contract crate $name should be spelled *_proto" >&2; exit 1 ;;
    esac
done
```

POSIX `sh` leaves an unmatched glob in place as a literal word, so `basename` yields the string
`*proto`, which does not match the `*_proto` arm, which means it falls to the failure branch:

```console
$ /bin/sh replay/script/lint
lint: contract crate *proto should be spelled *_proto (notes/naming.md)
$ echo $?
1
```

It would have gone red, loudly, with a message naming a crate that does not exist. That is a bad
message and a real defect. It is not a silent pass, and the difference matters: the tree has been
carrying this instance as the exhibit for a whole class, and the exhibit does not hold.

**The class is real anyway**, at four other sites, and one of them is worse than the one that was
filed.

## What was found, by site

Guarded here. Each of these could return an empty set from an ordinary rename and each treated
empty as clean.

| Site | Selector | What empty meant before |
| --- | --- | --- |
| `script/ci-build` | `names_in_tier local` | The no-argument path runs zero checks and prints `ci-build: all pass`. |
| `script/lint` CFI check | `git ls-files 'kernel/src/arch/*/*.s'` | "hand-written asm carries CFI", having read no asm. |
| `script/lint` README check | `grep -oE "script/[a-z-]+" README.md` | "every README reference resolves", having resolved none. |
| `script/lint` dead-code ratchet | `git ls-files '*.rs'` | No module-wide `#![allow(dead_code)]` anywhere, over no files. |
| `script/lint` rule 7 scan | `grep -r ... components/src fixtures/src` (all `2>/dev/null`) | No shared `#[path]` modules, over a source tree that moved. |
| `script/lint` toolchain stamp | `bench/baseline-*.txt` | `0 file(s) at <pin>`, printed as a pass. |
| `script/lint` notes index | `glob('notes/*.md')`, `glob('**/*.md')` | Every note indexed, over no notes. |
| `script/lint` host-exclusion cross-check | `Path("xtask/src").glob("*.rs")` | An empty xtask source, so every crate reads as unexcluded or none does. |

`script/ci-build` is the one that earns the milestone. It is the command a developer runs before
pushing and the enumeration CI fans out from, and its tier column is a literal string matched by
`awk`. Rename the tier and the script runs nothing and says everything passed.

Already guarded, found and left alone:

- `script/vendor-verify` and `script/vendor-watch` both exit on an empty `vendor/*.pin`, with a
  message that says nothing claims to be vendored.
- `script/mutation-census` exits when a downloaded run has no `caught.txt` under it.
- `script/fatal-risks`' `no-history` check fires when a shallow clone makes its own check unable to
  fail, on the stated ground that a check which cannot fail is worse than one that is absent. That
  comment is the argument for this whole note, written by a gate about itself.

Read and judged not to need one:

- The `os.listdir` and `iterdir` record readers listed under the rule above: they raise.
- `script/lint`'s `git grep` finders (the TODO-milestone scan, the forbidden-vocabulary scan, the conflict-marker
  scan). These look for bad things rather than selecting a set to judge, so empty genuinely means
  clean and an assertion would be a false alarm by construction.
- `script/audits`' `crates/*` count, which feeds a threshold rather than a verdict. An empty count
  delays an audit trigger; it does not report a pass.

## BUGS

- **A non-empty assertion is not a correct selector.** A glob matching three of fifteen crates
  passes every assertion added here and still covers a fifth of its subject. Catching that needs the
  right count, which is the hand-kept list `script/verify` and `script/falsifications` both moved
  away from. Nothing here addresses it.
- **Nothing gates the convention.** A new selector added tomorrow gets no assertion unless its author
  remembers this note, which is rung three. A lint for it would have to tell a selector from a
  finder in someone else's shell, and the two are the same syntax.
- **The `--exclude` family is not covered.** `cargo` accepts an unknown `--exclude` silently, and
  `.cargo/mutants.toml`'s exclusion globs are not an error when they match nothing. Those are the
  adjacent failure `design/naming.md` already records, and they need a different mechanism: an
  exclusion that goes stale still covers everything else, so a non-empty test says nothing about it.
- The tell is still the durable half, and no machine can check it. A gate that passed before your
  change and passes after it, on a change that is exactly what the gate is about, has probably
  stopped looking.

## EXAMPLES

Show the selection going empty, which is the input the guard now refuses. Rename the tier and ask
the same question `names_in_tier` asks:

```console
$ sed 's/|local|/|default|/' script/ci-build | awk -F'|' '$2 == "local" { print $1 }' | wc -l
       0
```

Before this milestone the loop over that empty list ran nothing and the script finished with
`ci-build: all pass`. It now exits 1 and says the tier selected no checks.

Confirm the replay of the historical check for yourself, which is the measurement this note's
correction rests on:

```console
$ mkdir -p /tmp/replay/script /tmp/replay/crates/file_system_contract
$ printf '#!/bin/sh\n' > /tmp/replay/script/lint
$ git show 51fee8248^:script/lint | sed -n '1856,1862p' >> /tmp/replay/script/lint
$ /bin/sh /tmp/replay/script/lint; echo "exit=$?"
lint: contract crate *proto should be spelled *_proto (notes/naming.md)
exit=1
```
