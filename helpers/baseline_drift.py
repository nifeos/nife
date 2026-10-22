"""Cumulative icount drift per benchmark row since a fixed anchor, and the part of it on `main` that
no record accounts for.

Milestone 415 (sub-tripwire drift accumulates across baseline saves), item 3, as calef ruled it on
2026-09-26 (UTC): "3b", a report and not a gate, in §190 (must an icount baseline save record why it
moved). `cargo xtask bench --check` compares
against the last saved floor and `--save` rewrites that floor, so steps under the 10% tripwire add
up and nothing fires. This module says how far each row has moved since a fixed anchor, beside the
`# why:` lines each save wrote, so slow drift is visible to someone who did not go looking.

What the weekly chart plots is narrower, by calef's ruling on #2554 (2026-10-22 UTC): **unexplained
drift on `main`**. A row's drift on `main` is the merge queue's reading at the week's last merge
against the anchor, and it is a product of steps, one per floor save and a last one for `main`'s gap
to the current floor. A step is explained by a line in bench/attributions.tsv (that file's header
says what a line may claim); everything else is unexplained, and a save that absorbs `main`'s drift
without separating it leaves that step unexplained until a line names it. The figure is drift on
`main` divided by the product of the explained steps, so an explained improvement does not cancel
an unexplained cost. Above 5% is a lane to open.

    python3 helpers/baseline_drift.py              # the report for HEAD, as markdown, on stdout
    python3 helpers/baseline_drift.py --selftest   # the arithmetic against fixtures; script/lint
    python3 helpers/baseline_drift.py --audit      # reproduce the 2026-09-15 audit's five figures
    python3 helpers/baseline_drift.py --update     # fetch each week's reading on main; metrics.yml

`script/metrics` imports it for the weekly series (`unexplained-drift.csv`), the status line on
notes/project-metrics.md, and the generated appendix notes/project-metrics/baseline-drift.md.

Name: provisional, minted by milestone 415's lane on 2026-09-26. A shared python module under
`helpers/`, outside `script/names`' scope, so its provenance is this paragraph. Refused
`drift_report`, which names the output and not the thing measured.

The anchor is the 2026-09-15 audit's (notes/benchmarks/baseline-save-audit.md), one commit per
architecture, pinned in ANCHORS below. aarch64 starts at `74431429`, the first save after the
one-hart fix and the QEMU pin, because every earlier number means something else. riscv64 and
x86_64 start at their birth. Refused: anchoring at the first save that carries `# why:` lines
(2026-09-23). That would read every row as zero on the day the report started, and riscv64's
`ctx_switch`, the +10.78% that motivated this milestone, would disappear from the one place meant
to show it. The saves before the ledger are listed with their commit subject instead, marked so.

What a row's number means. Every figure is per iteration (`ticks / iters`), so a save that changes
an iteration count reads as zero, which is the audit's first correction. `floor` is the committed
number against the anchor. `restamp` is the compiler term `cargo xtask bench --restamp` has carried
since the last `--save` (xtask/src/restamp.rs), read back from the last `# why:` line that holds
its ledger: a restamp moves `# toolchain:` without moving a number, so without this term the floor
would understate what the pinned compiler produces. `combined` compounds the two. A row born after
the anchor is measured from the first save that carries it, and says so.

BUGS

- The restamp ledger is read by string shape, mirroring `prior_cumulative` in
  xtask/src/restamp.rs. If that marker or its `name +0.123%` listing changes there, this reads
  nothing and reports a zero restamp term, silently. The selftest pins the shape both sides use
  today, and nothing compares the two.
- A row that is renamed reads as one row removed and one born. None has been renamed since the
  anchors.
- Like the audit, this reads what each save recorded, not what the tree measured between saves.
- `main` is read on CI's x86_64 Linux runner and the floors are saved on patagonia. At two save
  commits CI reproduced every x86_64 floor exactly and riscv64 within 0.11%, aarch64 within 0.5%
  except `map_el0` (-2.75%, a host term) and `ipc_rtt` (+3.79%, bimodal under the preemption tick
  until milestone 939 (the icount bench runs without a preemption tick in its windows)). Those two
  rows can carry a few percent no code caused.
- The readings for 2026W39 to 2026W43 came from the merge queue's job logs, before the bench job
  kept them as an artifact; 2026W39's is 2026-09-24's, the oldest the run listing reached. Weeks
  before it have no reading and chart as unmeasured.
- An attribution line applies to every step whose range holds its commit, by ancestry. A line for a
  commit that later reaches `main` a second way (a revert, a cherry-pick) would count twice.
"""

import csv
import functools
import io
import json
import subprocess
import sys
import zipfile
from datetime import datetime, timezone

