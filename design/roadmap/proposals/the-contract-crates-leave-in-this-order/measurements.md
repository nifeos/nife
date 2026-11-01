# The contract crates leave in this order: the measurements

[The proposal](../the-contract-crates-leave-in-this-order.md) holds the order and the policy. This
appendix holds the numbers behind them and the commands that produce them. Every number here comes
from `nifeos/nife@a8b7524d7`, the tip of `main` on 2026-11-01 at 18:15 (UTC).

## How to rerun it

From a checkout of nife, with the script below saved outside the tree as `census.py` and the 22
names, one per line or space-separated, in `set.txt`:

```sh
git fetch origin
python3 census.py a8b7524d7 2026-11-01T23:59:59Z set.txt
```

The 22 are the set pull request #3022 derives for milestone 956 (the linked interfaces carry
checkable versions): `abi address_space_map block_roster byte_sink_contract calendar
capability_manifest_note clock_contract counter_frequency_contract current_cpu_contract
domain_name_resolution_contract domain_name_system entropy_contract environment_contract
filesystem_contract filesystem_fixture glob globally_unique_identifier_partition_table grant_plan
machine_statistics_contract program_argument_contract socket_contract user_mode_runtime`.

What each column means:

- Layer: the longest chain of normal or build dependencies inside the 22, leaves at 0.
- In 22 and outside: workspace crates that link the crate (normal or build), split by membership.
  Dev only outside: crates outside the 22 that use it in tests only.
- 30d and 90d: commits that touch the crate, merges excluded, from 2026-10-02 and from 2026-08-03.
  The crate's directory follows git's rename detection on `src/lib.rs`, so `crates/clock_protocol`
  and `crates/clock_proto` count as `clock_contract`. Crossing: the commit also touches a path
  outside the crate that is not Markdown.
- Identity bumps: commits in which a public constant named `*VERSION*`, `*MAGIC*`, `*REVISION*` or
  `*INTERFACE_ID*` keeps its name and changes its value.
- API 4w and 13w: the `breaking` column of milestone 610 (the interface's stability is measured
  weekly)'s per-crate series, summed over 2026W41 to W44 and W32 to W44. It counts changed or
  removed public items from rustdoc's JSON. A dash means the series does not measure the crate.

## The table

| crate | layer | in 22 | outside | dev only outside | 30d | crossing | 90d | crossing | identity bumps 90d | API 4w | API 13w |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `abi` | 0 | 1 | 12 | 1 | 74 | 66 | 124 | 114 | 0 | 13 | 46 |
| `address_space_map` | 0 | 2 | 17 | 2 | 19 | 18 | 21 | 19 | 0 | 1 | 1 |
| `block_roster` | 0 | 0 | 2 | 1 | 10 | 8 | 19 | 17 | 1 | 1 | 1 |
| `byte_sink_contract` | 0 | 0 | 6 | 1 | 19 | 18 | 38 | 37 | 0 | 2 | 8 |
| `calendar` | 0 | 0 | 3 | 1 | 22 | 21 | 37 | 36 | 0 | 5 | 5 |
| `clock_contract` | 0 | 1 | 6 | 1 | 20 | 18 | 43 | 40 | 0 | 6 | 11 |
| `counter_frequency_contract` | 0 | 1 | 2 | 1 | 15 | 13 | 27 | 23 | 0 | 3 | 4 |
| `domain_name_system` | 0 | 1 | 1 | 1 | 17 | 16 | 17 | 16 | 0 | - | - |
| `entropy_contract` | 0 | 1 | 4 | 1 | 17 | 16 | 32 | 30 | 0 | 2 | 5 |
| `environment_contract` | 0 | 0 | 3 | 1 | 14 | 12 | 23 | 21 | 0 | 7 | 8 |
| `filesystem_contract` | 0 | 1 | 10 | 1 | 52 | 45 | 118 | 105 | 0 | 263 | 305 |
| `glob` | 0 | 1 | 2 | 0 | 10 | 9 | 24 | 22 | 0 | 0 | 0 |
| `globally_unique_identifier_partition_table` | 0 | 0 | 4 | 0 | 31 | 27 | 59 | 50 | 0 | 75 | 103 |
| `program_argument_contract` | 0 | 1 | 0 | 0 | 11 | 9 | 12 | 10 | 2 | 1 | 1 |
| `socket_contract` | 0 | 1 | 4 | 1 | 28 | 26 | 49 | 45 | 0 | 6 | 21 |
| `current_cpu_contract` | 1 | 2 | 2 | 1 | 23 | 16 | 26 | 18 | 2 | 6 | 6 |
| `domain_name_resolution_contract` | 1 | 1 | 5 | 1 | 30 | 27 | 30 | 27 | 0 | - | - |
| `filesystem_fixture` | 1 | 0 | 5 | 1 | 14 | 13 | 14 | 13 | 0 | - | - |
| `grant_plan` | 1 | 1 | 7 | 3 | 151 | 120 | 255 | 212 | 0 | 227 | 312 |
| `capability_manifest_note` | 2 | 0 | 5 | 0 | 48 | 42 | 57 | 49 | 0 | 8 | 8 |
| `machine_statistics_contract` | 2 | 0 | 6 | 1 | 12 | 9 | 14 | 11 | 0 | 0 | 0 |
| `user_mode_runtime` | 2 | 0 | 7 | 0 | 91 | 80 | 158 | 142 | 0 | - | - |

