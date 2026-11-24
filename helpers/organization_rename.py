#!/usr/bin/env python3
"""Count, or rewrite, the references to a GitHub organization's name in a checkout.

Usage:
    organization_rename.py census  [--old NAME] [--new NAME] [--tsv FILE] [--repo LABEL] TREE [PATH...]
    organization_rename.py rewrite [--old NAME] [--new NAME] [--check] TREE [PATH...]

TREE is a git checkout. PATH limits the run to those paths inside it (a top-level directory, for
one pull request per directory). Only tracked files are read, through `git ls-files`.

`census` prints one row per occurrence, as tab-separated `repo, path, line, form, disposition,
file kind, text`, to FILE or standard output, and a summary to standard error. `rewrite` changes
the forms whose disposition is `breaks` or `redirects` in every file that is not history, and
prints the files it changed. With `--check` it changes nothing and exits 1 when it would change a
file, so a cutover lane can rerun it on a moved `main` and see what is left.

A reference is one of these forms, tested in this order on each occurrence of the old name:

- `uses`: a `uses: OLD/...` line in a workflow. GitHub Actions follows no redirect for an action
  or a reusable workflow, so it breaks.
- `api-org`: a call that names the organization itself: `orgs/OLD`, `users/OLD`, `--owner`,
  `--org`, a crates.io team owner (`github:OLD:<team>`), a GraphQL or action `owner:` or `login:`,
  a search qualifier (`org:`, `repo:`, `user:`), `gh repo list OLD`, or an organization settings
  page. The old organization answers 404, so it breaks.
- `api-repo`: a call that names one repository: `repos/OLD/<repo>`, `--repo`, `-R`. A read through
  the old name follows the redirect (measured on the 2026-10-03 rename's old name), so it
  redirects. A write through it is unverified.
- `team`: `@OLD/<team>`, as in CODEOWNERS. Team mentions do not redirect.
- `package`: `ghcr.io/OLD`. Packages move to the new namespace.
- `pages`: `OLD.github.io`, the organization's Pages host name.
- `org-literal`: the bare name as a quoted string or a configuration value in code (`"OLD"`,
  `'OLD'`, `= OLD`, `github = ["OLD"]`). Code that names the organization builds API calls or
  compares `github.repository_owner`, so it breaks.
- `record-path`: `OLD/record:<path>`, the qualified record citation. The resolver builds a fetch
  URL from its `ORGANIZATION` constant, and the lints match the literal prefix. The fetch follows
  the redirect, so nothing breaks at the rename, but the text and the tooling are `tied`: they
  change together.
- `qualified-path`: `OLD/<repo>:<path>` for any other repository, resolved the same way.
- `slug-code`: `OLD/<repo>` in code, a workflow or configuration. Fed to `gh` or git, it
  redirects. Compared with a name the API returns, it breaks, since the API returns the new name;
  the census cannot tell the two apart, so a person reads each comparison.
- `cargo-git`: a `git = "https://github.com/OLD/..."` dependency, or its `Cargo.lock` source. git
  fetches follow the redirect, but cargo keys a source by its URL, so a manifest and its lock move
  together, and cargo-deny's allowed organizations with them.
- `cargo-metadata`: `repository`, `homepage` or `documentation` in a Cargo manifest.
- `pin`: a `repository =` line in basalt's `pins.toml`.
- `raw`: `raw.githubusercontent.com/OLD/...`, which redirects (measured).
- `web-link`: any other `github.com/OLD/...` URL, or an SSH remote `github.com:OLD/`, which
  redirects.
- `citation`: `OLD/<repo>`, `OLD/<repo>@<sha>` or `OLD/<repo>#<n>` in prose. Nothing resolves it
  but a reader, so it goes stale rather than breaking.
- `dns`: `OLD.org` and its subdomains, and the reverse-DNS `org.OLD.` labels. These stay.
- `local-path`: a directory named OLD inside a path (`projects/OLD/`). It stays.
- `word`: anything else, the bare name as a word in prose or a token such as `0.10.0-OLD.1`. It
  stays unless a person reads it as the organization; the census lists each one for that reading.

The disposition is `breaks`, `tied`, `redirects`, `stale` or `stays`. A file under a history path
(see HISTORY), and a line that also names the organization's name before 2026-10-03, is
`stays (history)` whatever its form: logs, bench records, vendored and patched
sources and audit reports keep their text. A falsification patch is not history: `git apply` must
still apply it to the rewritten source, so its context lines are rewritten with the source.
"""

import os
import re
import subprocess
import sys
from collections import Counter

HISTORY = (
    re.compile(r'(^|/)vendor/'),
    re.compile(r'(^|/)patches/'),
    re.compile(r'^(?!.*(^|/)falsifications/).*\.(patch|diff)$'),
    re.compile(r'(^|/)bench/'),
    re.compile(r'(^|/)notes/mutation-testing/'),
    re.compile(r'(^|/)notes/project-metrics/'),
    re.compile(r'(^|/)design/audit-reports/'),
    re.compile(r'(^|/)CHANGELOG\.md$'),
    re.compile(r'(^|/)handoff-[0-9-]+\.md$'),
    re.compile(r'\.log$'),
)