# (anchor commit, the path the file had at that commit). The audit's anchors, full SHAs so a
# short one never becomes ambiguous. aarch64's file was `bench/baseline.txt` until 2026-08-03.
ANCHORS = {
    'aarch64': ('744314290a65a6e5fce285f4d06b9d6f31e1869c', 'bench/baseline.txt'),
    'riscv64': ('8c279536fd1b2165d4516ba437a82de8f134e2c2', 'bench/baseline-riscv64.txt'),
    'x86_64': ('d31aa77c98e67b5dd2c7ee72c2631baf24d6e7e0', 'bench/baseline-x86_64.txt'),
}
# Every name the file has had, newest first. A save is read from the first that exists.
PATHS = {
    'aarch64': ('bench/baseline-aarch64.txt', 'bench/baseline.txt'),
    'riscv64': ('bench/baseline-riscv64.txt',),
    'x86_64': ('bench/baseline-x86_64.txt',),
}
ARCHES = tuple(ANCHORS)
TRIPWIRE_PCT = 10.0      # `cargo xtask bench --check`'s bound against the last floor
WATCH_PCT = 5.0          # half the tripwire: unexplained drift past it is a lane to open
# What explains a step, and what `main` measured each week. The second is the one input to
# notes/project-metrics.md that git cannot restate: GitHub keeps a run's artifacts 90 days, so
# `--update` commits each week's reading once and `script/metrics` restates everything else from it.
# Both are read from the working tree, because metrics.yml runs `--update` and then `script/metrics`
# before either is committed. Names provisional, minted 2026-10-22 (UTC) by
# lane/bench/unexplained-drift-chart.
ATTRIBUTIONS = 'bench/attributions.tsv'
READINGS = 'notes/project-metrics/bench-on-main.csv'
READING_FIELDS = ('week', 'commit', 'arch', 'row', 'ticks', 'iters')
REPO = 'nifeos/nife'
ARTIFACT = 'bench-readings'    # ci.yml's bench job uploads it; the file inside is the same name
# Must match CUMULATIVE_MARKER in xtask/src/restamp.rs.
RESTAMP_MARKER = 'cumulative since the last --save:'


# --- reading one file ---------------------------------------------------------------------------

def parse(text):
    """{'rows': {name: (ticks, iters)}, 'why': [reason, ...]} from one baseline file's text."""
    rows, why = {}, []
    for line in text.split('\n'):
        stripped = line.strip()
        if stripped.startswith('# why:'):
            why.append(stripped[len('# why:'):].strip())
            continue
        if not stripped or stripped.startswith('#'):
            continue
        parts = stripped.split()
        if len(parts) == 3 and parts[1].isdigit() and parts[2].isdigit() and int(parts[2]) > 0:
            rows[parts[0]] = (int(parts[1]), int(parts[2]))
    return {'rows': rows, 'why': why}


def restamp_terms(why):
    """{row: percent} from the last `# why:` line carrying the restamp ledger, else {}."""
    ledger = None
    for reason in why:
        if RESTAMP_MARKER in reason:
            ledger = reason.rsplit(RESTAMP_MARKER, 1)[1].strip()
    out = {}
    if not ledger or ledger == 'none':
        return out
    for item in ledger.split(', '):
        name, _, pct = item.strip().partition(' ')
        try:
            out[name] = float(pct.strip().rstrip('%'))
        except ValueError:
            continue
    return out


# --- the arithmetic -----------------------------------------------------------------------------

def per_iter(cell):
    ticks, iters = cell
    return ticks / iters


def pct(before, after):
    """Per-iteration move from `before` to `after`, in percent."""
    return (per_iter(after) / per_iter(before) - 1.0) * 100.0


def compound(a, b):
    """Two successive percent moves as one."""
    return ((1.0 + a / 100.0) * (1.0 + b / 100.0) - 1.0) * 100.0


def cumulative(anchor, saves):
    """Per row, how far the last save sits from the anchor, and how it got there.

    `anchor` and each of `saves` are `parse()` results with a 'date' key, oldest save first. A save
    whose rows cost the same per iteration as the previous one's still counts as a save (a restamp, a header change) but
    moves nothing. Returns {row: {...}} for every row the last state carries.
    """
    states = [anchor] + list(saves)
    last = states[-1]
    restamp = restamp_terms(last['why'])
    out = {}
    for name, now in last['rows'].items():
        base, since = None, None
        steps, prev = [], None
        for state in states:
            cell = state['rows'].get(name)
            if cell is None:
                prev = None
                continue
            if base is None:
                base, since = cell, state
            elif prev is not None and per_iter(cell) != per_iter(prev):
                steps.append((pct(prev, cell), state))
            prev = cell
        floor = pct(base, now)
        term = restamp.get(name, 0.0)
        largest = max(steps, key=lambda s: abs(s[0]), default=None)
        out[name] = {
            'since': since['date'],
            'at_anchor': since is anchor,
            'floor': floor,
            'restamp': term,
            'combined': compound(floor, term),
            'moves': len(steps),
            'largest': largest[0] if largest else 0.0,
            'largest_date': largest[1]['date'] if largest else '',
            'tripped': sum(1 for s, _ in steps if abs(s) > TRIPWIRE_PCT),
        }
    return out


def save_summary(prev, save):
    """(rows moved, largest move as (row, percent) or None) between two consecutive states."""
    moved = []
    for name, cell in save['rows'].items():
        before = prev['rows'].get(name)
        if before is not None and per_iter(before) != per_iter(cell):
            moved.append((name, pct(before, cell)))
    largest = max(moved, key=lambda m: abs(m[1]), default=None)
    return len(moved), largest


# --- reading history ----------------------------------------------------------------------------

def _git(*args):
    return subprocess.run(['git', *args], capture_output=True, check=True).stdout.decode()