Over the 30 days: 6,764 commits on `main`, 6,610 of them focused (no more than 40 files and 8
top-level directories, the mass mechanical rule of milestone 610). 348 focused commits touch the 22.
56 of those span two or more of the 22, 244 reach outside the 22, and 5 span two or more while
staying inside. 39 distinct crates outside the 22 link at least one of them.

Where the 244 reach, by top-level directory or crate, from the same commit lists: `components` 80,
`kernel` 72, `system_tests` 51, `xtask` 45, `crates/system_initializer` 45, `design` 38, `fixtures`
37, `crates/swish` 24, `notes` 23, `helpers` 21, `script` 18, `packages` 15.

The five identity bumps, read by hand from `git log -p`: `block_roster` re-minted its magic to
`blkrosv1` on 2026-10-29. `current_cpu_contract` moved `CURRCPU1` to `CURRCPU2` on 2026-10-20 and
back on 2026-10-21. `program_argument_contract` moved `nifeargv` to `nifargv1` on 2026-10-29 and back
on 2026-10-30. Each revert's commit message names basalt's published programs as the reason.
`capability_manifest_note` also minted descriptor versions 2 and 3 in the window as new constants
beside version 1; the rule counts value changes, so it counts neither.

## Per package

The same script names, for each first-cut program, the workspace crates its source names by a
`crate::` path, closed over normal dependencies. A program in `components` is its `[[bin]]` file and
the directory beside it. "With the note macro" adds the closure of `capability_manifest_note`, which
pull request #3022's `carry_interfaces!` brings into every program.

| package | crates of the 22 it reaches | with the note macro |
|---|---|---|
| `disk-tools` | 12: `abi`, `address_space_map`, `block_roster`, `clock_contract`, `counter_frequency_contract`, `current_cpu_contract`, `entropy_contract`, `filesystem_contract`, `filesystem_fixture`, `globally_unique_identifier_partition_table`, `socket_contract`, `user_mode_runtime` | 18 |
| `process-tools` | 13: `abi`, `address_space_map`, `byte_sink_contract`, `clock_contract`, `counter_frequency_contract`, `current_cpu_contract`, `entropy_contract`, `glob`, `grant_plan`, `machine_statistics_contract`, `program_argument_contract`, `socket_contract`, `user_mode_runtime` | 16 |
| `core-tools` | 15: the 13 of `process-tools` less `machine_statistics_contract`, plus `calendar`, `environment_contract` and `filesystem_contract` | 18 |

The macro has lived in the leaf crate `interfaces_note` since 2026-11-01 (UTC), so "with the note
macro" now means the column's crates plus that one: 13, 14 and 16.

## References back into nife

