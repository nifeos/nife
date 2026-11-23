"""The booted-row floor checker for an outsider pass at the confinement claim (`script/booted-rows`).

Milestone 920 (an outsider pass is launched pinned and floor-checked). A pass counts toward fatal
risk 7's criterion (c) only if it boots every row a boot can reach, and that count was taken by
hand three times running and came out wrong each time. This takes it from the tree and the logs.

    script/booted-rows --checkout DIR --pinned SHA --log FILE [--log FILE..] [--results FILE]
    script/booted-rows --from-table NOTE.md     # replay a recorded pass from its per-row table
    script/booted-rows --short-only ...         # only the short rows, the completion round's message
    script/booted-rows --selftest

Name: provisional, minted by milestone 920's lane on 2026-10-14 (UTC). A shared python module
under `helpers/`, which `script/names` puts out of its own scope, so its provenance is this
paragraph; the entry point's `Name:` block is in `script/booted-rows`.

**The rules are the brief's**, `briefs/confinement-outsider-pass.md` under "The booted-row floor",
and the exempt rows, the rows carried from pass 7 and the rows one ISA alone can reach are read
from the indented block there, so a growing table moves the count without an edit here. The row
total is the claims table in `notes/confinement-claims.md`.

- A pass-written test is a `#[test_case]` or `#[test]` function present at the pass's head and
  absent from the same file at the pinned commit. Anything else that ran is a rerun.
- Its rows are an `Attacks rows: 4, 5, 9` line (ranges as `6 to 9`) in the function's own doc
  comment, or else in its file's `//!` header.
- A row is booted when one of its pass-written `#[test_case]` tests reached `ok` or a `[PANIC]`
  under QEMU on an ISA the row applies to. A test that started and never finished, a `WATCHDOG`
  line or a log that ends mid-test, is inconclusive. A `skipped:` line is not a run.
- A carried row is also booted when the results file holds `Accepted row N: <why>`.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import record_root  # noqa: E402

ISAS = ('aarch64', 'riscv64', 'x86_64')
FLOOR_KEYS = {'exempt': 'exempt', 'carried from pass 7': 'carried', 'x86_64 only': 'x86_64_only'}
FLOOR_LINE = re.compile(r'^ {4}(exempt|carried from pass 7|x86_64 only): (.+)$')
TAG = re.compile(r'Attacks rows:\s*([0-9][0-9 ,toand]*)')
ACCEPTED = re.compile(r'^\s*(?:[-*]\s*)?Accepted row (\d+):\s*\S')
TEST_LINE = re.compile(r'^test (\S+) \.\.\. ?(.*)$')
# The banner's parenthetical has carried the emulator version since milestone 414 (a red
# post-run check does not say which emulator produced it). A version starts with a digit, so
# x86_64's machine word ("QEMU q35, ...") still does not match and read_log keeps the
# fallback-ISA behavior it has always had for that leg.
LEG_LINE = re.compile(r'^--- kernel tests, (\w+) \(QEMU(?: \d[\w.]*)?\) ---')


def parse_rows(text):
    """`4, 5`, `6 to 9`, `27 to 29, 33 and 34`, as a sorted list of row numbers."""
    rows = set()
    for part in re.split(r',|\band\b', text):
        bounds = [int(n) for n in re.findall(r'\d+', part)]
        if len(bounds) == 1:
            rows.add(bounds[0])
        elif len(bounds) == 2 and ' to ' in part:
            rows.update(range(bounds[0], bounds[1] + 1))
        elif bounds:
            rows.update(bounds)
    return sorted(rows)


def read_floor(brief_text, claims_text):
    """The floor's inputs: every row in the claims table, and the brief's exempt and carried sets."""
    floor = {'exempt': [], 'carried': [], 'x86_64_only': []}
    for line in brief_text.splitlines():
        found = FLOOR_LINE.match(line)
        if found:
            floor[FLOOR_KEYS[found.group(1)]] = parse_rows(found.group(2))
    rows = {}
    in_claims = False
    for line in claims_text.splitlines():
        if line.startswith('## '):
            in_claims = line.strip() == '## The claims'
        found = re.match(r'^\| (\d+) \|', line)
        if in_claims and found:
            rows[int(found.group(1))] = line
    if not rows or not floor['exempt']:
        raise SystemExit('booted-rows: no claims table or no exempt rows; read the brief\'s floor section')
    floor['rows'] = rows
    return floor


def tests_in(source):
    """{function name: (kind, rows)} for every test function in one Rust file's text."""
    header = []
    for line in source.splitlines():
        if line.startswith('//!'):
            header.append(line)
    module_rows = tag_rows('\n'.join(header))
    found = {}
    doc, kind = [], None
    for line in source.splitlines():
        stripped = line.strip()
        if stripped.startswith('///'):
            doc.append(stripped)
        elif stripped in ('#[test_case]', '#[test]'):
            kind = 'boot' if stripped == '#[test_case]' else 'host'
        elif stripped.startswith('#['):
            continue
        else:
            name = re.match(r'(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)', stripped)
            if name and kind:
                found[name.group(1)] = (kind, tag_rows('\n'.join(doc)) or module_rows)
            doc, kind = [], None
    return found


