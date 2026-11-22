#!/usr/bin/env python3
"""Run the ext4 write oracle: apply each case's operations through `rsext4`, judge the image with
`e2fsck -fn`, read it back through `debugfs`, and hold every outcome to `write-expected.txt`.

Usage: write_compare.py <writer> <work-directory> [--write-expected]

`<writer>` is the built `ext4_oracle_write` binary. The script builds the read oracle's corpus
into `<work-directory>` with `corpus/make-corpus.sh`, so every case starts from an image Linux's
e2fsprogs wrote, with its 3,060 entries, its htree and its depth-2 extent trees. Each case in
`write-cases.txt` runs on a fresh copy of each base image in BASES, at that base's sector size.
The writer mounts the copy read-write, applies the operations, and unmounts.

The oracle side is a model, not `rsext4`: the `debugfs` manifest of the base image, with each
operation applied to it here. When the writer stops at a failed operation, the model stops before
it, so a failed operation must leave the volume as it was. The readback is the `debugfs` manifest
of the written image, and the two must agree on each entry's path, kind, permission bits, owner,
group, link count, size (files and links only), content and extended attributes. Times, inode
numbers and extents are the writer's to choose and are not compared.

Each run reduces to one outcome line, tab-separated: the case, the base image, the sector size,
then three verdicts.

- apply: `applied`, or `failed@<index>:<error>` for the first operation the writer refused;
- fsck: `clean`, or `exit<status>` and the problems `e2fsck -fn` printed, joined with ` | `
  (`exit0:` and a problem is an optimization `e2fsck` offers on a volume it calls clean);
- readback: `equal`, or `differs` and each `<path>:<field>,...` that disagrees with the model
  (`missing` or `extra` for an entry only one side has).

The outcomes must equal `write-expected.txt` line for line, so a new failure and a fixed one both
fail the run: a fix is a change to the record (the block's BUGS entry and the appendix), and the
expected file moves with it. `--write-expected` rewrites the file from this run instead.

The operation language, one per line, paths absolute, words separated by single spaces:

    mkdir P | create P | symlink P TARGET | link EXISTING NEW
    write P OFFSET LENGTH SEED       LENGTH bytes of stream(SEED) at OFFSET
    stripes P COUNT STRIDE LENGTH SEED
                                     for i < COUNT: stream(SEED + i), LENGTH bytes at i * STRIDE
    truncate P SIZE | rename OLD NEW | unlink P | rmdir P | sync
    fill DIRECTORY COUNT PREFIX [LENGTH SEED]
                                     COUNT files PREFIX00000..., each LENGTH bytes of stream(SEED + i)
    setxattr P NAME LENGTH SEED | removexattr P NAME
                                     NAME carries its namespace: user., trusted. or security.

Needs e2fsprogs 1.47 or later and Linux; see corpus/make-corpus.sh.

See design/roadmap/0190-ext4-read-write/write-oracle.md.

Name: provisional. calef names files.
"""
import concurrent.futures
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CASES = os.path.join(HERE, 'write-cases.txt')
EXPECTED = os.path.join(HERE, 'write-expected.txt')
# (corpus image, sector size). 1 KiB blocks reach depth 2 at a few hundred extents; 4 KiB blocks
# are the drives' own, on both sector sizes a disk reports.
BASES = (('default-4k', 4096), ('default-4k', 512), ('default-1k', 512))
FIELDS = ('path', 'kind', 'mode', 'uid', 'gid', 'links', 'size', 'content', 'attributes')
MASK = (1 << 64) - 1
FSCK_ENVIRONMENT = dict(os.environ, E2FSCK_TIME='1600000000', E2FSPROGS_FAKE_TIME='1600000000')


def stream(seed, length):
    """The bytes the writer puts down for `seed`: its xorshift64 stream, eight bytes a step."""
    state = ((seed * 0x9e3779b97f4a7c15) & MASK) | 1
    out = bytearray()
    while len(out) < length:
        state ^= (state << 13) & MASK
        state ^= state >> 7
        state ^= (state << 17) & MASK
        out += state.to_bytes(8, 'little')
    return bytes(out[:length])


def fnv(data):
    h = 0xcbf29ce484222325
    for b in data:
        h ^= b
        h = (h * 0x100000001b3) & MASK
    return h


def manifest(image):
    """{path: fields} as `debugfs_manifest.py` reads the image."""
    text = subprocess.run([sys.executable, '-I', os.path.join(HERE, 'debugfs_manifest.py'), image],
                          check=True, capture_output=True, text=True).stdout
    return {line.split('\t')[0]: line.split('\t') for line in text.splitlines()}


