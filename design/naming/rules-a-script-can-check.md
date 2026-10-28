# Rules a script can check

*An appendix to [`design/naming.md`](../naming.md), which is the rule. It says which of that page's
rules a script can apply to a name, the gate that holds them, and how a lane works the list. The file
and its stem are provisional names; naming is an architect's.*

calef, 2026-10-18 (UTC): "Honestly I think we could automate making our existing unratified names
meet our existing rules. But we need to continue to ratify and revise our rules." Nothing is
ratified by rule, because "the rules are not yet complete enough to ratify using."

## The cycle

1. calef ratifies names and revises rules.
2. A revised rule lands with its check in `helpers/name_checks.py`, and that change bumps `RULES`.
   It is the one change whose new baseline rows the gate admits, so new debt arrives with its
   cause.
3. Lanes rename the unratified names that break it until they comply. A renamed name takes
   `Name: provisional, minted <date> (UTC) by <lane>.` and is never `ratified`.

## The gate

`script/lint`'s step "names meet the current naming rules" runs `name_checks.py --check`:

- A name a change adds or renames, and that is not ratified, passes every gated check, with no
  baseline allowance.
- The unratified names that break one today are `design/naming-rule-baseline.txt`, which only
  shrinks. The change that fixes a name runs `--bank` to drop its row.
- A ratified name is exempt, since a later rule never silently overrules calef's ruling. The
  worklist ends with each ratified name that fails a current check, so he can reopen it.
- Spelling and `cap` are reported, and held by `helpers/house_style.py` and
  `helpers/cap_abbreviation.py` rather than twice.

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
4. `wire or format`: defined in `abi`, a `*_contract` crate, `network_time_protocol`
   or a format §307 (how long a name may be) lists. Another program parses
   these, so list them for calef and never rename them in a lane.

A hit that is a foreign identifier (DesignWare's `PA` for a PHY address, Intel's `IA32_*`) keeps its
owner's spelling. A lane adding one writes it into `name_checks.py`'s `KEPT` with its owner, and
the gate passes it. Names in an `extern` block are their owner's and are never checked.

## Which rules a script can check

Another package reads a name only through the defining crate's path, never the bare word. A method
or associated constant is read when its type is, or when it is called as `.name(` in a file that
names the crate. `helpers/name_crossings.py`'s `Reach` is the measure, and
`script/names --unratified` ranks by the same one.

| Rule | Kind | Checked by |
|---|---|---|
| Rust's casing for the item's kind | mechanical | `name_checks.py` |
| Abbreviations a ruling spells out (§154 (the acronym test is whether the phrase is spoken)'s table, [spelled-out-rulings.md](spelled-out-rulings.md)) | mechanical | `name_checks.py`; `cap` also by `cap_abbreviation.py`, as a ratchet |
| American spelling | mechanical | `typos --locale en-us` and `house_style.py`'s list |
| No `-d`, no Unix service word | mechanical | `script/lint`'s naming block for programs; `name_checks.py` for a trailing `d` component |
| Lowercase, hyphenated file names | mechanical | `script/lint`'s naming check 6 |
| Questions are `is_`, `has_`, `can_` or a finite verb | partly: the shape is checked, the exemptions are judgment | `name_checks.py` lists a `bool` function outside the shapes |
| Crossing a package boundary | partly: read by path, not by type | `name_crossings.Reach` |
| The acronym test for an abbreviation nobody has ruled on | judgment | not checked |
| Nouns, not verbs; generic words; the standard word | judgment | not checked |
| A word whose kernel meaning differs (`map`, `frame`, `tick`) | judgment | not checked |
| A half implies two; a foreign identifier keeps its spelling | judgment | not checked |

## BUGS

- The question-shape check reads a first word ending in `s` as a finite verb, so a plural noun
  (`bits_set`) passes. An action that reports success is told from a question only by taking
  `&mut`, so one that reports through a shared reference is listed.
- The measure reads text, not types. A method called on a value whose type the reader never names,
  through a call that is not `.name(`, is missed, and so is a re-export under another name. A reader
  outside every Cargo package (a script that parses a constant out of a `.rs` file) is not counted.
- `wire or format` is a list of packages, so a wire name defined anywhere else is reported as
  `local` or `crosses`. A lane renaming one checks that no other program parses it.
- The gate judges what the census counts: every module and document, every `pub` item, and any
  item with a block. A private or `pub(crate)` item with no block is outside it.