def tag_rows(text):
    found = TAG.search(text)
    return parse_rows(found.group(1)) if found else []


def git(checkout, *args, check=True):
    done = subprocess.run(['git', '-C', checkout, *args], capture_output=True, text=True)
    if check and done.returncode != 0:
        raise SystemExit('booted-rows: git %s: %s' % (' '.join(args), done.stderr.strip()))
    return done


def pass_tests(checkout, pinned, head):
    """{(file stem, function): (kind, rows, written)} over every .rs file the pass's diff touches."""
    changed = git(checkout, 'diff', '--name-only', pinned, head, '--', '*.rs').stdout.split()
    tests = {}
    for path in changed:
        now = git(checkout, 'show', '%s:%s' % (head, path), check=False)
        if now.returncode != 0:
            continue
        before = git(checkout, 'show', '%s:%s' % (pinned, path), check=False)
        old = tests_in(before.stdout) if before.returncode == 0 else {}
        stem = os.path.splitext(os.path.basename(path))[0]
        for name, (kind, rows) in tests_in(now.stdout).items():
            tests[(stem, name)] = (kind, rows, name not in old)
    return tests


def read_log(text, fallback_isa=None):
    """[(isa, segments, verdict)] for every kernel test a `script/test` log started."""
    runs, isa, pending = [], fallback_isa, None
    for line in text.splitlines():
        leg = LEG_LINE.match(line)
        if leg:
            isa = leg.group(1)
        started = TEST_LINE.match(line)
        if started:
            if pending:
                runs.append((isa, pending, 'ok'))
            pending = started.group(1).split('::')
            line = started.group(2)
        if not pending:
            continue
        verdict = line_verdict(line.strip())
        if verdict:
            runs.append((isa, pending, verdict))
            pending = None
    if pending:
        runs.append((isa, pending, 'inconclusive'))
    return runs


def line_verdict(line):
    """What one line says about the test in progress, or None if it says nothing."""
    if line == 'ok' or line.endswith(' ok') or line.startswith('test result:'):
        return 'ok'
    if line.startswith('skipped'):
        return 'skipped'
    if line.startswith('[PANIC]'):
        return 'assertion'
    if line.startswith('WATCHDOG'):
        return 'inconclusive'
    return None


def accepted_rows(results_text):
    return {int(m.group(1)) for m in map(ACCEPTED.match, results_text.splitlines()) if m}


