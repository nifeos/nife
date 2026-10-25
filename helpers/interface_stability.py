"""How stable nife's interface is, per ISO week, recomputed from git history alone.

Milestone 610 (the interface's stability is measured weekly), its number provisional. calef,
2026-09-27 (UTC): "What I want are good metrics for capturing the stability of our interface." What
"the interface" means, what each column counts, the threshold proposal and every blind spot are in
notes/interface-stability.md; this header is the manual for the program.

    python3 helpers/interface_stability.py --update     # this week, and any week the CSV lacks
    python3 helpers/interface_stability.py --backfill   # every week from the first commit
    python3 helpers/interface_stability.py --table      # this week's cells, printed, nothing written
    python3 helpers/interface_stability.py --selftest   # fixtures only, no git, no cargo; script/lint

It writes `notes/project-metrics/interface-stability.csv`, the per-crate and per-package CSVs
beside it, and the appendix `notes/project-metrics/interface-stability.md`, and nothing else.
`script/metrics` draws the chart and the generated line on notes/project-metrics.md from the first
and never recomputes a cell of it (its INTERFACE_FIELDS are carried the way coverage is). The
per-package co-change cells need only git, so every run restates them for every week.

**Why this builds when `script/metrics` may not.** The public API is read from rustdoc's JSON
output (`--output-format json`, on the pinned nightly), which needs `cargo doc` over a copy of each
week's tree. That copy comes from `git archive` into a scratch directory under `target/`, so the
working tree is never checked out or touched. The toolchain is always HEAD's, forced with
`RUSTUP_TOOLCHAIN`, so every week's JSON has one format; an old tree the new nightly cannot document
is reported as unmeasured, crate by crate, and never guessed at.

**Why past cells are carried rather than recomputed.** A measurement run today against a week's
tree can fail next month, when the pinned nightly has moved on and that old source no longer
compiles. Recomputing every week on every run would let a past week's number change or vanish for a
reason that has nothing to do with the interface. So `--update` writes the current week and any
week the CSV does not have, and only `--backfill` restates the past, the same contract
`script/metrics --update` keeps.

Name: provisional, minted by milestone 610's lane on 2026-09-26 (UTC). A shared python module under
`helpers/`, outside `script/names`' scope, so its provenance is this paragraph. Refused
`api_diff`, which names one of its five measures and not the thing measured.
"""

import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
from datetime import date, datetime, timezone

import crate_release
import packages

CSV = pathlib.Path('notes/project-metrics/interface-stability.csv')
# The second view calef's maintainer asked for: the same counts per crate, one row per crate per
# week it moved, and the appendix page that reads it.
CRATES_CSV = pathlib.Path('notes/project-metrics/interface-stability-crates.csv')
APPENDIX = pathlib.Path('notes/project-metrics/interface-stability.md')
CRATE_FIELDS = ['breaking', 'additions', 'commits', 'commits_crossing']
# Per-package co-change: §235 (the OS is built and updated from packages, and the tree divides by what releases together), in its Fork 3
# amendment, sizes a home by what changes together. One row per unit per week a commit touched it;
# `kind` is `package` (a packages/*.package.toml file, §235's package) or `crate` (one Cargo
# package). Recomputed whole on every run: it reads git alone.
PACKAGES_CSV = pathlib.Path('notes/project-metrics/interface-stability-packages.csv')
PACKAGE_FIELDS = ['commits', 'commits_crossing', 'commits_touching_another',
                  'commits_excluding_mass', 'commits_crossing_excluding_mass']
SCRATCH = pathlib.Path('target/interface-stability')

# The contracts division: notes/packages-and-divisions.md (#1389, not yet on main), `abi`, the
# fifteen `*_protocol` crates that are contracts (not `network_time_protocol`, a service whose name
# says protocol), and the named formats two programs agree on. `capability_manifest_note` is the one addition:
# it was born on 2026-09-26, after that note's census, and it is exactly the kind of crate the
# division names. Membership is by the crate's name TODAY; a week in the past counts the crates
# whose directory later became one of these (LINEAGE below), which is a restatement like every
# other series on the metrics page.
CONTRACTS = (
    'abi',
    'byte_sink_contract', 'capability_witness_contract', 'clock_contract',
    'counter_frequency_contract', 'credential_contract', 'current_cpu_contract',
    'entropy_contract', 'environment_contract', 'filesystem_contract', 'graphics_contract',
    'login_contract', 'socket_contract', 'std_runtime_contract', 'supervision_protocol',
    'hot_swap_contract',
    'elf', 'measured_boot', 'package_archive', 'installed_programs', 'boot_slot', 'nifefs',
    'globally_unique_identifier_partition_table', 'grant_plan', 'capability_plan', 'compositor',
    'line_editor', 'capability_manifest_note',
)

# The std ABI is every crate `cargo xtask std-src` copies into the standard library's platform
# layer: read from xtask/src/farm.rs at HEAD, so the list cannot drift from the copy that makes it
# true. All are contracts but one (`allocation_cache`, the runtime division), which is why the
# interface is the union of the two and not the contracts alone.
STD_ABI_SOURCE = 'xtask/src/farm.rs'
STD_ABI_GLOBS = ('xtask/src/farm.rs', 'xtask/src/farm/*.rs')
STD_ABI_RE = re.compile(r'"crates/(\w+)/src/lib\.rs"')

# A format identity: a public constant a reader checks before trusting the bytes after it. A value
# change is a format bump. `BOTH_VERSIONS` is a flag bit in hot_swap_contract, not an identity.
FORMAT_CONST = re.compile(r'(^|_)(VERSION|MAGIC|REVISION)($|_)')

# The amendment ledger, kept at the format (AGENTS.md rung 3): a `# Amendments` section in the
# crate's own module documentation, one list item per in-place change to a released version,
# each opening with its UTC date. Convention provisional, this lane's, 2026-09-26.
LEDGER_HEAD = re.compile(r'^//!\s*#+\s*Amendments\s*$')
LEDGER_ITEM = re.compile(r'^//!\s*-\s*(\d{4}-\d{2}-\d{2})\b')

# Co-change, with the method of the measurement that produced #1389's 76% (maintainer's forwarded
# definitions, 2026-09-26): merges excluded, documentation excluded from both sides, a commit
# crosses when it touches any non-documentation path outside the contract directories. A commit
# is "mass mechanical" when it touches more than 40 files, spans more than 8 buckets, or its
# subject reads like a rename or a migration; the share is reported with and without those.
DOC_PREFIXES = ('design/', 'notes/', 'briefs/', 'ARCHITECTS.md', 'CLAUDE.md')
MASS_FILES, MASS_BUCKETS = 40, 8
MASS_SUBJECT = re.compile(r'\b(renam|migrat|lint sweep|sweep|reformat|format(ted)? (the )?roadmap|'
                          r'frontmatter|mass|bulk|mechanical)\w*', re.IGNORECASE)

FIELDS = [
    'interface_breaking', 'interface_additions',
    'interface_std_abi_breaking', 'interface_std_abi_additions',
    'interface_contract_api_breaking', 'interface_contract_api_additions',
    'interface_syscall_added', 'interface_syscall_changed', 'interface_syscall_renamed',
    'interface_format_bumps', 'interface_format_amendments',
    'interface_contract_commits', 'interface_contract_commits_crossing',
    'interface_cochange_pct', 'interface_cochange_pct_excluding_mass',
    'interface_crates_unmeasured',
    'interface_package_commits', 'interface_package_commits_crossing',
    'interface_package_cochange_pct',
]


def git(*args):
    return subprocess.run(['git', *args], capture_output=True, check=True).stdout.decode()


def week_of(iso_datetime):
    when = datetime.fromisoformat(iso_datetime).astimezone(timezone.utc)
    year, week, _day = when.isocalendar()
    return '%04dW%02d' % (year, week)


def week_of_date(stamp):
    year, week, _day = date.fromisoformat(stamp).isocalendar()
    return '%04dW%02d' % (year, week)


def week_commits():
    """{week: first-parent commit that stands for it}; `script/metrics`' `week_commits`, exactly."""
    out = {}
    for entry in git('log', '--first-parent', '--format=%H %cI', 'HEAD').split('\n'):
        if entry.strip():
            sha, when = entry.split()
            out.setdefault(week_of(when), sha)
    return dict(sorted(out.items()))


# ---------------------------------------------------------------------------------------------
# Lineage: which directory a crate had before each rename.