```sh
# Paths that leave each crate, in source and manifests:
for c in $(cat set.txt); do
  grep -rn -E 'CARGO_MANIFEST_DIR|"(\.\./)+[^"]+"' crates/$c --include=*.rs --include=*.toml
done
# Dev-dependencies and workspace inheritance, per manifest:
for c in $(cat set.txt); do echo "$c"; sed -n '/^\[dev-dependencies\]/,/^\[/p' crates/$c/Cargo.toml; grep -n 'workspace = true' crates/$c/Cargo.toml; done
# Fuzz targets in nife that build a crate, and files in nife that name its path:
for c in $(cat set.txt); do
  echo "$c fuzz=$(grep -c -E "^$c\s*=" fuzz/Cargo.toml) paths=$(git grep -l -E "crates/$c\b" -- ':!*.md' ":!crates/$c" | wc -l)"
done
```

Results: every crate inherits `license` and `lints`. No crate has a `build.rs`. The six blocking
references are in the proposal's table. `grant_plan`'s `../../..` strings are path arguments in its
own tests, not files. Fuzz targets build `byte_sink_contract`, `capability_manifest_note`,
`domain_name_resolution_contract`, `domain_name_system`, `filesystem_contract`,
`globally_unique_identifier_partition_table` and `grant_plan`. Files in nife that name a crate's
path range from 5 (`glob`) to 36 (`user_mode_runtime`).

`nifeos/jh7110_entropy` at `9b8769f`: its `Cargo.toml` has 31 lines and no path or `git` dependency.
`git grep` finds `nifeos/nife` only in its README and two documents, and its workflows call
`nifeos/crate-workflows` at a pinned commit.

## The landing durations

- `jh7110_entropy` pull request #5: its `check` and `proofs` runs took 6 and 7 minutes
  (`gh run list -R nifeos/jh7110_entropy --branch lane/rename-constants`). Merged 22:22:46 UTC.
- nife pull request #2979: opened 22:27:02, merged 23:15:31 (`gh pr view 2979 --json
  createdAt,mergedAt`).
- basalt's gate: 15 minutes cold and 10 warm, from milestone 755 (basalt v0 pins nife and runs its
  gate).

## Blind spots

- The crossing count reads paths. A commit that touches a crate and a document beside another
  crate counts as crossing only when the document is not Markdown.
- The API column inherits milestone 610's blind spots, and four crates have no series.
- A commit inside the window that predates a crate's directory counts under its old name only when
  git's rename detection pairs the two `lib.rs` files.
- The per-package reach reads source text, as the declaration check does. A crate reached only
  through another crate's macro counts as that other crate's.

## The script

```python
import collections, csv, json, re, subprocess, sys
REV, UNTIL = sys.argv[1], sys.argv[2]          # a8b7524d7 2026-11-01T23:59:59Z
SET = set(open(sys.argv[3]).read().split())    # the 22 crate names
def run(*a): return subprocess.run(a, capture_output=True, text=True, check=True).stdout
meta = json.loads(run('cargo', 'metadata', '--format-version', '1', '--offline'))
ws = [p for p in meta['packages'] if p['id'] in meta['workspace_members']]
at = {p['manifest_path'].rsplit('/', 1)[0]: p['name'] for p in ws}
root = meta['workspace_root'] + '/'
home = {p['name']: p['manifest_path'][len(root):].rsplit('/', 1)[0] for p in ws}
deps, dev = collections.defaultdict(set), collections.defaultdict(set)
for p in ws:
    for d in p['dependencies']:
        if d.get('path') in at and at[d['path']] != p['name']:
            (dev if d['kind'] == 'dev' else deps)[p['name']].add(at[d['path']])
rdeps, rdev = collections.defaultdict(set), collections.defaultdict(set)
for a in deps:
    for b in deps[a]: rdeps[b].add(a)
for a in dev:
    for b in dev[a]: rdev[b].add(a)
layer = {}
def lay(c):
    if c not in layer:
        inner = deps[c] & SET
        layer[c] = 1 + max(map(lay, inner)) if inner else 0
    return layer[c]
def dirs(c):   # every directory the crate has lived in, by git's rename detection
    out = run('git', 'log', '--follow', '--name-only', '--format=', REV, '--', home[c] + '/src/lib.rs')
    return {home[c]} | {f[:-11] for f in out.split() if f.endswith('/src/lib.rs')}