def grade_row(row, floor, tests, runs, accepted):
    """`(booted?, reason)` for one non-exempt row, from the pass's tests and the logs."""
    applies = ('x86_64',) if row in floor['x86_64_only'] else ISAS
    written = [(key, kind) for key, (kind, rows, new) in tests.items() if new and row in rows]
    seen = {}
    for isa, segments, verdict in runs:
        for (stem, name), kind in written:
            if kind == 'boot' and segments[-1] == name and stem in segments:
                seen.setdefault(verdict, set()).add(isa)
    ran = (seen.get('ok', set()) | seen.get('assertion', set())) & set(applies)
    if ran:
        return True, 'booted %s' % ', '.join(sorted(ran))
    if row in floor['carried'] and row in accepted:
        return True, 'booted (pass 7\'s test, accepted in writing)'
    if seen.get('inconclusive'):
        return False, 'timed out on every ISA (inconclusive)'
    if written and all(kind == 'host' for _, kind in written):
        return False, 'host only'
    if written:
        return False, 'pass-written test never ran on an ISA the row applies to'
    if is_rerun(row, floor, tests, runs):
        return False, 'rerun of the row\'s cited test'
    return False, 'no pass-written test'


def is_rerun(row, floor, tests, runs):
    """Did a test that is not the pass's own run against this row, by tag or by the table's citation?"""
    cited = floor['rows'][row]
    for _, segments, verdict in runs:
        stem = segments[-2] if len(segments) > 1 else ''
        old = tests.get((stem, segments[-1]))
        if old and not old[2] and row in old[1]:
            return True
        # A citation names a function, or a whole module as `path::module` closed by a backtick.
        named = re.search(r'\b%s\b' % re.escape(segments[-1]), cited) or \
            (stem and re.search(r'\b%s`' % re.escape(stem), cited))
        if verdict != 'skipped' and named:
            return True
    return False


TABLE_REASONS = {'host': 'host only', 'rerun': 'rerun of the row\'s cited test', 'read': 'read only',
                 'untestable': 'no pass-written test (graded untestable)'}


def grade_table(note_text, floor):
    """{row: (booted?, reason)} from a pass note's per-row table, for a pass whose tests carry no tags."""
    grades, grade_col = {}, None
    for line in note_text.splitlines():
        # A cell's own `\|` (pass 9's `READ \| ENUMERATE`) is text, not a column break.
        cells = [c.strip() for c in re.split(r'(?<!\\)\|', line.strip().strip('|'))]
        if not line.startswith('|') or len(cells) < 2:
            grade_col = None if not line.startswith('|') else grade_col
            continue
        if cells[0] == '#' and 'Grade' in cells:
            grade_col = cells.index('Grade')
            continue
        if grade_col is None or not re.match(r'^\d', cells[0]):
            continue
        grade = cells[grade_col]
        word = grade.split(',')[0].split()[0].lower()
        for row in parse_rows(cells[0]):
            if word == 'booted' or (word == 'accepted' and row in floor['carried']):
                grades[row] = (True, grade)
            else:
                grades[row] = (False, TABLE_REASONS.get(word, 'graded %s' % grade))
    return grades


def report(floor, grades, short_only=False):
    """The lines to print and whether the floor is met."""
    lines, booted = [], 0
    needed = [r for r in sorted(floor['rows']) if r not in floor['exempt']]
    for row in sorted(floor['rows']):
        if row in floor['exempt']:
            if not short_only:
                lines.append('row %d: host (exempt)' % row)
            continue
        ok, reason = grades.get(row, (False, 'no pass-written test'))
        booted += ok
        if not (short_only and ok):
            lines.append('row %d: %s' % (row, reason))
    short = len(needed) - booted
    verdict = 'floor met' if short <= 0 else 'floor short by %d' % short
    if not short_only:
        lines.append('booted %d of %d, %s' % (booted, len(needed), verdict))
    return lines, short <= 0


def log_isa(path):
    for isa in ISAS:
        if isa in os.path.basename(path):
            return isa
    return None


