"""Prepare the architect's review of one crate: a single markdown packet, written under target/.

Read by script/crate-review-packet. The reader of a packet is the architect, who reads every crate
by hand before it leaves nife (condition 4 of section 247, ruled 2026-11-01 UTC). The packet gathers
what a reader would otherwise collect by hand and leaves the judgments, which no script can make,
as a checklist.

Name: provisional, minted 2026-11-01 (UTC). It follows `crate_release.py`, whose static findings it
reuses so that this packet and `script/crate-standalone` cannot disagree about a reference back.
"""
import json
import os
import re
import subprocess
import sys
import tomllib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import crate_release  # noqa: E402

DEP_TABLES = ('dependencies', 'dev-dependencies', 'build-dependencies')
CONSTANT = re.compile(
    r'^\s*pub(?:\([a-z]+\))?\s+const\s+([A-Z0-9_]*(?:INTERFACE_ID|VERSION|MAGIC)[A-Z0-9_]*)\s*:\s*[^=]+=\s*(.+?);?\s*$')
UNSAFE = re.compile(r'\bunsafe\b')
PROOF = re.compile(r'#\[\s*kani::proof(?:_for_contract[^\]]*)?\s*\]')
FN = re.compile(r'\bfn\s+([A-Za-z0-9_]+)')
OUTSIDE_STRING = re.compile(r'"[^"\n]*\.\./[^"\n]*"')


def git(root, *args):
    return subprocess.run(['git', '-C', root, *args], capture_output=True, text=True).stdout


def tracked(root, crate):
    return [f for f in git(root, 'ls-files', f'crates/{crate}').split()]


def read(root, rel):
    return open(os.path.join(root, rel), errors='replace').read()


def code_lines(text):
    """[(line number, line)] with whole-line comments and string-free trailing comments removed."""
    out = []
    for n, line in enumerate(text.split('\n'), 1):
        s = line.strip()
        if s.startswith('//'):
            continue
        out.append((n, line.split('//')[0] if '"' not in line else line))
    return out


def resolve_workspace_dep(ws, name):
    return ws.get('dependencies', {}).get(name)


def describe_dep(root, crate, name, spec, ws):
    """One dependency's source: where it points."""
    inherited = False
    if isinstance(spec, dict) and spec.get('workspace'):
        inherited = True
        base = resolve_workspace_dep(ws, name)
        spec = base if base is not None else spec
    if isinstance(spec, str):
        return f'registry, version `{spec}`'
    note = ' (inherited from the nife workspace)' if inherited else ''
    pkg = spec.get('package')
    rename = f' as `{pkg}`' if pkg else ''
    if 'path' in spec:
        target = os.path.normpath(os.path.join(root, 'crates', crate, spec['path']))
        rel = os.path.relpath(target, root)
        if rel.startswith('..'):
            return f'path `{spec["path"]}`, outside the nife tree{rename}{note}'
        if rel.startswith('crates/'):
            where = f'a sibling crate in nife, `{rel}`'
        else:
            where = f'inside nife at `{rel}`'
        return f'path `{spec["path"]}`, {where}{rename}{note}'
    if 'git' in spec:
        pin = next((f'{k} `{spec[k]}`' for k in ('rev', 'tag', 'branch') if k in spec), 'no rev')
        return f'git `{spec["git"]}`, {pin}{rename}{note}'
    return f'registry, version `{spec.get("version", "*")}`{rename}{note}'


def dependency_rows(root, crate, manifest, ws):
    rows = []
    tables = [(t, manifest.get(t, {})) for t in DEP_TABLES]
    for tgt, body in manifest.get('target', {}).items():
        tables += [(f'{t} [target {tgt}]', body.get(t, {})) for t in DEP_TABLES]
    for table, deps in tables:
        for name, spec in sorted(deps.items()):
            rows.append((name, table, describe_dep(root, crate, name, spec, ws)))
    return rows


def all_manifests(root):
    out = {}
    for f in git(root, 'ls-files', '*Cargo.toml').split():
        if f.startswith('vendor/') or '/vendor/' in f:
            continue
        try:
            out[f] = tomllib.loads(read(root, f))
        except (OSError, tomllib.TOMLDecodeError):
            pass
    return out