@functools.lru_cache(maxsize=None)
def _show(rev, paths):
    for path in paths:
        out = subprocess.run(['git', 'show', '%s:%s' % (rev, path)], capture_output=True)
        if out.returncode == 0:
            return out.stdout.decode()
    return None


def _utc_date(iso):
    return datetime.fromisoformat(iso).astimezone(timezone.utc).strftime('%Y-%m-%d')


@functools.lru_cache(maxsize=None)
def _subject(sha):
    """A save's commit subject, or the pull request title a merge carries in its body."""
    subject, _, body = _git('show', '-s', '--format=%s%n%b', sha).partition('\n')
    if subject.startswith('Merge'):
        for line in body.split('\n'):
            if line.strip():
                return line.strip()
    return subject


def history(arch, rev='HEAD'):
    """(anchor, [save, ...]) for one architecture, or None where `rev` predates the anchor.

    Read from git with nothing checked out: first-parent commits after the anchor that touched the
    file under any of its names, skipping any whose text is unchanged."""
    sha, path = ANCHORS[arch]
    if subprocess.run(['git', 'merge-base', '--is-ancestor', sha, rev]).returncode != 0:
        return None
    anchor = parse(_show(sha, (path,)))
    anchor.update(sha=sha, commit=sha[:9], date=_utc_date(_git('show', '-s', '--format=%cI', sha).strip()),
                  subject='the anchor')
    log = _git('log', '--first-parent', '--format=%H %cI', '%s..%s' % (sha, rev), '--',
               *PATHS[arch])
    saves, prev_text = [], _show(sha, (path,))
    for line in reversed([entry for entry in log.split('\n') if entry.strip()]):
        commit, when = line.split()
        text = _show(commit, PATHS[arch])
        if text is None or text == prev_text:
            continue
        state = parse(text)
        state.update(sha=commit, commit=commit[:9], date=_utc_date(when),
                     subject=_subject(commit))
        saves.append(state)
        prev_text = text
    return anchor, saves


# --- what explains a step, and what main measured ------------------------------------------------

def parse_attributions(text):
    """[{arch, commit, row, pct, cause}] from bench/attributions.tsv; a malformed line raises.

    `pct` is a float, or None for `*`: the whole step of the save `commit` names, which only a save
    can claim (a merge's share of a step must carry its number)."""
    out = []
    for n, line in enumerate(text.split('\n'), 1):
        if not line.strip() or line.startswith('#'):
            continue
        parts = line.split('\t')
        if len(parts) != 5 or not all(p.strip() for p in parts):
            raise ValueError('%s line %d: want arch, commit, row, percent, cause, tab-separated'
                             % (ATTRIBUTIONS, n))
        arch, commit, row, pct, cause = (p.strip() for p in parts)
        if arch not in ARCHES or len(commit) < 7:
            raise ValueError('%s line %d: unknown architecture or a commit under 7 characters'
                             % (ATTRIBUTIONS, n))
        try:
            value = None if pct == '*' else float(pct.rstrip('%'))
        except ValueError:
            raise ValueError('%s line %d: percent %r is neither a number nor *' % (ATTRIBUTIONS, n, pct))
        out.append({'arch': arch, 'commit': commit, 'row': row, 'pct': value, 'cause': cause})
    return out


def step_explained(lines, arch, row, save, contains):
    """The factor a step's records account for: the whole step (returned as None), or the product of
    the numbered lines whose commit falls in the step. `save` is the full SHA of the save that ends
    the step, or None for `main`'s gap to the floor; `contains(commit)` says whether a commit is in
    the step's range."""
    factor = 1.0
    for line in lines:
        if line['arch'] != arch or line['row'] not in (row, '*'):
            continue
        if line['pct'] is None:
            if save and save.startswith(line['commit']):
                return None
            continue
        if contains(line['commit']):
            factor *= 1.0 + line['pct'] / 100.0
    return factor


def unexplained(arch, anchor, saves, reading, lines, contains):
    """Per row of `reading` ({row: (ticks, iters)}, `main` at one commit), its drift since the anchor
    and the part no line explains, both in percent, plus each unexplained save step.

    `contains(commit, a, b)` says whether `commit` is in the range (a, b], by full SHA; `b` is None
    for the reading's own commit. A row the last floor does not carry is not reported."""
    states = [anchor] + list(saves)
    last = states[-1]
    restamp = restamp_terms(last['why'])
    # A step's range starts at the last state that measured the tree. A restamp rewrites the file
    # without measuring anything (its rows are its predecessor's), so a merge that landed before a
    # restamp still belongs to the next save's step.
    measured = [states[0]['sha']]
    for prev_state, state in zip(states, states[1:]):
        measured.append(state['sha'] if state['rows'] != prev_state['rows'] else measured[-1])
    out = {}
    for name, now in reading.items():
        if name not in last['rows']:
            continue
        base, prev, left, open_steps = None, None, 1.0, []
        for n, state in enumerate(states):
            cell = state['rows'].get(name)
            if cell is None:
                prev = None
                continue
            if base is None:
                base = cell
            elif prev is not None and per_iter(cell) != per_iter(prev):
                step = per_iter(cell) / per_iter(prev)
                a, b = measured[n - 1], state['sha']
                factor = step_explained(lines, arch, name, b, lambda c: contains(c, a, b))
                if factor is not None:
                    left *= step / factor
                    if step / factor != 1.0:
                        open_steps.append((state, (step / factor - 1.0) * 100.0))
            prev = cell
        gap = per_iter(now) / per_iter(last['rows'][name])
        floor_sha = measured[-1]
        factor = step_explained(lines, arch, name, None, lambda c: contains(c, floor_sha, None))
        factor *= 1.0 + restamp.get(name, 0.0) / 100.0
        left *= gap / factor
        out[name] = {'main': pct(base, now), 'gap': (gap - 1.0) * 100.0,
                     'unexplained': (left - 1.0) * 100.0, 'open_steps': open_steps}
    return out


