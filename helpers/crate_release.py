"""What can be read from git about the crates milestone 857 (crates released on their own) releases.

Two readers share it, so they cannot disagree about what "clean" means:

- `script/crate-standalone`, which adds what git cannot answer: a build, the name's ratification
  through `script/names`, and stock Kani with each falsification patch.
- `script/metrics`, which may read only git, so its `crate-release` and `crate-proofs` measures are
  the static half alone. Its CSV says so in its column names: `crates_standalone_static`.

Name: provisional, minted 2026-10-11 (UTC) by lane/crate-release-gates. A python module under
`helpers/`, outside `script/names`' scope, so this paragraph is its provenance. calef names things;
expect this to change.

# What is read

- A crate's own files, by path relative to `crates/<crate>/`, as a {path: text} dictionary.
- Its milestone block's `## What is proven` table: `| property | harness |`, one row per property
  the crate must prove, the harness cell a backticked `module::path::harness` or `none yet`.
- `Cargo.lock`, for a crate that has left: a `[[package]]` of that name from crates.io.
- `ALLOWLIST`, the publish allowlist milestone 887 (crate repositories share one CI template)
  builds. Until that file exists, nothing is allowlisted.

# BUGS

- "Proven" here is static: every row names a harness that exists in the crate's source and has a
  falsification patch on disk. That the patch turns its harness red at its own assertion is
  `script/crate-standalone`'s check and the falsification sweep's, which git cannot replay.
- After a crate leaves the tree, its harnesses and patches live in its own repository, which this
  module cannot read. Its proof columns go empty, not zero, and its stage reads from `Cargo.lock`.
"""

import re
import tomllib

# The eight per-crate milestones of 857, by number. A crate milestone minted later is added here; a
# block that is not listed is not counted, which is a foot gun, and this comment is where it is
# written down. Derived from the blocks' titles rather than a second list of crate names. 821
# (`portable_executable`) is refused (784's item (g)), so its crate is not counted.
MILESTONES = (813, 814, 815, 816, 817, 818, 819, 820)

# Provisional path, read by milestone 887's allowlist lint once it is built.
ALLOWLIST = 'crates/publish-allowlist.txt'

STAGES = ('in_tree', 'standalone_static', 'proven', 'allowlisted', 'published')

# nife is MIT OR Apache-2.0, and calef ruled on 2026-10-11 (UTC) that its crates are dual licensed
# the same way: "nife is MIT OR Apache-2.0, so we should dual license our crates as well." Both
# files, because GitHub's license detection reads nife itself as Apache-2.0 alone.
LICENSE = 'MIT OR Apache-2.0'
LICENSE_FILES = ('LICENSE-MIT', 'LICENSE-APACHE')

CITATIONS = [
    ('section citation', re.compile(r'§\s?\d+')),
    ('milestone citation', re.compile(r'\bmilestones? \d+', re.I)),
    ('path into the nife tree', re.compile(r'\b(?:design|notes|script|helpers|briefs)/[\w.-]')),
    ('provenance block', re.compile(r'\bName: *(?:provisional|ratified|recorded|unrecorded)', re.I)),
    ("architect's name", re.compile(r'\bcalef\b')),
    ('nife target_os', re.compile(r'target_os\s*=\s*"nife"')),
]
INCLUDE = re.compile(r'include_(?:bytes|str)!\(\s*"([^"]+)"')
TEXT = ('.rs', '.toml', '.md', '.patch', '.txt')

_TITLE = re.compile(r'^# (\d+)\. `([a-z0-9_]+)`', re.M)
_ROW = re.compile(r'^\|(.*)\|(.*)\|\s*$')
_HARNESS = re.compile(r'`([A-Za-z0-9_:]+)`')


def _normpath(parts):
    out = []
    for p in parts:
        if p in ('', '.'):
            continue
        if p == '..':
            if not out:
                return None
            out.pop()
        else:
            out.append(p)
    return out