def dependents(root, crate, ws):
    """[(manifest path, package, table)] for every nife manifest that depends on `crate`."""
    found = []
    wsdeps = ws.get('dependencies', {})
    for path, m in sorted(all_manifests(root).items()):
        if path == f'crates/{crate}/Cargo.toml':
            continue
        pkg = m.get('package', {}).get('name', '(workspace root)')
        tables = [(t, m.get(t, {})) for t in DEP_TABLES]
        for tgt, body in m.get('target', {}).items():
            tables += [(f'{t} [target {tgt}]', body.get(t, {})) for t in DEP_TABLES]
        for table, deps in tables:
            for name, spec in deps.items():
                real = name
                if isinstance(spec, dict):
                    if spec.get('workspace') and name in wsdeps and isinstance(wsdeps[name], dict):
                        real = wsdeps[name].get('package', name)
                    real = spec.get('package', real)
                if real == crate:
                    found.append((path, pkg, table))
    return found


def line_counts(root, files):
    rows = []
    for f in sorted(files):
        if '/src/' in f and f.endswith('.rs'):
            text = read(root, f)
            rows.append((f, text.count('\n') + (0 if text.endswith('\n') or not text else 1)))
    return rows


def unsafe_sites(root, files):
    sites = []
    for f in sorted(files):
        if not f.endswith('.rs') or '/src/' not in f:
            continue
        for n, line in code_lines(read(root, f)):
            if not UNSAFE.search(line):
                continue
            s = line.strip()
            if re.search(r'unsafe_code|unsafe_op_in_unsafe_fn', s):
                continue
            if re.search(r'unsafe\s+impl', s):
                kind = 'unsafe impl'
            elif re.search(r'unsafe\s+(?:extern\s+"[^"]*"\s+)?fn', s) or re.search(r'unsafe\s+extern', s):
                kind = 'unsafe fn or extern'
            elif re.search(r'unsafe\s*\{', s) or s.endswith('unsafe'):
                kind = 'unsafe block'
            else:
                kind = 'unsafe (other)'
            sites.append((f, n, kind, s[:110]))
    return sites


def unsafe_policy(root, files):
    hits = []
    for f in sorted(files):
        if f.endswith('.rs') or f == f'crates/{f.split("/")[1]}/Cargo.toml':
            for n, line in enumerate(read(root, f).split('\n'), 1):
                if re.search(r'(forbid|deny|warn)\s*\(\s*unsafe_code', line):
                    hits.append(f'{f}:{n}: `{line.strip()}`')
    return hits


def harnesses(root, files):
    found = []
    for f in sorted(files):
        if not f.endswith('.rs'):
            continue
        lines = read(root, f).split('\n')
        for i, line in enumerate(lines):
            if PROOF.search(line):
                for j in range(i, min(i + 8, len(lines))):
                    m = FN.search(lines[j])
                    if m and 'kani::' not in lines[j]:
                        found.append((m.group(1), f, j + 1))
                        break
    return found


def falsifications(root, crate, files):
    out = []
    for f in sorted(files):
        if '/falsifications/' in f and f.endswith('.patch'):
            stem = os.path.basename(f)[:-len('.patch')]
            out.append((stem, f))
    return out


def constants(root, files):
    found = []
    for f in sorted(files):
        if not f.endswith('.rs'):
            continue
        for n, line in enumerate(read(root, f).split('\n'), 1):
            m = CONSTANT.match(line)
            if m:
                found.append((m.group(1), m.group(2), f, n))
    return found


# ---- the public API, from rustdoc JSON when the pinned toolchain can write it -------------------

