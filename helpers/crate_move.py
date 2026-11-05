"""Stage a crate's move out of this tree into a repository of its own, as a dry run.

Read by script/crate-move, whose header says what it does and what it never does. The steps it
stands in for, and who takes each, are notes/crate-move.md.

Name: provisional, minted 2026-11-05 (UTC) for milestone 1034 (a runbook for moving a crate out of
nife), a maintainer decision. It reuses `crate_release.py`'s citation rules and its `Name:` block
reader, so the move, `script/crate-standalone` and the review packet agree on what a block is.
"""
import argparse
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


def rewrite_patch(text, crate):
    """A falsification patch as the crate repository applies it: `-p1` from the crate's root.

    Paths lose their `crates/<crate>/` prefix, and a context line that is part of a `Name:` block
    is dropped with its hunk's counts, since the move removes those lines from the source. Returns
    (text, problems); a problem is a removed or added line inside a block, which only a person can
    rewrite.
    """
    out, problems, lines, i = [], [], text.split('\n'), 0
    prefix = re.compile(r'(^|\s)([ab])/crates/' + re.escape(crate) + '/')
    while i < len(lines):
        line = lines[i]
        if line.startswith(('diff --git ', '--- ', '+++ ')):
            out.append(prefix.sub(r'\1\2/', line))
            i += 1
            continue
        m = HUNK.match(line)
        if not m:
            out.append(line)
            i += 1
            continue
        j = i + 1
        while j < len(lines) and lines[j][:1] in (' ', '-', '+', '\\') and not HUNK.match(lines[j]):
            j += 1
        body = lines[i + 1:j]
        old = [(n, b) for n, b in enumerate(body) if b[:1] in (' ', '-')]
        stripped = crate_release.strip_name_blocks('\n'.join(b[1:] for _, b in old)).split('\n')
        drop = {old[k][0] for k, s in enumerate(stripped) if s == '' and old[k][1][1:] != ''}
        for n in sorted(drop):
            if body[n][:1] == '-':
                problems.append(f'a removed line is part of a Name: block: {body[n][1:].strip()}')
        drop = {n for n in drop if body[n][:1] == ' '}
        old_start, old_count = int(m.group(1)), int(m.group(2) or 1)
        new_start, new_count = int(m.group(3)), int(m.group(4) or 1)
        out.append(f'@@ -{old_start},{old_count - len(drop)} +{new_start},{new_count - len(drop)} @@'
                   f'{m.group(5)}')
        out += [b for n, b in enumerate(body) if n not in drop]
        i = j
    return '\n'.join(out), problems


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


def run(argv, cwd, env=None):
    p = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True)
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
                new, problems = rewrite_patch(text, crate)
                for p in problems:
                    report('FAIL', 'patch', f'{rel}: {p}')
                patches += new != text
            else:
                new = crate_release.strip_name_blocks(text, remove=True)
                blocks += new != text
                if rel.endswith('.rs'):
                    moved = falsification_first(new)
                    records += moved != new
                    new = moved
                # A path into the crate's own directory is a path from the repository's root.
                new, n = re.subn(r'(?<![\w/.])crates/' + re.escape(crate) + '/', '', new)
                paths += n
            if rel == 'Cargo.toml':
                new = add_repository(new, crate)
            if new != text:
                open(path, 'w').write(new)
    report('ok', 'rewrite', f'Name: blocks removed from {blocks} files, {patches} patches rewritten '
                            f'to -p1, {paths} paths made repository-relative, falsification records '
                            f'moved to the head of their comments in {records} files, repository set '
                            'in Cargo.toml')
    return manifest, targets


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


def follow_up(root, crate, version):
    print('crate-move: the nife-side pull request, once the repository holds the tag'
          f' v{version} (rev is that tag\'s commit):')
    line = (f'{crate} = {{ version = "{version}", git = "{REPOSITORY_URL.format(crate=crate)}", '
            'rev = "<commit of the tag>" }')
    for manifest, table, spec in consumers(root, crate):
        if table == 'workspace.members':
            print(f'  {manifest}: drop "crates/{crate}" from [workspace] members')
        else:
            extra = {k: v for k, v in spec.items() if k not in ('path', 'version')}
            note = f'  (keep {extra})' if extra else ''
            print(f'  {manifest} [{table}]: {line}{note}')
    if baseline_row(root, crate) is not None:
        print(f'  {BASELINE}: delete the row for {crate}')
    rest = [f for f in mentions(root, crate)
            if not f.endswith(('Cargo.toml', 'Cargo.lock')) and f != BASELINE]
    if rest:
        print('crate-move: other files that name the crate, each read in the same pull request:')
        for f in rest:
            print(f'  {f}')


def selftest():
    patch = ('Falsifies `x::verification::h`.\n\n'
             'diff --git a/crates/x/src/a.rs b/crates/x/src/a.rs\n'
             '--- a/crates/x/src/a.rs\n+++ b/crates/x/src/a.rs\n'
             '@@ -10,5 +10,5 @@ fn f() {\n'
             '     /// Doc.\n     ///\n     /// Name: ratified 2026-10-25 (#1).\n'
             '-    a\n+    b\n')
    new, problems = rewrite_patch(patch, 'x')
    assert 'diff --git a/src/a.rs b/src/a.rs' in new and '--- a/src/a.rs' in new, new
    assert '@@ -10,3 +10,3 @@ fn f() {' in new and 'Name:' not in new and not problems, new
    bad = 'diff --git a/crates/x/s b/crates/x/s\n@@ -1,1 +1,1 @@\n-/// Name: ratified\n+x\n'
    assert rewrite_patch(bad, 'x')[1], 'a removed Name: line is a problem'
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
    manifest, targets = stage(root, args.crate, into, tree, sha, report)
    if args.no_build:
        report('note', 'build', 'skipped (--no-build); the staged tree has no Cargo.lock')
    else:
        build(into, manifest, targets, work, report)
    run(['git', 'init', '--quiet', '--initial-branch=main'], into)
    run(['git', 'add', '--all'], into)
    checks(into, tree, rev, report)
    shutil.rmtree(work, ignore_errors=True)
    follow_up(root, args.crate, manifest.get('package', {}).get('version', '0.1.0'))
    print(f'crate-move: staged in {into}, with git initialized and every file added, nothing committed;'
          ' no repository was created and nothing was pushed')
    print(f'crate-move: {args.crate}: ' + ('clean' if not failures else
                                         f'{len(failures)} finding(s) for the move lane'))
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
