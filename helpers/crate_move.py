"""Stage a crate's move out of this tree into a repository of its own, as a dry run.

Read by script/crate-move, whose header says what it does and what it never does. The steps it
stands in for, and who takes each, are notes/crate-move.md.

Name: provisional, minted 2026-11-05 (UTC) for milestone 1034 (a runbook for moving a crate out of
nife), a maintainer decision. It reuses `crate_release.py`'s citation rules and its `Name:` block
reader, so the move, `script/crate-standalone` and the review packet agree on what a block is.
"""
import argparse
import difflib
import filecmp
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import crate_release  # noqa: E402

TEMPLATE_URL = 'https://github.com/nifeos/crate-workflows'
REPOSITORY_URL = 'https://github.com/nifeos/{crate}'
BARE_METAL = ('aarch64-unknown-none-softfloat', 'riscv64imac-unknown-none-elf', 'x86_64-unknown-none')
DEP_TABLES = ('dependencies', 'dev-dependencies', 'build-dependencies')
# The word, not a longer one: `nifeos` in a repository URL is allowed.
NIFE_WORD = re.compile(r'(?<![A-Za-z0-9_])nife(?![A-Za-z0-9_])', re.I)
HUNK = re.compile(r'^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)$')
# The scaffold's own description of itself, and the crate README's starting shape, which is used
# only when the crate brings no README.
SCAFFOLD_SKIP = ('README.md', 'README-crate.md')

# The template's cheap checks, as `check / standalone` and `proofs / falsify` run them. Each is
# (label, helper, argument builder over the staged repository). A helper absent at the pinned
# template revision is reported as absent, never as passing.
TEMPLATE_CHECKS = (
    ('standalone', 'standalone.py', lambda s: ['--manifest-dir', '.', '--allow-unpublished']),
    ('prose caps', 'prose_caps.py', lambda s: s.readmes + s.changelog),
    ('passive voice', 'passive_voice.py', lambda s: s.readmes + s.changelog + s.docs),
    ('history phrases', 'history_phrases.py', lambda s: s.readmes + s.docs),
    ('file length', 'file_length.py', lambda s: s.rust),
    ('comment blocks', 'comment_blocks.py', lambda s: s.rust),
    ('function length', 'function_length.py', lambda s: s.rust),
    ('spelled-out words', 'spelled_out.py', lambda s: ['--manifest', 'Cargo.toml'] + s.rust),
    ('unit symbols', 'unit_symbols.py', lambda s: ['--manifest', 'Cargo.toml'] + s.rust),
    ('orphan modules', 'orphan_modules.py', lambda s: ['.']),
    ('markdown links', 'check_links.py', lambda s: s.markdown),
    ('falsification records', 'falsify.py', lambda s: ['--manifest-path', 'Cargo.toml', '--check']),
)


def moved_source(text, crate):
    """A source file as the crate repository carries it.

    The `Name:` blocks are deleted, each `Falsification:` record moves to the head of its comment
    run, and a path into the crate's own directory becomes a path from the repository's root.
    Returns (text, blocks removed, records moved, paths made relative).
    """
    stripped = crate_release.strip_name_blocks(text, remove=True)
    moved = falsification_first(stripped)
    new, paths = re.subn(r'(?<![\w/.])crates/' + re.escape(crate) + '/', '', moved)
    return new, int(stripped != text), int(moved != stripped), paths


def diff_files(text):
    """The crate-relative paths a patch changes, in order, read from its `diff --git` lines."""
    return [m.group(1) for m in re.finditer(r'(?m)^diff --git a/crates/[^/]+/(\S+) b/', text)]


def regenerate_patch(text, crate, source):
    """A falsification patch as the crate repository applies it: `-p1` from the crate's root.

    `source` is the crate's directory in this tree. The patch is applied there, as the lane that
    wrote it did (`patch -p3`); the files before and after are then put through `moved_source`, the
    change the move makes to every source file, and the patch is written again as the difference
    between those two. Every hunk header therefore matches the stripped source, whatever the
    move deleted before it. Returns (text, problems); a problem is an edit that cannot be carried:
    a patch that does not apply here, an edit that lands inside a `Name:` block (the move deletes
    the block, so nothing is left to change) or an edit the move leaves empty.
    """
    files = diff_files(text)
    if not files:
        return text, ['the patch names no file under crates/' + crate + '/']
    preamble = text[:text.index('diff --git ')]
    with tempfile.TemporaryDirectory(prefix='crate-move-patch-') as tmp:
        before = {}
        for rel in files:
            to = os.path.join(tmp, rel)
            os.makedirs(os.path.dirname(to), exist_ok=True)
            shutil.copy2(os.path.join(source, rel), to)
            before[rel] = open(to, errors='replace').read()
        code, out = run(['patch', '-s', '-p3'], tmp, stdin=text)
        if code:
            return text, [f'the patch does not apply to the source in this tree: {tail(out)}']
        pieces, problems = [], []
        for rel in files:
            after = open(os.path.join(tmp, rel), errors='replace').read()
            old_lines, new_lines = before[rel].split('\n'), after.split('\n')
            blanked = {n for n, (o, b) in enumerate(zip(old_lines, crate_release.strip_name_blocks(
                before[rel]).split('\n'))) if o and not b}
            for tag, i1, i2, _j1, _j2 in difflib.SequenceMatcher(None, old_lines, new_lines,
                                                                 autojunk=False).get_opcodes():
                if tag != 'equal' and blanked & set(range(i1, max(i2, i1 + 1))):
                    problems.append(f'{rel}: an edit lands inside a Name: block, which the move deletes')
            old, new = moved_source(before[rel], crate)[0], moved_source(after, crate)[0]
            if old == new:
                problems.append(f'{rel}: nothing is left of the edit once the move has run')
                continue
            for name, body in (('old', old), ('new', new)):
                open(os.path.join(tmp, name), 'w').write(body)
            _code, delta = run(['diff', '-u', '--label', 'a/' + rel, '--label', 'b/' + rel,
                                os.path.join(tmp, 'old'), os.path.join(tmp, 'new')], tmp)
            pieces.append(f'diff --git a/{rel} b/{rel}\n' + delta + '\n')
    return preamble + ''.join(pieces).rstrip('\n') + '\n', problems


def patch_application(into, patch):
    """What `patch -p1 --dry-run -F0` says of one patch against the staged tree, or '' when clean.

    A patch that applies at an offset or with fuzz still exits 0, which is how a stale hunk header
    passes: the move accepts only a patch that applies exactly where it says.
    """
    code, out = run(['patch', '-p1', '--dry-run', '-F0', '-i', patch], into)
    if code:
        return 'does not apply: ' + tail(out)
    notes = [line for line in out.splitlines() if re.search(r'offset|fuzz', line)]
    return '; '.join(notes[:2])


