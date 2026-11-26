#!/usr/bin/env python3
"""Run the ext4 read oracle: compare what `rsext4` reads with what `debugfs` reads, over the
generated corpus, and hold the result to `expected.txt`.

Usage: compare.py <reader> <work-directory> [--server <server-reader>] [--write-expected]

`<reader>` is the built `ext4_oracle` binary. The script builds the corpus into
`<work-directory>` with `corpus/make-corpus.sh`, prints each image's manifest with
`debugfs_manifest.py`, and runs the reader on each image twice, as a device of 512-byte logical
sectors and of 4,096-byte ones. Each run reduces to one outcome line:

- `<image>\t<sector>\tequal`: every line of the two manifests is equal;
- `<image>\t<sector>\tdiffers\t<path>:<field>,...`: the entries and fields that differ, sorted
  (`missing` or `extra` in place of the fields for an entry only one side lists);
- `<image>\t<sector>\trefused\t<error>`: the mount failed, with the crate's error.

With `--server`, the same corpus also judges the server path: `<server-reader>` is the built
`ext4_oracle_server`, which reads each image only through file-service requests to `ext4_server`,
once, at the sector size the server reports. A client sees fewer fields than `debugfs` prints, so
the `debugfs` manifest is projected onto them first (`project`), and the outcomes, `<image>\tserver`
in place of the sector, are held to `server-expected.txt` the same way.

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
SERVER_FIELDS = ('path', 'kind', 'size', 'content', 'attributes', 'modification')
SECTORS = (512, 4096)
HERE = os.path.dirname(os.path.abspath(__file__))
EXPECTED = os.path.join(HERE, 'expected.txt')
SERVER_EXPECTED = os.path.join(HERE, 'server-expected.txt')
# The attribute store's reserved name, which the server never lists
# (`file_system_contract::extended_attribute::STORE_DIRECTORY`).
STORE_DIRECTORY = '.extended-attributes'


def by_path(text):
    """{path: fields} from a manifest, one tab-separated line per entry."""
    entries = {}
    for line in text.splitlines():
        fields = line.split('\t')
        entries[fields[0]] = fields
    return entries


def seconds(time):
    """A `debugfs_manifest.py` time as the server answers it: whole seconds, rounded down, or
    `EOVERFLOW` before 1970, which the contract's reply word cannot carry."""
    whole = int(time.split('.')[0])
    if time.startswith('-'):
        return 'EOVERFLOW'
    return str(whole)


def project(oracle):
    """The `debugfs` manifest as a client of the server sees it, in SERVER_FIELDS: no inode,
    owner, mode, links, other times or extents; a symbolic link is the `ELOOP` and any other
    special file the `ENXIO` that opening it answers, with no size, content or attributes; and
    nothing at or under the attribute store's reserved name."""
    lines = []
    for line in oracle.splitlines():
        f = dict(zip(FIELDS, line.split('\t')))
        if STORE_DIRECTORY in f['path'].split('/'):
            continue
        kind = f['kind']
        if kind in ('RegularFile', 'Directory'):
            regular = kind == 'RegularFile'
            size = f['size'] if regular else '-'
            content = f['content'] if regular else '-'
            attributes = f['attributes']
        else:
            kind = 'ELOOP' if kind == 'Symlink' else 'ENXIO'
            size = content = attributes = '-'
        lines.append('\t'.join((f['path'], kind, size, content, attributes,
                                seconds(f['modification']))))
    return '\n'.join(lines) + '\n'


def outcome(image, sector, oracle, reader_output, fields=FIELDS):
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
            named = [fields[i] if i < len(fields) else str(i)
                     for i in range(max(len(want[path]), len(got[path])))
                     if want[path][i:i + 1] != got[path][i:i + 1]]
            differences.append(f'{path}:{",".join(named)}')
    if not differences:
        return f'{head}\tequal'
    return f'{head}\tdiffers\t{" ".join(differences)}'


def hold(lines, expected_path):
    """Hold the outcomes to an expected file, or rewrite it with `--write-expected`. Returns
    whether they agree."""
    name = os.path.basename(expected_path)
    result = '\n'.join(lines) + '\n'
    if '--write-expected' in sys.argv[1:]:
        with open(expected_path, 'w') as f:
            f.write(result)
        print(f'compare.py: wrote {len(lines)} outcomes to {expected_path}')
        return True
    with open(expected_path) as f:
        expected = f.read()
    if result == expected:
        print(f'compare.py: {len(lines)} outcomes, each as {name} records')
        return True
    want = expected.splitlines()
    for line in sorted(set(want) - set(lines)):
        print(f'{name}: expected, not seen: {line}')
    for line in sorted(set(lines) - set(want)):
        print(f'{name}: seen, not expected: {line}')
    return False


def run_reader(command, image, label):
    """A reader's output on one image; exit on anything but success or a refused mount."""
    run = subprocess.run(command, capture_output=True, text=True)
    if run.returncode not in (0, 2):
        sys.exit(f'compare.py: {command[0]} exited {run.returncode} on {image} at {label}:\n'
                 + run.stderr)
    return run.stdout


def main():
    args = [a for a in sys.argv[1:] if a != '--write-expected']
    server = None
    if '--server' in args:
        i = args.index('--server')
        if i + 1 >= len(args):
            sys.exit('compare.py: --server needs the server reader')
        server = os.path.abspath(args[i + 1])
        del args[i:i + 2]
    if len(args) != 2:
        sys.exit('usage: compare.py <reader> <work-directory> [--server <server-reader>] '
                 '[--write-expected]')
    reader, work = os.path.abspath(args[0]), os.path.abspath(args[1])
    subprocess.run(['sh', os.path.join(HERE, 'corpus', 'make-corpus.sh'), work], check=True,
                   stdout=subprocess.DEVNULL)
    images = sorted(f[:-4] for f in os.listdir(work) if f.endswith('.img'))
    lines, server_lines = [], []
    for image in images:
        path = os.path.join(work, image + '.img')
        oracle = subprocess.run([sys.executable, '-I', os.path.join(HERE, 'debugfs_manifest.py'),
                                 path], check=True, capture_output=True, text=True).stdout
        for sector in SECTORS:
            output = run_reader([reader, path, str(sector)], image, sector)
            lines.append(outcome(image, sector, oracle, output))
        if server:
            output = run_reader([server, path], image, 'server')
            server_lines.append(outcome(image, 'server', project(oracle), output,
                                        SERVER_FIELDS))
    agree = hold(lines, EXPECTED)
    if server:
        agree = hold(server_lines, SERVER_EXPECTED) and agree
    if not agree:
        sys.exit('compare.py: the oracle disagrees with the expected file. A difference that is a '
                 'fix changes the record too: the block\'s BUGS entry, the appendix, and the '
                 'expected file (--write-expected).')


if __name__ == '__main__':
    main()
