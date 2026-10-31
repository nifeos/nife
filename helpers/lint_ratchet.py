"""The lint ratchet: clippy lints that are counted against a baseline that only falls.

Built by milestone 888 (a ratchet on function length) to the specification in
`design/roadmap/0896-panics-in-the-kernel-and-the-parsers-are-counted-and-fall.md` ("The
mechanism, and its rung"), so the other lint proposals register a lint here instead of writing a
gate. It copies `helpers/file_length_ratchet.py`'s rules (milestone 841 (a ratchet on Rust file length)), which
copied the prose
ratchet's.

    python3 helpers/lint_ratchet.py --flags              # the --force-warn arguments the clippy sections add
    python3 helpers/lint_ratchet.py --check DIR          # judge the clippy sections' saved JSON under DIR; what script/lint runs
    python3 helpers/lint_ratchet.py --bank               # lower rows to the tree; never raises or adds
    python3 helpers/lint_ratchet.py --selftest           # the rules, as fixtures
    python3 helpers/lint_ratchet.py --init               # write the baseline from scratch (milestone 888 only)

Name: provisional, minted by milestone 888's lane on 2026-10-11 (UTC). A shared module under
`helpers/`, which `script/names` puts out of its own scope, so its provenance is this paragraph.

**Why a separate verdict, and why not a separate pass.** §61 (a lint is adopted on evidence from this
tree) keeps `script/lint` at `-D warnings`, so a lint in `[workspace.lints]` must be clean on the
day it lands. A lint with hundreds of hits can only be adopted as a count that falls. The clippy
sections of `script/lint` run through `helpers/clippy_record.py`, which adds the ratcheted lints as
`--force-warn` (a `-D warnings` does not turn that into an error, and a source `allow` cannot hide
a site from it) and saves clippy's JSON under `target/lint-clippy/`, so the ratchet reads what
those sections already linted and runs no clippy of its own (milestone 980, part 6; it ran a second
pass over every configuration, 23 to 26 seconds). The configurations are the lint census's five
(`notes/lint-census-2026-10-09.md`) with `--all-targets` where `script/lint`'s own passes build test
code, plus `system_tests` and the host-only workspaces; `CONFIGURATIONS` names each and the cargo
arguments its section must have run with. A nested workspace's JSON sits at its own path under the
directory (`DIR/fuzz/clippy.json`), which is how `--check` puts its workspace-relative paths back
under the repository root. A site compiled for several targets counts once, so a parity gap cannot
hide a hit.

**A saved file that is not the tree's current answer fails loudly, naming the file.** Before it
reads a count, `--check` refuses (and exits non-zero on) every one of these, for each configuration:
the JSON or its sidecar is missing or unreadable; the JSON's SHA-256 is not the one the sidecar
recorded; the clippy run exited non-zero (its own section failed above); the rustc flags the
section ran with do not hold every ratcheted lint as `--force-warn` or hold an `-A` or `-W` of one;
the cargo arguments lack the selection `CONFIGURATIONS` requires, or select `--features`; or the run
started before the newest input of the tree. **The freshness rule:** the sidecar records the time
the clippy run started, and the file is stale when any tracked or untracked-unignored file changed
after that, except Markdown, Python and shell files and anything under `design/`, `notes/`,
`briefs/`, `.github/` and `script/`, which no compilation reads. That is every source file, `Cargo.toml`, `Cargo.lock`, `clippy.toml` and
`rust-toolchain.toml`, by modification time. A file edited while clippy ran counts as newer than the
run, because the run may have read it before the edit. In `--no-cargo` mode the section is not run
(it is a cargo section by `helpers/lint-no-cargo.awk`'s rule), as the pass it replaces was not.

**Two kinds of row.** A `count` lint (the panic proposal's kind) has one row per file: the number of
hits in it, held to the baseline, with no hit allowed in a file that has no row. A `measure` lint
has one row per item over a threshold, keyed by file and item, holding the item's size. The only
lint registered today is `too_many_lines`, a measure keyed by function, at 100 code lines (calef,
2026-10-09 UTC). The panic proposal registers its own lints when it is built.

**The rules**, the file-length ratchet's, per row:

- An item over the threshold, or a file with a hit, that has no row fails.
- A listed row fails if its value grew against the merge base, or is over its row.
- A row is a ceiling that `--bank` lowers and nothing raises. A row whose item is gone, or at or
  under the threshold, or whose file has no hits, fails until it is removed.
- Against the merge base, no row may be added and no row may rise. A renamed file moves its rows
  (git's rename detection). A renamed function moves its row: an added row is a move when a base
  row's function is gone from the tree, the two bodies share at least half their code lines (git's
  own `-M` default), and the number does not rise.

**Function length is the text measure, held to clippy.** `helpers/function_length.py` reproduces
`too_many_lines` from source text. The row value is that measure, over its scope: every tracked
`.rs` file outside `vendor/`, test code included (§283 (a function is held to 100 code lines by a
lint ratchet)). It also reaches files no clippy pass here compiles: about 1,160 functions in
`patches/`, `c_library/`, the UEFI loader, the std exercisers, `jig/` and kernel build modes, which
build only for nife's std targets or in a separate step. Those are held by the text measure alone.
On every function clippy does report, `--check` requires the two to agree exactly, so a clippy
upgrade that counts differently fails here loudly rather than drifting the text measure away from
clippy.

**A pass with no hit still checked something.** With no function over the threshold, clippy
reports nothing, so an empty report cannot be the sign of a pass that never ran. `--check` instead
requires every JSON file to end in a successful build and to name at least one in-scope target
clippy compiled (milestone 401 (a gate that selects the set it judges can pass by checking nothing)).

**The failure message** says the remedy, as §266 (a Rust source file stays under 2,000 lines)'s does: split the function at a seam, one step
or phase per function. Joining lines or deleting code to buy a number is not the remedy, and
comments are already not counted, so stripping them buys nothing.
"""