CODE_SUFFIXES = {'.py', '.sh', '.rs', '.jq', '.js', '.mjs', '.ts', '.yml', '.yaml', '.toml',
                 '.json', '.bash', '.zsh', '.plist', '.service', '.timer', '.nix'}
PROSE_SUFFIXES = {'.md', '.txt', '.rst', '.html', ''}


def file_kind(path):
    base = os.path.basename(path)
    suffix = os.path.splitext(base)[1]
    if path.startswith('.github/workflows/') or path.startswith('scaffold') and suffix == '.yml':
        return 'workflow'
    if path.startswith('.github/'):
        return 'github-config'
    if base in ('Cargo.toml', 'Cargo.lock'):
        return 'cargo'
    if base == 'CODEOWNERS':
        return 'codeowners'
    if base == 'pins.toml':
        return 'pins'
    if base == 'deny.toml':
        return 'deny'
    top = path.split('/', 1)[0]
    if top in ('helpers', 'script', 'xtask', 'tools', 'bin') or suffix in ('.py', '.sh', '.jq'):
        if suffix not in ('.md',):
            return 'script'
    if suffix == '.rs':
        return 'rust'
    if suffix in ('.md', '.txt', '.rst', '.html'):
        return 'prose'
    if suffix in ('.tsv', '.csv'):
        return 'data'
    if suffix in CODE_SUFFIXES:
        return 'config'
    return 'other'


def is_code(kind):
    return kind in ('workflow', 'github-config', 'cargo', 'codeowners', 'pins', 'deny', 'script',
                    'rust', 'config')


def history(path):
    return any(p.search(path) for p in HISTORY)


def patterns(old):
    o = re.escape(old)
    # Each entry: (form, regex for the whole reference, group name that holds the old name).
    return [
        ('dns', re.compile(r'(?<![\w-])(?P<n>%s)\.org\b|\borg\.(?P<n2>%s)\.' % (o, o))),
        ('pages', re.compile(r'(?<![\w-])(?P<n>%s)\.github\.io\b' % o)),
        ('local-path', re.compile(r'(?:projects|src|code|work)/(?P<n>%s)(?=/|\b)' % o)),
        ('uses', re.compile(r'\buses:\s*["\']?(?P<n>%s)/' % o)),
        ('api-org', re.compile(
            r'(?:api\.github\.com/(?:orgs|users)/|(?<![\w.-])(?:orgs|users)/'
            r'|github\.com/(?:organizations|orgs)/'
            r'|--owner[= ]+|--org[= ]+|\bowner:\s*\\?["\']?|\blogin:\s*\\?["\']?'
            r'|(?<![\w.])github:'
            r'|\b(?:org|repo|user):|repo list\s+)(?P<n>%s)\b' % o)),
        ('api-repo', re.compile(
            r'(?:api\.github\.com/repos/|(?<![\w.-])repos/|--repo[= ]+|(?<![\w-])-R\s+)(?P<n>%s)\b'
            % o)),
        ('team', re.compile(r'@(?P<n>%s)/' % o)),
        ('package', re.compile(r'ghcr\.io/(?P<n>%s)\b' % o)),
        ('raw', re.compile(r'(?:raw\.)?githubusercontent(?:\\?\.)com/(?P<n>%s)\b' % o)),
        ('record-path', re.compile(r'(?<![\w.~/-])(?P<n>%s)/record:(?=[\w.<{$/-])' % o)),
        ('qualified-path', re.compile(r'(?<![\w.~/-])(?P<n>%s)/[\w.<>{}$-]+:(?=[\w.<{$-])' % o)),
        ('cargo-git', re.compile(r'(?:\bgit\s*=\s*"|git\+)https://github\.com/(?P<n>%s)/' % o)),
        ('cargo-metadata', re.compile(
            r'\b(?:repository|homepage|documentation)\s*=\s*"https://github\.com/(?P<n>%s)\b' % o)),
        ('web-link', re.compile(r'github(?:\\?\.)com[/:](?P<n>%s)\b' % o)),
        ('org-literal', re.compile(
            r'(?:(?P<q>["\'])(?P<n>%s)(?P=q)|=\s*(?P<n2>%s)\b(?![/.-]))' % (o, o))),
        # A stubbed API path flattened into a fixture file name: api_repos_<org>_<repo>_...
        ('api-repo', re.compile(r'(?<=_repos_)(?P<n>%s)(?=_)' % o)),
        ('slug', re.compile(r'(?:(?<=:-)|(?<![\w.~/-]))(?P<n>%s)/(?=[\w.<{$*\[(-])' % o)),
    ]


def classify(path, line, old):
    """Yield (start, end, form) for each occurrence of `old` in `line`, end exclusive."""
    taken = {}
    for form, rx in patterns(old):
        for m in rx.finditer(line):
            for g in ('n', 'n2'):
                if g in rx.groupindex and m.group(g) is not None:
                    s = m.start(g)
                    taken.setdefault(s, form)
    for m in re.finditer(r'(?<![A-Za-z0-9])%s(?![A-Za-z0-9])' % re.escape(old), line,
                         re.IGNORECASE):
        s = m.start()
        form = taken.get(s, 'word')
        yield s, m.end(), form