def lineage():
    """{old crate directory name: the name it carries at HEAD}, from git's own rename detection.

    Renames are read off every commit, not the first-parent trunk only, because a lane renames on
    its branch and the merge shows no rename at all. `src/lib.rs` pairs win over `Cargo.toml`
    pairs: two manifests are a dozen near-identical lines, and in the 2026-08 naming sweep git
    paired `uheap`'s manifest with `line_editor`'s. The sources are what make a crate itself.
    """
    step = {}
    log = git('log', '-M', '--diff-filter=R', '--name-status', '--format=@@%h', 'HEAD', '--',
              'crates/*/Cargo.toml', 'crates/*/src/lib.rs')
    for commit in log.split('@@')[1:]:
        by_source, by_manifest = {}, {}
        for line in commit.split('\n'):
            parts = line.split('\t')
            if len(parts) != 3 or not parts[0].startswith('R'):
                continue
            old, new = parts[1].split('/')[1], parts[2].split('/')[1]
            if old != new:
                (by_source if parts[1].endswith('lib.rs') else by_manifest)[old] = new
        pairs = dict(by_source)
        taken = set(pairs.values())
        for old, new in by_manifest.items():
            if old not in pairs and new not in taken:
                pairs[old] = new
        for old, new in pairs.items():
            step.setdefault(old, new)   # newest first: a name reused later keeps its last move
    return resolve_lineage(step)


def resolve_lineage(step):
    final = {}
    for name in step:
        seen, cur = {name}, name
        while cur in step and step[cur] not in seen:
            cur = step[cur]
            seen.add(cur)
        final[name] = cur
    return final


# The lineage `run` computed, for the co-change walk's per-crate names. Empty in `--selftest`.
LINEAGE_CACHE = {}


def identity(name, lineage_map):
    return lineage_map.get(name, name)


def std_abi_crates(text):
    return sorted(set(STD_ABI_RE.findall(text)))


def std_abi_set(texts):
    """The std copy crates named across the farm sources; an empty set is an error.

    Zero crates means the pattern or the files moved, not that nothing is copied (the W42 std
    columns read an empty set for exactly that reason). Returning it would write a row of
    zeros that looks like a measurement.
    """
    found = set()
    for text in texts:
        found.update(std_abi_crates(text))
    if not found:
        raise SystemExit('interface stability: no "crates/<name>/src/lib.rs" copy found in %s, '
                         'so the std copy set would read empty. Update STD_ABI_GLOBS or '
                         'STD_ABI_RE.' % ', '.join(STD_ABI_GLOBS))
    return found


def std_abi_source_texts():
    # The copy list lives in the farm module. farm.rs became an index of its children (2026-10-12)
    # and the list moved with the copies, so read the index and every child and a future split
    # moves the strings without silencing this. The W42 std columns silently read an empty set
    # for exactly that reason; the weekly read found no `"crates/<name>/src/lib.rs"` left in the
    # index file.
    paths = [p for one_glob in STD_ABI_GLOBS for p in sorted(pathlib.Path('.').glob(one_glob))]
    return [p.read_text() for p in paths] or [pathlib.Path(STD_ABI_SOURCE).read_text()]


# ---------------------------------------------------------------------------------------------
# A public API, from rustdoc JSON, as {item path: fingerprint}.
#
# The path is the crate-relative path a consumer writes (`rendezvous::SEND`, `Header::len`,
# `Slot: impl Clone`). A `pub use` is the item it names: the consumer writes the re-export's
# path, so the target's fingerprint is recorded under the re-export's local name (the
# "re-export rule" in notes/interface-stability.md), and moving a definition behind `pub use`,
# as a §282 (when a file is split, the parent becomes an index) split does, moves no surface.
# The fingerprint is the item's rustdoc
# description with everything that is not interface removed: ids (which rustdoc renumbers per
# build), spans, docs, links. Children (fields, variants, methods, trait items) are items of
# their own, so a struct's fingerprint is its shape, its field order and its repr, and a
# field's is its type. A constant's fingerprint carries its VALUE, and a variant's its
# discriminant, because on this tree those are wire numbers.

# Bump when `surface` changes what it records, so a cached surface from the old reader is not
# compared with a fresh one from the new.
SURFACE_VERSION = 'v4'
# The interface as a nife program sees it: every contract crate is `no_std` and builds for the
# kernel's own first target, and two (`supervision_protocol`, `hot_swap_contract`) link
# `user_mode_runtime`, which does not build for the host at all. BUGS: items behind a
# `cfg(target_arch)` for riscv64 or x86_64 are not read.
TARGET = 'aarch64-unknown-none-softfloat'

STRIP = {'id', 'span', 'docs', 'links', 'deprecation', 'stability', 'const_stability', 'crate_id'}


def _clean(value, key=None):
    """`value` with every non-interface detail removed, recursively.

    Two things a consumer cannot see are dropped besides ids and prose: a function parameter's
    NAME (only its type is in the caller's contract), and a constant's source spelling when
    rustdoc also gives its value, so `1 << 2` becoming `4` is not a change and `4` becoming `5` is.
    """
    if isinstance(value, dict):
        if key == 'const' and value.get('value') is not None:
            return {'value': value['value']}
        return {k: _clean(v, k) for k, v in sorted(value.items()) if k not in STRIP}
    if isinstance(value, list):
        if key == 'inputs':
            return [_clean(v[1]) if isinstance(v, list) and len(v) == 2 else _clean(v)
                    for v in value]
        return [_clean(v) for v in value]
    return value


def _layout_attrs(item):
    kept = []
    for attr in item.get('attrs') or []:
        text = json.dumps(attr, sort_keys=True)
        if 'repr' in text or 'non_exhaustive' in text:
            kept.append(_clean(attr))
    return kept


def _fp(obj):
    return json.dumps(obj, sort_keys=True, separators=(',', ':'))


def _trait_name(path):
    if not path:
        return '?'
    name = path.get('path') or path.get('name') or '?'
    args = path.get('args')
    return name + (_fp(_clean(args)) if args else '')


def const_source(text):
    """The value half of `const NAME: T = <this>;`, whitespace collapsed, or None."""
    m = re.search(r'=\s*(.*?)\s*;\s*$', text, re.S)
    return ' '.join(m.group(1).split()) if m else None


def surface(doc, read_span=None):
    """{path: fingerprint} for one crate's rustdoc JSON document.

    `read_span(span)` returns the source text an item's span covers. It is how a constant rustdoc
    cannot evaluate gets a value anyway: rustdoc writes `_` for `*b"CRKR0002"`, which is the
    spelling of every byte-string magic in this tree, so without it the one change most likely to
    be a format bump would be invisible.
    """
    index = doc['index']
    out = {}

    def item(id_):
        return index.get(str(id_)) or index.get(id_)

    def impls(ids, owner):
        for iid in ids or []:
            imp = item(iid)
            if not imp:
                continue
            body = imp['inner'].get('impl', {})
            # A blanket impl is core's, and an auto-trait impl (`Send`, `Sync`, `Unpin`, the unwind
            # pair) is the compiler's inference from the fields, so neither is something a person
            # wrote; counting them would add five items to every new type. BUGS: an auto trait
            # lost through a field change is therefore invisible here.
            if body.get('blanket_impl') is not None or body.get('is_synthetic'):
                continue
            if body.get('trait'):
                key = '%s: impl %s' % (owner, _trait_name(body['trait']))
                out[key] = _fp({'negative': body.get('is_negative'), 'unsafe': body.get('is_unsafe'),
                                'generics': _clean(body.get('generics'))})
                continue
            for mid in body.get('items') or []:
                member = item(mid)
                if member and member.get('visibility') == 'public' and member.get('name'):
                    out['%s::%s' % (owner, member['name'])] = _fp(_clean(member['inner']))

    def fields_of(kind):
        if 'plain' in kind:
            return kind['plain'].get('fields') or [], kind['plain'].get('has_stripped_fields')
        if 'tuple' in kind:
            return [f for f in kind['tuple'] if f is not None], None in kind['tuple']
        return [], False

    def walk(id_, prefix, depth=0):
        it = item(id_)
        if it is None or depth > 32:
            return
        kind, body = next(iter(it['inner'].items()))
        name = it.get('name')
        if kind == 'use':
            if body.get('is_glob'):
                out[(prefix or '') + '*' + body.get('source', '')] = _fp(
                    {'use': body.get('source')})
                return
            # Format 61 carries a re-export's local name and its target's id inside
            # `inner.use`; the item-level `name` is None there. Follow chains of re-exports to
            # the item a consumer actually names.
            name = body.get('name') or name
            if not name:
                return
            target = it
            seen = set()
            while next(iter(target['inner'].items()))[0] == 'use':
                inner_use = target['inner']['use']
                tid = inner_use.get('id')
                if inner_use.get('is_glob') or tid is None or tid in seen:
                    target = None
                    break
                seen.add(tid)
                target = item(tid)
                if target is None:
                    break
            if target is None:
                # A foreign or dangling re-export: the name is surface, the shape is not ours.
                out[(prefix or '') + name] = _fp({'use': body.get('source')})
                return
            it = target
            kind, body = next(iter(it['inner'].items()))
        path = prefix + name if prefix is not None and name else ''
        if kind == 'module':
            for child in body.get('items') or []:
                walk(child, (path + '::') if path else '', depth + 1)
            return
        if kind in ('struct', 'union'):
            skind = body.get('kind', {}) if kind == 'struct' else {'plain': body}
            fids, stripped = fields_of(skind)
            names = []
            for n, fid in enumerate(fids):
                f = item(fid)
                if f is None:
                    continue
                fname = f.get('name') or str(n)
                names.append(fname)
                out['%s::%s' % (path, fname)] = _fp(_clean(f['inner']))
            out[path] = _fp({'kind': kind, 'shape': sorted(skind) if kind == 'struct' else 'union',
                             'fields': names, 'stripped': bool(stripped),
                             'generics': _clean(body.get('generics')),
                             'layout': _layout_attrs(it)})
            impls(body.get('impls'), path)
            return
        if kind == 'enum':
            vnames = []
            for vid in body.get('variants') or []:
                v = item(vid)
                if v is None:
                    continue
                vnames.append(v['name'])
                vbody = v['inner'].get('variant', {})
                vk = vbody.get('kind', {})
                fields = []
                if isinstance(vk, dict):
                    fids, _s = fields_of(vk) if ('plain' in vk or 'tuple' in vk) else ([], False)
                    if 'struct' in vk:
                        fids = vk['struct'].get('fields') or []
                    for fid in fids:
                        f = item(fid)
                        if f is not None:
                            fields.append([f.get('name'), _clean(f['inner'])])
                out['%s::%s' % (path, v['name'])] = _fp({'fields': fields,
                                                         'discriminant': _clean(vbody.get('discriminant'))})
            out[path] = _fp({'kind': 'enum', 'variants': vnames,
                             'stripped': body.get('has_stripped_variants'),
                             'generics': _clean(body.get('generics')), 'layout': _layout_attrs(it)})
            impls(body.get('impls'), path)
            return
        if kind == 'trait':
            members = []
            for mid in body.get('items') or []:
                m = item(mid)
                if m is not None:
                    members.append(m['name'])
                    out['%s::%s' % (path, m['name'])] = _fp(_clean(m['inner']))
            out[path] = _fp({'kind': 'trait', 'items': sorted(members),
                             'generics': _clean(body.get('generics')),
                             'bounds': _clean(body.get('bounds')),
                             'unsafe': body.get('is_unsafe'), 'auto': body.get('is_auto')})
            return
        if kind == 'impl':
            return
        if kind == 'constant' and read_span and (body.get('const') or {}).get('value') is None:
            text = const_source(read_span(it.get('span')) or '')
            if text is not None:
                body = dict(body, const={'value': text})
        out[path] = _fp({kind: _clean(body)})

    walk(doc['root'], None)
    out.pop('', None)
    return out


