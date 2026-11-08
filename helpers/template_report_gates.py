"""The template's report-only naming and README gates, held to a baseline that only shrinks.

nife vendors, from nifeos/crate-workflows at the commit `vendor/crate-workflows.pin` names, six
gates that read a crate: `register_names.py`, `abbreviations.py`, `prose_acronyms.py`,
`readme_acronyms.py`, `inflated_wording.py` and `readme_sections.py`. In the template they report
(or, for the register and acronym gates, fail a crate at ceiling 0). nife has not swept its crates
for them, so this file runs each over every crate under `crates/` and holds the findings to
`helpers/template_report_baseline.txt`:

    python3 helpers/template_report_gates.py --check      # what script/lint runs
    python3 helpers/template_report_gates.py --list [CRATE..]  # every finding, crate: gate: line
    python3 helpers/template_report_gates.py --count      # the baseline's shape, from the tree
    python3 helpers/template_report_gates.py --bank       # lower the baseline to the tree; never raises
    python3 helpers/template_report_gates.py --selftest   # the counting and the comparison

**What fails.** A crate whose count for a gate is over the baseline for that crate and gate; a crate
with no entry has a baseline of 0. Existing findings never fail, and the check prints a line per
crate and gate that has any, so the backlog is visible. A count under its baseline passes and says
so, and `--bank` writes the lower number. `--bank` never raises a number and never adds an entry,
so the baseline only shrinks. Fixing a finding is a separate lane and renames nothing here.

**What each gate reads.** The crate's Rust under `src/` for the register, abbreviation and
acronym gates (the last also reads the README, the CHANGELOG and `docs/*.md`), and the crate's
README and crate-root documentation for the three README gates, each as the template runs it.
There is no `.github/accepted-words` file in this tree, so a word is accepted only where the
template's own lists accept it.

The baseline is one `<crate> <gate> <count>` line per nonzero entry, sorted, with no total.

Name: provisional, from the lane that took the gates (lane/pin-bump-name-gates-d4q).
"""

import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

sys.dont_write_bytecode = True

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HELPERS = os.path.join(ROOT, 'vendor', 'crate-workflows', 'helpers')
BASELINE = os.path.join(ROOT, 'helpers', 'template_report_baseline.txt')
CRATES = 'crates'

GATES = ('register_names', 'abbreviations', 'prose_acronyms', 'readme_acronyms',
         'inflated_wording', 'readme_sections')

# A finding is a line that begins with a location, optionally behind the gate's own label.
FINDING = re.compile(r'^(?:prose_acronyms: )?[^\s:]+:\d+: ')


def rust_files(crate):
    found = []
    for base, _, names in os.walk(os.path.join(ROOT, CRATES, crate, 'src')):
        found += [os.path.relpath(os.path.join(base, n), ROOT) for n in names if n.endswith('.rs')]
    return sorted(found)


def documents(crate):
    found = []
    for name in ('README.md', 'CHANGELOG.md'):
        if os.path.isfile(os.path.join(ROOT, CRATES, crate, name)):
            found.append(f'{CRATES}/{crate}/{name}')
    docs = os.path.join(ROOT, CRATES, crate, 'docs')
    if os.path.isdir(docs):
        found += [f'{CRATES}/{crate}/docs/{n}' for n in sorted(os.listdir(docs)) if n.endswith('.md')]
    return found


def command(gate, crate):
    script = os.path.join(HELPERS, gate + '.py')
    directory = f'{CRATES}/{crate}'
    manifest = f'{directory}/Cargo.toml'
    if gate in ('register_names', 'abbreviations'):
        return [sys.executable, '-B', script, '--manifest', manifest, *rust_files(crate)]
    if gate == 'prose_acronyms':
        return [sys.executable, '-B', script, *documents(crate), *rust_files(crate)]
    return [sys.executable, '-B', script, '--crate-dir', directory]


def findings_in(output):
    return [line for line in output.splitlines() if FINDING.match(line)]


def run_gate(gate, crate):
    """The finding lines of one gate over one crate. A gate that cannot run is an error, not zero."""
    with tempfile.TemporaryDirectory() as scratch:
        # Run from an empty directory so no `.github/*-ceiling` file of ours is read by accident;
        # the paths in the command are absolute-from-ROOT, so make them so.
        argv = [os.path.join(ROOT, a) if a.startswith(CRATES + '/') else a for a in command(gate, crate)]
        done = subprocess.run(argv, cwd=scratch, capture_output=True, text=True)
    output = done.stdout + done.stderr
    if gate == 'prose_acronyms':
        usable = done.returncode in (0, 1)
    else:
        usable = done.returncode in (0, 1)
    if not usable:
        raise RuntimeError(f'{gate} could not run over {crate} (exit {done.returncode}): {output[-400:]}')
    return [line.replace(ROOT + os.sep, '') for line in findings_in(output)]


def crates():
    base = os.path.join(ROOT, CRATES)
    return sorted(n for n in os.listdir(base) if os.path.isfile(os.path.join(base, n, 'Cargo.toml')))


def survey(names):
    """{(crate, gate): [finding lines]} for every crate and gate, nonzero or not."""
    jobs = [(c, g) for c in names for g in GATES]
    with ThreadPoolExecutor(max_workers=min(16, os.cpu_count() or 4)) as pool:
        results = list(pool.map(lambda job: run_gate(job[1], job[0]), jobs))
    return dict(zip(jobs, results))


