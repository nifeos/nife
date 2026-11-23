#!/usr/bin/env python3
"""Run the ext4 read oracle: compare what `rsext4` reads with what `debugfs` reads, over the
generated corpus, and hold the result to `expected.txt`.

Usage: compare.py <reader> <work-directory> [--write-expected]

`<reader>` is the built `ext4_oracle` binary. The script builds the corpus into
`<work-directory>` with `corpus/make-corpus.sh`, prints each image's manifest with
`debugfs_manifest.py`, and runs the reader on each image twice, as a device of 512-byte logical
sectors and of 4,096-byte ones. Each run reduces to one outcome line:

- `<image>\t<sector>\tequal`: every line of the two manifests is equal;
- `<image>\t<sector>\tdiffers\t<path>:<field>,...`: the entries and fields that differ, sorted
  (`missing` or `extra` in place of the fields for an entry only one side lists);
- `<image>\t<sector>\trefused\t<error>`: the mount failed, with the crate's error.

The outcomes must equal `expected.txt` line for line, so the job fails on a new difference and on
a fixed one alike: a fix is a change to the record (the block's BUGS entry and the appendix), and
the expected file moves with it. `--write-expected` rewrites the file from this run instead.

Needs e2fsprogs 1.47 or later and Linux; see corpus/make-corpus.sh.

See nifeos/record:design/roadmap/0190-ext4-read-write/read-oracle.md.

Name: provisional. calef names files.
"""
import os
import subprocess
import sys

FIELDS = ('path', 'inode', 'kind', 'mode', 'uid', 'gid', 'links', 'size', 'access', 'change',
          'modification', 'birth', 'content', 'attributes', 'extents')
SECTORS = (512, 4096)
HERE = os.path.dirname(os.path.abspath(__file__))
EXPECTED = os.path.join(HERE, 'expected.txt')


def by_path(text):
    """{path: fields} from a manifest, one tab-separated line per entry."""
    entries = {}
    for line in text.splitlines():
        fields = line.split('\t')
        entries[fields[0]] = fields
    return entries


def outcome(image, sector, oracle, reader_output):
    """One outcome line for one reader run against the oracle's manifest."""
    head = f'{image}\t{sector}'
    if reader_output.startswith('MOUNT-ERROR\t'):
        return f'{head}\trefused\t{reader_output.splitlines()[0].split(chr(9), 1)[1]}'
    want, got = by_path(oracle), by_path(reader_output)
    differences = []
    for path in sorted(set(want) | set(got)):
        if path not in got:
            differences.append(f'{path}:missing')
        elif path not in want:
            differences.append(f'{path}:extra')
        elif want[path] != got[path]:
            named = [FIELDS[i] if i < len(FIELDS) else str(i)
                     for i in range(max(len(want[path]), len(got[path])))
                     if want[path][i:i + 1] != got[path][i:i + 1]]
            differences.append(f'{path}:{",".join(named)}')
    if not differences:
        return f'{head}\tequal'
    return f'{head}\tdiffers\t{" ".join(differences)}'


def main():
    args = [a for a in sys.argv[1:] if a != '--write-expected']
    if len(args) != 2:
        sys.exit('usage: compare.py <reader> <work-directory> [--write-expected]')
    reader, work = os.path.abspath(args[0]), os.path.abspath(args[1])
    subprocess.run(['sh', os.path.join(HERE, 'corpus', 'make-corpus.sh'), work], check=True,
                   stdout=subprocess.DEVNULL)
    images = sorted(f[:-4] for f in os.listdir(work) if f.endswith('.img'))
    lines = []
    for image in images:
        path = os.path.join(work, image + '.img')
        oracle = subprocess.run([sys.executable, '-I', os.path.join(HERE, 'debugfs_manifest.py'),
                                 path], check=True, capture_output=True, text=True).stdout
        for sector in SECTORS:
            run = subprocess.run([reader, path, str(sector)], capture_output=True, text=True)
            if run.returncode not in (0, 2):
                sys.exit(f'compare.py: the reader exited {run.returncode} on {image} at {sector}:\n'
                         + run.stderr)
            lines.append(outcome(image, sector, oracle, run.stdout))
    result = '\n'.join(lines) + '\n'
    if '--write-expected' in sys.argv[1:]:
        with open(EXPECTED, 'w') as f:
            f.write(result)
        print(f'compare.py: wrote {len(lines)} outcomes to {EXPECTED}')
        return
    with open(EXPECTED) as f:
        expected = f.read()
    if result == expected:
        print(f'compare.py: {len(lines)} outcomes, each as expected.txt records')
        return
    want, got = expected.splitlines(), lines
    for line in sorted(set(want) - set(got)):
        print(f'expected, not seen: {line}')
    for line in sorted(set(got) - set(want)):
        print(f'seen, not expected: {line}')
    sys.exit('compare.py: the oracle disagrees with expected.txt. A difference that is a fix '
             'changes the record too: the block\'s BUGS entry, the appendix, and expected.txt '
             '(--write-expected).')


if __name__ == '__main__':
    main()