# ---------------------------------------------------------------------------------------------
# Comparing two weeks.

def is_wire_variant(fingerprint):
    """Is this the fingerprint of an enum VARIANT (`walk` writes only variants with these two keys)?"""
    try:
        d = json.loads(fingerprint)
    except ValueError:
        return False
    return isinstance(d, dict) and set(d) == {'fields', 'discriminant'}


def is_syscall_item(crate, path, fingerprint):
    """Is this item part of the syscall surface? abi's public constants and enum variants.

    The syscall numbers (`SYS_*`), the object types (`objtype::*`), and every method number and
    argument or result encoding the object modules define are constants. The error codes
    (`Error::*`) and the other `#[repr]` enums' values are variants, and a program outside the
    tree matches on them by number exactly as it does on a constant, so a renumbered or removed
    variant is a boundary move too (calef, 2026-10-18 UTC). Functions and the enum items
    themselves are helpers, not surface.
    """
    return crate == 'abi' and (fingerprint.startswith('{"constant"')
                               or is_wire_variant(fingerprint))


def syscall_class(path):
    leaf = path.rsplit('::', 1)[-1]
    if '::' not in path and leaf.startswith('SYS_'):
        return 'syscall'
    if path.startswith('objtype::'):
        return 'object type'
    if path.startswith('Error::'):
        return 'error code'
    return 'method or encoding'


def _const_value(fingerprint):
    try:
        return json.loads(fingerprint)['constant']['const'].get('value')
    except (ValueError, KeyError, TypeError, AttributeError):
        return None


def syscall_renames(crate, removed, added, old, new):
    """{removed abi constant: its successor} where only the NAME changed and the number did not.

    `endpoint::SEND` becoming `rendezvous::SEND` at the same value breaks every caller's source
    and no binary. The successor must be unambiguous: exactly one added constant with the same
    value that keeps either the leaf name (a module renamed) or the module (a leaf renamed).
    Anything less certain stays a change, which overcounts rather than hides.
    """
    out = {}
    if crate != 'abi':
        return out
    for p in removed:
        v = _const_value(old[p]) if is_syscall_item(crate, p, old[p]) else None
        if v is None:
            continue
        parent, _, leaf = p.rpartition('::')
        match = [q for q in added if is_syscall_item(crate, q, new[q]) and _const_value(new[q]) == v
                 and (q.rpartition('::')[2] == leaf or q.rpartition('::')[0] == parent)]
        if len(match) == 1:
            out[p] = match[0]
    # Second pass: a module whose other constants were renamed unambiguously tells us where an
    # ambiguous one went. `frame::MAP` and `untyped::MAP` both look like either `page_frame::MAP`
    # or `memory_region::MAP` at value 0, until `frame::REVOKE` has already said which is which.
    moved = {p.rpartition('::')[0]: q.rpartition('::')[0] for p, q in out.items()
             if p.rpartition('::')[2] == q.rpartition('::')[2]}
    for p in removed:
        parent, _, leaf = p.rpartition('::')
        target = moved.get(parent)
        if p in out or not target:
            continue
        q = target + '::' + leaf
        if q in added and _const_value(new[q]) == _const_value(old[p]) is not None:
            out[p] = q
    return out


def is_format_const(path, fingerprint):
    return (fingerprint.startswith('{"constant"') and 'VERSIONS' not in path
            and bool(FORMAT_CONST.search(path.rsplit('::', 1)[-1])))


def compare(old, new, std_abi):
    """Classify one week against the week before.

    `old` and `new` are {crate identity: {'dir': directory name, 'items': {path: fingerprint}}},
    over crates measured on BOTH sides unless a crate exists on only one side. Returns the counts
    and a per-crate breakdown.
    """
    counts = {'breaking': 0, 'additions': 0, 'std_breaking': 0, 'std_additions': 0,
              'contract_breaking': 0, 'contract_additions': 0,
              'syscall_added': 0, 'syscall_changed': 0, 'syscall_renamed': 0, 'format_bumps': 0,
              'renamed': 0}
    per_crate = {}
    events = []
    for crate in sorted(set(old) | set(new)):
        a = old.get(crate, {}).get('items', {}) if crate in old else {}
        b = new.get(crate, {}).get('items', {}) if crate in new else {}
        mine = per_crate.setdefault(crate, {'breaking': 0, 'additions': 0})
        std = crate in std_abi
        if crate in old and crate in new and old[crate]['dir'] != new[crate]['dir']:
            # A crate rename breaks every `use` of it, and is one decision, not one per item. It
            # is reported where the crate's own items land, std copy set or contract API, not on
            # the boundary.
            counts['renamed'] += 1
            mine['breaking'] += 1
            if std:
                counts['std_breaking'] += 1
            else:
                counts['contract_breaking'] += 1
            events.append((crate, 'crate renamed', '%s -> %s' % (old[crate]['dir'],
                                                                 new[crate]['dir'])))
        removed = [p for p in a if p not in b]
        changed = [p for p in a if p in b and a[p] != b[p]]
        added = [p for p in b if p not in a]
        mine['breaking'] += len(removed) + len(changed)
        mine['additions'] += len(added)
        renamed_to = syscall_renames(crate, removed, added, a, b)
        for p in added:
            if is_syscall_item(crate, p, b[p]):
                counts['syscall_added'] += 1
            elif std:
                counts['std_additions'] += 1
            else:
                counts['contract_additions'] += 1
        # Every event lands in exactly one bucket, widest first: the syscall surface, then a
        # format identity, then the std copy set, then the contract crates' remaining API. The
        # headline (`interface_breaking`, `interface_additions`) is the boundary a program
        # outside the tree is written against, the syscall surface and the wire formats (calef,
        # 2026-10-16 UTC). The std copy set and the contract API are reported beside it, like
        # co-change: a program built from packages is written against stable `std` and the
        # kernel boundary, not against a contract crate's internals.
        for p in sorted(removed + changed):
            what = 'removed' if p in removed else 'changed'
            if p in renamed_to:
                counts['syscall_renamed'] += 1
                what = 'syscall renamed, number kept (%s), now `%s`' % (syscall_class(p),
                                                                       renamed_to[p])
            elif p in changed and is_format_const(p, a[p]):
                counts['format_bumps'] += 1
                what = 'format bump'
            elif is_syscall_item(crate, p, a[p]):
                counts['syscall_changed'] += 1
                what = 'syscall %s, %s' % (what, syscall_class(p))
            elif std:
                counts['std_breaking'] += 1
            else:
                counts['contract_breaking'] += 1
            events.append((crate, what, p))
    counts['breaking'] = (counts['syscall_changed'] + counts['syscall_renamed']
                          + counts['format_bumps'])
    counts['additions'] = counts['syscall_added']
    return counts, per_crate, events