import difflib
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import function_length as fl  # noqa: E402

BASELINE = 'design/lint-baseline.tsv'

# Where helpers/clippy_record.py saves what the clippy sections print, and the configurations the
# ratchet needs from them: name (a path under the directory, no extension) to the tokens that
# section's cargo arguments must hold. The names are script/lint's section arguments; a section
# that narrows its selection fails here and not silently.
SAVED = 'target/lint-clippy'
CONFIGURATIONS = {
    'host': ('--workspace', '--all-targets', 'kernel', 'system_tests'),
    'aarch64': ('-p', 'kernel', 'components', 'fixtures', 'user_mode_runtime',
                'aarch64-unknown-none-softfloat', '--all-targets'),
    'system_tests': ('-p', 'system_tests', 'aarch64-unknown-none-softfloat', '--all-targets'),
    'riscv64': ('-p', 'kernel', 'riscv64imac-unknown-none-elf'),
    'x86_64': ('-p', 'kernel', 'x86_64-unknown-none'),
    'redoxfs_server/clippy': ('--manifest-path', 'redoxfs_server/Cargo.toml', '--all-targets'),
    'tools/redoxfs_host/clippy': ('--manifest-path', 'tools/redoxfs_host/Cargo.toml',
                                  '--all-targets'),
    'tools/glyph_atlas_generator/clippy': ('--manifest-path',
                                           'tools/glyph_atlas_generator/Cargo.toml',
                                           '--all-targets'),
    'fuzz/clippy': ('--manifest-path', 'fuzz/Cargo.toml', '--all-targets'),
}
# Files whose change cannot alter what clippy reports (the freshness rule's exception): prose,
# records, workflows, and the scripts and Python helpers that never reach rustc. A `.rs` file under
# `helpers/` (the Kani shim) is not excepted.
NOT_COMPILED = ('design/', 'notes/', 'briefs/', '.github/', 'script/')
NOT_COMPILED_SUFFIXES = ('.md', '.py', '.sh')

# name: (kind, threshold, scope). `scope` says which files are counted; widening it is a reviewed
# edit to this table, which is the panic proposal's "the scope is the baseline's".
LINTS = {
    'too_many_lines': ('measure', fl.THRESHOLD, fl.in_scope),
}

HEADER = """\
# The lint ratchet's baseline: helpers/lint_ratchet.py, from milestone 888 (a ratchet on function
# length). One row per ratcheted finding: the clippy lint, the file, the item (a function's name for
# a measure lint, `-` for a per-file count), and the value (the function's code lines, or the file's
# hit count).
# A value is a ceiling that may only go DOWN: lower it with `python3 helpers/lint_ratchet.py --bank`,
# which never raises and never adds. A row is removed in the same change that takes its item to the
# threshold or below, or deletes it. A rename moves its row at the same number. script/lint compares
# this file against the merge base, so a row cannot be added and a number cannot rise.
"""

REMEDY = {'too_many_lines': 'The remedy is to split the function at a seam, one step or phase per '
                            'function; joining lines buys nothing a reader can use, and comments '
                            'are not counted.'}


def fmt(n):
    return f'{n:,}'


# --- the baseline ---------------------------------------------------------------------------------

def read_baseline(text):
    rows = {}
    for line in text.splitlines():
        if not line.strip() or line.startswith('#'):
            continue
        cells = line.split('\t')
        if cells[0] == 'lint':
            continue
        rows[(cells[0], cells[1], cells[2])] = int(cells[3])
    return rows


def write_baseline(rows):
    out = [HEADER, 'lint\tpath\titem\tvalue\n']
    for key in sorted(rows):
        out.append('\t'.join(key) + f'\t{rows[key]}\n')
    with open(BASELINE, 'w') as f:
        f.write(''.join(out))


def show(key):
    lint, path, item = key
    return f'{path} {item}' if item != '-' else path


# --- clippy's JSON --------------------------------------------------------------------------------

def clippy_files(directory):
    """[(json path, workspace prefix)] for every configuration's JSON that exists under
    `directory`. A file in a subdirectory came from the nested workspace at that path
    (`fuzz/clippy.json`)."""
    out = []
    for name in sorted(CONFIGURATIONS):
        path = os.path.join(directory, name + '.json')
        if os.path.exists(path):
            out.append((path, os.path.dirname(name) + '/' if '/' in name else ''))
    return out