def falsification_first(text):
    """Each harness's `/// Falsification:` record moved to the head of its comment run.

    The template's `falsify.py` reads the record only when the doc comment above `#[kani::proof]`
    opens with it; this tree writes it last. The record's continuation lines move with it.
    """
    lines = text.split('\n')
    for n, line in enumerate(lines):
        if line.strip() != '#[kani::proof]':
            continue
        start = n
        while start > 0 and lines[start - 1].lstrip().startswith('///'):
            start -= 1
        run_ = lines[start:n]
        at = next((k for k, r in enumerate(run_) if r.lstrip().startswith('/// Falsification:')), None)
        if at is None or at == 0:
            continue
        end = at + 1
        while end < len(run_) and run_[end].lstrip()[3:].strip():
            end += 1
        lines[start:n] = run_[at:end] + run_[:at] + run_[end:]
    return '\n'.join(lines)


def add_repository(manifest, crate):
    """The manifest with `repository` set to the crate's own repository, after `license`."""
    if re.search(r'(?m)^repository\s*=', manifest):
        return manifest
    line = f'repository = "{REPOSITORY_URL.format(crate=crate)}"'
    new, n = re.subn(r'(?m)^(license\s*=.*)$', lambda m: m.group(1) + '\n' + line, manifest, count=1)
    return new if n else manifest


def caller_workflow(text, sha, targets):
    """A scaffold caller with its pin filled in; `ci.yml` also gets its targets and the soak flag."""
    text = text.replace('@PINS@', '@' + sha)
    if "targets: ''" in text:
        text = text.replace("targets: ''", 'targets: ' + ' '.join(targets) + '\n'
                            '      # The soak publishes nothing, so the manifest carries publish = false on purpose.\n'
                            '      allow-unpublished: true')
    return text


def consumers(root, crate):
    """[(manifest, table, spec)] for every manifest in the tree that links the crate by path."""
    found = []
    files = subprocess.run(['git', '-C', root, 'ls-files', '*Cargo.toml'], capture_output=True,
                           text=True, check=True).stdout.split()
    for f in files:
        if f.startswith(f'crates/{crate}/'):
            continue
        try:
            m = tomllib.load(open(os.path.join(root, f), 'rb'))
        except (OSError, tomllib.TOMLDecodeError):
            continue
        tables = [(t, m.get(t, {})) for t in DEP_TABLES]
        tables += [(f'target.{k}.{t}', v.get(t, {})) for k, v in m.get('target', {}).items()
                   for t in DEP_TABLES]
        tables.append(('workspace.dependencies', m.get('workspace', {}).get('dependencies', {})))
        for table, deps in tables:
            spec = deps.get(crate)
            if isinstance(spec, dict) and 'path' in spec:
                found.append((f, table, spec))
        if f'crates/{crate}' in m.get('workspace', {}).get('members', []):
            found.append((f, 'workspace.members', {'path': f'crates/{crate}'}))
    return found


def mentions(root, crate):
    """Tracked files outside the crate and outside the records that name it: the follow-up list."""
    out = subprocess.run(['git', '-C', root, 'grep', '-l', '-w', '-e', crate, '--', '.',
                          f':!crates/{crate}', ':!design', ':!notes'],
                         capture_output=True, text=True).stdout.split()
    return out


class Staged:
    """The file lists the template's checks read, from the staged repository's index."""

    def __init__(self, repo):
        files = subprocess.run(['git', '-C', repo, 'ls-files'], capture_output=True, text=True,
                               check=True).stdout.split()
        self.files = files
        self.rust = [f for f in files if f.endswith('.rs')]
        self.markdown = [f for f in files if f.endswith('.md')]
        self.readmes = [f for f in files if '/' not in f and f.upper().startswith('README')]
        self.changelog = [f for f in files if f == 'CHANGELOG.md']
        self.docs = [f for f in files if f.startswith('docs/') and f.endswith('.md')]


BASELINE = '.cargo/mutants-baseline.txt'


def baseline_row(root, crate):
    """The crate's census row as (caught, missed, timeout, unviable), or None when it has none."""
    try:
        text = open(os.path.join(root, BASELINE)).read()
    except OSError:
        return None
    for line in text.splitlines():
        fields = line.split()
        if len(fields) == 5 and fields[0] == crate and all(f.isdigit() for f in fields[1:]):
            return tuple(int(f) for f in fields[1:])
    return None


def exempts_note(row):
    """Comment lines for the crate repository's `.github/mutants-exempts`.

    The source tree's baseline holds four counts per crate. The template's gate reads one line per
    surviving mutant, `<file>:<line> # <reason>`, and the counts do not say which mutants survive,
    so the row cannot become exempt lines mechanically. It travels as a comment, and the first
    weekly run names each survivor to list.
    """
    caught, missed, timeout, unviable = row
    todo = (f'The {missed} missed mutant(s) are not listed here because the census did not record '
            'which; the first weekly mutation run names each, and each goes below as '
            '`<file>:<line> # <reason>`.' if missed else
            'No mutant was missed, so no line is expected below.')
    return ('# Mutation census row carried from the source tree, as counts: '
            f'caught {caught}, missed {missed}, timeout {timeout}, unviable {unviable}.\n'
            f'# {todo}\n')


FUZZ_MANIFEST = 'fuzz/Cargo.toml'
# Files in a fixture directory that are tools or prose, not inputs.
NOT_AN_INPUT = ('.sh', '.md', '.py', '.toml')
# Output lines of a fuzz run that are safe to show. A crashing run prints its input, and the
# move lane never reads or quotes one, so everything else of a failed run is withheld.
FUZZ_SAFE_LINE = re.compile(r'^(SUMMARY: |ERROR: libFuzzer: |error: |warning: unused)')


def code_text(src):
    """Rust source with comments, string literals and character literals blanked out."""
    out, i, n = [], 0, len(src)
    while i < n:
        two = src[i:i + 2]
        if two == '//':
            j = src.find('\n', i)
            i = n if j < 0 else j
        elif two == '/*':
            depth, i = 1, i + 2
            while i < n and depth:
                if src[i:i + 2] == '/*':
                    depth, i = depth + 1, i + 2
                elif src[i:i + 2] == '*/':
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
            out.append(' ')
        elif (m := re.match(r'b?r(#*)"', src[i:])):
            end = src.find('"' + m.group(1), i + m.end())
            i = n if end < 0 else end + 1 + len(m.group(1))
            out.append(' ')
        elif src[i] == '"':
            i += 1
            while i < n and src[i] != '"':
                i += 2 if src[i] == '\\' else 1
            i += 1
            out.append(' ')
        elif (m := re.match(r"'(?:\\.[^']*|[^\\'])'", src[i:])):
            i += m.end()
            out.append(' ')
        else:
            out.append(src[i])
            i += 1
    return ''.join(out)