D = {c: dirs(c) for c in SET}
def owner(f): return next((c for c in SET if any(f.startswith(d + '/') for d in D[c])), None)
def log(since):
    out = run('git', 'log', '--no-merges', f'--since={since}', f'--until={UNTIL}', '--format=@%h', '--name-only', REV, '--')
    return [c.split()[1:] for c in out.split('@')[1:]]
L30, L90 = log('2026-10-02T00:00:00Z'), log('2026-08-03T00:00:00Z')
def churn(c, commits):
    mine = [fs for fs in commits if any(owner(f) == c for f in fs)]
    return len(mine), sum(any(owner(f) != c and not f.endswith('.md') for f in fs) for fs in mine)
CONST = re.compile(r'^([-+])\s*pub(?:\([a-z]+\))?\s+const\s+(\w*(?:VERSION|MAGIC|REVISION|INTERFACE_ID)\w*)\s*:[^=]*=\s*(.+?);')
def bumps(c):  # commits where an identity constant keeps its name and changes its value
    out = run('git', 'log', '--no-merges', '--since=2026-08-03T00:00:00Z', f'--until={UNTIL}', '-p',
              '--unified=0', '--format=commit@%h', REV, '--', *D[c])
    n = 0
    for commit in out.split('commit@')[1:]:
        side = {'-': {}, '+': {}}
        for line in commit.splitlines():
            m = CONST.match(line)
            if m: side[m[1]][m[2]] = m[3]
        n += any(k in side['+'] and side['+'][k] != v for k, v in side['-'].items())
    return n
rows = list(csv.DictReader(open('notes/project-metrics/interface-stability-crates.csv')))
def api(c, first):  # milestone 610's rustdoc-measured breaking items, weeks first..44 of 2026
    r = [int(x['breaking']) for x in rows if x['crate'] == c and int(x['week'][5:]) >= first]
    return sum(r) if any(x['crate'] == c for x in rows) else '-'
print('crate|layer|in 22|outside|dev only outside|30d|crossing|90d|crossing|identity bumps 90d|API 4w|API 13w')
for c in sorted(SET, key=lambda c: (lay(c), c)):
    print('|'.join(map(str, [c, lay(c), len(rdeps[c] & SET), len(rdeps[c] - SET), len(rdev[c] - rdeps[c] - SET),
          *churn(c, L30), *churn(c, L90), bumps(c), api(c, 41), api(c, 32)])))
focused = [fs for fs in L30 if len(fs) <= 40 and len({f.split('/')[0] for f in fs}) <= 8]
touch = [fs for fs in focused if any(owner(f) for f in fs)]
span = [fs for fs in touch if len({owner(f) for f in fs} - {None}) > 1]
out = [fs for fs in touch if any(owner(f) is None and not f.endswith('.md') for f in fs)]
print(f'30 days: {len(L30)} commits, {len(focused)} focused; {len(touch)} touch the 22; '
      f'{len(span)} span two or more; {len(out)} reach outside the 22; '
      f'{len([fs for fs in span if fs not in out])} span two or more and stay inside')
print('distinct crates outside the 22 that link one of them:', len(set().union(*(rdeps[c] - SET for c in SET))))
import os, tomllib
def close(s):
    s, todo = set(s), list(s)
    while todo:
        for d in deps[todo.pop()] - s: s.add(d); todo.append(d)
    return s
bins = {b['name']: 'components/' + b['path'] for b in tomllib.load(open('components/Cargo.toml', 'rb'))['bin']}
for pkg in ('process-tools', 'core-tools', 'disk-tools'):
    reach = set()
    for prog in tomllib.load(open(f'packages/{pkg}.package.toml', 'rb'))['programs']:
        if prog not in bins:
            reach |= close(deps[prog]); continue
        files = [bins[prog]] + [os.path.join(r, x) for r, _, xs in os.walk(bins[prog][:-3]) for x in xs]
        code = '\n'.join(l for f in files for l in open(f) if not l.lstrip().startswith('//'))
        reach |= close({c for c in at.values() if re.search(r'(?<![\w:])' + c + r'::', code)})
    note = close({'capability_manifest_note'})
    print(pkg, len(reach & SET), sorted(reach & SET), '| with the note macro:', len((reach | note) & SET))
```

The where-the-244-reach counts come from the same commit lists, grouped by the first path component
(the first two under `crates/`).