def newest_input():
    """(modification time in ns, path) of the newest file a compilation could read."""
    names = (git('ls-files', '-z', '--cached', '--others', '--exclude-standard') or '').split('\0')
    newest = (0, '')
    for name in names:
        if not name or name.endswith(NOT_COMPILED_SUFFIXES) or name.startswith(NOT_COMPILED):
            continue
        try:
            newest = max(newest, (os.stat(name).st_mtime_ns, name))
        except OSError:
            continue  # a tracked file deleted in the working tree
    return newest


def saved_problems(directory, newest=None):
    """Every reason a configuration's saved JSON cannot be trusted as the current tree's answer."""
    import hashlib
    newest = newest or newest_input()
    flags = ratchet_flags()
    bad = []
    for name, tokens in sorted(CONFIGURATIONS.items()):
        path = os.path.join(directory, name + '.json')
        meta_path = os.path.join(directory, name + '.meta')
        try:
            with open(path, 'rb') as f:
                digest = hashlib.sha256(f.read()).hexdigest()
        except OSError as e:
            bad.append(f'{path}: the clippy section\'s saved JSON is missing or unreadable '
                       f'({e.strerror or e}). Run the clippy sections of script/lint first')
            continue
        try:
            with open(meta_path) as f:
                meta = json.load(f)
            ran_flags, args, started, status = (meta['rustc_flags'], meta['cargo_args'],
                                                int(meta['started_ns']), meta['exit'])
        except (OSError, ValueError, KeyError, TypeError) as e:
            bad.append(f'{meta_path}: the sidecar that says how {path} was made is missing or '
                       f'unreadable ({e}). Run the clippy sections of script/lint again')
            continue
        if meta.get('sha256') != digest:
            bad.append(f'{path}: its SHA-256 is not the one {meta_path} recorded, so it is not '
                       f'what the section wrote. Run the clippy sections again')
        if status != 0:
            bad.append(f'{path}: clippy exited {status} when it was run, so its own section '
                       f'failed; fix that section first')
        text = ' '.join(ran_flags)
        ours = {f'clippy::{lint}' for lint in LINTS}
        undone = [f'{a} {b}' for a, b in zip(ran_flags, ran_flags[1:])
                  if a in ('-A', '-W', '--allow', '--warn') and b in ours]
        if undone or ran_flags[-len(flags):] != flags:
            bad.append(f'{path}: it was made with rustc flags `{text}`, and the ratchet needs '
                       f'`{" ".join(flags)}` and no `-A` or `-W` of the same lint. Run the clippy '
                       f'sections of script/lint again, through helpers/clippy_record.py')
        missing = [t for t in tokens if t not in args]
        if missing or '--features' in args:
            bad.append(f'{path}: its section ran `cargo {" ".join(args)}`, which is not the '
                       f'selection the ratchet needs for {name} (lacks {missing or "nothing"}; '
                       f'`--features` is not allowed). Edit CONFIGURATIONS and the section together')
        if newest[0] > started:
            bad.append(f'{path}: stale. Its clippy run started before {newest[1]} changed, and a '
                       f'change to a compiled file can change what clippy reports. Run the clippy '
                       f'sections of script/lint again')
    return bad


def ratchet_flags():
    return [w for lint in sorted(LINTS) for w in ('--force-warn', f'clippy::{lint}')]


def clippy_messages(path):
    with open(path) as f:
        for line in f:
            try:
                yield json.loads(line)
            except ValueError:
                continue


def unchecked(directory):
    """The JSON files that do not show a pass that compiled something in scope: no successful
    `build-finished`, or no compiler artifact for an in-scope target."""
    root = os.path.realpath(os.getcwd())
    bad = []
    for path, _prefix in clippy_files(directory):
        finished = targets = 0
        for m in clippy_messages(path):
            if m.get('reason') == 'build-finished' and m.get('success'):
                finished = 1
            if m.get('reason') != 'compiler-artifact':
                continue
            src = os.path.realpath((m.get('target') or {}).get('src_path') or '/')
            if src.startswith(root + '/') and fl.in_scope(os.path.relpath(src, root)):
                targets += 1
        if not finished or not targets:
            bad.append(os.path.relpath(path, directory))
    return bad


def clippy_hits(directory):
    """{(lint, path, line): message} from every `*.json` clippy wrote under `directory`.

    A site reported by several configurations is one key. A path clippy gives as absolute, or
    relative to a nested workspace, is made relative to the repository root.
    """
    hits = {}
    root = os.getcwd()
    for name, prefix in clippy_files(directory):
        for m in clippy_messages(name):
            if m.get('reason') != 'compiler-message':
                continue
            msg = m['message']
            code = ((msg.get('code') or {}).get('code') or '').split('::')[-1]
            spans = [s for s in msg['spans'] if s.get('is_primary')]
            if code not in LINTS or not spans:
                continue
            path = spans[0]['file_name']
            path = os.path.relpath(path, root) if path.startswith('/') else prefix + path
            hits[(code, path, spans[0]['line_start'])] = msg['message']
    return hits