def ledger_dates(text):
    """The UTC dates of a crate's `# Amendments` list items, in its `//!` documentation."""
    dates, inside = [], False
    for line in text.split('\n'):
        if LEDGER_HEAD.match(line):
            inside = True
            continue
        if inside:
            if re.match(r'^//!\s*#', line) or not line.startswith('//!'):
                inside = False
                continue
            m = LEDGER_ITEM.match(line)
            if m:
                dates.append(m.group(1))
    return dates


# ---------------------------------------------------------------------------------------------
# Co-change.

def is_doc(path):
    return path.endswith('.md') or any(path == p or path.startswith(p) for p in DOC_PREFIXES)


def _bucket(path):
    if is_doc(path):
        return None
    parts = path.split('/')
    if parts[0] == 'crates' and len(parts) > 1:
        return 'crate:' + parts[1]
    return 'top:' + parts[0]


def contract_dirs(lineage_map):
    names = set(CONTRACTS) | {old for old, new in lineage_map.items() if new in CONTRACTS}
    return tuple('crates/%s/' % n for n in sorted(names))


def classify_commit(subject, files, dirs):
    """(is a contract commit, crosses another division, is mass mechanical)."""
    contract = [p for p in files if p.startswith(dirs)]
    if not contract:
        return False, False, False
    crosses = any(not p.startswith(dirs) for p in files if not is_doc(p))
    return True, crosses, is_mass(subject, files)


def is_mass(subject, files):
    buckets = {b for b in (_bucket(p) for p in files) if b}
    return len(files) > MASS_FILES or len(buckets) > MASS_BUCKETS or bool(MASS_SUBJECT.search(subject))


def commit_log():
    """[(week, subject, files)] for every non-merge commit reachable from HEAD."""
    out = []
    log = git('log', '--no-merges', '--format=@@%H|%cI|%s', '--name-only', 'HEAD')
    for block in log.split('@@')[1:]:
        lines = [l for l in block.split('\n') if l.strip()]
        _sha, when, subject = (lines[0].split('|', 2) + [''])[:3]
        out.append((week_of(when), subject, lines[1:]))
    return out


def cochange_by_week(dirs, commits):
    """{week: [commits, crossing, commits excluding mass, crossing excluding mass]}, and per crate.

    The per-crate view is {week: {crate: [commits, crossing]}}, keyed by the crate's HEAD name;
    a commit touching two contracts counts once for each, so its rows do not sum to the week's.
    """
    out, per_crate = {}, {}
    names = {d: identity(d.split('/')[1], LINEAGE_CACHE) for d in dirs}
    for week, subject, files in commits:
        is_c, crosses, mass = classify_commit(subject, files, dirs)
        if not is_c:
            continue
        cell = out.setdefault(week, [0, 0, 0, 0])
        cell[0] += 1
        cell[1] += crosses
        if not mass:
            cell[2] += 1
            cell[3] += crosses
        touched = {names[d] for d in dirs for p in files if p.startswith(d)}
        for crate in touched:
            c = per_crate.setdefault(week, {}).setdefault(crate, [0, 0])
            c[0] += 1
            c[1] += crosses
    return out, per_crate


# ---------------------------------------------------------------------------------------------
# Per-package co-change: the same method, with every package (and every crate) in the contracts'
# place. A commit touches a unit when one of its non-documentation paths belongs to it; it crosses
# when another of its non-documentation paths does not; it touches another unit when one of them
# belongs to a different unit of the same kind (a path in packages/homes.toml, such as `script/`,
# is outside every package but is no other package). Membership is HEAD's: a past path is renamed
# through LINEAGE first, then placed by the longest prefix HEAD's declarations claim.

def unit_tables(tracked=None, texts=None):
    """({prefix: package or None}, {prefix: crate}, {crate: package}) from HEAD's declarations."""
    if tracked is None:
        tracked, texts = packages.working_tree()
    result = packages.evaluate(tracked, texts)
    crate_claims = {info['dir'] + '/': name for name, info in result['crates'].items()
                    if info['dir']}
    return result['claims'], crate_claims, result['crate_pkg']


_UNCLAIMED = object()


def owner(path, claims):
    """The value of the longest prefix of `path` in `claims`, or None when nothing claims it."""
    best = _UNCLAIMED
    for prefix in packages._prefixes(path):
        if prefix in claims:
            best = claims[prefix]
    return None if best is _UNCLAIMED else best


def head_path(path, lineage_map):
    parts = path.split('/')
    if len(parts) > 2 and parts[0] == 'crates':
        parts[1] = identity(parts[1], lineage_map)
    return '/'.join(parts)


def unit_cochange(units):
    """{unit: (crosses, touches another)} for one commit, from each code path's unit or None."""
    out = {}
    for u in {u for u in units if u}:
        out[u] = (any(v != u for v in units), any(v and v != u for v in units))
    return out


def package_cochange_by_week(commits, tables, lineage_map):
    """({week: {(kind, unit): PACKAGE_FIELDS}}, {week: [commits, crossing]}, {week: {pair: n}}).

    The second is the week's headline: commits touching any package, and those crossing one,
    each commit once. The third counts, for each unordered pair of packages, the commits outside
    a mass change that touched both, the evidence for which packages would share a home.
    """
    package_claims, crate_claims, _crate_pkg = tables
    per, summary, pairs = {}, {}, {}
    for week, subject, files in commits:
        code = [head_path(p, lineage_map) for p in files if not is_doc(p)]
        if not code:
            continue
        mass = is_mass(subject, files)
        for kind, claims in (('package', package_claims), ('crate', crate_claims)):
            hits = unit_cochange([owner(p, claims) for p in code])
            for unit, (crosses, another) in hits.items():
                cell = per.setdefault(week, {}).setdefault((kind, unit), [0] * len(PACKAGE_FIELDS))
                cell[0] += 1
                cell[1] += crosses
                cell[2] += another
                if not mass:
                    cell[3] += 1
                    cell[4] += crosses
            if kind == 'package' and hits:
                s = summary.setdefault(week, [0, 0])
                s[0] += 1
                s[1] += any(c for c, _a in hits.values())
                if not mass:
                    names = sorted(hits)
                    for i, a in enumerate(names):
                        for b in names[i + 1:]:
                            w = pairs.setdefault(week, {})
                            w[(a, b)] = w.get((a, b), 0) + 1
    return per, summary, pairs


def read_package_rows(path=PACKAGES_CSV):
    return _packages_from_text(path.read_text()) if path.exists() else {}


def _packages_from_text(text):
    out = {}
    for line in text.strip().split('\n')[1:]:
        if line.strip():
            week, kind, unit, *cells = line.split(',')
            out.setdefault(week, {})[(kind, unit)] = [int(c or 0) for c in cells]
    return out


def render_package_rows(per, weeks):
    out = [','.join(['week', 'kind', 'unit'] + PACKAGE_FIELDS)]
    for week in sorted(w for w in per if w in weeks):
        for kind, unit in sorted(per[week]):
            cells = per[week][(kind, unit)]
            if any(cells):
                out.append(','.join([week, kind, unit] + [str(c) for c in cells]))
    return '\n'.join(out) + '\n'


def _share(part, whole):
    return '' if not whole else '%d%%' % round(100.0 * part / whole)


def render_package_section(per, weeks, pairs, release_crates):
    """The appendix's co-change tables: every package, then each release-track crate."""
    total = {}
    for w in weeks:
        for key, cells in per.get(w, {}).items():
            t = total.setdefault(key, [0] * len(PACKAGE_FIELDS))
            for i, c in enumerate(cells):
                t[i] += c
    together = {}
    for w in weeks:
        for pair, n in pairs.get(w, {}).items():
            together[pair] = together.get(pair, 0) + n
    partner = {}
    for (a, b), n in together.items():
        for x, y in ((a, b), (b, a)):
            if n > partner.get(x, (0, ''))[0] or (n == partner[x][0] and y < partner[x][1]):
                partner[x] = (n, y)

    def row(name, t, *extra):
        cells = ['`%s`' % name, str(t[0]), '%d (%s)' % (t[1], _share(t[1], t[0]) or '0%'),
                 '%d (%s)' % (t[2], _share(t[2], t[0]) or '0%'), _share(t[4], t[3]) or 'none']
        return '| %s |' % ' | '.join(cells + list(extra))
    span = '%s to %s' % (weeks[0], weeks[-1]) if weeks else 'no weeks'
    out = ['', '## Co-change per package', '',
           'Every package in `packages/`, %s, by the method of the contracts\' co-change: a commit '
           'touching the package crosses when it also touches code outside it, and touches another '
           'when that code is another package\'s. The partner is the package most often in the '
           'same commit outside a mass change. A package that crosses rarely could take a home of '
           'its own; packages that keep naming each other share one, since a home is sized by what '
           'changes together.' % span, '',
           '| package | commits | crossing | touching another | crossing, mass excluded '
           '| most often with |',
           '|---|---:|---:|---:|---:|---|']
    names = sorted((u for k, u in total if k == 'package'),
                   key=lambda u: (-total[('package', u)][0], u))
    for u in names:
        p = partner.get(u)
        out.append(row(u, total[('package', u)], '`%s` (%d)' % (p[1], p[0]) if p else 'none'))
    out += ['', '## Co-change per release-track crate', '',
            'Each crate a per-crate release milestone names (`helpers/crate_release.py`), on its '
            'own: crossing is code outside the crate\'s directory, and touching another is another '
            'Cargo package. Least crossing first, mass changes excluded; every crate is a row in '
            'the CSV.', '',
            '| crate | commits | crossing | touching another | crossing, mass excluded |',
            '|---|---:|---:|---:|---:|']
    def least_crossing(c):
        t = total.get(('crate', c), [0] * len(PACKAGE_FIELDS))
        return (t[4] / t[3] if t[3] else 1.0, c)
    for c in sorted(release_crates, key=least_crossing):
        out.append(row(c, total.get(('crate', c), [0] * len(PACKAGE_FIELDS))))
    return out