def manifest_findings(manifest):
    """[(step, why)] from a crate's own `Cargo.toml` text, and whether a dependency leaves it."""
    found, leaves = [], False
    m = tomllib.loads(manifest)
    pkg = m.get('package', {})
    for table in ('dependencies', 'dev-dependencies', 'build-dependencies'):
        for name, spec in m.get(table, {}).items():
            if isinstance(spec, dict) and 'path' in spec and _normpath(spec['path'].split('/')) is None:
                found.append(('manifest', f"path dependency leaves the crate: {name} ({spec['path']})"))
                leaves = True
    inherited = [k for k, v in pkg.items() if isinstance(v, dict) and v.get('workspace')]
    if m.get('lints', {}).get('workspace'):
        inherited.append('lints')
    found += [('manifest', f'inherits `{key}` from the nife workspace') for key in inherited]
    found += [('manifest', f'feature named for nife: {f}') for f in m.get('features', {})
              if 'nife' in f.lower()]
    if not pkg.get('rust-version'):
        found.append(('manifest', 'no rust-version'))
    return found, leaves


def file_findings(root_names, license_field):
    """[(step, why)] for what a stranger needs at the crate's root."""
    found = []
    if not any(n.upper().startswith('README') for n in root_names):
        found.append(('files', 'no README'))
    found += [('files', f'no {f}') for f in LICENSE_FILES if f not in root_names]
    if license_field != LICENSE:
        found.append(('files', f'license field is {license_field!r}, not {LICENSE!r}'))
    return found


def citation_findings(files):
    """[(step, why)], one per kind of citation, over {relative path: text}."""
    sites = {}
    for path, text in sorted(files.items()):
        if not path.endswith(TEXT):
            continue
        for n, line in enumerate(text.split('\n'), 1):
            for label, rx in CITATIONS:
                if rx.search(line):
                    sites.setdefault(label, []).append(f'{path}:{n}')
        # Over the whole file, since rustfmt wraps a long include onto its own line.
        here = path.split('/')[:-1]
        for inc in INCLUDE.finditer(text):
            if _normpath(here + inc.group(1).split('/')) is None:
                line = text.count('\n', 0, inc.start()) + 1
                sites.setdefault('file read from outside the crate', []).append(f'{path}:{line}')
    return [('citations', f'{len(s)} {label} line(s), first {s[0]}') for label, s in sites.items()]


def static_findings(files, root_names):
    """Everything git can answer about whether a crate stands alone, as [(step, why)]."""
    manifest = files.get('Cargo.toml', '')
    found, _leaves = manifest_findings(manifest)
    pkg = tomllib.loads(manifest).get('package', {})
    field = pkg.get('license')
    if isinstance(field, dict) and field.get('workspace'):
        field = LICENSE   # inherited, so nife's own; the inheritance is reported above
    found += file_findings(root_names, field)
    found += citation_findings(files)
    return found


def crate_of(block_text):
    """The crate a per-crate block releases, from its title, or None."""
    m = _TITLE.search(block_text)
    return m.group(2) if m else None


def required_properties(block_text):
    """[(property, harness or None)] from the block's `## What is proven` table."""
    if '## What is proven' not in block_text:
        return []
    section = block_text.split('## What is proven', 1)[1].split('\n## ', 1)[0]
    rows = []
    for line in section.split('\n'):
        m = _ROW.match(line)
        if not m or set(m.group(1).strip()) <= set('-') or m.group(1).strip() == 'property':
            continue
        cell = m.group(2).strip()
        h = _HARNESS.fullmatch(cell)
        rows.append((m.group(1).strip(), h.group(1) if h else None))
    return rows


def harness_count(files):
    return sum(len(re.findall(r'#\[kani::proof', t)) for p, t in files.items() if p.endswith('.rs'))


def is_proven(harness, files, paths):
    """A named harness that exists in the crate's source and carries a falsification patch."""
    name = harness.split('::')[-1]
    patch = 'falsifications/' + harness.replace('::', '.') + '.patch'
    return patch in paths and any(re.search(r'\bfn ' + re.escape(name) + r'\b', t)
                                  for p, t in files.items() if p.endswith('.rs'))


def published(crate, lock_text):
    """True when `Cargo.lock` takes this crate from crates.io."""
    return bool(re.search(r'name = "' + re.escape(crate) + r'"\nversion = "[^"]+"\nsource = '
                          r'"registry\+https://github.com/rust-lang/crates.io-index"', lock_text))


def allowlisted(crate, allowlist_text):
    return crate in {line.split('#')[0].strip() for line in (allowlist_text or '').split('\n')}