def render_type(t, paths):
    if t is None:
        return '()'
    if not isinstance(t, dict):
        return '?'
    k, v = next(iter(t.items()))
    if k == 'primitive' or k == 'generic':
        return v
    if k == 'resolved_path':
        name = v.get('path') or v.get('name', '?')
        args = v.get('args') or {}
        ab = args.get('angle_bracketed', {}).get('args', []) if isinstance(args, dict) else []
        inner = ', '.join(render_type(a.get('type'), paths) if 'type' in a else
                          a.get('lifetime', '?') if 'lifetime' in a else
                          '..' for a in ab)
        return f'{name}<{inner}>' if inner else name
    if k == 'borrowed_ref':
        lt = (v.get('lifetime') + ' ') if v.get('lifetime') else ''
        return f'&{lt}{"mut " if v.get("is_mutable") else ""}{render_type(v["type"], paths)}'
    if k == 'raw_pointer':
        return f'*{"mut" if v.get("is_mutable") else "const"} {render_type(v["type"], paths)}'
    if k == 'tuple':
        return '(' + ', '.join(render_type(x, paths) for x in v) + ')'
    if k == 'slice':
        return f'[{render_type(v, paths)}]'
    if k == 'array':
        return f'[{render_type(v["type"], paths)}; {v.get("len")}]'
    if k == 'impl_trait':
        return 'impl ..'
    if k == 'dyn_trait':
        return 'dyn ..'
    if k == 'qualified_path':
        return f'<{render_type(v.get("self_type"), paths)}>::{v.get("name")}'
    if k == 'function_pointer':
        return 'fn(..)'
    return f'<{k}>'


PREAMBLE = ('core', 'alloc', 'std')


def external_paths(t, out, paths):
    """Collect the full paths of types from crates other than core, alloc and std that `t` names."""
    if isinstance(t, dict):
        for k, v in t.items():
            if k == 'resolved_path' and isinstance(v, dict):
                info = paths.get(str(v.get('id')))
                if info and info.get('crate_id') != 0 and info['path'][0] not in PREAMBLE:
                    out.append('::'.join(info['path']))
            external_paths(v, out, paths)
    elif isinstance(t, list):
        for x in t:
            external_paths(x, out, paths)


def rustdoc_api(root, crate, outdir):
    """([(kind, path, signature)], [foreign type names], None) or (None, None, why)."""
    td = os.path.join(outdir, 'rustdoc-target')
    p = subprocess.run(
        ['cargo', 'rustdoc', '-p', crate, '--lib', '--target-dir', td, '--',
         '-Z', 'unstable-options', '--output-format', 'json'],
        cwd=root, capture_output=True, text=True)
    path = os.path.join(td, 'doc', f'{crate}.json')
    if p.returncode or not os.path.exists(path):
        why = ' | '.join([l for l in (p.stdout + p.stderr).strip().splitlines() if l.strip()][-3:])
        return None, None, why[:300] or 'no JSON written'
    doc = json.load(open(path))
    index, paths = doc['index'], doc.get('paths', {})
    api, foreign, seen = [], [], set()

    def visit(item_id, prefix):
        item = index.get(str(item_id))
        if item is None or item_id in seen:
            return
        inner = item.get('inner', {})
        kind = next(iter(inner), None)
        name = item.get('name')
        if kind == 'use':
            u = inner['use']
            tgt = u.get('id')
            if tgt is not None and str(tgt) in index:
                if u.get('is_glob'):
                    for c in index[str(tgt)].get('inner', {}).get('module', {}).get('items', []):
                        visit(c, prefix)
                else:
                    visit(tgt, prefix)
            else:
                api.append(('re-export', f'{prefix}{u.get("name")}', f'from `{u.get("source")}`'))
            return
        if item.get('visibility') != 'public' and kind != 'module':
            return
        seen.add(item_id)
        full = f'{prefix}{name}'
        if kind == 'module':
            if item.get('visibility') == 'public':
                api.append(('mod', full, ''))
                for c in inner['module'].get('items', []):
                    visit(c, full + '::')
            return
        sig = ''
        if kind == 'function':
            fn = inner['function']
            sg = fn['sig']
            args = ', '.join(f'{n}: {render_type(ty, paths)}' for n, ty in sg['inputs'])
            ret = sg.get('output')
            sig = f'fn {name}({args})' + (f' -> {render_type(ret, paths)}' if ret else '')
            ext = []
            for _n, ty in sg['inputs']:
                external_paths(ty, ext, paths)
            external_paths(ret, ext, paths)
            foreign.extend((full, e) for e in ext)
        elif kind in ('constant', 'static'):
            ty = inner[kind].get('type')
            sig = f': {render_type(ty, paths)}'
        elif kind in ('struct', 'enum', 'union', 'trait', 'type_alias'):
            if kind == 'enum':
                vs = [index[str(v)]['name'] for v in inner['enum'].get('variants', []) if str(v) in index]
                sig = 'variants: ' + ', '.join(vs)
            elif kind == 'struct':
                s = inner['struct']['kind']
                if isinstance(s, dict) and 'plain' in s:
                    fs = [index[str(f)] for f in s['plain'].get('fields', []) if str(f) in index]
                    pub = [f['name'] for f in fs if f.get('visibility') == 'public']
                    sig = f'pub fields: {", ".join(pub) or "none"}' + \
                          (f' ({len(fs) - len(pub)} private)' if len(fs) != len(pub) else '')
                    for f in fs:
                        if f.get('visibility') == 'public':
                            ext = []
                            external_paths(f['inner'].get('struct_field'), ext, paths)
                            foreign.extend((f'{full}.{f["name"]}', e) for e in ext)
                elif isinstance(s, dict) and 'tuple' in s:
                    sig = 'tuple struct'
                else:
                    sig = 'unit struct'
            for imp in inner[kind].get('impls', []):
                it = index.get(str(imp))
                if not it:
                    continue
                impl = it['inner'].get('impl', {})
                if impl.get('is_synthetic') or impl.get('blanket_impl'):
                    continue
                tr = impl.get('trait')
                if tr:
                    api.append(('impl', f'{full}', f'{tr.get("path") or tr.get("name")} for {full}'))
                else:
                    for m in impl.get('items', []):
                        visit(m, full + '::')
        api.append((kind, full, sig))

    root_item = index[str(doc['root'])]
    for c in root_item['inner']['module']['items']:
        visit(c, '')
    return api, foreign, None