def agreement(hits, fns):
    """Every `too_many_lines` hit must name a function the text measure found, at the same count."""
    at = {(path, start): (name, n) for (path, name), (start, n, _lines) in fns.items()}
    bad = []
    for (lint, path, line), message in sorted(hits.items()):
        if lint != 'too_many_lines' or not fl.in_scope(path):
            continue
        m = re.search(r'\((\d+)/(\d+)\)', message)
        n = int(m.group(1)) if m else None
        found = at.get((path, line))
        if found is None:
            bad.append(f'{path}:{line}: clippy reports a function of {n} code lines that '
                       f'helpers/function_length.py does not find. Fix the text measure before '
                       f'this gate can hold the function')
        elif found[1] != n:
            bad.append(f'{path}:{line}: {found[0]} is {n} code lines to clippy and {found[1]} to '
                       f'helpers/function_length.py. The text measure and clippy must count alike')
    return bad


def counted(hits):
    """{(lint, path, '-'): hits} for the `count` lints."""
    out = {}
    for (lint, path, _line) in hits:
        kind, _threshold, scope = LINTS[lint]
        if kind == 'count' and scope(path):
            key = (lint, path, '-')
            out[key] = out.get(key, 0) + 1
    return out


def measured(fns):
    """{('too_many_lines', path, name): n} for every function over the threshold."""
    _kind, threshold, scope = LINTS['too_many_lines']
    return {('too_many_lines', path, name): n for (path, name), (_s, n, _l) in fns.items()
            if n > threshold and scope(path)}


# --- the rules, pure so the selftest can drive them -----------------------------------------------

def similar(a, b):
    """The share of code lines two bodies have in common, git's rename-similarity in spirit."""
    if not a or not b:
        return 0.0
    return difflib.SequenceMatcher(None, a, b, autojunk=False).ratio()


def problems(values, rows, base_values=None, base_rows=None, renames=(), bodies=None,
             base_bodies=None):
    """Every failure in one tree.

    `values` is the tree's {key: value} over the threshold (measure) or with a hit (count); `rows`
    the baseline; `base_values` and `base_rows` the same at the merge base, where there was one;
    `renames` git's (old, new) file pairs since the base; `bodies` and `base_bodies` {key: [code
    lines]} for the measure rows, used to tell a renamed function from a new one. The baseline
    comparison is skipped when `base_rows` is None: the change that first lands a baseline.
    """
    bad = []
    file_was = {new: old for old, new in renames}
    bodies, base_bodies = bodies or {}, base_bodies or {}

    def base_key(key):
        lint, path, item = key
        return (lint, file_was.get(path, path), item)

    # A renamed function: an added row whose body matches a base row whose function is gone.
    moved = {}
    if base_rows is not None:
        gone = [k for k in base_rows if k not in {base_key(r) for r in rows}
                and not any(base_key(v) == k for v in values)]
        for key in sorted(rows):
            if key in base_rows or base_key(key) in base_rows:
                continue
            best = max(gone, key=lambda g: similar(bodies.get(key), base_bodies.get(g)),
                       default=None)
            if (best is not None and best[0] == key[0]
                    and similar(bodies.get(key), base_bodies.get(best)) >= 0.5):
                moved[key] = best
                gone.remove(best)

    for key in sorted(values):
        if key not in rows:
            lint = key[0]
            kind, threshold, _scope = LINTS.get(lint, ('measure', 0, None))
            what = (f'{fmt(values[key])} code lines against the {fmt(threshold)}-line threshold'
                    if kind == 'measure' else f'{fmt(values[key])} {lint} hits')
            bad.append(f'{show(key)}: {what}, and it has no row. A new one meets the threshold '
                       f'outright. {REMEDY.get(lint, "")}'.rstrip())
    for key in sorted(rows):
        n = values.get(key)
        if n is None:
            bad.append(f'{BASELINE}: {show(key)} ({key[0]}) is gone or no longer over its '
                       f'threshold. Remove its row, or move it with the rename; the list cannot '
                       f'go stale')
            continue
        if n > rows[key]:
            bad.append(f'{show(key)}: {fmt(n)}, over its baseline row of {fmt(rows[key])}. '
                       f'{REMEDY.get(key[0], "")}'.rstrip())
        if base_values is not None:
            was = base_values.get(moved.get(key, base_key(key)))
            if was is not None and n > was:
                bad.append(f'{show(key)}: grew from {fmt(was)} to {fmt(n)} against the merge '
                           f'base. A listed item may only shrink. {REMEDY.get(key[0], "")}'.rstrip())

    if base_rows is not None:
        for key in sorted(rows):
            prior = base_rows.get(key, base_rows.get(base_key(key)))
            if prior is None and key in moved:
                prior = base_rows[moved[key]]
            if prior is None:
                bad.append(f'{BASELINE}: {show(key)} ({key[0]}) was added. The list only '
                           f'shrinks; a new item meets the threshold outright')
            elif rows[key] > prior:
                bad.append(f'{BASELINE}: {show(key)} ({key[0]}) rose from {fmt(prior)} to '
                           f'{fmt(rows[key])}. A row only goes down')
    return bad