def save_accounts(lines, arch, prev, save, contains):
    """What a save's step on each moved row is: 'all', 'part' or 'none' explained, per row."""
    out = {}
    for name, cell in save['rows'].items():
        before = prev['rows'].get(name)
        if before is None or per_iter(before) == per_iter(cell):
            continue
        a, b = prev.get('measured', prev['sha']), save['sha']
        factor = step_explained(lines, arch, name, b, lambda c: contains(c, a, b))
        out[name] = ('all' if factor is None
                     else 'none' if factor == 1.0
                     else 'part')
    return out


@functools.lru_cache(maxsize=None)
def _ancestor(commit, of):
    return subprocess.run(['git', 'merge-base', '--is-ancestor', commit, of],
                          capture_output=True).returncode == 0


def make_contains(reading_sha):
    """`contains(commit, a, b)` over git: in (a, b] when an ancestor of b and not of a."""
    def contains(commit, a, b):
        b = b or reading_sha
        return _ancestor(commit, b) and not _ancestor(commit, a)
    return contains


def read_attributions():
    try:
        with open(ATTRIBUTIONS) as f:
            return parse_attributions(f.read())
    except FileNotFoundError:
        return []


def parse_readings(text):
    """{week: (commit, {arch: {row: (ticks, iters)}})} from the readings CSV."""
    out = {}
    for rec in csv.DictReader(io.StringIO(text)):
        commit, arches = out.setdefault(rec['week'], (rec['commit'], {}))
        if rec['commit'] != commit:
            raise ValueError('%s: week %s names two commits' % (READINGS, rec['week']))
        arches.setdefault(rec['arch'], {})[rec['row']] = (int(rec['ticks']), int(rec['iters']))
    return out


def read_readings():
    try:
        with open(READINGS) as f:
            return parse_readings(f.read())
    except FileNotFoundError:
        return {}


def check_whole_step_lines(lines, saves_by_arch):
    """Refuse a `*` line whose commit is no save of its architecture: it would explain nothing,
    silently, and the reader of the file would believe it did. Checked against HEAD's saves, since
    an earlier week's history lacks the saves after it."""
    bad = []
    for line in lines:
        if line['pct'] is None and not any(s['sha'].startswith(line['commit'])
                                           for s in saves_by_arch.get(line['arch'], ())):
            bad.append('%s %s %s' % (line['arch'], line['commit'], line['row']))
    if bad:
        raise SystemExit('%s: a whole-step line (percent *) must name a save of its architecture: %s'
                         % (ATTRIBUTIONS, '; '.join(bad)))


@functools.lru_cache(maxsize=None)
def week_result(week):
    """{arch: unexplained(...)} for the week's reading, or None where the week has none."""
    readings = read_readings()
    if week not in readings:
        return None
    commit, arches = readings[week]
    lines = read_attributions()
    contains = make_contains(commit)
    out = {}
    for arch in ARCHES:
        found = history(arch, commit)
        if found is None or arch not in arches:
            continue
        anchor, saves = found
        out[arch] = unexplained(arch, anchor, saves, arches[arch], lines, contains)
    return out


def week_cells(result):
    """One architecture's cells: the largest unexplained drift (never below zero), rows past 5%."""
    ups = [r['unexplained'] for r in result.values()]
    return round(max([0.0] + ups), 2), sum(1 for u in ups if u > WATCH_PCT)


def series_cells(week):
    """{column: cell} for `script/metrics`' unexplained-drift measure in `week`; empty if unread."""
    result = week_result(week)
    out = {}
    for arch, rows in (result or {}).items():
        top, over = week_cells(rows)
        out['unexplained_drift_max_pct_' + arch] = '%.2f' % top
        out['unexplained_drift_rows_over_5pct_' + arch] = over
    return out


# --- fetching what main measured ----------------------------------------------------------------

def _gh(path, binary=False):
    for _ in range(3):
        done = subprocess.run(['gh', 'api', path], capture_output=True)
        if done.returncode == 0:
            return done.stdout if binary else json.loads(done.stdout)
    raise RuntimeError('gh api %s: %s' % (path, done.stderr.decode(errors='replace').strip()))


def week_of(iso):
    year, week, _day = datetime.fromisoformat(iso).astimezone(timezone.utc).isocalendar()
    return '%04dW%02d' % (year, week)


def parse_artifact_tsv(text):
    """{arch: {row: (ticks, iters)}} from the bench job's `arch row ticks iters` lines."""
    out = {}
    for line in text.split('\n'):
        parts = line.split('\t')
        if len(parts) == 4 and parts[0] in ARCHES and parts[2].isdigit() and parts[3].isdigit():
            out.setdefault(parts[0], {})[parts[1]] = (int(parts[2]), int(parts[3]))
    return out


