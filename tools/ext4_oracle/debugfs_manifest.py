#!/usr/bin/env python3
"""Print the manifest of an ext4 image as debugfs (e2fsprogs) reads it: the oracle side of the
ext4 read oracle.

Usage: debugfs_manifest.py <image>

One line per directory entry, sorted by path, with tab-separated fields: path, inode number, kind,
permission bits in octal, uid, gid, link count, size, the access, change, modification and birth
times, the content, the extended attributes and the extents. A time is seconds since 1970, signed,
with a nine-digit nanosecond part when it is not zero. Content is `length:FNV-1a-64` for a regular
file and the hexadecimal target for a symbolic link. Extended attributes are `name=length:FNV-1a-64`,
sorted. Extents are the leaf extents as `logical+length@physical`, `u` marking an unwritten one, or
`inline` for inline data. A reader under test prints the same fields, and the two manifests must be
equal line for line.

See tools/ext4_oracle/corpus/make-corpus.sh, which builds the images, and
nifeos/record:design/roadmap/0190-ext4-read-write.md.

Name: provisional. calef names files.
"""
import os
import re
import subprocess
import sys
import tempfile

KINDS = {0o100000: 'RegularFile', 0o040000: 'Directory', 0o120000: 'Symlink', 0o010000: 'Fifo',
         0o020000: 'CharacterDevice', 0o060000: 'BlockDevice', 0o140000: 'Socket'}


def fnv(data):
    h = 0xcbf29ce484222325
    for b in data:
        h ^= b
        h = (h * 0x100000001b3) & 0xffffffffffffffff
    return h


def debugfs(image, commands):
    """Run commands in one debugfs and split its output per command."""
    with tempfile.NamedTemporaryFile('w', delete=False) as f:
        f.write('\n'.join(commands) + '\n')
        script = f.name
    out = subprocess.run(['debugfs', '-f', script, image], capture_output=True).stdout
    os.unlink(script)
    parts, current = [], None
    for line in out.split(b'\n'):
        if line.startswith(b'debugfs: '):
            current = []
            parts.append(current)
        elif current is not None:
            current.append(line)
    assert len(parts) == len(commands), (len(parts), len(commands))
    return parts


def time_field(stat, name):
    m = re.search(rb'^ *' + name + rb': 0x([0-9a-f]{8}):([0-9a-f]{8})', stat, re.M)
    if not m:
        m = re.search(rb'^ *' + name + rb': 0x([0-9a-f]{8})', stat, re.M)
        low, extra = int(m.group(1), 16), 0
    else:
        low, extra = int(m.group(1), 16), int(m.group(2), 16)
    # Linux (ext4_decode_extra_time): the seconds field is signed, the extra field's low two bits
    # add multiples of 2^32 and its upper thirty bits are nanoseconds.
    seconds = (low - (1 << 32) if low & 0x80000000 else low) + ((extra & 3) << 32)
    nanos = extra >> 2
    return f'{seconds}.{nanos:09d}' if nanos else str(seconds)


def main():
    image = sys.argv[1]
    entries = []  # (path, inode, mode)
    level = [('', 2)]
    while level:
        outputs = debugfs(image, [f'ls -p <{ino}>' for _, ino in level])
        nxt = []
        for (prefix, _), lines in zip(level, outputs):
            for line in lines:
                if not line.startswith(b'/'):
                    continue
                fields = line.split(b'/')
                ino, mode, name = int(fields[1]), int(fields[2], 8), b'/'.join(fields[5:-2])
                if name in (b'.', b'..') or ino == 0:
                    continue
                path = prefix + '/' + name.decode('utf-8', 'surrogateescape')
                entries.append((path, ino, mode))
                if mode & 0o170000 == 0o040000:
                    nxt.append((path, ino))
        level = nxt
    inodes = sorted({ino for _, ino, _ in entries})
    commands = []
    for ino in inodes:
        commands += [f'stat <{ino}>', f'ex <{ino}>', f'ea_list <{ino}>']
    outputs = debugfs(image, commands)
    info = {}
    with tempfile.TemporaryDirectory() as scratch:
        for i, ino in enumerate(inodes):
            stat, ex, ea = (b'\n'.join(x) for x in outputs[3 * i:3 * i + 3])
            names = re.findall(rb'^  (\S+) \((\d+)\)', ea, re.M)
            info[ino] = (stat, ex, names)
        ea_commands, ea_keys = [], []
        for ino in inodes:
            for name, _ in info[ino][2]:
                target = f'{scratch}/ea-{len(ea_keys)}'
                ea_commands.append(f'ea_get -f {target} <{ino}> {name.decode()}')
                ea_keys.append((ino, name.decode(), target))
        content_commands, content_keys = [], []
        for path, ino, mode in entries:
            if mode & 0o170000 in (0o100000, 0o120000) and ino not in dict(content_keys):
                target = f'{scratch}/data-{ino}'
                content_commands.append(f'dump <{ino}> {target}')
                content_keys.append((ino, target))
        if ea_commands or content_commands:
            debugfs(image, ea_commands + content_commands)
        values = {}
        for ino, name, target in ea_keys:
            try:
                v = open(target, 'rb').read()
                values.setdefault(ino, []).append(f'{name}={len(v)}:{fnv(v):016x}')
            except FileNotFoundError:
                values.setdefault(ino, []).append(f'{name}=MISSING')
        contents = {ino: open(t, 'rb').read() if os.path.exists(t) else None for ino, t in content_keys}
    lines = []
    for path, ino, mode in entries:
        stat, ex, _ = info[ino]
        kind = KINDS.get(mode & 0o170000, 'Unknown')
        uid = int(re.search(rb'User: +(\d+)', stat).group(1))
        gid = int(re.search(rb'Group: +(\d+)', stat).group(1))
        links = int(re.search(rb'Links: +(\d+)', stat).group(1))
        size = int(re.search(rb'Size: +(\d+)\n', stat).group(1))
        times = [time_field(stat, n) for n in (rb'atime', rb'ctime', rb'mtime', rb'crtime')]
        content, extents = '-', '-'
        if kind in ('RegularFile', 'Symlink'):
            data = contents.get(ino)
            if data is None:
                content = 'READ-ERROR'
            elif kind == 'Symlink':
                fast = re.search(rb'^Fast link dest: "(.*)"$', stat, re.M)
                content = (fast.group(1) if fast else data).hex()
            else:
                content = f'{len(data)}:{fnv(data):016x}'
        if kind in ('RegularFile', 'Directory'):
            if b'Size of inline data' in stat:
                extents = 'inline'
            else:
                leaf = []
                for m in re.finditer(rb'^ *(\d+)/ *(\d+) +\d+/ *\d+ +(\d+) - +(\d+) +(\d+) - +(\d+) +(\d+) *(\S*)', ex, re.M):
                    if m.group(1) == m.group(2):
                        flag = 'u' if m.group(8) == b'Uninit' else ''
                        leaf.append(f'{int(m.group(3))}+{int(m.group(7))}@{int(m.group(5))}{flag}')
                extents = ','.join(leaf) if leaf else 'none'
        xattrs = ','.join(sorted(values.get(ino, []))) or 'none'
        lines.append('\t'.join([path, str(ino), kind, f'{mode & 0o7777:o}', str(uid), str(gid), str(links),
                                str(size), *times, content, xattrs, extents]))
    for line in sorted(lines):
        print(line)


if __name__ == '__main__':
    main()