class Node:
    """One inode in the model: a file, link or directory, and what the readback must show."""

    def __init__(self, kind, mode, uid, gid, links, size, content, attributes, inode=None):
        self.kind, self.mode, self.uid, self.gid = kind, mode, uid, gid
        # base_links: the link count before the case, which a new file starts at 0 and a new
        # directory at 2 (its own '.' and its parent's entry).
        self.base_links, self.size, self.content = int(links), size, content
        self.attributes = attributes
        self.inode = inode  # the base image's inode number, or None for a node the case made
        self.data = None    # the file's bytes, once an operation needs them
        # size None: data changed, and the size and content are taken from it when compared


class Model:
    """The tree a case must leave: the base image's manifest with each operation applied."""

    def __init__(self, base_image, base, scratch):
        self.image, self.scratch = base_image, scratch
        self.paths, inodes = {}, {}
        for path, f in base.items():
            inode = int(f[1])
            if inode not in inodes:
                attributes = ({} if f[13] == 'none'
                              else dict(a.split('=', 1) for a in f[13].split(',')))
                inodes[inode] = Node(f[2], f[3], f[4], f[5], f[6], f[7], f[12], attributes,
                                     inode=inode)
            self.paths[path] = inodes[inode]  # hard links share one node
        self.base_names = self.names()
        self.base_subdirectories = self.subdirectories()

    def names(self):
        counts = {}
        for node in self.paths.values():
            counts[node] = counts.get(node, 0) + 1
        return counts

    def subdirectories(self):
        counts = {}
        for path, node in self.paths.items():
            if node.kind == 'Directory':
                parent = self.paths.get(path.rsplit('/', 1)[0])
                if parent is not None:
                    counts[parent] = counts.get(parent, 0) + 1
        return counts

    def bytes_of(self, node):
        if node.data is None:
            if node.inode is None:
                node.data = bytearray()
            else:
                target = f'{self.scratch}.dump-{node.inode}'
                subprocess.run(['debugfs', '-R', f'dump <{node.inode}> {target}', self.image],
                               check=True, capture_output=True)
                with open(target, 'rb') as f:
                    node.data = bytearray(f.read())
                os.unlink(target)
        return node.data

    def write(self, path, offset, data):
        node = self.paths[path]
        buffer = self.bytes_of(node)
        if len(buffer) < offset:
            buffer.extend(bytes(offset - len(buffer)))
        buffer[offset:offset + len(data)] = data
        node.size = None

    def add(self, path, node):
        self.paths[path] = node

    def apply(self, line):
        w = line.split(' ')
        op = w[0]
        if op == 'mkdir':
            self.add(w[1], Node('Directory', '755', '0', '0', 2, '-', '-', {}))
        elif op == 'create':
            node = Node('RegularFile', '644', '0', '0', 0, None, None, {})
            node.data = bytearray()
            self.add(w[1], node)
        elif op == 'symlink':
            target = w[2].encode()
            self.add(w[1], Node('Symlink', '777', '0', '0', 0, str(len(target)), target.hex(), {}))
        elif op == 'link':
            self.paths[w[2]] = self.paths[w[1]]
        elif op == 'write':
            self.write(w[1], int(w[2]), stream(int(w[4]), int(w[3])))
        elif op == 'stripes':
            count, stride, length, seed = (int(x) for x in w[2:6])
            for i in range(count):
                self.write(w[1], i * stride, stream(seed + i, length))
        elif op == 'truncate':
            node, size = self.paths[w[1]], int(w[2])
            buffer = self.bytes_of(node)
            del buffer[size:]
            buffer.extend(bytes(size - len(buffer)))
            node.size = None
        elif op == 'rename':
            old, new = w[1], w[2]
            moved = {p: n for p, n in self.paths.items() if p == old or p.startswith(old + '/')}
            for p in moved:
                del self.paths[p]
            self.paths.pop(new, None)
            for p, n in moved.items():
                self.paths[new + p[len(old):]] = n
        elif op in ('unlink', 'rmdir'):
            del self.paths[w[1]]
        elif op == 'fill':
            for i in range(int(w[2])):
                path = f"{w[1].rstrip('/')}/{w[3]}{i:05d}"
                self.apply(f'create {path}')
                if len(w) > 4:
                    self.write(path, 0, stream(int(w[5]) + i, int(w[4])))
        elif op == 'setxattr':
            value = stream(int(w[4]), int(w[3]))
            self.paths[w[1]].attributes[w[2]] = f'{len(value)}:{fnv(value):016x}'
        elif op == 'removexattr':
            del self.paths[w[1]].attributes[w[2]]
        elif op == 'sync':
            pass
        else:
            sys.exit(f'write_compare.py: unknown operation {line!r}')

    def entries(self):
        """{path: compared fields}, with each link count moved by the names and subdirectories
        the operations added or took away."""
        names, subdirectories = self.names(), self.subdirectories()
        out = {}
        for path, node in self.paths.items():
            if node.kind == 'Directory':
                links = (node.base_links + subdirectories.get(node, 0)
                         - self.base_subdirectories.get(node, 0))
                size = '-'
            else:
                links = node.base_links + names[node] - self.base_names.get(node, 0)
                if node.size is None:
                    node.size = str(len(node.data))
                    node.content = f'{len(node.data)}:{fnv(node.data):016x}'
                size = node.size
            attributes = ','.join(sorted(f'{k}={v}' for k, v in node.attributes.items())) or 'none'
            out[path] = [path, node.kind, node.mode, node.uid, node.gid, str(links), size,
                         node.content, attributes]
        return out


