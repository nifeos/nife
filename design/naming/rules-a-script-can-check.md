# Rules a script can check

*An appendix to [`design/naming.md`](../naming.md#rules-a-script-can-check), which states the rules.
It holds which naming rules a script applies, the gates that hold them, and how a lane works the
list of names that break one. The directory `design/naming/` and this file's stem are provisional
names; naming is an architect's.*

## The cycle

1. An architect ratifies names and revises rules.
2. A revised rule lands with its check in `helpers/name_checks.py`, and that change bumps `RULES`.
   It is the one change whose new baseline rows the gate admits, so new debt arrives with its cause.
3. Lanes rename the unratified names that break it until they comply. A renamed name takes
   `Name: provisional, minted <date> (UTC) by <lane>.` and is never `ratified`.

## The gate

`script/lint`'s step "names meet the current naming rules" runs `name_checks.py --check`:

- A name a change adds or renames, and that is not ratified, passes every gated check, with no
  baseline allowance.
- The unratified names that break one today are `design/naming-rule-baseline.txt`, which only
  shrinks. The change that fixes a name runs `--bank` to drop its row.
- A ratified name is exempt, since a later rule never silently overrules a ruling. The worklist ends
  with each ratified name that fails a current check, so an architect can reopen it.
- Spelling and `cap` are reported here and held by `helpers/house_style.py` and
  `helpers/vendored_name_gates.py`, not twice.

## The worklist

```
python3 helpers/name_checks.py                    # every unratified name that breaks a checked rule
python3 helpers/name_checks.py crates/e1000e      # one area
```

Each line gives the name, its reach and the checks it fails, in the order to work them:

1. `local`: no other Cargo package reads it. Rename it in its package, one area per pull request.
2. `crosses into N`: N other packages read it. Rename by family, so a type and the names that follow
   it move together.
3. Crates, programs, scripts and documents: a newcomer meets these first. Rename by family.
4. `wire or format`: defined in `abi`, a `*_contract` crate, `network_time_protocol` or a format
   [§307 (how long a name may be)](../decisions/0307-how-long-a-name-may-be.md) lists. Another
   program parses these, so list them for an architect and never rename them in a lane.

A hit that is a foreign identifier (DesignWare's `PA` for a PHY address, Intel's `IA32_*`) keeps its
owner's spelling. A lane adding one writes it into `name_checks.py`'s `KEPT` with its owner, and the
gate passes it. Names in an `extern` block are their owner's, and the check never reads them.

## The naming block in `script/lint`

The block's first five checks are cheap greps, because lint runs constantly. Take the count from the
script.

1. No name ending in `-d`, over `components/src/*.rs`, `fixtures/src/*.rs`, the `[[bin]]` names and
   `crates/*`, four characters or more, so a three-letter name ending in `d` reads as an
   abbreviation. A real word ending in `d` goes in `naming_allow` with a reason (`uuid`).
2. The Unix service word appears nowhere outside `design/` and `script/lint`, which have to name it
   to argue about it.
3. Contract crates spelled `*_contract`, with `network_time_protocol` as the one exception.
4. The current branch is not a near miss of a milestone claim
   ([documents-numbers-and-branches.md](documents-numbers-and-branches.md#branches)).
5. No `#[path]` module shared by two or more binaries (`AGENTS.md` rule 7). A module with one
   consumer is an ordinary submodule. The allow-list is empty, which is the intended steady state.
6. `notes/` and `design/` file names are lowercase and hyphenated, `README.md` aside.
7. Every crate, program and `script/` entry carries exactly one `Name:` block in a known state, in
   the plain spelling the parse reads ([provenance.md](provenance.md)). Presence and form only: it
   never checks that a reason is still true, and never keys on `ratified`.

A separate check refuses a count above two before "halves" ([vocabulary.md](vocabulary.md)).

## Which rules a script can check

Another package reads a name only through the defining crate's path, never the bare word. A method
or associated constant counts as read when its type is, or when a file that names the crate calls it
as `.name(`. `helpers/name_crossings.py`'s `Reach` is the measure, and `script/names --unratified`
ranks by the same one.

| Rule | Kind | Checked by |
|---|---|---|
| Rust's casing for the item's kind | mechanical | `name_checks.py` |
| Short forms a ruling spells out ([acronyms-and-abbreviations.md](acronyms-and-abbreviations.md)) | mechanical, for the forms the template's `[spelled_out]` list holds | `name_checks.py`; `cap` also by `vendored_name_gates.py`, as a ratchet |
| Units spelled out ([design/naming.md](../naming.md#units)) | mechanical, for the symbols the helper counts | `vendored_name_gates.py`, as a ratchet |
| American spelling | mechanical | `typos --locale en-us` and `house_style.py`'s list |
| No `-d`, no Unix service word | mechanical | the naming block for programs; `name_checks.py` for a trailing `d` component |
| Lowercase, hyphenated file names | mechanical | the naming block's check 6 |
| Questions are `is_`, `has_`, `can_` or a finite verb | partly: the shape is checked, the exemptions are judgment | `name_checks.py` lists a `bool` function outside the shapes |
| Crossing a package boundary | partly: read by path, not by type | `name_crossings.Reach` |
| The acronym test for an abbreviation nobody has ruled on | judgment | not checked |
| Nouns, not verbs; generic words; the standard word | judgment | not checked |
| A word whose kernel meaning differs (`map`, `frame`, `tick`) | judgment | not checked |
| A half implies two, beyond the literal count; a foreign identifier keeps its spelling | judgment | not checked |

No gate catches the jargon failure. `linedisc` would have passed every check above, and what caught
it was a person reading the name and not knowing it. That remains the test.

## BUGS

- The question-shape check reads a first word ending in `s` as a finite verb, so a plural noun
  (`bits_set`) passes. Only `&mut` tells an action that reports success from a question, so the
  check lists one that reports through a shared reference.
- The measure reads text, not types. It misses a method called on a value whose type the reader
  never names, through a call that is not `.name(`, and a re-export under another name. It does not
  count a reader outside every Cargo package (a script that parses a constant out of a `.rs` file).
- `wire or format` is a list of packages, so the worklist reports a wire name defined anywhere else
  as `local` or `crosses`. A lane renaming one checks that no other program parses it.
- The gate judges what the census counts: every module and document, every `pub` item, and any
  item with a block. A private or `pub(crate)` item with no block is outside it.
- The naming block reads the filesystem for names and `git grep` for the service word, so an
  untracked file is invisible until someone adds it. Check 1 sees names, not behavior: a component
  whose name passes and whose behavior is a daemon is not its problem.