def package_cells(summary):
    return {'interface_package_commits': summary[0],
            'interface_package_commits_crossing': summary[1],
            'interface_package_cochange_pct': pct(summary[1], summary[0])}


def release_track_crates():
    names = []
    for number in crate_release.MILESTONES:
        found = sorted(pathlib.Path('design/roadmap').glob('%04d-*.md' % number))
        name = crate_release.crate_of(found[0].read_text()) if found else None
        if name:
            names.append(name)
    return names


def pct(part, whole):
    return '' if not whole else '%.1f' % (100.0 * part / whole)


# ---------------------------------------------------------------------------------------------
# Measuring one tree.

def toolchain():
    text = pathlib.Path('rust-toolchain.toml').read_text()
    m = re.search(r'^channel\s*=\s*"([^"]+)"', text, re.M)
    return m.group(1) if m else 'nightly'


def _package_and_lib(cargo_toml):
    pkg = re.search(r'^\[package\][^\[]*?^name\s*=\s*"([^"]+)"', cargo_toml, re.M | re.S)
    lib = re.search(r'^\[lib\][^\[]*?^name\s*=\s*"([^"]+)"', cargo_toml, re.M | re.S)
    if not pkg:
        return None, None
    return pkg.group(1), (lib.group(1) if lib else pkg.group(1)).replace('-', '_')


def measure(rev, lineage_map, wanted):
    """{identity: {'dir', 'items', 'ledger'}} for the interface crates in `rev`, and the unmeasured.

    `wanted` is the set of identities (HEAD names) that make up the interface. A cached surface is
    trusted only when it covers every wanted identity, present or unmeasured: a cache from a run
    with a narrower `wanted` is missing crates silence, not absence, and reading one turns every
    crate it left out into a mass removal on one side.
    """
    cache = SCRATCH / 'surfaces' / ('%s-%s.json' % (SURFACE_VERSION, rev))
    if cache.exists():
        saved = json.loads(cache.read_text())
        covered = set(saved['crates']) | set(saved['unmeasured'])
        if covered >= set(wanted):
            return saved['crates'], saved['unmeasured']
    tree = SCRATCH / 'tree'
    if tree.exists():
        shutil.rmtree(tree)
    tree.mkdir(parents=True)
    archive = subprocess.run(['git', 'archive', rev], capture_output=True, check=True).stdout
    # `-m`: every file gets today's mtime. `git archive` stamps files with the commit's time, which
    # is OLDER than the artifacts the previous week left in the shared target directory, and cargo
    # then trusts a stale `abi` rmeta and fails to build the crates that use it. Measured: that
    # failure is what first showed `supervision_protocol` as unmeasured on five weeks.
    subprocess.run(['tar', '-x', '-m', '-C', str(tree)], input=archive, check=True)
    present = {}
    for toml in sorted((tree / 'crates').glob('*/Cargo.toml')):
        d = toml.parent.name
        ident = identity(d, lineage_map)
        if ident not in wanted:
            continue
        pkg, lib = _package_and_lib(toml.read_text())
        if pkg and (toml.parent / 'src' / 'lib.rs').exists():
            present[ident] = (d, pkg, lib)
    env = dict(os.environ, RUSTUP_TOOLCHAIN=toolchain(),
               CARGO_TARGET_DIR=str((SCRATCH / 'cargo').resolve()),
               RUSTDOCFLAGS='-Zunstable-options --output-format json --cap-lints allow')
    env.pop('RUSTFLAGS', None)
    docdir = SCRATCH / 'cargo' / TARGET / 'doc'

    def doc(pkgs):
        args = ['cargo', 'doc', '--no-deps', '-q', '--target', TARGET]
        for p in pkgs:
            args += ['-p', p]
        return subprocess.run(args, cwd=tree, env=env, capture_output=True).returncode == 0

    if docdir.exists():
        shutil.rmtree(docdir)
    ok = doc([p for _d, p, _l in present.values()]) if present else True
    unmeasured = []
    crates = {}
    for ident, (d, pkg, lib) in sorted(present.items()):
        out = docdir / (lib + '.json')
        if not ok and not out.exists():
            doc([pkg])
        if not out.exists():
            unmeasured.append(ident)
            continue
        src = (tree / 'crates' / d / 'src' / 'lib.rs').read_text(errors='replace')
        def read_span(span, root=tree):
            try:
                lines = (root / span['filename']).read_text(errors='replace').split('\n')
                (l0, c0), (l1, c1) = span['begin'], span['end']
                chunk = lines[l0 - 1:l1]
                chunk[-1] = chunk[-1][:c1]
                chunk[0] = chunk[0][c0:]
                return '\n'.join(chunk)
            except (OSError, KeyError, TypeError, ValueError, IndexError):
                return None
        crates[ident] = {'dir': d, 'items': surface(json.loads(out.read_text()), read_span),
                         'ledger': ledger_dates(src)}
    shutil.rmtree(tree)
    cache.parent.mkdir(parents=True, exist_ok=True)
    cache.write_text(json.dumps({'crates': crates, 'unmeasured': unmeasured}))
    return crates, unmeasured


# ---------------------------------------------------------------------------------------------
# The CSV, in `script/metrics`' own format: `week` then the columns, sorted, a row only when it has
# something to say. `script/metrics` reads and rewrites this file too, so the two must agree
# byte for byte on an unchanged row, which `--selftest` checks.

def read_rows(path=CSV):
    return read_rows_from_text(path.read_text()) if path.exists() else {}


def render_rows(rows):
    out = [','.join(['week'] + FIELDS)]
    for week in sorted(rows):
        cells = [str(rows[week].get(f, '')) for f in FIELDS]
        if any(cells):
            out.append(','.join([week] + cells))
    return '\n'.join(out) + '\n'


def read_crate_rows(path=CRATES_CSV):
    return _crates_from_text(path.read_text()) if path.exists() else {}


def _crates_from_text(text):
    crates = {}
    lines = text.strip().split('\n')
    for line in lines[1:]:
        if line.strip():
            week, crate, *cells = line.split(',')
            crates.setdefault(week, {})[crate] = [int(c or 0) for c in cells]
    return crates


def render_crate_rows(crates):
    out = [','.join(['week', 'crate'] + CRATE_FIELDS)]
    for week in sorted(crates):
        for crate in sorted(crates[week]):
            cells = crates[week][crate]
            if any(cells):
                out.append(','.join([week, crate] + [str(c) for c in cells]))
    return '\n'.join(out) + '\n'