def banked(rows, values):
    """Rows lowered to the tree, never raised, never added; a row with no value drops out."""
    return {k: min(v, values[k]) for k, v in rows.items() if k in values}


# --- git ------------------------------------------------------------------------------------------

def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


BASE_OVERRIDE = None


def merge_base():
    if BASE_OVERRIDE:
        return (git('rev-parse', BASE_OVERRIDE) or '').strip() or None
    base = (git('merge-base', 'HEAD', 'origin/main') or '').strip()
    head = (git('rev-parse', 'HEAD') or '').strip()
    return base if base and base != head else None


def renamed_rust(base):
    pairs = []
    for line in (git('diff', '--name-status', '-M', base, '--', '*.rs') or '').splitlines():
        cells = line.split('\t')
        if cells[0].startswith('R') and len(cells) == 3:
            pairs.append((cells[1], cells[2]))
    return pairs


def body_lines(text, fns, key):
    """The code lines of one function's body, stripped, for the rename comparison."""
    _lint, path, item = key
    found = fns.get((path, item))
    if not found:
        return None
    lines = text[path].split('\n')
    return [lines[k - 1].strip() for k in sorted(found[2])]


def base_files(base, paths):
    out = {}
    for path in paths:
        t = git('show', f'{base}:{path}')
        if t is not None:
            out[path] = t
    return out


def gather(hits):
    """(values, rows, base_values, base_rows, renames, bodies, base_bodies, fns)."""
    tree = fl.working_tree()
    fns = fl.measure(tree)
    values = {**measured(fns), **counted(hits)}
    rows = read_baseline(open(BASELINE).read())
    bodies = {k: body_lines(tree, fns, k) for k in rows if k[0] == 'too_many_lines'}
    base = merge_base()
    base_values = base_rows = None
    renames, base_bodies = (), {}
    if base:
        renames = renamed_rust(base)
        old = git('show', f'{base}:{BASELINE}')
        if old is not None:
            base_rows = read_baseline(old)
        # The merge base's measure, for the files the rows name and the files they were renamed
        # from. Counts at the base would need clippy run there; a count row is held to its row only.
        wanted = {k[1] for k in rows} | {k[1] for k in (base_rows or {})} | {o for o, _n in renames}
        then = base_files(base, sorted(wanted))
        base_fns = fl.measure(then)
        base_values = measured(base_fns)
        base_bodies = {k: body_lines(then, base_fns, k) for k in (base_rows or {})
                       if k[0] == 'too_many_lines'}
    return values, rows, base_values, base_rows, renames, bodies, base_bodies, fns


# --- the verbs ------------------------------------------------------------------------------------

def clippy_toml_threshold():
    m = re.search(r'^too-many-lines-threshold\s*=\s*(\d+)', open('clippy.toml').read(), re.M)
    return int(m.group(1)) if m else None


def run_check(directory):
    if not os.path.exists(BASELINE):
        print(f'lint ratchet: {BASELINE} is missing. The gate and its list are one change',
              file=sys.stderr)
        return 1
    if clippy_toml_threshold() != fl.THRESHOLD:
        print(f'lint ratchet: clippy.toml must set too-many-lines-threshold = {fl.THRESHOLD}, the '
              f'threshold helpers/function_length.py measures against', file=sys.stderr)
        return 1
    stale = saved_problems(directory)
    if stale:
        print('lint ratchet: the clippy sections\' saved diagnostics cannot be trusted, so nothing '
              'was checked:', file=sys.stderr)
        for line in stale:
            print(f'  {line}', file=sys.stderr)
        return 1
    idle = unchecked(directory)
    if idle:
        print(f'lint ratchet: these clippy passes show no finished build of an in-scope target, so '
              f'they checked nothing (design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md): '
              f'{", ".join(idle)}', file=sys.stderr)
        return 1
    hits = clippy_hits(directory)
    values, rows, base_values, base_rows, renames, bodies, base_bodies, fns = gather(hits)
    bad = agreement(hits, fns)
    bad += problems(values, rows, base_values, base_rows, renames, bodies, base_bodies)
    if bad:
        print('lint ratchet: the tree breaks its baseline:', file=sys.stderr)
        for b in bad:
            print(f'  {b}', file=sys.stderr)
        print('\nSplit at a seam, bank a fall with `python3 helpers/lint_ratchet.py --bank`, or '
              'take a ruling to an architect. See design/roadmap/0888-a-ratchet-on-function-'
              'length.md.', file=sys.stderr)
        return 1
    clippy_fns = sum(1 for k in hits if k[0] == 'too_many_lines' and fl.in_scope(k[1]))
    print(f'lint ratchet: {len(fns)} functions measured, {len(values)} over threshold and held to '
          f'{len(rows)} rows; clippy agrees on all {clippy_fns} it reported')
    return 0