def newest_per_week(main_commits, artifacts):
    """{week: (commit, artifact id)}: per week, the newest first-parent `main` commit that kept a
    reading. `main_commits` is [(sha, iso date)] newest first; `artifacts` is GitHub's list."""
    kept = {}
    for art in artifacts:
        sha = (art.get('workflow_run') or {}).get('head_sha')
        if sha and not art.get('expired'):
            kept.setdefault(sha, art['id'])
    out = {}
    for sha, when in main_commits:
        week = week_of(when)
        if week not in out and sha in kept:
            out[week] = (sha, kept[sha])
    return out


def to_readings_csv(readings):
    buf = io.StringIO()
    writer = csv.writer(buf, lineterminator='\n')
    writer.writerow(READING_FIELDS)
    for week in sorted(readings):
        commit, arches = readings[week]
        for arch in ARCHES:
            for row, (ticks, iters) in sorted(arches.get(arch, {}).items()):
                writer.writerow((week, commit, arch, row, ticks, iters))
    return buf.getvalue()


def update():
    """Record each week's reading of `main` that GitHub still keeps, newest merge per week.

    A week already recorded is rewritten only when a newer merge of that week kept a reading, so a
    week whose artifacts have expired keeps what was read while they lived."""
    artifacts, page = [], 1
    while True:
        got = _gh('repos/%s/actions/artifacts?name=%s&per_page=100&page=%d' % (REPO, ARTIFACT, page))
        artifacts += got.get('artifacts', [])
        if len(got.get('artifacts', [])) < 100:
            break
        page += 1
    log = _git('log', '--first-parent', '--since=100 days ago', '--format=%H %cI', 'HEAD')
    main_commits = [tuple(line.split()) for line in log.split('\n') if line.strip()]
    order = {sha: n for n, (sha, _when) in enumerate(main_commits)}
    readings = read_readings()
    for week, (sha, art) in sorted(newest_per_week(main_commits, artifacts).items()):
        have = readings.get(week)
        if have and order.get(have[0], len(order)) <= order[sha]:
            continue
        archive = zipfile.ZipFile(io.BytesIO(_gh('repos/%s/actions/artifacts/%d/zip' % (REPO, art),
                                                 binary=True)))
        arches = parse_artifact_tsv(archive.read(ARTIFACT + '.tsv').decode())
        if set(arches) != set(ARCHES):
            print('%s: %s kept readings for %s only; skipped' % (week, sha[:9], sorted(arches)),
                  file=sys.stderr)
            continue
        readings[week] = (sha, arches)
        print('%s: main at %s' % (week, sha[:9]))
    with open(READINGS, 'w') as f:
        f.write(to_readings_csv(readings))
    return 0


# --- the report ---------------------------------------------------------------------------------

def _p(value):
    return '%+.2f%%' % value


def _cell_text(text):
    return text.replace('|', '\\|').replace('\n', ' ')


# The report's table headings. helpers/prose_ratchet.py skips a table under any heading in its
# GENERATED_TABLES, because nobody can cut words a script writes, and the selftest fails if one of
# these is missing there. Fixed strings for that reason: an anchor date in a heading would break it.
def table_headings(arch):
    return ('## %s: drift per row since the anchor' % arch,
            '## %s: every save since the anchor, and its reason' % arch)


TABLE_HEADINGS = tuple(h for arch in ARCHES for h in table_headings(arch))