def run(args):
    root = args.tree
    with open(os.path.join(root, 'briefs/confinement-outsider-pass.md')) as brief, \
            open(record_root.path('notes/confinement-claims.md', top=root)) as claims:
        floor = read_floor(brief.read(), claims.read())
    if args.from_table:
        with open(args.from_table) as note:
            grades = grade_table(note.read(), floor)
    else:
        if not (args.checkout and args.pinned and args.log):
            raise SystemExit('booted-rows: --checkout, --pinned and at least one --log are required')
        tests = pass_tests(args.checkout, args.pinned, args.head)
        runs = []
        for path in args.log:
            with open(path, errors='replace') as log:
                runs.extend(read_log(log.read(), log_isa(path)))
        accepted = set()
        if args.results:
            with open(args.results) as results:
                accepted = accepted_rows(results.read())
        grades = {row: grade_row(row, floor, tests, runs, accepted)
                  for row in floor['rows'] if row not in floor['exempt']}
    lines, met = report(floor, grades, args.short_only)
    print('\n'.join(lines))
    return 0 if met else 1


# ----------------------------------------------------------------------------------------------
# The selftest: fixtures, one git repository in a temporary directory, no QEMU.
# ----------------------------------------------------------------------------------------------

BRIEF_FIXTURE = """## The booted-row floor

    exempt: 3
    carried from pass 7: 1
    x86_64 only: 5
"""
CLAIMS_FIXTURE = """## The claims

| # | The claim | Stated in | Tested by | Falsified |
|---|---|---|---|---|
| 1 | one | x | `old::one_holds` | yes |
| 2 | two | x | `kernel::user::cited_tests::two_holds` | yes |
| 3 | three | x | `host::three` | yes |
| 4 | four | x | `four_holds` | yes |
| 5 | five | x | `ports::five_holds` | yes |
| 6 | six | x | `six_holds` | yes |
| 7 | seven | x | `seven_holds` | yes |
"""
OLD_TESTS = """//! Attacks rows: 2
#[test_case]
fn two_holds() {}
"""
NEW_TESTS = """//! A pass's module.
//!
//! Attacks rows: 4, 6 to 7
#[test_case]
fn attack_four_six_seven() {}

/// Attacks rows: 5
#[test_case]
fn attack_five() {}
"""
HOST_TESTS = """#[cfg(test)]
mod tests {
    /// Attacks rows: 6
    #[test]
    fn six_on_the_host() {}
}
"""


def selftest_repo(tmp):
    """A two-commit repository: the pinned commit, then a pass that adds tests."""
    def commit(files, message):
        for path, text in files.items():
            full = os.path.join(tmp, path)
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, 'w') as out:
                out.write(text)
        git(tmp, 'add', '-A')
        git(tmp, '-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', message)
        return git(tmp, 'rev-parse', 'HEAD').stdout.strip()
    git(tmp, 'init', '-q')
    pinned = commit({'system_tests/src/user/cited_tests.rs': OLD_TESTS}, 'pinned')
    commit({'system_tests/src/user/pass_tests.rs': NEW_TESTS,
            'crates/page_tables/src/lib.rs': HOST_TESTS}, 'the pass')
    return pinned


def selftest_logs():
    aarch64 = ('--- kernel tests, aarch64 (QEMU 11.1.2) ---\n'
               'test kernel::user::cited_tests::two_holds ... ok\n'
               'test kernel::user::pass_tests::attack_four_six_seven ... a line it printed\n'
               '[PANIC] panicked at an assertion\n')
    x86 = ('test kernel::user::pass_tests::attack_five ... ok\n'
           'test kernel::user::pass_tests::attack_four_six_seven ... ok\n')
    riscv64 = 'test kernel::user::pass_tests::attack_five ... ok\n'
    return aarch64, x86, riscv64


# The recorded passes the checker must reproduce (milestone 920's Done means): each note's own count.
REPLAYS = (('notes/confinement-outsider-pass-8.md', 25), ('notes/confinement-outsider-pass-9.md', 20))


def selftest_replays(tree):
    """Pass 8's and pass 9's per-row tables, replayed against this tree's brief and claims table."""
    with open(os.path.join(tree, 'briefs/confinement-outsider-pass.md')) as brief, \
            open(record_root.path('notes/confinement-claims.md', top=tree)) as claims:
        floor = read_floor(brief.read(), claims.read())
    for path, booted in REPLAYS:
        with open(record_root.path(path, top=tree)) as note:
            last = report(floor, grade_table(note.read(), floor))[0][-1]
        assert last.startswith('booted %d of ' % booted), (path, last)