def refine(form, path, kind, line):
    if form in ('web-link', 'cargo-metadata') and kind == 'pins':
        return 'pin'
    if form == 'cargo-metadata' and kind != 'cargo':
        return 'web-link'
    if form == 'web-link' and os.path.basename(path) == 'Cargo.lock':
        return 'cargo-git'
    if form == 'slug':
        return 'slug-code' if is_code(kind) else 'citation'
    if form == 'org-literal' and not is_code(kind):
        return 'word'
    return form


DISPOSITION = {
    'uses': 'breaks', 'api-org': 'breaks', 'team': 'breaks', 'package': 'breaks',
    'pages': 'breaks', 'org-literal': 'breaks',
    'record-path': 'tied', 'qualified-path': 'tied',
    'api-repo': 'redirects', 'slug-code': 'redirects', 'raw': 'redirects',
    'cargo-git': 'redirects', 'cargo-metadata': 'redirects', 'pin': 'redirects',
    'web-link': 'redirects',
    'citation': 'stale',
    'dns': 'stays', 'local-path': 'stays', 'word': 'stays',
}
REWRITE = {'breaks', 'tied', 'redirects', 'stale'}

# This file, which names both organizations as its defaults, is never rewritten. A line that names
# the organization's name before 2026-10-03 is history of that rename. A line that already names
# the new name beside the old one, or names FORMER_ORGANIZATION, is about this rename on purpose:
# a masking pair, or code that accepts the old name while some repository still says it. Both
# stay.
SELF = re.compile(r'(^|/)organization_rename\.py$')
EARLIER = 'crickertech'
FORMER = 'FORMER_ORGANIZATION'


def tracked(tree, paths):
    out = subprocess.run(['git', '-C', tree, 'ls-files', '-z', '--'] + list(paths),
                         capture_output=True, check=True).stdout
    return [p for p in out.decode().split('\0') if p]


def scan(tree, paths, old, new):
    for path in tracked(tree, paths):
        full = os.path.join(tree, path)
        if os.path.islink(full) or not os.path.isfile(full) or SELF.search(path):
            continue
        try:
            with open(full, 'rb') as f:
                raw = f.read()
        except OSError:
            continue
        if b'\0' in raw[:8192] or old.encode().lower() not in raw.lower():
            continue
        text = raw.decode('utf-8', 'replace')
        kind = file_kind(path)
        hist = history(path)
        for number, line in enumerate(text.split('\n'), 1):
            if old.lower() not in line.lower():
                continue
            for s, e, form in classify(path, line, old):
                form = refine(form, path, kind, line)
                disp = ('stays (history)' if hist or EARLIER in line.lower()
                        else 'stays (former name)' if FORMER in line or new in line.lower()
                        else DISPOSITION[form])
                yield path, number, s, e, form, disp, kind, line


def main(argv):
    if len(argv) < 2 or argv[0] not in ('census', 'rewrite'):
        sys.exit(__doc__)
    mode, args = argv[0], argv[1:]
    old, new, tsv, label, check = 'nifeos', 'partsfoundry', None, None, False
    rest = []
    while args:
        a = args.pop(0)
        if a == '--old':
            old = args.pop(0)
        elif a == '--new':
            new = args.pop(0)
        elif a == '--tsv':
            tsv = args.pop(0)
        elif a == '--repo':
            label = args.pop(0)
        elif a == '--check':
            check = True
        else:
            rest.append(a)
    tree, paths = rest[0], rest[1:]
    label = label or os.path.basename(os.path.abspath(tree))
    if mode == 'census':
        out = open(tsv, 'a') if tsv else sys.stdout
        summary = Counter()
        for path, number, s, e, form, disp, kind, line in scan(tree, paths, old, new):
            out.write('\t'.join([label, path, str(number), form, disp, kind,
                                 line.strip().replace('\t', ' ')[:300]]) + '\n')
            summary[(form, disp)] += 1
        for (form, disp), n in sorted(summary.items()):
            sys.stderr.write('%s\t%s\t%s\t%d\n' % (label, form, disp, n))
        return 0
    edits = {}
    for path, number, s, e, form, disp, kind, line in scan(tree, paths, old, new):
        if disp in REWRITE:
            edits.setdefault(path, []).append((number, s, e))
    changed = 0
    for path, spots in sorted(edits.items()):
        full = os.path.join(tree, path)
        with open(full, encoding='utf-8', newline='') as f:
            lines = f.read().split('\n')
        for number, s, e in sorted(spots, key=lambda x: (x[0], -x[1])):
            text = lines[number - 1]
            if text[s:e].lower() != old.lower():
                raise SystemExit('organization_rename: %s:%d moved under the scan' % (path, number))
            lines[number - 1] = text[:s] + new + text[e:]
        changed += 1
        print(path)
        if not check:
            with open(full, 'w', encoding='utf-8', newline='') as f:
                f.write('\n'.join(lines))
    if check and changed:
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
