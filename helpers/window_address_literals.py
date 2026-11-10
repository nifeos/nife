"""Literal window addresses in components: counted, held to a ceiling that only falls, to zero.

A component reads each of its windows from the startup page (`startup_page_contract`, through
`user_mode_runtime::startup_window`) instead of compiling the address in, so the spawner that maps
a window is the only side that names its address. See
design/roadmap/1069-a-process-reads-its-window-addresses-from-a-startup-page.md.

    python3 helpers/window_address_literals.py --check     # what script/lint runs
    python3 helpers/window_address_literals.py --list      # every counted hit, path:line: text
    python3 helpers/window_address_literals.py --selftest  # the traps, as fixtures

Name: provisional. A python module under `helpers/`, which `script/names` puts out of its own
scope, so its provenance is this paragraph.

**What counts**: in a Rust file under `components/src/`, a call `pair_page(0x`, `runtime_window(0x`
or `service_window(0x` outside a `//` comment, with any whitespace between the parenthesis and the
literal. Each such call places a window at a number the component compiled in.

**The ratchet.** A count over `CEILING` fails: a change added a literal window address. A count
under it fails too, naming the new count, so the change that converts a component lowers `CEILING`
in the same commit and the slack is never spent. Zero is the end, and then the gate holds it.

BUGS:
- A window address written as a named constant in another crate, or as arithmetic on one, is not
  counted. The contract crates' window addresses (`driver_process_contract` and the like) are read
  by both sides, which is code rather than prose, and their spawners still name them.
- Line comments only. A literal inside a `/* */` block comment is counted.
- One line at a time. A call whose literal sits on the next line is not counted; rustfmt keeps
  these calls on one line, and a fixture holds the gap visible.
"""

import os
import re
import sys

CEILING = 63

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCOPE = os.path.join('components', 'src')
CALL = re.compile(r'\b(?:pair_page|runtime_window|service_window)\(\s*0x')


def hits(text):
    """Each counted call in `text`, as (line number, line)."""
    found = []
    for number, line in enumerate(text.splitlines(), 1):
        code = line.split('//', 1)[0]
        for _ in CALL.finditer(code):
            found.append((number, line.strip()))
    return found


def tree_hits():
    """Every counted call under `components/src/`, as (path, line number, line)."""
    found = []
    base = os.path.join(ROOT, SCOPE)
    for directory, _, files in os.walk(base):
        for name in sorted(files):
            if not name.endswith('.rs'):
                continue
            path = os.path.join(directory, name)
            with open(path, encoding='utf-8') as f:
                text = f.read()
            relative = os.path.relpath(path, ROOT)
            found.extend((relative, number, line) for number, line in hits(text))
    return sorted(found)


def selftest():
    cases = [
        ('pub const DATA: u64 = pair_page(0x0098_0000);', 1),
        ('let a = runtime_window(\n    0x1000_0000);', 0),
        ('let a = runtime_window( 0x1000_0000);', 1),
        ('const B: u64 = service_window(0x10);', 1),
        ('// pair_page(0x0098_0000) in a comment', 0),
        ('let x = pair_page(BASE);', 0),
        ('let y = my_pair_page(0x10);', 0),
        ('a(pair_page(0x1), pair_page(0x2));', 2),
        ('let z = pair_page(0x1); // pair_page(0x2)', 1),
    ]
    failed = 0
    for text, want in cases:
        got = len(hits(text))
        if got != want:
            print(f'window address selftest: {text!r} counted {got}, wanted {want}', file=sys.stderr)
            failed += 1
    if failed:
        sys.exit(1)
    print(f'window address selftest: {len(cases)} fixtures')


def check():
    count = len(tree_hits())
    if count > CEILING:
        print(
            f'lint: {count} literal window addresses in {SCOPE}/, over the ceiling of {CEILING}. '
            'A component reads its windows from the startup page '
            '(`user_mode_runtime::startup_window`); list them with '
            '`python3 helpers/window_address_literals.py --list`.',
            file=sys.stderr,
        )
        sys.exit(1)
    if count < CEILING:
        print(
            f'lint: {count} literal window addresses in {SCOPE}/, under the ceiling of {CEILING}. '
            f'Lower CEILING in helpers/window_address_literals.py to {count} in this change.',
            file=sys.stderr,
        )
        sys.exit(1)
    print(f'window addresses: {count} literal window addresses in {SCOPE}/ (ceiling held)')


def main(argv):
    if argv == ['--selftest']:
        selftest()
    elif argv == ['--check']:
        check()
    elif argv == ['--list']:
        for path, number, line in tree_hits():
            print(f'{path}:{number}: {line}')
    else:
        print(__doc__.split('\n\n')[1], file=sys.stderr)
        sys.exit(2)


if __name__ == '__main__':
    main(sys.argv[1:])