def unsafe_files(root, crate):
    """The crate's tracked Rust files that hold the `unsafe` keyword in code."""
    files = subprocess.run(['git', '-C', root, 'ls-files', f'crates/{crate}'], capture_output=True,
                           text=True, check=True).stdout.split()
    return [f for f in files if f.endswith('.rs') and
            re.search(r'\bunsafe\b', code_text(open(os.path.join(root, f), errors='replace').read()))]


def crate_targets(root, crate):
    """[(name, path)] of the fuzz targets that exercise this crate and no other crate of the tree.

    A target belongs to the crate when its source names the crate and none of the other path
    dependencies of `fuzz/Cargo.toml`: one that links several crates tests their combination, and
    stays where the combination is. The path is relative to `fuzz/`.
    """
    try:
        manifest = tomllib.load(open(os.path.join(root, FUZZ_MANIFEST), 'rb'))
    except OSError:
        return [], []
    others = [d for d, spec in manifest.get('dependencies', {}).items()
              if isinstance(spec, dict) and 'path' in spec and d != crate]
    mine, mixed = [], []
    for b in manifest.get('bin', []):
        text = code_text(open(os.path.join(root, 'fuzz', b['path']), errors='replace').read())
        if not re.search(r'\b' + re.escape(crate) + r'\b', text):
            continue
        shared = [d for d in others if re.search(r'\b' + re.escape(d) + r'::', text)]
        if shared:
            mixed.append(b['name'])
        else:
            mine.append((b['name'], b['path']))
    return mine, mixed


def fixture_dirs(root, crate, target):
    """The crate's fixture directories `script/fuzz` passes to libFuzzer as read-only corpora."""
    text = open(os.path.join(root, 'script', 'fuzz')).read()
    found = []
    for m in re.finditer(r'(?m)^\s*([\w|]+)\)\s*(?:\n\s*)?set -- "\$@"([^;]*?)\s*;;', text):
        if target in m.group(1).split('|'):
            found += [t for t in m.group(2).split() if t.startswith(f'crates/{crate}/')]
    return found


def corpus_files(root, crate, target):
    """[(source path in this tree, name in the corpus)] for one target.

    The seeds committed for the target, then the inputs in each fixture directory `script/fuzz`
    adds. Only the files directly inside a directory count, as libFuzzer reads them, and a tool or
    a note beside the inputs does not.
    """
    found = []
    for d in [f'fuzz/seeds/{target}'] + fixture_dirs(root, crate, target):
        full = os.path.join(root, d)
        for name in sorted(os.listdir(full)) if os.path.isdir(full) else []:
            if os.path.isfile(os.path.join(full, name)) and not name.endswith(NOT_AN_INPUT):
                found.append((os.path.join(full, name), name))
    return found


def seed_scripts(root, targets):
    """The scripts in fuzz/seeds that write a seed of one of these targets."""
    directory = os.path.join(root, 'fuzz', 'seeds')
    return sorted(n for n in os.listdir(directory) if n.endswith('.py') and
                  any(t in open(os.path.join(directory, n)).read() for t in targets))


def readme_sections(path, headings):
    """The sections of a markdown file whose `## ` heading names one of `headings`, verbatim."""
    text = open(path).read()
    parts = re.split(r'(?m)^(?=## )', text)
    return [p.rstrip('\n') + '\n' for p in parts
            if p.startswith('## ') and any(h in p.split('\n', 1)[0] for h in headings)]


def fuzz_readme(root, crate, names, weekly):
    """The staged fuzz/README.md: how to run, then the targets' sections from this tree's notes.

    The sections are text about this crate alone, so they move with it. A path into this tree's
    seed directory becomes the path in the crate's own layout.
    """
    head = (f'# Fuzz targets of `{crate}`\n\n'
            'The targets use cargo-fuzz and need a nightly toolchain. From the repository root:\n\n'
            '```sh\ncargo install --locked cargo-fuzz\ncargo +nightly fuzz list\n'
            'cargo +nightly fuzz build\n```\n\n'
            + ('The weekly workflow runs every target for five minutes. ' if weekly else
               'The weekly workflow does not run them. ') +
            'To run one by hand, give libFuzzer a scratch directory first, so that what it finds '
            'does not land among the committed inputs:\n\n'
            '```sh\nmkdir -p "$TMPDIR/work"\n'
            'cargo +nightly fuzz run <target> "$TMPDIR/work" corpus/<target>\n```\n\n'
            'Each `corpus/<target>` directory holds the inputs a run starts from. A crash input is '
            'not committed: it becomes a regression test in the crate, beside the code it broke.\n\n')
    for name in names:
        copied = [d.replace(f'crates/{crate}/', '') for d in fixture_dirs(root, crate, name)]
        if copied:
            head += (f'`corpus/{name}` also holds copies of the inputs in '
                     + ', '.join(f'`{d}`' for d in copied) + '.\n\n')
    sections = readme_sections(os.path.join(root, 'notes', 'fuzzing', 'targets.md'),
                               [f'`{n}`' for n in names])
    sections += readme_sections(os.path.join(root, 'fuzz', 'seeds', 'README.md'),
                                [f'`{n}/' for n in names])
    body = '\n'.join(sections)
    for name in names:
        body = body.replace(f'fuzz/seeds/{name}', f'corpus/{name}')
    body = re.sub(r'fuzz/seeds/(?=[\w-]+\.py)', '', body)
    body = body.replace('notes/fuzzing/targets.md', 'README.md')
    return head + moved_source(body, crate)[0]


def target_source(text, crate):
    """A target's source as the crate repository carries it."""
    text = moved_source(text, crate)[0].replace('notes/fuzzing/targets.md', 'fuzz/README.md')
    return text


def seed_script_source(text, names):
    """A seed script as it runs from fuzz/: its output goes under corpus/<target>."""
    text = re.sub(r'fuzz/seeds/(?=[\w-]+\.py)', 'fuzz/', text)
    for name in names:
        text = text.replace(f"fuzz/seeds/{name}", f'fuzz/corpus/{name}')
        text = text.replace(f"os.path.join(here, '{name}'", f"os.path.join(here, 'corpus', '{name}'")
    return text