def render_appendix(rows, crates, week, events, rev, package_lines=()):
    """The per-crate appendix beside the metrics page: generated, never edited by hand."""
    weeks = sorted(rows)
    recent = weeks[-4:]
    total, last4 = {}, {}
    for w, per in crates.items():
        for crate, cells in per.items():
            t = total.setdefault(crate, [0, 0, 0, 0])
            r = last4.setdefault(crate, [0, 0, 0, 0])
            for i, c in enumerate(cells):
                t[i] += c
                if w in recent:
                    r[i] += c
    out = ['# Interface stability, per crate', '',
           '*Generated by `helpers/interface_stability.py` from '
           '[interface-stability-crates.csv](interface-stability-crates.csv); do not edit. What '
           'each column counts, and what none of them can see, is '
           '[notes/interface-stability.md](../interface-stability.md).*', '',
           '## Every crate, since the first commit', '',
           'Sorted by breaking changes in the last four weeks (%s to %s), then by all time. '
           'Co-change counts a commit once per contract crate it touches, so a column does not '
           'sum to the page\'s weekly share.' % (recent[0], recent[-1]), '',
           '| crate | breaking, 4 weeks | additions, 4 weeks | breaking | additions | commits '
           '| crossing another division |',
           '|---|---:|---:|---:|---:|---:|---:|']
    for crate in sorted(total, key=lambda c: (-last4[c][0], -total[c][0], c)):
        t, r = total[crate], last4[crate]
        share = '' if not t[2] else ' (%d%%)' % round(100.0 * t[3] / t[2])
        out.append('| `%s` | %d | %d | %d | %d | %d | %d%s |'
                   % (crate, r[0], r[1], t[0], t[1], t[2], t[3], share))
    out += ['', '## What broke in %s' % week, '',
            'Read at `%s` against the week before. Each line is one removed or changed public '
            'item, quoted as it stood that week; a crate rename is one line for the whole '
            'crate.' % rev[:12], '']
    if not events:
        out.append('Nothing.')
    # A blockquote: the item's name is quoted with the spelling it had in the earlier week's tree,
    # so a name the American-spelling sweep has since retired (2026-10-06, helpers/house_style.py)
    # appears here spelled the only way an honest record can spell it, and a quotation keeps its
    # source's words.
    for crate, what, path in events:
        out.append('> - `%s`: %s `%s`' % (crate, what, path))
    out += list(package_lines)
    return '\n'.join(out) + '\n'


def cells_for(week, prev_measure, this_measure, std_abi, cochange):
    old, old_un = prev_measure
    new, new_un = this_measure
    unmeasured = sorted(set(old_un) | set(new_un))
    # A crate measured on one side only because the other side FAILED is left out of both, so a
    # documentation failure is never read as a crate appearing or vanishing.
    old = {k: v for k, v in old.items() if k not in new_un}
    new = {k: v for k, v in new.items() if k not in old_un}
    counts, per_crate, events = compare(old, new, std_abi)
    amendments = sum(1 for c in new.values() for d in c.get('ledger', [])
                     if week_of_date(d) == week)
    cc = cochange.get(week, [0, 0, 0, 0])
    row = {
        'interface_breaking': counts['breaking'],
        'interface_additions': counts['additions'],
        'interface_std_abi_breaking': counts['std_breaking'],
        'interface_std_abi_additions': counts['std_additions'],
        'interface_contract_api_breaking': counts['contract_breaking'],
        'interface_contract_api_additions': counts['contract_additions'],
        'interface_syscall_added': counts['syscall_added'],
        'interface_syscall_changed': counts['syscall_changed'],
        'interface_syscall_renamed': counts['syscall_renamed'],
        'interface_format_bumps': counts['format_bumps'],
        'interface_format_amendments': amendments,
        'interface_contract_commits': cc[0],
        'interface_contract_commits_crossing': cc[1],
        'interface_cochange_pct': pct(cc[1], cc[0]),
        'interface_cochange_pct_excluding_mass': pct(cc[3], cc[2]),
        'interface_crates_unmeasured': len(unmeasured),
    }
    return row, per_crate, events


def run(mode):
    lineage_map = lineage()
    LINEAGE_CACHE.update(lineage_map)
    std_abi = std_abi_set(std_abi_source_texts())
    wanted = set(CONTRACTS) | std_abi
    weeks = week_commits()
    head = git('rev-parse', 'HEAD').strip()
    current = week_of(git('show', '-s', '--format=%cI', head).strip())
    rows = read_rows()
    order = list(weeks)
    if mode == '--backfill':
        targets = order
    else:
        targets = [w for w in order if (w not in rows and mode == '--update') or w == current]
    commits = commit_log()
    cochange, cochange_crates = cochange_by_week(contract_dirs(lineage_map), commits)
    per_unit, package_summary, pairs = package_cochange_by_week(commits, unit_tables(), lineage_map)
    measured = {}

    def at(week):
        if week not in measured:
            measured[week] = measure(weeks[week], lineage_map, wanted) if week else ({}, [])
        return measured[week]

    result = {}
    for week in targets:
        i = order.index(week)
        prev = order[i - 1] if i else None
        row, per_crate, events = cells_for(week, at(prev), at(week), std_abi, cochange)
        result[week] = (row, per_crate, events)
        print('interface stability: %s  breaking %s, additions %s, unmeasured %s'
              % (week, row['interface_breaking'], row['interface_additions'],
                 row['interface_crates_unmeasured']), file=sys.stderr)
        for crate, what, detail in events[:20]:
            print('    %s: %s %s' % (crate, what, detail), file=sys.stderr)
    if mode == '--table':
        row = dict(result[current][0], **package_cells(package_summary.get(current, [0, 0])))
        for f in FIELDS:
            print('%-42s %s' % (f, row[f]))
        return 0
    if mode == '--backfill':
        rows = {}
    crates = {} if mode == '--backfill' else read_crate_rows()
    for week, (row, per_crate, _e) in result.items():
        rows[week] = row
        crates[week] = {c: [v['breaking'], v['additions']] + cochange_crates.get(week, {}).get(c, [0, 0])
                        for c, v in per_crate.items()}
        for c, v in cochange_crates.get(week, {}).items():
            crates[week].setdefault(c, [0, 0] + v)
    # Git alone answers the package cells, so every week the series holds is restated, not
    # carried: a week's cell changes only when HEAD's package declarations or lineage do.
    for week, row in rows.items():
        row.update(package_cells(package_summary.get(week, [0, 0])))
    CSV.parent.mkdir(parents=True, exist_ok=True)
    CSV.write_text(render_rows(rows))
    CRATES_CSV.write_text(render_crate_rows(crates))
    PACKAGES_CSV.write_text(render_package_rows(per_unit, set(rows)))
    last = max(result)
    APPENDIX.write_text(render_appendix(
        rows, crates, last, result[last][2], weeks[last],
        render_package_section(per_unit, sorted(rows), pairs, release_track_crates())))
    print('interface stability: %d weeks in %s' % (len(rows), CSV), file=sys.stderr)
    return 0


# ---------------------------------------------------------------------------------------------