def render(reports, stamp, week=None, main=None, contains=None, lines=()):
    """The appendix page. `reports` is [(arch, anchor, saves)]; `stamp` names the tree it read.
    `main` is `week_result(week)` for the newest reading, or None; `contains` and `lines` say which
    saves are accounted for."""
    anchors = ', '.join('%s `%s` (%s)' % (arch, anchor['commit'], anchor['date'])
                        for arch, anchor, _saves in reports)
    out = [
        '# Cumulative icount drift since the anchor',
        '',
        '*Generated by `script/metrics` from git history (helpers/baseline_drift.py), read %s. '
        'Do not edit it by hand. An appendix to [notes/project-metrics.md](../project-metrics.md), '
        'for milestone 415 item 3. calef ruled on 2026-09-26 that this is a report and not a gate '
        '(§190).*' % stamp,
        '',
        'Every figure is per iteration, against a fixed anchor rather than the last floor. The '
        'anchors are the 2026-09-15 audit\'s: %s. `floor` is the committed number. `restamp` is '
        'the compiler term `--restamp` has carried since the last `--save`, and `combined` is both. '
        'The tripwire is 10%% per save.' % anchors,
        '',
        '`on main` is %s against the anchor, and `unexplained` is the part of it no line in '
        '[bench/attributions.tsv](../../bench/attributions.tsv) accounts for (calef, 2026-10-22, '
        '#2554). The weekly chart plots the largest `unexplained` per architecture; past 5%% is a '
        'lane. `accounted for` says which rows of each save\'s step a line explains.'
        % ('the merge queue\'s reading in %s, `main` at `%s`'
           % (week, read_readings()[week][0][:9]) if main else 'not measured yet'),
    ]
    for arch, anchor, saves in reports:
        result = cumulative(anchor, saves)
        on_main = (main or {}).get(arch, {})
        rows_heading, saves_heading = table_headings(arch)
        out += ['', rows_heading, '',
                '| row | since | floor | restamp | combined | on main | unexplained | '
                'saves that moved it | largest step | steps over 10% |',
                '|---|---|---:|---:|---:|---:|---:|---:|---|---:|']
        def order(kv):
            return -on_main.get(kv[0], {}).get('unexplained', kv[1]['combined'])
        for name, r in sorted(result.items(), key=order):
            since = 'anchor' if r['at_anchor'] else r['since']
            step = '%s (%s)' % (_p(r['largest']), r['largest_date']) if r['moves'] else '-'
            m = on_main.get(name)
            out.append('| `%s` | %s | %s | %s | %s | %s | %s | %d | %s | %d |'
                       % (name, since, _p(r['floor']), _p(r['restamp']), _p(r['combined']),
                          _p(m['main']) if m else '-', _p(m['unexplained']) if m else '-',
                          r['moves'], step, r['tripped']))
        out += ['', saves_heading, '',
                '| date | commit | rows moved | largest | accounted for | reason |',
                '|---|---|---:|---|---|---|']
        prev = anchor
        for save in saves:
            moved, largest = save_summary(prev, save)
            big = '`%s` %s' % (largest[0], _p(largest[1])) if largest else '-'
            fresh = new_reasons(prev, save)
            if fresh:
                reason = '<br>'.join('`# why:` ' + _cell_text(w) for w in fresh)
            elif save['why']:
                reason = 'no new `# why:` line; commit: ' + _cell_text(save['subject'])
            else:
                reason = 'no `# why:` ledger; commit: ' + _cell_text(save['subject'])
            accounted = '-'
            if moved and contains:
                accounted = _accounted(save_accounts(lines, arch, prev, save, contains))
            out.append('| %s | `%s` | %d | %s | %s | %s |'
                       % (save['date'], save['commit'], moved, big, accounted, reason))
            save['measured'] = (save['sha'] if save['rows'] != prev['rows']
                                else prev.get('measured', prev['sha']))
            prev = save
    return '\n'.join(out) + '\n'


def _accounted(rows):
    """A save's `accounted for` cell: every row, none, or which rows are and are not."""
    kinds = set(rows.values())
    if kinds == {'all'}:
        return 'every row'
    if kinds == {'none'}:
        return 'none'
    named = lambda k: ', '.join('`%s`' % n for n in sorted(rows) if rows[n] == k)
    parts = [('whole: ' + named('all')) if 'all' in kinds else '',
             ('in part: ' + named('part')) if 'part' in kinds else '',
             ('none: ' + named('none')) if 'none' in kinds else '']
    return '; '.join(p for p in parts if p)


def new_reasons(prev, save):
    """The `# why:` lines this save added, in order: the ones its predecessor did not carry.

    A baseline file keeps every `# why:` line since its last `--save`, and the nightly
    `--restamp` appends one a day, so a save's whole list repeats every line the rows above it
    already showed. Rendering the whole list grew the newest cell by a restamp line a day, and on
    2026-10-02 one cell reached 2,472 bytes, past `documentation::LINE_MAX` (2048), which a line in
    the shipped manual must fit. A row says why THAT save moved; the earlier reasons are on the rows
    above it.
    """
    seen = set(prev.get('why', ())) if prev else set()
    return [w for w in save['why'] if w not in seen]


def status(week, result):
    """One line for the page: the rows past 5% unexplained on `main`, and each architecture's
    largest, or that the week has no reading."""
    if result is None:
        return '%s: `main` not measured yet.' % week
    commit = read_readings()[week][0][:9]
    over, tops = [], []
    for arch in ARCHES:
        rows = result.get(arch, {})
        for name, r in sorted(rows.items(), key=lambda kv: -kv[1]['unexplained']):
            if r['unexplained'] > WATCH_PCT:
                over.append('%s `%s` %s' % (arch, name, _p(r['unexplained'])))
        if rows:
            name, r = max(rows.items(), key=lambda kv: kv[1]['unexplained'])
            tops.append('%s `%s` %s' % (arch, name, _p(r['unexplained'])))
    head = '%s, `main` at `%s`: ' % (week, commit)
    if over:
        return head + 'unexplained past 5%%: %s.' % ', '.join(over)
    return head + 'nothing unexplained past 5%%; the largest, %s.' % ', '.join(tops)


def report(rev='HEAD'):
    reports = [(arch, *history(arch, rev)) for arch in ARCHES]
    sha = _git('rev-parse', '--short=9', rev).strip()
    return reports, 'at `%s`' % sha


def page(rev='HEAD'):
    """(the page's status line, the appendix) for `rev` and the newest week `main` was read."""
    reports, stamp = report(rev)
    check_whole_step_lines(read_attributions(), {arch: saves for arch, _a, saves in reports})
    readings = read_readings()
    week = max(readings) if readings else None
    result = week_result(week) if week else None
    contains = make_contains(_git('rev-parse', rev).strip())
    body = render(reports, stamp, week, result, contains, read_attributions())
    return (status(week, result) if week else '`main` not measured yet.'), body


# --- selftest -----------------------------------------------------------------------------------