def selftest(tree):
    selftest_replays(tree)
    floor = read_floor(BRIEF_FIXTURE, CLAIMS_FIXTURE)
    assert parse_rows('27 to 29, 33 and 34') == [27, 28, 29, 33, 34]
    assert floor['exempt'] == [3] and floor['carried'] == [1] and len(floor['rows']) == 7
    assert read_log('test a::b ... \nWATCHDOG: no progress\n') == [(None, ['a', 'b'], 'inconclusive')]
    assert read_log('test a::b ... skipped: no disk\n')[0][2] == 'skipped'
    assert read_log('test a::b ... ')[0][2] == 'inconclusive', 'a log that ends mid-test'
    # Both banner shapes attribute their leg: the versioned one `script/test` prints now
    # (milestone 414) and the bare one every log printed before it.
    assert read_log('--- kernel tests, aarch64 (QEMU 11.1.2) ---\ntest a::b ... ok\n')[0][0] == 'aarch64'
    assert read_log('--- kernel tests, aarch64 (QEMU) ---\ntest a::b ... ok\n')[0][0] == 'aarch64'
    with tempfile.TemporaryDirectory() as tmp:
        pinned = selftest_repo(tmp)
        tests = pass_tests(tmp, pinned, 'HEAD')
        assert tests[('pass_tests', 'attack_five')] == ('boot', [5], True)
        assert tests[('pass_tests', 'attack_four_six_seven')] == ('boot', [4, 6, 7], True)
        assert tests[('lib', 'six_on_the_host')] == ('host', [6], True)
        aarch64, x86, riscv64 = selftest_logs()
        runs = read_log(aarch64) + read_log(x86, 'x86_64') + read_log(riscv64, 'riscv64')
        grades = {r: grade_row(r, floor, tests, runs, {1}) for r in floor['rows'] if r != 3}
    assert grades[1] == (True, 'booted (pass 7\'s test, accepted in writing)'), grades[1]
    assert grades[2] == (False, 'rerun of the row\'s cited test'), grades[2]
    assert grades[4] == (True, 'booted aarch64, x86_64'), 'a [PANIC] is a run: ' + grades[4][1]
    assert grades[5] == (True, 'booted x86_64'), 'riscv64 is not an ISA row 5 applies to'
    lines, met = report(floor, grades)
    assert lines[-1] == 'booted 5 of 6, floor short by 1' and not met, lines
    assert report(floor, grades, short_only=True)[0] == ['row 2: rerun of the row\'s cited test']
    timed_out = read_log('test kernel::user::pass_tests::attack_five ... \n', 'x86_64')
    assert grade_row(5, floor, tests, timed_out, set())[1] == 'timed out on every ISA (inconclusive)'
    table = ('| # | Attack | Outcome | Grade |\n|---|---|---|---|\n| 1 | a | held | accepted |\n'
             '| 2, 4 | a `READ \\| ENUMERATE` | held | booted, three ISAs |\n| 5 to 6 | a | held | host |\n'
             '| 7 | a | held | rerun, aarch64 |\n')
    grades = grade_table(table, floor)
    assert report(floor, grades)[0][-1] == 'booted 3 of 6, floor short by 3'
    assert grades[7] == (False, 'rerun of the row\'s cited test') and grades[6] == (False, 'host only')
    print('booted-rows: selftest passed')
    return 0


def main(argv):
    parser = argparse.ArgumentParser(prog='script/booted-rows')
    parser.add_argument('--tree', default=os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    parser.add_argument('--checkout')
    parser.add_argument('--pinned')
    parser.add_argument('--head', default='HEAD')
    parser.add_argument('--log', action='append', default=[])
    parser.add_argument('--results')
    parser.add_argument('--from-table')
    parser.add_argument('--short-only', action='store_true')
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args(argv)
    return selftest(args.tree) if args.selftest else run(args)


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