GREP_ITEM = re.compile(
    r'^\s*pub\s+(?:unsafe\s+)?(?:const\s+)?(?:unsafe\s+)?(fn|struct|enum|trait|const|static|type|mod|union|use)\s+([A-Za-z0-9_:{}, *]+)')


def grep_api(root, files):
    out = []
    for f in sorted(files):
        if f.endswith('.rs') and '/src/' in f:
            for n, line in enumerate(read(root, f).split('\n'), 1):
                m = GREP_ITEM.match(line)
                if m:
                    out.append((m.group(1), m.group(2).strip(), f'{f}:{n}'))
    return out


# ---- the packet ---------------------------------------------------------------------------------

def table(header, rows):
    if not rows:
        return ['_none_', '']
    out = ['| ' + ' | '.join(header) + ' |', '|' + '---|' * len(header)]
    out += ['| ' + ' | '.join(str(c).replace('|', '\\|') for c in r) + ' |' for r in rows]
    return out + ['']


def build(root, crate, outdir, use_rustdoc=True):
    files = tracked(root, crate)
    if not files:
        raise SystemExit(f'crate-review-packet: no tracked files under crates/{crate}')
    ws = tomllib.load(open(os.path.join(root, 'Cargo.toml'), 'rb')).get('workspace', {})
    manifest_text = read(root, f'crates/{crate}/Cargo.toml')
    manifest = tomllib.loads(manifest_text)
    pkg = manifest.get('package', {})
    version = pkg.get('version')
    if isinstance(version, dict):
        version = ws.get('package', {}).get('version', '(workspace)')
    commit = git(root, 'rev-parse', '--short=9', 'HEAD').strip()
    L = [f'# Review packet: `{crate}`', '',
         f'Generated by `script/crate-review-packet {crate}` at nife `{commit}`. Not committed: it lives '
         'under `target/`. The tool gathers facts; every judgment is the reviewer\'s.', '']

    L += ['## The crate', '', f'- Path: `crates/{crate}`', f'- Version: `{version}`',
          f'- Edition: `{pkg.get("edition")}`, rust-version: `{pkg.get("rust-version", "none")}`, '
          f'publish: `{pkg.get("publish", "default")}`',
          f'- Tracked files: {len(files)}']
    tag = git(root, 'tag', '--list', f'v{version}').strip()
    L += [f'- Tag `v{version}` in this clone: {"present" if tag else "none (the review needs a tagged baseline)"}', '']

    counts = line_counts(root, files)
    L += ['## Source size', '', f'`src` totals {sum(n for _f, n in counts)} lines in {len(counts)} file(s).', '']
    L += table(['file', 'lines'], [(f, n) for f, n in counts])

    # Public API
    api, foreign, why = (None, None, 'skipped')
    if use_rustdoc:
        api, foreign, why = rustdoc_api(root, crate, outdir)
    L += ['## Public API', '']
    if api is not None:
        L += ['Source: rustdoc JSON from the pinned toolchain (`cargo rustdoc --output-format json`). '
              '`cargo public-api` is not installed in this tree and was not added.', '']
        L += table(['kind', 'item', 'signature'], [(k, f'`{p}`', f'`{s}`' if s else '') for k, p, s in api])
        L += ['### Types from other crates in public signatures', '',
              'Types defined in other crates, other than `core`, `alloc` and `std`, that a public '
              'signature or public field names. Empty means the API names only its own types and the '
              'standard library. A nife capability type or nife error enum would appear here.', '']
        L += table(['public item', 'type named'], sorted(set(foreign)))
    else:
        L += [f'Source: grep-based fallback (rustdoc JSON was not available: {why}). It lists lines that '
              'begin `pub`; it misses items made public by macros and re-exports through globs, and it '
              'shows no signatures. Read the source for those.', '']
        L += table(['kind', 'item', 'where'], [(k, f'`{p}`', w) for k, p, w in grep_api(root, files)])

    # Unsafe
    sites = unsafe_sites(root, files)
    policy = unsafe_policy(root, files)
    L += ['## `unsafe`', '']
    L += [f'- Lint policy lines: {"; ".join(policy) if policy else "none in the crate (it may come from the workspace lints)"}',
          f'- Sites: {len(sites)}', '']
    L += table(['file:line', 'kind', 'text'], [(f'{f}:{n}', k, f'`{t}`') for f, n, k, t in sites])

    # Dependencies
    L += ['## Dependencies, and where each points', '']
    L += table(['dependency', 'table', 'points to'], dependency_rows(root, crate, manifest, ws))
    inherits = [k for k, v in pkg.items() if isinstance(v, dict) and v.get('workspace')]
    if manifest.get('lints', {}).get('workspace'):
        inherits.append('lints')
    L += [f'Keys inherited from the nife workspace: {", ".join(f"`{k}`" for k in inherits) or "none"}.', '']

    # References back
    L += ['## References back into nife', '',
          'What `script/crate-standalone` flags without building: the manifest, the files a stranger '
          'needs, and citations of this tree in shipped text (`Name:` blocks stripped, as the move '
          'will leave them). Build, test and Kani are not run here; run `script/crate-standalone '
          f'{crate}` for those.', '']
    texts = {}
    for f in files:
        rel = os.path.relpath(f, f'crates/{crate}')
        if rel.endswith(crate_release.TEXT):
            texts[rel] = crate_release.strip_name_blocks(read(root, f))
    root_names = {r.split('/')[0] for r in (os.path.relpath(f, f'crates/{crate}') for f in files)}
    findings = crate_release.static_findings(texts, root_names)
    L += table(['step', 'finding'], findings)
    L += ['Dependencies that point into nife:', '']
    back = [(n, t, w) for n, t, w in dependency_rows(root, crate, manifest, ws) if 'path ' in w]
    L += table(['dependency', 'table', 'points to'], back)
    outside = []
    for f in sorted(files):
        if f.endswith('.rs'):
            for n, line in enumerate(read(root, f).split('\n'), 1):
                if line.strip().startswith('//'):
                    continue
                if OUTSIDE_STRING.search(line):
                    outside.append((f'{f}:{n}', f'`{line.strip()[:100]}`'))
    L += ['String literals naming a parent path (a test reading a file outside the crate shows here):', '']
    L += table(['file:line', 'text'], outside)
    fuzz = [f for f in git(root, 'grep', '-l', '-e', crate, '--', 'fuzz/*Cargo.toml').split()]
    named = git(root, 'grep', '-l', '-F', f'crates/{crate}', '--', ':!design', ':!notes', ':!briefs').split()
    L += [f'- Fuzz manifests in nife naming the crate (these point from nife to the crate): '
          f'{", ".join(f"`{f}`" for f in fuzz) or "none"}',
          f'- Files in nife (outside `design/`, `notes/`, `briefs/`) that name `crates/{crate}`: {len(named)}', '']

    # Harnesses
    hs = harnesses(root, files)
    fs = falsifications(root, crate, files)
    L += ['## Kani harnesses and falsification patches', '']
    claimed = set()
    rows = []
    for name, f, n in hs:
        match = [p for stem, p in fs if stem.endswith('.' + name) or stem == name]
        claimed.update(match)
        rows.append((f'`{name}`', f'{f}:{n}', f'`{match[0]}`' if match else '**none**'))
    L += [f'- Harnesses: {len(hs)}; falsification patches: {len(fs)}', '']
    L += table(['harness', 'where', 'falsification patch'], rows)
    orphans = [p for _s, p in fs if p not in claimed]
    if orphans:
        L += ['Patches matching no harness here (they may claim a test; `script/falsifications` decides):', '']
        L += [f'- `{p}`' for p in orphans] + ['']
    if not hs:
        L += ['**No Kani harness in this crate.** The hostile-input checklist item below has nothing '
              'to rest on except tests.', '']

    # Dependents
    deps = dependents(root, crate, ws)
    L += ['## Crates in nife that depend on it', '']
    L += table(['package', 'manifest', 'table'], [(p, f'`{m}`', t) for m, p, t in deps])

    # Constants
    cs = constants(root, files)
    L += ['## Identity values', '', 'Constants named `INTERFACE_ID`, `VERSION` or `MAGIC` (or containing one).', '']
    L += table(['name', 'value', 'where'], [(f'`{n}`', f'`{v}`', f'{f}:{ln}') for n, v, f, ln in cs])

    readme = [f for f in files if os.path.basename(f).upper().startswith('README')]
    L += ['## README', '']
    if readme:
        L += [f'- `{readme[0]}`, {read(root, readme[0]).count(chr(10))} lines. `script/stranger-test` is the tree\'s stranger test.', '']
    else:
        L += ['- **None.** The crate has no README; the stranger test cannot pass.', '']

    L += ['## Reviewer checklist', '',
          'Fill in each line with a verdict and a note. Nothing here is decided by the tool.', '',
          '- [ ] The API speaks the device\'s or the contract\'s vocabulary, not nife\'s. Notes:',
          '- [ ] No nife capability type, spawn handoff or nife error enum appears in a public signature '
          '(see the foreign-types table above). Notes:',
          '- [ ] The hostile-input surface is covered by harnesses, each with a falsification patch. Notes:',
          '- [ ] The README passes the stranger test. Notes:',
          '- [ ] No reference back into nife remains, or each has a named resolution. Notes:',
          '- [ ] Every `unsafe` site has a stated invariant a reader can check. Notes:',
          '- [ ] The identity values (`INTERFACE_ID`, `VERSION`, `MAGIC`) are what the contract means to '
          'publish. Notes:', '',
          'Verdict: [ ] clears   [ ] clears after changes   [ ] does not clear', '']
    return '\n'.join(L)


def main(argv):
    args = argv[1:]
    root = args[0]
    args = args[1:]
    out = os.path.join(root, 'target', 'crate-review-packets')
    use_rustdoc = True
    crates = []
    i = 0
    while i < len(args):
        a = args[i]
        if a == '--out' and i + 1 < len(args):
            out = os.path.abspath(args[i + 1])
            i += 2
            continue
        if a == '--no-rustdoc':
            use_rustdoc = False
        elif a.startswith('--'):
            sys.exit(USAGE)
        else:
            crates.append(a)
        i += 1
    if not crates:
        sys.exit(USAGE)
    os.makedirs(out, exist_ok=True)
    for crate in crates:
        text = build(root, crate, out, use_rustdoc)
        path = os.path.join(out, f'{crate}.md')
        open(path, 'w').write(text)
        print(path)


USAGE = 'usage: script/crate-review-packet [--out DIR] [--no-rustdoc] <crate>...'

if __name__ == '__main__':
    main(sys.argv)