def fuzz_manifest_text(crate, edition, libfuzzer, targets):
    bins = ''.join(f'\n[[bin]]\nname = "{n}"\npath = "{p}"\ntest = false\ndoc = false\nbench = false\n'
                   for n, p in targets)
    return ('[package]\nname = "fuzz"\nversion = "0.0.0"\npublish = false\n'
            f'edition = "{edition}"\n\n[package.metadata]\ncargo-fuzz = true\n\n'
            f'[dependencies]\nlibfuzzer-sys = "{libfuzzer}"\n'
            f'{crate} = {{ path = ".." }}\n\n[workspace]\nmembers = ["."]\n{bins}')


def stage_fuzz(root, crate, into, report):
    """Stage the crate's fuzz targets in the cargo-fuzz layout under `into`/fuzz.

    Returns (target names, whether the weekly caller may pass `fuzz: true`). The ruling that weekly
    fuzzing is opt-in applies: only a crate with no `unsafe` code is switched on.
    """
    targets, mixed = crate_targets(root, crate)
    for name in mixed:
        report('note', 'fuzz', f'{name} links other crates of this tree as well, so it stays')
    if not targets:
        report('note', 'fuzz', 'no fuzz target of this crate alone')
        return [], False
    names = [n for n, _ in targets]
    fuzz = os.path.join(into, 'fuzz')
    manifest = tomllib.load(open(os.path.join(root, FUZZ_MANIFEST), 'rb'))
    crate_edition = tomllib.load(open(os.path.join(into, 'Cargo.toml'), 'rb'))['package'].get('edition', '2021')
    libfuzzer = manifest['dependencies']['libfuzzer-sys']
    os.makedirs(os.path.join(fuzz, 'fuzz_targets'))
    open(os.path.join(fuzz, 'Cargo.toml'), 'w').write(
        fuzz_manifest_text(crate, crate_edition, libfuzzer, targets))
    open(os.path.join(fuzz, '.gitignore'), 'w').write('target/\nartifacts/\ncoverage/\n')
    counts = []
    for name, path in targets:
        text = open(os.path.join(root, 'fuzz', path), errors='replace').read()
        open(os.path.join(fuzz, path), 'w').write(target_source(text, crate))
        corpus = os.path.join(fuzz, 'corpus', name)
        os.makedirs(corpus)
        files = corpus_files(root, crate, name)
        for source, base in files:
            shutil.copy2(source, os.path.join(corpus, base))
        counts.append(f'{name} ({len(files)} inputs)')
    for script in seed_scripts(root, names):
        text = open(os.path.join(root, 'fuzz', 'seeds', script)).read()
        open(os.path.join(fuzz, script), 'w').write(seed_script_source(text, names))
    report('ok', 'fuzz', 'staged fuzz/ with ' + ', '.join(counts))
    for dictionary in (f'fuzz/dictionaries/{n}.dict' for n in names):
        if os.path.exists(os.path.join(root, dictionary)):
            report('note', 'fuzz', f'{dictionary} is not staged: the weekly job passes no dictionary')
    unsafe = unsafe_files(root, crate)
    open(os.path.join(fuzz, 'README.md'), 'w').write(fuzz_readme(root, crate, names, not unsafe))
    if unsafe:
        report('note', 'fuzz opt-in', f'weekly fuzz stays off: unsafe code in {", ".join(unsafe[:3])}'
                                      + (' and more' if len(unsafe) > 3 else ''))
        return names, False
    report('ok', 'fuzz opt-in', 'no unsafe code in the crate, so the weekly caller passes fuzz: true')
    return names, True


def turn_on_weekly_fuzz(into, report):
    path = os.path.join(into, '.github', 'workflows', 'weekly.yml')
    text = open(path).read() if os.path.exists(path) else ''
    if 'fuzz: false' not in text:
        report('FAIL', 'fuzz opt-in', 'the staged weekly caller has no `fuzz: false` line to turn on')
        return
    open(path, 'w').write(text.replace('fuzz: false', 'fuzz: true', 1))


def fuzz_dry_run(into, names, work, report):
    """Build every staged target, then run each once over its corpus (`-runs=0`).

    A failed run prints its input, so its output is filtered down to the lines that name no input.
    """
    fuzz = os.path.join(into, 'fuzz')
    env = dict(os.environ, CARGO_TARGET_DIR=os.path.join(work, 'fuzz-target'))
    env.pop('RUSTUP_TOOLCHAIN', None)
    code, out = run(['cargo', '+nightly', 'fuzz', 'build'], into, env)
    if code:
        report('FAIL', 'fuzz build', f'cargo +nightly fuzz build: {tail(out)}')
        return
    report('ok', 'fuzz build', f'{len(names)} target(s) built with cargo +nightly fuzz build')
    for name in names:
        corpus = os.path.join(fuzz, 'corpus', name)
        before = sorted(os.listdir(corpus))
        code, out = run(['cargo', '+nightly', 'fuzz', 'run', name, os.path.join('fuzz', 'corpus', name),
                         '--', '-runs=0'], into, env)
        shown = ' | '.join(line[:200] for line in out.splitlines() if FUZZ_SAFE_LINE.match(line))
        artifacts = os.path.join(fuzz, 'artifacts')
        if os.path.isdir(artifacts):
            shutil.rmtree(artifacts)
        if code:
            report('FAIL', 'fuzz run', f'{name} over its corpus, exit {code}; the output is withheld '
                                       'because it holds the input' + (f': {shown}' if shown else ''))
        elif sorted(os.listdir(corpus)) != before:
            report('FAIL', 'fuzz run', f'{name} changed its corpus directory during a -runs=0 pass')
        else:
            report('ok', 'fuzz run', f'{name} once over its {len(before)} corpus input(s), -runs=0')


def seed_script_reproduces(into, names, work, report):
    """Run each staged seed script in a scratch copy and compare its files with the staged corpus."""
    fuzz = os.path.join(into, 'fuzz')
    for script in sorted(n for n in os.listdir(fuzz) if n.endswith('.py')):
        scratch = os.path.join(work, 'seed-script')
        shutil.rmtree(scratch, ignore_errors=True)
        os.makedirs(scratch)
        shutil.copy(os.path.join(fuzz, script), os.path.join(scratch, script))
        code, out = run([sys.executable, '-I', os.path.join(scratch, script)], scratch)
        if code:
            report('FAIL', 'seed script', f'{script}: {tail(out)}')
            continue
        made = [os.path.join(d, f) for d, _s, fs in os.walk(os.path.join(scratch, 'corpus')) for f in fs]
        bad = [os.path.relpath(m, scratch) for m in made
               if not os.path.exists(os.path.join(fuzz, os.path.relpath(m, scratch)))
               or not filecmp.cmp(m, os.path.join(fuzz, os.path.relpath(m, scratch)), shallow=False)]
        if not made or bad:
            report('FAIL', 'seed script', f'{script} writes {len(made)} file(s) and {len(bad)} differ from the '
                                          'staged corpus' if made else f'{script} writes no file under corpus/')
        else:
            report('ok', 'seed script', f'{script} rebuilds {len(made)} staged input(s) byte for byte')