def bank(initialize=False):
    if initialize and os.path.exists(BASELINE):
        print(f'lint ratchet: {BASELINE} already exists; --init wrote it once, in milestone 888, '
              f'and --bank is the verb that never raises', file=sys.stderr)
        return 2
    if not initialize and not os.path.exists(BASELINE):
        print(f'lint ratchet: {BASELINE} does not exist; there is nothing to bank',
              file=sys.stderr)
        return 2
    # Banking reads the text measure only, so it runs without cargo. A count lint's rows need
    # clippy's hits and are left as they stand until such a lint is registered.
    values = measured(fl.measure(fl.working_tree()))
    if initialize:
        rows = values
    else:
        old = read_baseline(open(BASELINE).read())
        rows = {**{k: v for k, v in old.items() if LINTS.get(k[0], ('',))[0] != 'measure'},
                **banked({k: v for k, v in old.items()
                          if LINTS.get(k[0], ('',))[0] == 'measure'}, values)}
    write_baseline(rows)
    print(f'lint ratchet: {len(rows)} rows written to {BASELINE}')
    return 0


def saved_cases():
    """The saved-diagnostics rules: a planted warning is counted, and a file that is missing,
    stale, altered or made with other flags fails, naming the file."""
    import hashlib
    import tempfile
    cases = []
    root = os.path.realpath(os.getcwd())
    flags = ratchet_flags()
    done = {'reason': 'build-finished', 'success': True}

    def save(d, name, messages, **over):
        base = os.path.join(d, name)
        os.makedirs(os.path.dirname(base), exist_ok=True)
        body = ''.join(json.dumps(m) + '\n' for m in messages).encode()
        with open(base + '.json', 'wb') as f:
            f.write(body)
        meta = {'name': name, 'sha256': hashlib.sha256(body).hexdigest(), 'exit': 0,
                'started_ns': 1000, 'rustc_flags': ['-D', 'warnings'] + flags,
                'cargo_args': ['clippy', '--message-format=json'] + list(CONFIGURATIONS[name])}
        meta.update(over)
        with open(base + '.meta', 'w') as f:
            json.dump(meta, f)

    def fixture(d, plant=None, **over):
        for name in CONFIGURATIONS:
            art = {'reason': 'compiler-artifact', 'target': {'src_path': f'{root}/xtask/src/main.rs'}}
            messages = [art, done]
            if plant == name:
                messages.insert(1, {'reason': 'compiler-message', 'message': {
                    'code': {'code': 'clippy::too_many_lines'},
                    'message': 'this function has too many lines (120/100)',
                    'spans': [{'is_primary': True, 'file_name': 'kernel/src/planted.rs',
                               'line_start': 7}]}})
            save(d, name, messages, **over)

    def case(name, ok):
        cases.append((name, ok))

    fresh = (500, 'src/a.rs')
    with tempfile.TemporaryDirectory() as d:
        fixture(d, plant='riscv64')
        case('saved diagnostics that are whole, fresh and made with the ratchet\'s flags pass',
             not saved_problems(d, fresh))
        case('a warning planted in one configuration is counted',
             list(clippy_hits(d)) == [('too_many_lines', 'kernel/src/planted.rs', 7)])
        case('a configuration\'s JSON absent fails and names the file',
             (os.remove(os.path.join(d, 'x86_64.json')) or True)
             and any('x86_64.json' in b and 'missing' in b for b in saved_problems(d, fresh)))
        fixture(d)
        os.remove(os.path.join(d, 'fuzz', 'clippy.meta'))
        case('a missing sidecar fails and names the file',
             any('fuzz/clippy.meta' in b for b in saved_problems(d, fresh)))
        fixture(d)
        os.remove(os.path.join(d, 'host.json'))
        os.mkdir(os.path.join(d, 'host.json'))
        case('an unreadable JSON fails and names the file',
             any('host.json' in b and 'unreadable' in b for b in saved_problems(d, fresh)))
        os.rmdir(os.path.join(d, 'host.json'))
        fixture(d)
        stale = saved_problems(d, (2000, 'kernel/src/main.rs'))
        case('a tree edited after the run started fails, naming the file and the edit',
             len(stale) == len(CONFIGURATIONS) and all('stale' in b for b in stale)
             and any('host.json' in b for b in stale) and 'kernel/src/main.rs' in stale[0])
        case('a tree no newer than the run passes (equal times are fresh)',
             not saved_problems(d, (1000, 'kernel/src/main.rs')))
        for label, over in [
                ('without the ratchet\'s flags', {'rustc_flags': ['-D', 'warnings']}),
                ('with the lint at -W instead of --force-warn',
                 {'rustc_flags': ['-D', 'warnings', '-W', 'clippy::too_many_lines']}),
                ('with the lint allowed after it', {'rustc_flags': ['-D', 'warnings'] + flags
                                                    + ['-A', 'clippy::too_many_lines']}),
                ('with the flags before an -A of the lint',
                 {'rustc_flags': flags + ['-A', 'clippy::too_many_lines']})]:
            fixture(d, **over)
            case(f'a configuration made {label} fails and names the file',
                 any('x86_64.json' in b and 'flags' in b for b in saved_problems(d, fresh)))
        fixture(d)
        save(d, 'aarch64', [done], cargo_args=['clippy', '-p', 'kernel'])
        case('a section that narrowed its selection fails',
             any('aarch64.json' in b and 'selection' in b for b in saved_problems(d, fresh)))
        fixture(d)
        save(d, 'riscv64', [done], cargo_args=['clippy', '-p', 'kernel', '--target',
                                                'riscv64imac-unknown-none-elf', '--features', 'x'])
        case('a section that selected a feature fails',
             any('riscv64.json' in b and '--features' in b for b in saved_problems(d, fresh)))
        fixture(d)
        with open(os.path.join(d, 'system_tests.json'), 'a') as f:
            f.write('{"reason": "compiler-artifact"}\n')
        case('a JSON altered after the section wrote it fails',
             any('system_tests.json' in b and 'SHA-256' in b for b in saved_problems(d, fresh)))
        fixture(d, exit=101)
        case('a configuration whose clippy run failed fails and names the file',
             any('host.json' in b and 'exited 101' in b for b in saved_problems(d, fresh)))
        import contextlib
        import io
        with contextlib.redirect_stderr(io.StringIO()) as printed:
            status = run_check(d)
        case('--check on such a directory exits non-zero and prints the files',
             status == 1 and 'host.json' in printed.getvalue())
        case('every required configuration is checked, not only those present',
             len(CONFIGURATIONS) == 9)
    case('newest_input names a file of the tree', newest_input()[1] != '')
    return cases


