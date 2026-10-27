"""A split moves a name, not its debt: the rows of a path-keyed ratchet follow an item to its new file.

Two ratchets key their baselines by file: `helpers/name_checks.py` as `path::name check` and
`helpers/module_block_ratchet.py` as `path::name`. A change that splits a file to keep it under
§266 (a Rust source file stays under 2,000 lines) leaves the parent in place, since §282 (when a
file is split, the parent becomes an index) keeps it as the index, so git calls nothing renamed.
Every item that moved would then read as a new row at its new path, and every row it left as stale
at the old one. This pairs the two, for the gate and for `--bank` alike.

A row that appears at one path is a move when a row with the same name and the same remainder (the
check, for `name_checks`) disappears at another path in the same crate, and the sources agree: an
item of that name and kind is declared fewer times at the old path than it was, and more times at
the new one. Each item that moved carries one row per remainder, so a name that breaks two checks
moves both rows, and two `pub fn new` moved carry two. Anything else stays an addition: a new name,
a name that crossed into another crate, a name whose kind changed, or a copy that left the original
behind. A whole-file rename across crates is git's to see, and the ratchets still ask git for it.

Name: provisional, minted 2026-10-19 (UTC) by lane gates/ratchets-follow-moved-items. A shared
helper under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the
record. calef has not named it.
"""

import collections
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import name_provenance  # noqa: E402
import unit_symbols  # noqa: E402


def crate_of(path):
    """The directory holding a path's `src/`, or None for a path outside any `src/` directory."""
    if path.startswith('src/'):
        return ''
    head, sep, _ = path.partition('/src/')
    return head if sep else None


def declarations(text):
    """`Counter` of `(kind, name)` for every item and `pub` field a Rust file declares.

    Visibility is ignored on purpose: an item narrowed while it moves still moved. A module is its
    file, so `mod` is not an item here, and a field counts only where it is `pub`, since a bare
    `name:` line is as often a parameter.
    """
    out = collections.Counter()
    for line in (text or '').split('\n'):
        m = name_provenance._ITEM.match(line)
        if m:
            if m.group(1) not in ('mod', 'macro_rules!'):
                out[(m.group(1), m.group(2))] += 1
        elif line.lstrip().startswith('pub '):
            m = name_provenance._FIELD.match(line)
            if m:
                out[('field', m.group(1))] += 1
    return out


def moves(added, removed, before, after):
    """`Counter` of `(old_row, new_row)`: how many of each row in `added` moved from one in `removed`.

    `added` and `removed` are `Counter`s of `path::rest` rows; a row with no `::` (a module or a
    document) never moves here. `before(path)` and `after(path)` return a file's text where the
    removed rows held and now, or None where there is no such file.
    """
    texts = {}

    def declared(read, path):
        if (read, path) not in texts:
            texts[(read, path)] = declarations(read(path))
        return texts[(read, path)]

    # Items free to leave an old path and to arrive at a new one, per kind and remainder. An item
    # pays once per remainder, so each budget is keyed by the remainder it pays for.
    leaving, arriving = {}, {}

    def budget(table, path, kind, name, rest, gain):
        key = (path, kind, rest)
        if key not in table:
            was, now = declared(before, path)[(kind, name)], declared(after, path)[(kind, name)]
            table[key] = max(0, (now - was) if gain else (was - now))
        return key

    gone = collections.Counter(removed)
    out = collections.Counter()
    for new in sorted(added):
        to, sep, rest = new.partition('::')
        crate = crate_of(to)
        if not sep or crate is None:
            continue
        name = rest.split(' ', 1)[0]
        wanted = added[new]
        tail = rest.partition(' ')[2]
        for old in sorted(gone):
            source, _, old_rest = old.partition('::')
            old_name = old_rest.split(' ', 1)[0]
            if (wanted and source == to and old_rest.partition(' ')[2] == tail and old_name != name
                    and unit_symbols.respell(old_name) == name):
                # A unit-symbol respelling inside one file (design/naming/spelled-out-rulings.md,
                # "Unit symbols in names"): the item keeps its place on the list under its new name.
                for kind, declared_name in sorted(declared(before, source)):
                    if declared_name != old_name:
                        continue
                    lost = declared(before, source)[(kind, old_name)] - declared(after, source)[(kind, old_name)]
                    gained = declared(after, source)[(kind, name)] - declared(before, source)[(kind, name)]
                    n = min(wanted, gone[old], lost, gained)
                    if n > 0:
                        out[(old, new)] += n
                        wanted -= n
                        gone[old] -= n
            if wanted == 0 or old_rest != rest or source == to or crate_of(source) != crate:
                continue
            for kind, declared_name in sorted(declared(before, source)):
                if declared_name != name:
                    continue
                out_key = budget(leaving, source, kind, name, rest, gain=False)
                in_key = budget(arriving, to, kind, name, rest, gain=True)
                n = min(wanted, gone[old], leaving[out_key], arriving[in_key])
                if n:
                    out[(old, new)] += n
                    wanted -= n
                    gone[old] -= n
                    leaving[out_key] -= n
                    arriving[in_key] -= n
    return out


def arrivals(pairs):
    """The new rows of `moves`' pairs, as a `Counter`."""
    out = collections.Counter()
    for (_old, new), n in pairs.items():
        out[new] += n
    return out


def banked(old, now, pairs):
    """The baseline `--bank` writes: the old rows the tree still holds, and the rows that moved."""
    return (old & now) + (arrivals(pairs) & (now - old))