def stays_in_fuzz(root, crate):
    """Whether a target that does not move still links the crate."""
    manifest = tomllib.load(open(os.path.join(root, FUZZ_MANIFEST), 'rb'))
    leaving = {n for n, _ in crate_targets(root, crate)[0]}
    return any(b['name'] not in leaving and re.search(
        r'\b' + re.escape(crate) + r'\b',
        code_text(open(os.path.join(root, 'fuzz', b['path']), errors='replace').read()))
        for b in manifest.get('bin', []))


def fuzz_removals(root, crate, names):
    """Print the nife-side removals that follow from the targets moving."""
    print('crate-move: fuzz targets that moved; the same nife pull request removes them:')
    for n in names:
        print(f'  fuzz/Cargo.toml: delete the [[bin]] section of {n}')
    print('  fuzz/fuzz_targets/: git rm ' + ' '.join(f'{n}.rs' for n in names))
    print('  script/fuzz: drop ' + ', '.join(names) + ' from `targets=` and from the --list output'
          + (', and the fixtures case for ' + ', '.join(sorted({n for n in names if fixture_dirs(root, crate, n)}))
             if any(fixture_dirs(root, crate, n) for n in names) else ''))
    seeds = [n for n in names if os.path.isdir(os.path.join(root, 'fuzz', 'seeds', n))]
    if seeds:
        print('  fuzz/seeds/: git rm -r ' + ' '.join(seeds))
    for script in seed_scripts(root, names):
        print(f'  fuzz/seeds/{script}: git rm, with its paragraph and the rows of the moved targets in fuzz/seeds/README.md')
    print('  notes/fuzzing/targets.md: delete the section of ' + ', '.join(names))
    print('  fuzz/Cargo.lock: refresh with `cargo update --workspace` in fuzz/')


def run(argv, cwd, env=None, stdin=None):
    p = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True, input=stdin)
    return p.returncode, (p.stdout + p.stderr).strip()


def tail(text, n=3):
    lines = [line for line in text.splitlines() if line.strip()]
    return ' | '.join(lines[-n:])[:400]


def template_tree(args, work):
    """(directory holding the template's files at the pinned revision, its commit, its name)."""
    source = args.template
    if not source:
        source = os.path.join(work, 'template-clone')
        code, out = run(['git', 'clone', '--quiet', '--no-checkout', TEMPLATE_URL, source], work)
        if code:
            sys.exit(f'crate-move: cannot clone {TEMPLATE_URL}: {tail(out)}')
    rev = args.template_rev
    if not rev:
        code, out = run(['git', '-C', source, 'tag', '-l', 'v*', '--sort=-v:refname'], work)
        rev = out.split()[0] if out.split() else ''
        if not rev:
            sys.exit('crate-move: the template has no v* release to pin; name one with --template-rev')
    code, sha = run(['git', '-C', source, 'rev-parse', '--verify', rev + '^{commit}'], work)
    if code:
        sys.exit(f'crate-move: the template has no revision {rev}: {tail(sha)}')
    tree = os.path.join(work, 'template')
    os.makedirs(tree)
    archive = subprocess.run(['git', '-C', source, 'archive', sha], capture_output=True, check=True)
    subprocess.run(['tar', '-x', '-C', tree], input=archive.stdout, check=True)
    return tree, sha, rev


def stage(root, crate, into, tree, sha, report):
    """Build the crate repository's first tree in `into`. Returns the manifest as parsed."""
    files = subprocess.run(['git', '-C', root, 'ls-files', f'crates/{crate}'], capture_output=True,
                           text=True, check=True).stdout.split()
    if not files:
        sys.exit(f'crate-move: no tracked files under crates/{crate}')
    for f in files:
        to = os.path.join(into, os.path.relpath(f, f'crates/{crate}'))
        os.makedirs(os.path.dirname(to), exist_ok=True)
        shutil.copy2(os.path.join(root, f), to)
    report('ok', 'copy', f'{len(files)} tracked files of crates/{crate}')

    manifest = tomllib.load(open(os.path.join(into, 'Cargo.toml'), 'rb'))
    pkg = manifest.get('package', {})
    targets = pkg.get('metadata', {}).get('docs', {}).get('rs', {}).get('targets') or list(BARE_METAL)
    scaffold = os.path.join(tree, 'scaffold')
    kept, added = [], 0
    for dirpath, _dirs, names in os.walk(scaffold):
        for name in names:
            rel = os.path.relpath(os.path.join(dirpath, name), scaffold)
            if rel in SCAFFOLD_SKIP:
                continue
            dest = rel
            if rel.startswith('caller-workflows/'):
                dest = os.path.join('.github', 'workflows', os.path.basename(rel))
            to = os.path.join(into, dest)
            if os.path.exists(to):
                kept.append(dest)
                continue
            os.makedirs(os.path.dirname(to), exist_ok=True)
            text = open(os.path.join(dirpath, name)).read()
            if dest.startswith('.github/workflows/'):
                text = caller_workflow(text, sha, targets)
            open(to, 'w').write(text)
            shutil.copymode(os.path.join(dirpath, name), to)
            added += 1
    row = baseline_row(root, crate)
    exempts = os.path.join(into, '.github', 'mutants-exempts')
    if row is None:
        report('note', 'mutation baseline', f'no row for {crate} in {BASELINE}')
    elif not os.path.exists(exempts):
        report('FAIL', 'mutation baseline', 'the template staged no .github/mutants-exempts to carry the row')
    else:
        with open(exempts, 'a') as f:
            f.write('\n' + exempts_note(row))
        report('ok', 'mutation baseline', f'row carried into .github/mutants-exempts as a comment '
                                          f'(missed {row[1]}); survivors are listed after the first weekly run')
    readme = os.path.join(into, 'README.md')
    if not os.path.exists(readme):
        shutil.copy(os.path.join(scaffold, 'README-crate.md'), readme)
        report('FAIL', 'readme', 'the crate has no README; the scaffold shape is staged and needs writing')
    hooks = os.path.join(into, '.githooks')
    for name in os.listdir(hooks) if os.path.isdir(hooks) else []:
        os.chmod(os.path.join(hooks, name), 0o755)
    gitignore = os.path.join(into, '.gitignore')
    if not os.path.exists(gitignore):
        open(gitignore, 'w').write('target/\n')
    report('ok', 'scaffold', f'{added} files from the template, callers pinned at {sha[:12]}, '
                             f'targets {" ".join(targets)}'
                             + (f'; the crate\'s own {", ".join(sorted(kept))} kept' if kept else ''))

    blocks = patches = paths = records = 0
    source = os.path.join(root, 'crates', crate)
    for dirpath, _dirs, names in os.walk(into):
        if '/.git' in dirpath or dirpath.endswith('/.git'):
            continue
        for name in names:
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, into)
            if not rel.endswith(crate_release.TEXT) or rel.startswith('.github/'):
                continue
            text = open(path, errors='replace').read()
            if rel.startswith('falsifications/') and rel.endswith('.patch'):
                new, problems = regenerate_patch(text, crate, source)
                for p in problems:
                    report('FAIL', 'patch', f'{rel}: {p}')
                patches += new != text
            else:
                new, b, r, n = moved_source(text, crate)
                blocks, records, paths = blocks + b, records + r, paths + n
            if rel == 'Cargo.toml':
                new = add_repository(new, crate)
            if new != text:
                open(path, 'w').write(new)
    report('ok', 'rewrite', f'Name: blocks removed from {blocks} files, {patches} patches regenerated '
                            f'for -p1 over the stripped source, {paths} paths made repository-relative, '
                            f'falsification records moved to the head of their comments in {records} '
                            'files, repository set in Cargo.toml')
    check_patches(into, report)
    moved, opted_in = stage_fuzz(root, crate, into, report)
    if opted_in:
        turn_on_weekly_fuzz(into, report)
    return manifest, targets, moved