def unexplained_cases(state):
    """The ruled definition (#2554) against fixtures: each case is a way a wrong reading misleads.

    Commits are the fixture dates; `order` stands in for first-parent history, so a commit is in a
    step's range when it falls after the step's start and at or before its end."""
    order = ['m0', 'd0', 'm1', 'd1', 'm2', 'd2', 'm3', 'main']
    contains = lambda c, a, b: order.index(a) < order.index(c) <= order.index(b or 'main')

    def line(commit, row, pct, arch='aarch64'):
        return {'arch': arch, 'commit': commit, 'row': row, 'pct': pct, 'cause': 'c'}

    anchor = state('d0', 'a 1000 10\nb 1000 10\n')
    absorbed = state('d1', 'a 1100 10\nb 1050 10\n')    # +10% and +5%, nothing separated
    reading = {'a': (1100, 10), 'b': (1050, 10)}

    def u(lines, saves=(absorbed,), main=None):
        got = unexplained('aarch64', anchor, list(saves), main or reading, lines, contains)
        return {k: round(v['unexplained'], 4) for k, v in got.items()}

    def refused(text):
        try:
            parse_attributions(text)
        except ValueError:
            return True
        return False

    def whole_on_a_merge():
        try:
            check_whole_step_lines([line('m1', '*', None)], {'aarch64': [absorbed]})
        except SystemExit:
            return True
        return False

    readings = {'2026W43': ('c' * 40, {'aarch64': {'a': (5, 1)}, 'riscv64': {'a': (6, 2)}})}
    return [
        ('a save that absorbs drift unseparated leaves its step unexplained', u([]),
         {'a': 10.0, 'b': 5.0}),
        ('a whole-step line on the save explains every row it names', u([line('d1', '*', None)]),
         {'a': 0.0, 'b': 0.0}),
        ('a whole-step line names one row and not the other', u([line('d1', 'a', None)]),
         {'a': 0.0, 'b': 5.0}),
        ('a merge inside the step explains its number and no more', u([line('m1', 'a', 4.0)]),
         {'a': round((1.10 / 1.04 - 1) * 100, 4), 'b': 5.0}),
        ('a merge before the anchor explains nothing', u([line('m0', 'a', 10.0)]),
         {'a': 10.0, 'b': 5.0}),
        ('a merge after the floor explains main\'s gap',
         u([line('d1', '*', None), line('m3', 'a', 5.0)], main={'a': (1155, 10), 'b': (1050, 10)}),
         {'a': 0.0, 'b': 0.0}),
        ('an explained improvement does not cancel an unexplained cost',
         u([line('d1', 'a', None)], saves=(state('d1', 'a 970 10\nb 1000 10\n'),),
           main={'a': (1024, 10), 'b': (1000, 10)}),
         {'a': round((1024 / 970 - 1) * 100, 4), 'b': 0.0}),
        ('the restamp term explains its share of the gap',
         u([line('d1', '*', None)], saves=(state('d1', 'a 1100 10\nb 1050 10\n',
                                                 why=['%s a +1.000%%' % RESTAMP_MARKER]),),
           main={'a': (1111, 10), 'b': (1050, 10)}),
         {'a': 0.0, 'b': 0.0}),
        ('a merge before a restamp still belongs to the next save\'s step',
         u([line('m1', 'a', 10.0)], saves=(state('m2', 'a 1000 10\nb 1000 10\n'), absorbed)),
         {'a': 0.0, 'b': 5.0}),
        ('a whole-step line must name a save', whole_on_a_merge(), True),
        ('an attribution line parses, * as the whole step',
         [(l['commit'], l['pct']) for l in parse_attributions(
             '# c\naarch64\tabcdef12\t*\t*\twhy\nriscv64\t1234567\tx\t+1.5\twhy\n')],
         [('abcdef12', None), ('1234567', 1.5)]),
        ('a line missing its cause is refused', refused('aarch64\tabcdef12\tx\t1.0\n'), True),
        ('an unknown architecture is refused', refused('arm\tabcdef12\tx\t1.0\tc\n'), True),
        ('the readings file round-trips', parse_readings(to_readings_csv(readings)), readings),
        ('the bench artifact parses', parse_artifact_tsv('x86_64\ta\t10\t2\nbad\n'),
         {'x86_64': {'a': (10, 2)}}),
        ('per week, the newest main commit that kept a reading, never an expired one',
         newest_per_week([('s3', '2026-10-21T00:00:00+00:00'), ('s2', '2026-10-20T00:00:00+00:00'),
                          ('s1', '2026-10-13T00:00:00+00:00')],
                         [{'id': 7, 'expired': True, 'workflow_run': {'head_sha': 's3'}},
                          {'id': 6, 'workflow_run': {'head_sha': 's2'}},
                          {'id': 5, 'workflow_run': {'head_sha': 's1'}},
                          {'id': 4, 'workflow_run': {'head_sha': 'pr'}}]),
         {'2026W43': ('s2', 6), '2026W42': ('s1', 5)}),
    ]