def read_baseline(path=BASELINE):
    table = {}
    if not os.path.isfile(path):
        return table
    for number, line in enumerate(open(path, encoding='utf-8').read().splitlines(), 1):
        if not line.strip() or line.startswith('#'):
            continue
        parts = line.split()
        if len(parts) != 3 or parts[1] not in GATES or not parts[2].isdigit():
            raise ValueError(f'{path}:{number}: want `<crate> <gate> <count>`: {line!r}')
        table[(parts[0], parts[1])] = int(parts[2])
    return table


def write_baseline(table, path=BASELINE):
    lines = sorted(f'{c} {g} {n}' for (c, g), n in table.items() if n > 0)
    with open(path, 'w', encoding='utf-8') as out:
        out.write('\n'.join(lines) + ('\n' if lines else ''))


def compare(counts, baseline):
    """(over, under): the (crate, gate, count, ceiling) rows above and below the baseline."""
    over, under = [], []
    for key in sorted(set(counts) | set(baseline)):
        have, ceiling = counts.get(key, 0), baseline.get(key, 0)
        if have > ceiling:
            over.append((*key, have, ceiling))
        elif have < ceiling:
            under.append((*key, have, ceiling))
    return over, under


def tally(result):
    return {key: len(lines) for key, lines in result.items() if lines}


def check():
    result = survey(crates())
    counts = tally(result)
    baseline = read_baseline()
    for gate in GATES:
        rows = sorted(((c, n) for (c, g), n in counts.items() if g == gate), key=lambda r: (-r[1], r[0]))
        total = sum(n for _, n in rows)
        print(f'template report gates: {gate}: {total} findings in {len(rows)} crates '
              f'(baseline {sum(n for (c, g), n in baseline.items() if g == gate)})')
        for crate, n in rows:
            print(f'  {crate}: {n} (baseline {baseline.get((crate, gate), 0)})')
    over, under = compare(counts, baseline)
    if under:
        print(f'template report gates: {len(under)} entries are under their baseline; run '
              f'`python3 helpers/template_report_gates.py --bank` to lower it:')
        for crate, gate, have, ceiling in under:
            print(f'  {crate} {gate}: {have} < {ceiling}')
    if over:
        print('lint: a template report gate found more than the baseline allows (a new finding, or a new '
              'crate with findings). Fix the finding; list them with '
              '`python3 helpers/template_report_gates.py --list <crate>`:', file=sys.stderr)
        for crate, gate, have, ceiling in over:
            print(f'  {crate} {gate}: {have} > {ceiling}', file=sys.stderr)
        return 1
    print('template report gates: no crate is over its baseline')
    return 0


def listing(names):
    names = names or crates()
    result = survey(names)
    for (crate, gate), lines in sorted(result.items()):
        for line in lines:
            print(f'{crate}: {gate}: {line}')
    return 0


def bank():
    counts = tally(survey(crates()))
    baseline = read_baseline()
    lowered = {key: min(n, counts.get(key, 0)) for key, n in baseline.items()}
    write_baseline(lowered)
    print(f'template report gates: baseline lowered to {sum(lowered.values())} findings in '
          f'{sum(1 for n in lowered.values() if n)} entries')
    return 0


def count():
    counts = tally(survey(crates()))
    write_baseline(counts)
    print(f'template report gates: wrote {sum(counts.values())} findings in {len(counts)} entries')
    return 0


def selftest():
    failed = []
    sample = ('crates/a/src/x.rs:3: const SIZE: a size\nregister_names: 2 names judged, 1 refused\n'
              'prose_acronyms: crates/a/src/y.rs:9: CBMC is not expanded\n'
              'crates/a/README.md:4: FAT is used before the text defines it\n'
              'abbreviations: 5 names judged, 1 components are not words, in 1 file\n'
              'crates/a/src/x.rs: 1 components are not words\n')
    if len(findings_in(sample)) != 3:
        failed.append('finding lines are not the location lines')
    over, under = compare({('a', 'register_names'): 3, ('b', 'abbreviations'): 1},
                          {('a', 'register_names'): 2, ('c', 'readme_sections'): 4})
    if [r[:2] for r in over] != [('a', 'register_names'), ('b', 'abbreviations')]:
        failed.append(f'growth and a new crate are over: {over}')
    if [r[:2] for r in under] != [('c', 'readme_sections')]:
        failed.append(f'a shrunk entry is under: {under}')
    with tempfile.TemporaryDirectory() as scratch:
        path = os.path.join(scratch, 'baseline')
        write_baseline({('b', 'abbreviations'): 2, ('a', 'register_names'): 1, ('c', 'readme_sections'): 0}, path)
        if open(path).read() != 'a register_names 1\nb abbreviations 2\n':
            failed.append('the baseline is one sorted nonzero entry per line')
        if read_baseline(path) != {('a', 'register_names'): 1, ('b', 'abbreviations'): 2}:
            failed.append('the baseline does not read back')
        open(path, 'w').write('a nonsense 1\n')
        try:
            read_baseline(path)
            failed.append('a bad baseline line is accepted')
        except ValueError:
            pass
    for message in failed:
        print(f'template_report_gates selftest: FAIL {message}', file=sys.stderr)
    print('template_report_gates selftest: %s' % ('FAILED' if failed else 'ok'))
    return 1 if failed else 0


def main(argv):
    if not argv or argv[0] == '--check':
        return check()
    if argv[0] == '--selftest':
        return selftest()
    if argv[0] == '--list':
        return listing(argv[1:])
    if argv[0] == '--bank':
        return bank()
    if argv[0] == '--count':
        return count()
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