def check_patches(into, report):
    """Refuse a staged patch that applies with an offset or fuzz, or not at all."""
    directory = os.path.join(into, 'falsifications')
    names = sorted(n for n in os.listdir(directory) if n.endswith('.patch')) \
        if os.path.isdir(directory) else []
    bad = 0
    for name in names:
        said = patch_application(into, os.path.join(directory, name))
        if said:
            bad += 1
            report('FAIL', 'patch applies exactly', f'falsifications/{name}: {said}')
    if not bad:
        report('ok', 'patch applies exactly', f'{len(names)} patches apply with no offset and no fuzz '
                                              '(patch -p1 --dry-run -F0)')


def build(into, manifest, targets, work, report):
    """Lock, test and build the staged crate as `check / tests` would, at its rust-version."""
    toolchain = manifest.get('package', {}).get('rust-version') or 'stable'
    env = dict(os.environ, CARGO_TARGET_DIR=os.path.join(work, 'target'))
    env.pop('RUSTUP_TOOLCHAIN', None)
    code, out = run(['rustup', 'run', toolchain, 'rustc', '--version'], into, env)
    if code:
        report('FAIL', 'build', f'toolchain {toolchain} is not installed: rustup toolchain install {toolchain}')
        return
    code, out = run(['cargo', f'+{toolchain}', 'generate-lockfile'], into, env)
    if code:
        report('FAIL', 'build', f'cargo generate-lockfile: {tail(out)}')
        return
    code, out = run(['cargo', f'+{toolchain}', 'test', '--quiet'], into, env)
    report('FAIL' if code else 'ok', 'build', f'host tests on {toolchain}' + (f': {tail(out)}' if code else ''))
    for t in targets:
        code, out = run(['cargo', f'+{toolchain}', 'build', '--quiet', '--lib', '--target', t], into, env)
        report('FAIL' if code else 'ok', 'build', f'{t} on {toolchain}' + (f': {tail(out)}' if code else ''))


def checks(into, tree, rev, report):
    """The template's cheap checks, then the two rules the template does not hold."""
    staged = Staged(into)
    helpers = os.path.join(tree, 'helpers')
    for label, helper, argv in TEMPLATE_CHECKS:
        path = os.path.join(helpers, helper)
        if not os.path.exists(path):
            report('note', label, f'not at template {rev}, so the pinned template does not run it')
            continue
        code, out = run([sys.executable, path] + argv(staged), into)
        lines = [line for line in out.splitlines() if line.strip()]
        report('FAIL' if code else 'ok', label,
               f'{len(lines)} line(s), first: {lines[0][:300]}' if code and lines else '')
    texts = {f: open(os.path.join(into, f), errors='replace').read()
             for f in staged.files if f.endswith(crate_release.TEXT) or f.startswith('.githooks/')}
    for step, why in crate_release.citation_findings(texts):
        report('FAIL', 'citations', why)
    hits = [f'{f}:{n}' for f, t in sorted(texts.items())
            for n, line in enumerate(t.split('\n'), 1) if NIFE_WORD.search(line)]
    if hits:
        report('FAIL', 'mentions nife', f'{len(hits)} line(s), first {", ".join(hits[:4])}')
    else:
        report('ok', 'mentions nife', 'none')


def follow_up(root, crate, version, moved=()):
    print('crate-move: the nife-side pull request, once the repository holds the tag'
          f' v{version} (rev is that tag\'s commit):')
    line = (f'{crate} = {{ version = "{version}", git = "{REPOSITORY_URL.format(crate=crate)}", '
            'rev = "<commit of the tag>" }')
    for manifest, table, spec in consumers(root, crate):
        if manifest == FUZZ_MANIFEST and table == 'dependencies' and moved and not stays_in_fuzz(root, crate):
            print(f'  {manifest} [{table}]: delete the {crate} line, since no target left here links it')
        elif table == 'workspace.members':
            print(f'  {manifest}: drop "crates/{crate}" from [workspace] members')
        else:
            extra = {k: v for k, v in spec.items() if k not in ('path', 'version')}
            note = f'  (keep {extra})' if extra else ''
            print(f'  {manifest} [{table}]: {line}{note}')
    if baseline_row(root, crate) is not None:
        print(f'  {BASELINE}: delete the row for {crate}')
    covered = ('script/fuzz', 'fuzz/seeds/README.md') if moved else ()
    rest = [f for f in mentions(root, crate)
            if not f.endswith(('Cargo.toml', 'Cargo.lock')) and f != BASELINE and f not in covered
            and not (moved and f.startswith('fuzz/fuzz_targets/'))]
    if rest:
        print('crate-move: other files that name the crate, each read in the same pull request:')
        for f in rest:
            print(f'  {f}')
    if moved:
        fuzz_removals(root, crate, moved)