def projected(entries):
    """The compared fields of a `debugfs` manifest, directory sizes left out."""
    out = {}
    for path, f in entries.items():
        size = '-' if f[2] == 'Directory' else f[7]
        out[path] = [path, f[2], f[3], f[4], f[5], f[6], size, f[12], f[13]]
    return out


def readback(want, got):
    differences = []
    for path in sorted(set(want) | set(got)):
        if path not in got:
            differences.append(f'{path}:missing')
        elif path not in want:
            differences.append(f'{path}:extra')
        elif want[path] != got[path]:
            named = [FIELDS[i] for i in range(len(FIELDS)) if want[path][i] != got[path][i]]
            differences.append(f'{path}:{",".join(named)}')
    return 'equal' if not differences else 'differs ' + ' '.join(differences)


def fsck(image):
    run = subprocess.run(['e2fsck', '-fn', image], capture_output=True, text=True,
                         env=FSCK_ENVIRONMENT)
    problems = []
    for line in (run.stdout + run.stderr).splitlines():
        line = line.strip()
        if (not line or line.startswith(('Pass ', 'e2fsck ')) or image in line
                or os.path.basename(image) in line):
            continue
        problems.append(line)
    if run.returncode == 0 and not problems:
        return 'clean'
    return f'exit{run.returncode}' + (':' + ' | '.join(problems) if problems else '')


def parse_cases():
    cases, current = [], None
    with open(CASES) as f:
        for raw in f:
            line = raw.rstrip('\n')
            if not line or line.startswith('#'):
                continue
            if line.startswith('case '):
                current = (line[5:], [])
                cases.append(current)
            else:
                current[1].append(line)
    return cases


def run_one(writer, work, bases, case, base, sector):
    name, operations = case
    image = os.path.join(work, f'{name}.{base}.{sector}.img')
    shutil.copyfile(os.path.join(work, base + '.img'), image)
    run = subprocess.run([writer, image, str(sector)], input='\n'.join(operations) + '\n',
                         capture_output=True, text=True)
    first = (run.stdout.splitlines() or [''])[0].split('\t')
    if run.returncode == 0:
        apply, done = 'applied', len(operations)
    elif run.returncode == 3 and first[0] == 'APPLY-ERROR':
        done = int(first[1])
        apply = f'failed@{done}:{first[2]}'
    else:
        sys.exit(f'write_compare.py: the writer exited {run.returncode} on {name} at {base} '
                 f'{sector}:\n{run.stdout}{run.stderr}')
    model = Model(os.path.join(work, base + '.img'), bases[base], image)
    for line in operations[:done]:
        model.apply(line)
    verdict = fsck(image)
    back = readback(model.entries(), projected(manifest(image)))
    os.unlink(image)
    return f'{name}\t{base}\t{sector}\t{apply}\t{verdict}\t{back}'


def main():
    args = [a for a in sys.argv[1:] if a != '--write-expected']
    if len(args) != 2:
        sys.exit('usage: write_compare.py <writer> <work-directory> [--write-expected]')
    writer, work = os.path.abspath(args[0]), os.path.abspath(args[1])
    subprocess.run(['sh', os.path.join(HERE, 'corpus', 'make-corpus.sh'), work], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    bases = {base: manifest(os.path.join(work, base + '.img')) for base in {b for b, _ in BASES}}
    jobs = [(case, base, sector) for case in parse_cases() for base, sector in BASES]
    with concurrent.futures.ThreadPoolExecutor(os.cpu_count() or 2) as pool:
        lines = list(pool.map(lambda j: run_one(writer, work, bases, *j), jobs))
    result = '\n'.join(lines) + '\n'
    if '--write-expected' in sys.argv[1:]:
        with open(EXPECTED, 'w') as f:
            f.write(result)
        print(f'write_compare.py: wrote {len(lines)} outcomes to {EXPECTED}')
        return
    with open(EXPECTED) as f:
        expected = f.read()
    if result == expected:
        print(f'write_compare.py: {len(lines)} outcomes, each as write-expected.txt records')
        return
    want = expected.splitlines()
    for line in sorted(set(want) - set(lines)):
        print(f'expected, not seen: {line}')
    for line in sorted(set(lines) - set(want)):
        print(f'seen, not expected: {line}')
    sys.exit('write_compare.py: the oracle disagrees with write-expected.txt. A difference that '
             'is a fix changes the record too: the block\'s BUGS entry, the appendix, and '
             'write-expected.txt (--write-expected).')


if __name__ == '__main__':
    main()