def selftest():
    """The arithmetic against fixtures: each case is a way a wrong reader would mislead."""
    import prose_ratchet
    def state(date, body, why=()):
        text = ''.join('# why: %s\n' % w for w in why) + '# header\n' + body
        s = parse(text)
        s.update(date=date, commit=date, sha=date, subject='s')
        return s

    anchor = state('d0', 'a 1000 10\nb 500 5\ngone 1 1\n')
    # An iteration-count change with the same per-iteration cost reads as zero.
    s1 = state('d1', 'a 2000 20\nb 525 5\n')
    # Two ~4% steps on `a`: neither trips, together they are +8.15%.
    s2 = state('d2', 'a 2080 20\nb 525 5\nnew 100 1\n')
    s3 = state('d3', 'a 2163 20\nb 525 5\nnew 150 1\n')
    same = state('d4', 'a 2163 20\nb 525 5\nnew 150 1\n',
                 why=['restamped; %s a +1.000%%, new -0.500%%' % RESTAMP_MARKER])
    r = cumulative(anchor, [s1, s2, s3, same])
    cases = [
        ('parse skips comments and the indented header',
         parse('# why: x\n   # Updating\nrow 10 2\nbad line\n'),
         {'rows': {'row': (10, 2)}, 'why': ['x']}),
        ('iteration count change is zero', round(pct((1000, 10), (2000, 20)), 9), 0.0),
        ('per-iteration, not per-total', round(pct((1000, 10), (1100, 10)), 6), 10.0),
        ('compounding', round(compound(10.0, 10.0), 6), 21.0),
        ('steps accumulate past the tripwire', round(r['a']['floor'], 2), 8.15),
        ('no step tripped', r['a']['tripped'], 0),
        ('moves counted, an unchanged save is not a move', r['a']['moves'], 2),
        ('a row absent at the anchor starts at its birth', (r['new']['at_anchor'], r['new']['since']),
         (False, 'd2')),
        ('a born row measures from its birth', round(r['new']['floor'], 6), 50.0),
        ('a removed row is not reported', 'gone' in r, False),
        ('restamp term read from the ledger', r['a']['restamp'], 1.0),
        ('restamp compounds onto the floor', round(r['a']['combined'], 4),
         round(compound(r['a']['floor'], 1.0), 4)),
        ('a row the ledger omits has no term', r['b']['restamp'], 0.0),
        ('the last ledger line wins',
         restamp_terms(['%s a +1.000%%' % RESTAMP_MARKER, '%s a +2.500%%' % RESTAMP_MARKER]),
         {'a': 2.5}),
        ('an empty ledger is none', restamp_terms(['%s none' % RESTAMP_MARKER]), {}),
        ('largest step is signed', round(cumulative(state('x', 'a 100 1\n'),
                                                    [state('y', 'a 80 1\n'),
                                                     state('z', 'a 84 1\n')])['a']['largest'], 6),
         -20.0),
        ('a step over 10% is counted', cumulative(state('x', 'a 100 1\n'),
                                                  [state('y', 'a 111 1\n')])['a']['tripped'], 1),
        ('the largest unexplained is never below zero, and counts rows past 5%',
         week_cells({'a': {'unexplained': -9.0}, 'b': {'unexplained': 6.0},
                     'c': {'unexplained': 2.0}}), (6.0, 1)),
        ('an all-negative week reads zero', week_cells({'a': {'unexplained': -3.0}}), (0.0, 0)),
        ('save summary', save_summary(anchor, s1), (1, ('b', 5.000000000000004))),
        ('a save shows only the reasons it added',
         new_reasons(state('p', '', why=['x', 'y']), state('q', '', why=['x', 'y', 'z'])), ['z']),
        ('a fresh ledger is all new',
         new_reasons(state('p', '', why=['x']), state('q', '', why=['w'])), ['w']),
        ('the gate skips every generated table',
         sorted(set(TABLE_HEADINGS) - prose_ratchet.GENERATED_TABLES), []),
    ]
    cases += unexplained_cases(state)
    failed = [(name, got, want) for name, got, want in cases if got != want]
    for name, got, want in failed:
        print('baseline drift selftest: %s: got %r, want %r' % (name, got, want), file=sys.stderr)
    if failed:
        return 1
    print('baseline drift selftest: %d cases pass' % len(cases))
    return 0


# The 2026-09-15 audit's five cumulative figures (notes/benchmarks/baseline-save-audit.md), at the
# merge that carried the last save it walked. Reproducing them from the same anchors is the check
# that this module and the audit mean the same arithmetic. It needs full history, so it is a mode
# of its own rather than part of `--selftest`, which script/lint runs on whatever clone CI has.
AUDIT_REV = '770f44983'
AUDIT = [('riscv64', 'ctx_switch', 10.78), ('aarch64', 'yield_switch', 9.16),
         ('riscv64', 'ipc_rtt', 9.25), ('aarch64', 'ipc_rtt', 8.73),
         ('x86_64', 'yield_switch', 9.94)]


def audit():
    bad = 0
    for arch, row, want in AUDIT:
        got = round(cumulative(*history(arch, AUDIT_REV))[row]['floor'], 2)
        print('%s %s: %+.2f%% (audit: %+.2f%%)' % (arch, row, got, want))
        bad += got != want
    return 1 if bad else 0


def main(argv):
    if argv and argv[0] == '--selftest':
        return selftest()
    if argv and argv[0] == '--audit':
        return audit()
    if argv and argv[0] == '--update':
        return update()
    if argv:
        print(__doc__.split('\n\n')[1], file=sys.stderr)
        return 2
    sys.stdout.write(page()[1])
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