def patch_selftest():
    """A patch written against lines the move shifts is regenerated, and the gate sees a stale one."""
    lib = ('//! Doc.\n//!\n//! Name: ratified 2026-10-25 (#1).\n//!   Reason.\n\n'
           'pub fn a() {}\n\n/// Doc.\n///\n/// Name: ratified 2026-10-25 (#1).\n'
           'pub fn f(n: u32) -> bool {\n    let m = n + 1;\n    m > n\n}\n\n'
           'pub fn g() {}\npub fn h() {}\npub fn i() {}\npub fn j() {}\n')
    patch = ('Falsifies `x::verification::h`.\n\n'
             'diff --git a/crates/x/src/lib.rs b/crates/x/src/lib.rs\n'
             '--- a/crates/x/src/lib.rs\n+++ b/crates/x/src/lib.rs\n'
             '@@ -12,3 +12,3 @@ pub fn f\n     let m = n + 1;\n-    m > n\n+    m >= n\n }\n')
    with tempfile.TemporaryDirectory(prefix='crate-move-selftest-') as tmp:
        source, staged = os.path.join(tmp, 'crates', 'x'), os.path.join(tmp, 'into')
        for d in (os.path.join(source, 'src'), os.path.join(staged, 'src'), os.path.join(staged, 'falsifications')):
            os.makedirs(d)
        open(os.path.join(source, 'src', 'lib.rs'), 'w').write(lib)
        open(os.path.join(staged, 'src', 'lib.rs'), 'w').write(moved_source(lib, 'x')[0])
        stale = os.path.join(staged, 'falsifications', 'stale.patch')
        open(stale, 'w').write(patch.replace('a/crates/x/', 'a/').replace('b/crates/x/', 'b/'))
        said = patch_application(staged, stale)
        assert 'offset' in said, 'the gate refuses a patch whose header no longer matches: ' + said
        new, problems = regenerate_patch(patch, 'x', source)
        assert not problems, problems
        assert 'a/src/lib.rs' in new and 'crates/x' not in new and 'Name:' not in new, new
        fresh = os.path.join(staged, 'falsifications', 'fresh.patch')
        open(fresh, 'w').write(new)
        assert patch_application(staged, fresh) == '', patch_application(staged, fresh)
        assert new.startswith('Falsifies `x::verification::h`.\n'), new
        block = ('diff --git a/crates/x/src/lib.rs b/crates/x/src/lib.rs\n--- a/crates/x/src/lib.rs\n'
                 '+++ b/crates/x/src/lib.rs\n@@ -10,3 +10,3 @@\n ///\n-/// Name: ratified 2026-10-25 (#1).\n'
                 '+/// Name: provisional.\n pub fn f(n: u32) -> bool {\n')
        assert regenerate_patch(block, 'x', source)[1], 'an edit inside a Name: block is a problem'
        missing = patch.replace('m > n', 'no such line')
        assert regenerate_patch(missing, 'x', source)[1], 'a patch that does not apply is a problem'