def selftest():
    """Fixtures, no git and no cargo: each case is a way a wrong reader would mislead."""
    def doc(items, root_items):
        index = {str(k): v for k, v in items.items()}
        index['0'] = {'name': 'c', 'inner': {'module': {'items': root_items}}}
        return {'root': 0, 'index': index}

    def const(name, value, vis='public'):
        return {'name': name, 'visibility': vis,
                'inner': {'constant': {'type': {'primitive': 'u64'},
                                       'const': {'expr': value, 'value': value + 'u64'}}}}

    def fn(name, ret, id_=None):
        return {'name': name, 'visibility': 'public', 'id': id_,
                'inner': {'function': {'sig': {'inputs': [], 'output': {'primitive': ret}}}}}

    def strukt(name, fields, impls=(), repr_c=True):
        return {'name': name, 'visibility': 'public', 'attrs': ['#[repr(C)]'] if repr_c else [],
                'inner': {'struct': {'kind': {'plain': {'fields': fields,
                                                        'has_stripped_fields': False}},
                                     'generics': {}, 'impls': list(impls)}}}

    def field(name, ty):
        return {'name': name, 'visibility': 'public',
                'inner': {'struct_field': {'primitive': ty, 'id': 99}}}

    def module(name, items):
        return {'name': name, 'visibility': 'public', 'inner': {'module': {'items': items}}}

    def trait_impl(trait, id_=7):
        return {'inner': {'impl': {'trait': {'path': trait, 'id': id_}, 'items': [],
                                   'blanket_impl': None}}}

    def blanket():
        return {'inner': {'impl': {'trait': {'path': 'From'}, 'items': [],
                                   'blanket_impl': {'generic': 'T'}}}}

    old = surface(doc({1: const('SYS_EXIT', '0'), 2: module('objtype', [3]),
                       3: const('RENDEZVOUS', '1'), 4: fn('helper', 'u8', 1),
                       5: strukt('Header', [6, 7], [10, 11]), 6: field('len', 'u32'),
                       7: field('kind', 'u16'), 8: const('VERSION', '1'),
                       10: trait_impl('Clone'), 11: blanket()},
                      [1, 2, 4, 5, 8]))
    # Renumbered SYS_EXIT; a new object type; helper's id moved (not a change); fields swapped
    # (a layout change in a repr(C) struct even though no field changed type); version bumped.
    new = surface(doc({1: const('SYS_EXIT', '9'), 2: module('objtype', [3, 12]),
                       3: const('RENDEZVOUS', '1'), 12: const('NOTIFICATION', '2'),
                       4: fn('helper', 'u8', 44),
                       5: strukt('Header', [7, 6], [10, 11]), 6: field('len', 'u32'),
                       7: field('kind', 'u16'), 8: const('VERSION', '2'),
                       10: trait_impl('Clone', 1234), 11: blanket()},
                      [1, 2, 4, 5, 8]))
    counts, per_crate, events = compare({'abi': {'dir': 'abi', 'items': old}},
                                        {'abi': {'dir': 'abi', 'items': new}}, {'abi'})
    renamed, _p, _e = compare({'fs': {'dir': 'fs_proto', 'items': {'a': '1'}}},
                              {'fs': {'dir': 'fs_protocol', 'items': {'a': '1'}}}, set())
    born, _p, _e = compare({}, {'n': {'dir': 'n', 'items': {'a': '1', 'b': '2'}}}, set())
    gone, _p, _e = compare({'n': {'dir': 'n', 'items': {'a': '1', 'b': '2'}}}, {}, set())
    def use_item(name, source, id_, glob=False):
        return {'name': None, 'visibility': 'public',
                'inner': {'use': {'source': source, 'name': name, 'id': id_, 'is_glob': glob}}}

    # §282: the parent becomes an index. The definition moves to a private child module and the
    # root re-exports it; the consumer's paths do not move, so neither may the surface.
    whole = {5: strukt('Header', [6], [10]), 6: field('len', 'u32'), 10: trait_impl('Clone')}
    before_split = surface(doc(whole, [5]))
    items_after_split = dict(whole)
    items_after_split[20] = use_item('Header', 'header::Header', 5)
    after_split = surface(doc(items_after_split, [20]))
    churn, churn_pc, _e = compare(
        {'grant_plan': {'dir': 'grant_plan',
                        'items': {'trim': _fp(_clean(const('trim', '0')['inner']))}}},
        {'grant_plan': {'dir': 'grant_plan',
                        'items': {'trim': _fp(_clean(const('trim', '9')['inner']))}}},
        set())
    renamed_syscall, _pc, _e = compare(
        {'abi': {'dir': 'abi', 'items': {'irq::ACK': _fp(_clean(const('ACK', '3')['inner']))}}},
        {'abi': {'dir': 'abi',
                 'items': {'interrupt::ACK': _fp(_clean(const('ACK', '3')['inner']))}}},
        {'abi'})
    bumped, _pc, _e = compare(
        {'nifefs': {'dir': 'nifefs',
                    'items': {'DISK_VERSION': _fp(_clean(const('DISK_VERSION', '1')['inner']))}}},
        {'nifefs': {'dir': 'nifefs',
                    'items': {'DISK_VERSION': _fp(_clean(const('DISK_VERSION', '2')['inner']))}}},
        set())
    std_add, _pc, _e = compare({'n': {'dir': 'allocation_cache', 'items': {}}},
                               {'n': {'dir': 'allocation_cache', 'items': {'a': '1'}}}, {'n'})
    def variant(value):
        return _fp({'fields': [], 'discriminant': {'expr': str(value), 'value': str(value)}})

    def one(crate, old_items, new_items):
        return compare({crate: {'dir': crate, 'items': old_items}},
                       {crate: {'dir': crate, 'items': new_items}}, set())[0]

    err_renumbered = one('abi', {'Error::NoSuchSlot': variant(-1)}, {'Error::NoSuchSlot': variant(-7)})
    err_added = one('abi', {}, {'Error::Fresh': variant(-99)})
    err_removed = one('abi', {'Error::NoSuchSlot': variant(-1)}, {})
    enum_item = one('abi', {'Error': _fp({'kind': 'enum', 'variants': ['A']})},
                    {'Error': _fp({'kind': 'enum', 'variants': ['A', 'B']})})
    other_variant = one('grant_plan', {'Mode::A': variant(1)}, {'Mode::A': variant(2)})
    ledger = ledger_dates('//! Intro\n//!\n//! # Amendments\n//!\n//! - 2026-09-27: field order\n'
                          '//!   continued, not an item\n//! - 2026-10-02 (UTC): again\n'
                          '//! # BUGS\n//! - 2026-01-01: not an amendment\n')
    dirs = ('crates/abi/', 'crates/fs_proto/')
    rows = {'2026W38': {f: '' for f in FIELDS}, '2026W39': {f: str(i) for i, f in enumerate(FIELDS)}}
    rows['2026W38']['interface_breaking'] = '0'
    cases = [
        ('a module rename that keeps the number is a syscall rename, not a change',
         compare({'abi': {'dir': 'abi', 'items': {'endpoint::SEND': _fp(_clean(const('SEND', '0')['inner']))}}},
                 {'abi': {'dir': 'abi', 'items': {'rendezvous::SEND': _fp(_clean(const('SEND', '0')['inner']))}}},
                 set())[0]['syscall_renamed'], 1),
        ('a module rename learned from a sibling resolves an ambiguous constant',
         sorted(syscall_renames('abi', ['frame::MAP', 'frame::REVOKE', 'untyped::MAP'],
                                ['page_frame::MAP', 'page_frame::REVOKE', 'region::MAP'],
                                {'frame::MAP': _fp(_clean(const('MAP', '0')['inner'])),
                                 'frame::REVOKE': _fp(_clean(const('REVOKE', '1')['inner'])),
                                 'untyped::MAP': _fp(_clean(const('MAP', '0')['inner']))},
                                {'page_frame::MAP': _fp(_clean(const('MAP', '0')['inner'])),
                                 'page_frame::REVOKE': _fp(_clean(const('REVOKE', '1')['inner'])),
                                 'region::MAP': _fp(_clean(const('MAP', '0')['inner']))}).items()),
         [('frame::MAP', 'page_frame::MAP'), ('frame::REVOKE', 'page_frame::REVOKE')]),
        ('two same-valued successors are ambiguous, so a change',
         compare({'abi': {'dir': 'abi', 'items': {'m::A': _fp(_clean(const('A', '0')['inner']))}}},
                 {'abi': {'dir': 'abi', 'items': {'m::B': _fp(_clean(const('B', '0')['inner'])),
                                                  'm::C': _fp(_clean(const('C', '0')['inner']))}}},
                 set())[0]['syscall_changed'], 1),
        ('a byte-string magic rustdoc cannot evaluate is read from source',
         const_source('pub const MAGIC: [u8; 8] = *b"CRKR0002";'), '*b"CRKR0002"'),
        ('a renamed parameter is not a change',
         _clean({'sig': {'inputs': [['a', {'primitive': 'u8'}]]}})
         == _clean({'sig': {'inputs': [['b', {'primitive': 'u8'}]]}}), True),
        ('a respelled constant with the same value is not a change',
         _clean({'constant': {'const': {'expr': '1 << 2', 'value': '4u64'}}})
         == _clean({'constant': {'const': {'expr': '4', 'value': '4u64'}}}), True),
        ('the private constant is not surface',
         sorted(surface(doc({1: const('A', '1'), 2: const('B', '1', 'crate')}, [1]))), ['A']),
        ('a blanket impl is not surface', any('From' in k for k in old), False),
        ('a derived trait impl is surface', 'Header: impl Clone' in old, True),
        ('a struct field is its own item', 'Header::len' in old, True),
        ('a definition moved behind a root re-export moves no surface',
         after_split == before_split, True),
        ('the re-export path carries the members',
         'Header::len' in after_split and 'Header: impl Clone' in after_split, True),
        ('a re-export chain resolves to the item it names',
         sorted(surface(doc({40: use_item('A', 'c::A', 41), 41: use_item('A', 'd::A', 42),
                            42: const('A', '2')}, [40]))), ['A']),
        ('a use of a module re-exports its items at the alias',
         sorted(surface(doc({30: module('inner', [31]), 31: const('LIMIT', '4'),
                            32: use_item('types', 'inner', 30)}, [32]))), ['types::LIMIT']),
        ('a glob re-export stays keyed by its source',
         sorted(surface(doc({32: use_item(None, 'inner::x', None, glob=True)}, [32]))),
         ['*inner::x']),
        ('a foreign re-export is a name; its shape is not ours',
         surface(doc({32: use_item('X', 'other::X', 404)}, [32])),
         {'X': _fp({'use': 'other::X'})}),
        ('contract api churn is reported, not boundary',
         (churn['breaking'], churn['contract_breaking']), (0, 1)),
        ('per-crate breaking keeps the item truth the weekly view demotes',
         churn_pc['grant_plan']['breaking'], 1),
        ('a number-kept syscall rename is boundary', renamed_syscall['breaking'], 1),
        ('a format bump in a contract crate is boundary',
         (bumped['breaking'], bumped['format_bumps'], bumped['contract_breaking']), (1, 1, 0)),
        ('a std crate addition is reported, not headline',
         (std_add['additions'], std_add['std_additions']), (0, 1)),
        ('breaking: renumber and version bump; the field reorder is std churn',
         counts['breaking'], 2),
        ('additions: the new object type', counts['additions'], 1),
        ('an id rustdoc renumbered is not a change', 'helper' not in
         [e[2].split(' ')[0] for e in events], True),
        ('the renumbered syscall is a syscall change', counts['syscall_changed'], 1),
        ('the new object type is a syscall addition', counts['syscall_added'], 1),
        ('the version constant is a format bump', counts['format_bumps'], 1),
        ('the renumber is classed as a syscall', syscall_class('SYS_EXIT'), 'syscall'),
        ('an object type is classed as one', syscall_class('objtype::NOTIFICATION'), 'object type'),
        ('an error code is classed as one', syscall_class('Error::NoSuchSlot'), 'error code'),
        ('a renumbered abi error code is boundary',
         (err_renumbered['breaking'], err_renumbered['syscall_changed'],
          err_renumbered['contract_breaking']), (1, 1, 0)),
        ('a new abi error code is a boundary addition',
         (err_added['additions'], err_added['syscall_added']), (1, 1)),
        ('a removed abi error code is boundary', err_removed['breaking'], 1),
        ('the enum item itself is not boundary',
         (enum_item['breaking'], enum_item['contract_breaking']), (0, 1)),
        ('a variant outside abi stays contract api',
         (other_variant['breaking'], other_variant['contract_breaking']), (0, 1)),
        ('std abi reports the items that are not the boundary', counts['std_breaking'], 1),
        ('a crate rename is one decision; off the boundary it is contract churn',
         (renamed['breaking'], renamed['renamed'], renamed['contract_breaking']), (0, 1, 1)),
        ('a new crate is contract additions',
         (born['breaking'], born['contract_additions']), (0, 2)),
        ('a deleted crate is every item removed', gone['contract_breaking'], 2),
        ('BOTH_VERSIONS is a flag, not a format', is_format_const(
            'BOTH_VERSIONS', '{"constant":1}'), False),
        ('a magic in a submodule is a format identity',
         is_format_const('header::MAGIC_VALUE', '{"constant":1}'), True),
        ('SLOT_VERSION is a format identity', is_format_const('SLOT_VERSION', '{"constant":1}'),
         True),
        ('the ledger reads dated items under its heading only', ledger,
         ['2026-09-27', '2026-10-02']),
        ('lineage follows a chain of renames',
         resolve_lineage({'fs': 'fs_proto', 'fs_proto': 'filesystem_contract'}),
         {'fs': 'filesystem_contract', 'fs_proto': 'filesystem_contract'}),
        ('lineage survives a cycle', resolve_lineage({'a': 'b', 'b': 'a'})['a'] in ('a', 'b'), True),
        ('contract dirs include old names',
         'crates/fs_proto/' in contract_dirs({'fs_proto': 'filesystem_contract'}), True),
        ('a docs-only companion does not cross',
         classify_commit('x', ['crates/abi/src/lib.rs', 'notes/abi.md', 'design/x.md'], dirs),
         (True, False, False)),
        ('a kernel companion crosses',
         classify_commit('x', ['crates/abi/src/lib.rs', 'kernel/src/a.rs'], dirs),
         (True, True, False)),
        ('an old directory name is still a contract',
         classify_commit('x', ['crates/fs_proto/src/lib.rs'], dirs)[0], True),
        ('a rename subject is mass', classify_commit('rename the protos',
                                                     ['crates/abi/src/lib.rs'], dirs)[2], True),
        ('41 files is mass', classify_commit('x', ['crates/abi/%d' % i for i in range(41)],
                                             dirs)[2], True),
        ('no contract path, no commit', classify_commit('x', ['kernel/a.rs'], dirs),
         (False, False, False)),
        ('an empty week has no share, not zero', pct(0, 0), ''),
        ('the CSV round-trips', render_rows(read_rows_from_text(render_rows(rows))),
         render_rows(rows)),
        ('the per-crate CSV round-trips and drops an all-zero row',
         render_crate_rows(_crates_from_text(render_crate_rows(
             {'2026W39': {'abi': [1, 2, 3, 4], 'elf': [0, 0, 0, 0]}}))),
         'week,crate,breaking,additions,commits,commits_crossing\n2026W39,abi,1,2,3,4\n'),
        ('the appendix lists a week with no breaks as nothing',
         render_appendix({'2026W39': {}}, {'2026W39': {'abi': [0, 1, 1, 1]}}, '2026W39', [],
                         'f' * 40).rstrip().endswith('Nothing.'), True),
        ('a what-broke line is a quotation line, so a retired spelling stays quotable',
         render_appendix({'2026W39': {}}, {'2026W39': {}}, '2026W39',
                         [('c', 'removed', 'X')], 'f' * 40)
         .count('> - `c`: removed `X`'), 1),
        ('the std abi list comes from the copy',
         std_abi_crates('root.join("crates/abi/src/lib.rs"),\nroot.join("crates/abi/src/lib.rs"),'
                        ' root.join("crates/clock_contract/src/lib.rs")'),
         ['abi', 'clock_contract']),
    ]

    # Per-package co-change, over helpers/packages.py's own fixture tree: `toolbox` holds the crates
    # `lib`, `tool` and `host` and the program `b`; `extra` holds the program `a`, a file inside
    # `host`'s directory; `contracts` holds `proto`; `notes/` and the root manifest are homes only.
    tables = unit_tables(*packages._fixture())
    fixture_log = [
        ('2026W39', 'x', ['crates/lib/src/lib.rs', 'crates/tool/src/main.rs']),   # one package
        ('2026W39', 'x', ['crates/proto/src/lib.rs', 'crates/lib/src/lib.rs']),   # two packages
        ('2026W39', 'x', ['crates/lib/src/lib.rs', 'Cargo.toml']),                # a home, no package
        ('2026W39', 'x', ['crates/lib/src/lib.rs', 'notes/a.md']),                # documentation
        ('2026W39', 'x', ['notes/a.md']),                                         # nothing
        ('2026W40', 'rename everything', ['crates/old_lib/src/lib.rs', 'host/src/a.rs']),
    ]
    per, summary, pairs = package_cochange_by_week(fixture_log, tables, {'old_lib': 'lib'})
    cases += [
        ('a program file inside another crate belongs to its own package',
         owner('host/src/a.rs', tables[0]), 'extra'),
        ('the rest of that crate belongs to the crate\'s package',
         owner('host/src/c.rs', tables[0]), 'toolbox'),
        ('a home claim is no package', owner('notes/a.md', tables[0]), None),
        ('a path nothing claims is no package', owner('elsewhere/x', tables[0]), None),
        ('a crate is placed by its directory', owner('host/src/a.rs', tables[1]), 'host'),
        ('an old crate directory is read under its HEAD name',
         head_path('crates/old_lib/src/lib.rs', {'old_lib': 'lib'}), 'crates/lib/src/lib.rs'),
        ('one unit alone neither crosses nor touches another', unit_cochange(['a', 'a']),
         {'a': (False, False)}),
        ('a home path crosses without touching another package', unit_cochange(['a', None]),
         {'a': (True, False)}),
        ('two units each cross and touch the other', unit_cochange(['a', 'b']),
         {'a': (True, True), 'b': (True, True)}),
        ('a package\'s week: commits, crossing, another, and the same outside mass changes',
         per['2026W39'][('package', 'toolbox')], [4, 2, 1, 4, 2]),
        ('two crates of one package touch each other at the crate level',
         per['2026W39'][('crate', 'tool')], [1, 1, 1, 1, 1]),
        ('the headline counts each commit once', summary['2026W39'], [4, 2]),
        ('a mass commit counts, but not in the mass-excluded columns or the pairs',
         (per['2026W40'][('package', 'extra')], pairs.get('2026W40')), ([1, 1, 1, 0, 0], None)),
        ('a pair is counted once per commit', pairs['2026W39'], {('contracts', 'toolbox'): 1}),
        ('the per-package CSV round-trips and keeps only the series\' weeks',
         _packages_from_text(render_package_rows(per, {'2026W39'})) == {'2026W39': per['2026W39']},
         True),
        ('a package\'s partner is the one it shares most commits with',
         '| `toolbox` | 4 | 2 (50%) | 1 (25%) | 50% | `contracts` (1) |'
         in render_package_section(per, ['2026W39'], pairs, ['lib']), True),
    ]

    def empty_set_refused():
        try:
            std_abi_set(['no copies named here', ''])
        except SystemExit as e:
            return 'would read empty' in str(e)
        return False
    cases.append(('an empty std copy set is refused, not measured', empty_set_refused(), True))
    failed = [(name, got, want) for name, got, want in cases if got != want]
    for name, got, want in failed:
        print('interface stability selftest: %s: got %r, want %r' % (name, got, want),
              file=sys.stderr)
    if failed:
        return 1
    print('interface stability selftest: %d cases pass' % len(cases))
    return 0


def read_rows_from_text(text):
    rows = {}
    lines = text.strip().split('\n')
    header = lines[0].split(',')
    for line in lines[1:]:
        if line.strip():
            cells = dict(zip(header, line.split(',')))
            rows[cells['week']] = {f: cells.get(f, '') for f in FIELDS}
    return rows


def main(argv):
    mode = argv[0] if argv else ''
    if mode == '--selftest':
        return selftest()
    if mode in ('--update', '--backfill', '--table'):
        return run(mode)
    print(__doc__.split('\n\n')[2], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