def selftest():
    """The block's `Done when` list as fixtures, and the traps around it."""
    T = 'too_many_lines'

    def k(path, item='f'):
        return (T, f'x/{path}.rs', item)

    cases = []

    def fails(name, bad):
        cases.append((name, bool(bad)))

    def passes(name, bad):
        cases.append((name, not bad))

    # A function over the threshold with no row fails; one at the threshold is not a value at all.
    fails('a new function over 100 code lines fails', problems({k('a'): 101}, {}))
    passes('no function over the threshold passes', problems({}, {}))
    row = {k('a'): 150}
    fails('a listed function grown against the merge base fails',
          problems({k('a'): 141}, row, {k('a'): 140}, row))
    fails('a listed function over its row fails', problems({k('a'): 151}, row, {k('a'): 150}, row))
    passes('a listed function unchanged passes', problems({k('a'): 140}, row, {k('a'): 140}, row))
    passes('a listed function shrunk passes', problems({k('a'): 120}, row, {k('a'): 140}, row))
    fails('a row for a function now at the threshold fails', problems({}, row, {k('a'): 140}, row))
    passes('a function split under the threshold passes once its row is gone',
           problems({}, {}, {k('a'): 140}, row))
    # The list against its merge-base copy.
    fails('a row added against the merge base fails',
          problems({k('a'): 150, k('b'): 120}, {k('a'): 150, k('b'): 120}, {k('a'): 150}, row))
    fails('a row raised against the merge base fails',
          problems({k('a'): 150}, {k('a'): 160}, {k('a'): 150}, row))
    # A renamed file moves its rows at the same number.
    passes('a renamed file that moves its row passes',
           problems({k('b'): 150}, {k('b'): 150}, {k('a'): 150}, row, [('x/a.rs', 'x/b.rs')]))
    fails('a renamed file that leaves its row behind fails',
          problems({k('b'): 150}, row, {k('a'): 150}, row, [('x/a.rs', 'x/b.rs')]))
    # A renamed function moves its row: same body, new name, same or lower number.
    body = [f'line {i};' for i in range(150)]
    passes('a renamed function that moves its row passes',
           problems({k('a', 'g'): 150}, {k('a', 'g'): 150}, {k('a', 'f'): 150}, row, (),
                    {k('a', 'g'): body}, {k('a', 'f'): body}))
    fails('a renamed function that raises its row fails',
          problems({k('a', 'g'): 150}, {k('a', 'g'): 160}, {k('a', 'f'): 150}, row, (),
                   {k('a', 'g'): body}, {k('a', 'f'): body}))
    fails('a new function is not a rename because a long one went away',
          problems({k('a', 'g'): 120}, {k('a', 'g'): 120}, {k('a', 'f'): 150}, row, (),
                   {k('a', 'g'): [f'other {i};' for i in range(120)]}, {k('a', 'f'): body}))
    # A count lint: a file with hits and no row fails, and its row is a ceiling.
    LINTS['_count'] = ('count', 0, lambda p: True)
    c = ('_count', 'x/a.rs', '-')
    fails('a count lint: a hit in an unlisted file fails', problems({c: 1}, {}))
    fails('a count lint: more hits than the row fails', problems({c: 4}, {c: 3}, {}, {c: 3}))
    passes('a count lint: fewer hits than the row passes', problems({c: 2}, {c: 3}, {}, {c: 3}))
    fails('a count lint: a file with no hits left keeps no row', problems({}, {c: 3}, {}, {c: 3}))
    hits = {('_count', 'x/a.rs', 3): 'm', ('_count', 'x/a.rs', 9): 'm', ('_count', 'x/b.rs', 1): 'm'}
    if counted(hits) != {c: 2, ('_count', 'x/b.rs', '-'): 1}:
        cases.append(('a count lint counts each site once, per file', False))
    del LINTS['_count']
    # Banking lowers to the tree, never raises, never adds, drops dead rows.
    if banked({k('a'): 200, k('b'): 150}, {k('a'): 180, k('c'): 300}) != {k('a'): 180}:
        cases.append(('bank lowers a row, drops a dead one and adds none', False))
    if banked({k('a'): 150}, {k('a'): 170}) != {k('a'): 150}:
        cases.append(('bank never raises a row', False))
    # Clippy and the text measure must agree.
    fns = {('x/a.rs', 'f'): (10, 120, set())}
    if agreement({(T, 'x/a.rs', 10): 'this function has too many lines (120/100)'}, fns):
        cases.append(('clippy and the text measure agreeing passes', False))
    if not agreement({(T, 'x/a.rs', 10): 'this function has too many lines (121/100)'}, fns):
        cases.append(('clippy and the text measure disagreeing on a count fails', False))
    if not agreement({(T, 'x/a.rs', 30): 'this function has too many lines (130/100)'}, fns):
        cases.append(('a function clippy sees and the text measure misses fails', False))
    # A pass is judged by what it compiled, not by whether it found anything, and a nested
    # workspace's JSON sits at its own path.
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        root = os.path.realpath(os.getcwd())

        def write(rel, *msgs):
            os.makedirs(os.path.dirname(os.path.join(d, rel)), exist_ok=True)
            with open(os.path.join(d, rel), 'w') as f:
                f.write(''.join(json.dumps(m) + '\n' for m in msgs))

        done = {'reason': 'build-finished', 'success': True}
        art = {'reason': 'compiler-artifact', 'target': {'src_path': f'{root}/fuzz/src/lib.rs'}}
        hit = {'reason': 'compiler-message', 'message': {
            'code': {'code': 'clippy::too_many_lines'}, 'message': 'm',
            'spans': [{'is_primary': True, 'file_name': 'src/lib.rs', 'line_start': 3}]}}
        write('fuzz/clippy.json', art, hit, done)
        if unchecked(d):
            cases.append(('a finished pass that compiled an in-scope target passes with no hit', False))
        if list(clippy_hits(d)) != [(T, 'fuzz/src/lib.rs', 3)]:
            cases.append(('a nested workspace\'s path is put back under the repository root', False))
        write('host.json', art)
        if unchecked(d) != ['host.json']:
            cases.append(('a pass with no successful build-finished fails', False))
        write('host.json', {'reason': 'compiler-artifact',
                            'target': {'src_path': f'{root}/vendor/x/src/lib.rs'}}, done)
        if unchecked(d) != ['host.json']:
            cases.append(('a pass that compiled nothing in scope fails', False))
    if not (fl.in_scope('xtask/src/main.rs') and fl.in_scope('system_tests/src/user/a_tests.rs')
            and not fl.in_scope('vendor/x/src/lib.rs') and not fl.in_scope('notes/a.md')):
        cases.append(('the scope is every .rs file outside vendor/', False))
    # The measure itself is clippy's: comments and blank lines are free, braces are not counted,
    # a line with code before a comment counts, and a block comment spanning lines is free.
    src = 'fn f() {\n    a();\n\n    // note\n    b(); // trailing\n    /* x\n    y */\n    c();\n}\n'
    if fl.code_lines(src[src.index('{'):src.rindex('}') + 1]) != 3:
        cases.append(('the measure counts code lines the way too_many_lines does', False))
    got = fl.functions(src, src)
    if [(n, s, c) for n, s, c, _l in got] != [('f', 1, 3)]:
        cases.append(('a function is found at its first line with its count', False))
    impl = ('impl<T: Copy> Thing<T> for Other {\n    pub(crate) unsafe fn go(&self) {\n'
            '        x();\n    }\n}\n')
    if [n for n, *_ in fl.functions(impl, impl)] != ['Other::go']:
        cases.append(('a method is keyed by its self type', False))
    cases += saved_cases()
    # The message carries the remedy.
    msg = problems({k('a'): 101}, {})[0]
    if not all(s in msg for s in ('x/a.rs', '101', '100', 'split the function at a seam')):
        cases.append(('the failure message names the function, size, threshold and remedy', False))

    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f'lint ratchet selftest: {name}', file=sys.stderr)
    if not failed:
        print(f'lint ratchet selftest: {len(cases)} cases pass')
    return 1 if failed else 0


def main(argv):
    root = git('rev-parse', '--show-toplevel')
    if root:
        os.chdir(root.strip())
    global BASE_OVERRIDE
    if '--base' in argv:
        i = argv.index('--base')
        BASE_OVERRIDE = argv[i + 1]
        argv = argv[:i] + argv[i + 2:]
    if argv[:1] == ['--selftest']:
        return selftest()
    if argv[:1] == ['--flags']:
        print(' '.join(ratchet_flags()))
        return 0
    if argv[:1] == ['--check'] and len(argv) == 2:
        return run_check(argv[1])
    # The option keeps its command-line abbreviation, `--init`; the code behind it spells `initialize`.
    # The difference is deliberate (design/naming/precedents-set-by-example.md, "The command line
    # abbreviates and the code behind it does not").
    if argv[:1] in (['--bank'], ['--init']):
        return bank(initialize=argv[0] == '--init')
    print(__doc__.split('\n\n')[2], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