def fuzz_selftest():
    """The staging picks a crate's own targets, gates the weekly caller on `unsafe`, and stays inert otherwise."""
    assert re.search(r'\bunsafe\b', code_text('fn f() { unsafe { g() } }'))
    blanked = code_text('// unsafe\n/* unsafe /* unsafe */ */ let a = "unsafe"; let b = r#"un"safe"#; let c = \'"\'; ok')
    assert 'unsafe' not in blanked and 'ok' in blanked, blanked
    assert 'unsafe' in code_text('/// doc\nunsafe fn f() {}')
    manifest = ('[dependencies]\nlibfuzzer-sys = "0.4"\nx = { path = "../crates/x" }\n'
                'y = { path = "../crates/y" }\n'
                '[[bin]]\nname = "x_a"\npath = "fuzz_targets/x_a.rs"\n'
                '[[bin]]\nname = "x_with_y"\npath = "fuzz_targets/x_with_y.rs"\n'
                '[[bin]]\nname = "y_only"\npath = "fuzz_targets/y_only.rs"\n')
    sources = {'x_a': 'use x::F;\n// notes/fuzzing/targets.md\n', 'x_with_y': 'use x::F;\nuse y::G;\n',
               'y_only': 'use y::G;\n'}
    with tempfile.TemporaryDirectory(prefix='crate-move-fuzz-selftest-') as tmp:
        for d in ('fuzz/fuzz_targets', 'fuzz/seeds/x_a', 'crates/x/tests/fixtures', 'crates/x/src', 'script'):
            os.makedirs(os.path.join(tmp, d))
        open(os.path.join(tmp, FUZZ_MANIFEST), 'w').write(manifest)
        for name, text in sources.items():
            open(os.path.join(tmp, 'fuzz', 'fuzz_targets', name + '.rs'), 'w').write(text)
        open(os.path.join(tmp, 'script', 'fuzz'), 'w').write(
            'case "$target" in\n    other|x_a)\n        set -- "$@" crates/x/tests/fixtures crates/y/tests/fixtures ;;\n'
            '    y_only) set -- "$@" crates/y/tests/fixtures ;;\nesac\n')
        for path in ('fuzz/seeds/x_a/s.bin', 'crates/x/tests/fixtures/f.bin', 'crates/x/tests/fixtures/make.sh',
                     'crates/x/tests/fixtures/NOTE.md'):
            open(os.path.join(tmp, path), 'w').write('0')
        open(os.path.join(tmp, 'crates/x/src/lib.rs'), 'w').write('//! no unsafe here\npub fn f() {}\n')
        mine, mixed = crate_targets(tmp, 'x')
        assert mine == [('x_a', 'fuzz_targets/x_a.rs')] and mixed == ['x_with_y'], (mine, mixed)
        assert fixture_dirs(tmp, 'x', 'x_a') == ['crates/x/tests/fixtures'], fixture_dirs(tmp, 'x', 'x_a')
        assert fixture_dirs(tmp, 'x', 'y_only') == []
        assert sorted(n for _p, n in corpus_files(tmp, 'x', 'x_a')) == ['f.bin', 's.bin'], corpus_files(tmp, 'x', 'x_a')
        for args in (['init', '-q'], ['add', '--all'], ['-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 't']):
            run(['git'] + args, tmp)
        assert unsafe_files(tmp, 'x') == []
        open(os.path.join(tmp, 'crates/x/src/lib.rs'), 'a').write('pub unsafe fn g() {}\n')
        run(['git', 'add', '--all'], tmp)
        assert unsafe_files(tmp, 'x') == ['crates/x/src/lib.rs']
    toml = tomllib.loads(fuzz_manifest_text('x', '2024', '0.4', [('x_a', 'fuzz_targets/x_a.rs')]))
    assert toml['dependencies']['x'] == {'path': '..'} and toml['bin'][0]['name'] == 'x_a'
    assert toml['package']['metadata']['cargo-fuzz'] and toml['workspace']['members'] == ['.']
    assert 'fuzz/README.md' in target_source('// notes/fuzzing/targets.md\n', 'x')
    assert seed_script_source("fuzz/seeds/t/a.img; os.path.join(here, 't', 'a'); python3 fuzz/seeds/m.py", ['t']) \
        == "fuzz/corpus/t/a.img; os.path.join(here, 'corpus', 't', 'a'); python3 fuzz/m.py"
    with tempfile.TemporaryDirectory(prefix='crate-move-fuzz-selftest-') as tmp:
        doc = os.path.join(tmp, 'doc.md')
        open(doc, 'w').write('# T\n\n## `a`\n\nA.\n\n## `b`\n\nB.\n\n## `a/s.bin`\n\nS.\n')
        assert readme_sections(doc, ['`a`', '`a/']) == ['## `a`\n\nA.\n', '## `a/s.bin`\n\nS.\n']
        os.makedirs(os.path.join(tmp, '.github', 'workflows'))
        weekly = os.path.join(tmp, '.github', 'workflows', 'weekly.yml')
        failures = []
        turn_on_weekly_fuzz(tmp, lambda *a: failures.append(a))
        assert failures and failures[0][0] == 'FAIL', failures
        open(weekly, 'w').write('with:\n  fuzz: false\n')
        turn_on_weekly_fuzz(tmp, lambda *a: failures.append(a))
        assert open(weekly).read() == 'with:\n  fuzz: true\n'
    crash = ('INFO: Seed: 1\nERROR: libFuzzer: deadly signal\nSUMMARY: libFuzzer: deadly signal\n'
             'artifact_prefix=./; Test unit written to ./crash-1\nBase64: AAAA\n')
    assert [line for line in crash.splitlines() if FUZZ_SAFE_LINE.match(line)] == \
        ['ERROR: libFuzzer: deadly signal', 'SUMMARY: libFuzzer: deadly signal']
    print('crate_move fuzz selftest: ok')


def selftest():
    patch_selftest()
    fuzz_selftest()
    src = '//! Doc.\n//!\n//! Name: ratified 2026-11-01 (UTC), the\n//!   reason.\n\npub fn f() {}\n'
    assert crate_release.strip_name_blocks(src, remove=True) == '//! Doc.\n\npub fn f() {}\n'
    assert crate_release.strip_name_blocks(src).count('\n') == src.count('\n')
    record = '/// Falsification: replayable `crates/x/falsifications/a.h.patch`'
    assert re.sub(r'(?<![\w/.])crates/x/', '', record).endswith('`falsifications/a.h.patch`')
    assert re.sub(r'(?<![\w/.])crates/x/', '', '../crates/x/y') == '../crates/x/y'
    harness = ('/// **A property.** More.\n/// Falsification: replayable `falsifications/m.h.patch`\n'
               '#[kani::proof]\nfn h() {}\n')
    assert falsification_first(harness).startswith('/// Falsification: replayable'), harness
    assert falsification_first(falsification_first(harness)) == falsification_first(harness)
    m = '[package]\nname = "x"\nlicense = "MIT OR Apache-2.0"\n'
    assert 'repository = "https://github.com/nifeos/x"' in add_repository(m, 'x')
    assert add_repository(add_repository(m, 'x'), 'x').count('repository') == 1
    wf = "    uses: o/r/.github/workflows/check.yml@PINS@\n    with:\n      targets: ''\n"
    out = caller_workflow(wf, 'abc', ['t1', 't2'])
    assert '@abc' in out and 'targets: t1 t2' in out and 'allow-unpublished: true' in out, out
    assert NIFE_WORD.search('used by nife') and not NIFE_WORD.search('github.com/nifeos/x')
    with tempfile.TemporaryDirectory() as tmp:
        os.makedirs(os.path.join(tmp, '.cargo'))
        open(os.path.join(tmp, BASELINE), 'w').write('# c\nxy 5 1 0 2\nx 54 3 1 4\nx_y 1 1 1 1\n')
        assert baseline_row(tmp, 'x') == (54, 3, 1, 4), 'the whole name matches'
        assert baseline_row(tmp, 'z') is None and baseline_row('/nonexistent', 'x') is None
    note = exempts_note((54, 3, 1, 4))
    assert 'caught 54, missed 3, timeout 1, unviable 4' in note and '3 missed' in note, note
    assert all(l.startswith('#') for l in note.splitlines()) and not NIFE_WORD.search(note), note
    assert 'no line is expected' in exempts_note((9, 0, 0, 0))
    print('crate_move selftest: ok')


def main(argv):
    if argv[1:2] == ['--selftest']:
        selftest()
        return 0
    root = argv[1]
    p = argparse.ArgumentParser(prog='script/crate-move')
    p.add_argument('--dry-run', action='store_true', required=True)
    p.add_argument('--into', required=True)
    p.add_argument('--template')
    p.add_argument('--template-rev')
    p.add_argument('--no-build', action='store_true')
    p.add_argument('crate')
    args = p.parse_args(argv[2:])
    into = os.path.abspath(args.into)
    if os.path.exists(into) and os.listdir(into):
        sys.exit(f'crate-move: {into} is not empty; a dry run stages into an empty directory')
    os.makedirs(into, exist_ok=True)
    work = tempfile.mkdtemp(prefix=f'crate-move-{args.crate}-')
    failures = []

    def report(kind, step, why=''):
        if kind == 'FAIL':
            failures.append(step)
        print(f'  {kind:<4} {step}' + (f': {why}' if why else ''))

    print(f'crate-move: {args.crate}, a dry run into {into}')
    tree, sha, rev = template_tree(args, work)
    report('ok', 'template', f'{TEMPLATE_URL} at {rev} ({sha})')
    manifest, targets, moved = stage(root, args.crate, into, tree, sha, report)
    if args.no_build:
        report('note', 'build', 'skipped (--no-build); the staged tree has no Cargo.lock')
    else:
        build(into, manifest, targets, work, report)
        if moved:
            fuzz_dry_run(into, moved, work, report)
    if moved:
        seed_script_reproduces(into, moved, work, report)
    run(['git', 'init', '--quiet', '--initial-branch=main'], into)
    run(['git', 'add', '--all'], into)
    checks(into, tree, rev, report)
    shutil.rmtree(work, ignore_errors=True)
    follow_up(root, args.crate, manifest.get('package', {}).get('version', '0.1.0'), moved)
    print(f'crate-move: staged in {into}, with git initialized and every file added, nothing committed;'
          ' no repository was created and nothing was pushed')
    print(f'crate-move: {args.crate}: ' + ('clean' if not failures else
                                         f'{len(failures)} finding(s) for the move lane'))
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
