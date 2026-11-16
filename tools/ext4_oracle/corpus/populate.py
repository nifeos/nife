#!/usr/bin/env python3
"""Write the ext4 read-oracle corpus's source tree into an empty directory.

Usage: populate.py <empty-directory> <block-size-bytes>

The tree is deterministic: every byte comes from a seeded generator, and `make-corpus.sh` fixes
every time, so two runs build the same image from it. Each part exists to put one on-disk shape in
front of the reader: files across the block and inline-data boundaries, sparse files whose extent
trees reach depth 1 and depth 2 at the profile's block size, a directory large enough for
`e2fsck -D` to index (an htree), fast and slow symbolic links, a file with three hard links,
extended attributes that fit in the inode and one that needs its own block, a FIFO, long and
non-ASCII names, and three files whose times `make-corpus.sh` sets to nanoseconds, a time before
1970 and one past 2106.

See tools/ext4_oracle/corpus/make-corpus.sh, which runs it, and design/roadmap/0190-ext4-read-write.md.

Name: provisional. calef names files.
"""
import os
import random
import sys

def data(rng, length):
    return bytes(rng.getrandbits(8) for _ in range(length)) if length < 4096 else rng.randbytes(length)


def write(path, payload):
    with open(path, 'wb') as f:
        f.write(payload)


def sparse_file(rng, path, block_size, runs):
    """A file of `runs` one-block data runs, each followed by a one-block hole, so that every run
    is its own extent and the tree's depth is fixed by `runs` and the block size."""
    with open(path, 'wb') as f:
        for run in range(runs):
            f.seek(2 * run * block_size)
            # The first byte is never zero, so `mke2fs -d` cannot read the block as a hole.
            f.write(b'\x01' + rng.randbytes(block_size - 1))
        f.truncate(2 * runs * block_size)


def main():
    root, block_size = sys.argv[1], int(sys.argv[2])
    if os.listdir(root):
        sys.exit(f'populate.py: {root} is not empty')
    rng = random.Random(190)

    os.makedirs(f'{root}/files')
    for size in (0, 1, 59, 60, 61, 100, 160, block_size - 1, block_size, block_size + 1,
                 12 * block_size, 1_048_576 + 3):
        write(f'{root}/files/size-{size}', data(rng, size))

    # An extent leaf holds (block_size - 12) / 12 entries and the inode root holds four, so depth 1
    # needs more than 4 extents and depth 2 more than 4 * (block_size - 12) / 12.
    leaf = (block_size - 12) // 12
    os.makedirs(f'{root}/extents')
    sparse_file(rng, f'{root}/extents/depth-0', block_size, 3)
    sparse_file(rng, f'{root}/extents/depth-1', block_size, 4 * 3)
    sparse_file(rng, f'{root}/extents/depth-2', block_size, 4 * leaf + 8)
    with open(f'{root}/extents/hole-only', 'wb') as f:
        f.truncate(64 * block_size)

    os.makedirs(f'{root}/htree')
    for i in range(3000):
        write(f'{root}/htree/entry-{i:05d}-{rng.getrandbits(32):08x}', data(rng, i % 7))

    os.makedirs(f'{root}/links')
    os.symlink('../files/size-1', f'{root}/links/fast')
    slow = '/'.join(f'component-{i:02d}' for i in range(20))
    os.symlink(slow, f'{root}/links/slow')
    os.symlink('a' * 59, f'{root}/links/fast-59')
    os.symlink('b' * 60, f'{root}/links/slow-60')
    os.symlink('dangling-target', f'{root}/links/dangling')
    write(f'{root}/links/hard-a', data(rng, 333))
    os.link(f'{root}/links/hard-a', f'{root}/links/hard-b')
    os.link(f'{root}/links/hard-a', f'{root}/files/hard-c')

    os.makedirs(f'{root}/attributes')
    write(f'{root}/attributes/small', b'small\n')
    os.setxattr(f'{root}/attributes/small', 'user.color', b'blue')
    os.setxattr(f'{root}/attributes/small', 'user.empty', b'')
    write(f'{root}/attributes/large', b'large\n')
    os.setxattr(f'{root}/attributes/large', 'user.blob', data(rng, 900))
    os.setxattr(f'{root}/attributes/large', 'user.second', b'2')
    os.makedirs(f'{root}/attributes/directory')
    os.setxattr(f'{root}/attributes/directory', 'user.on-a-directory', b'yes')

    os.makedirs(f'{root}/names')
    for name in ('with space', 'café', '日本語', 'n' * 255, '.hidden', '-dash'):
        write(f'{root}/names/{name}', name.encode())
    os.mkfifo(f'{root}/names/fifo')
    os.makedirs(f'{root}/empty-directory')
    deep = f'{root}/deep'
    for level in range(12):
        deep = f'{deep}/level-{level}'
    os.makedirs(deep)
    write(f'{deep}/leaf', b'leaf\n')

    # make-corpus.sh sets these three files' times with debugfs after mke2fs, since `mke2fs -d`
    # copies whole seconds only and SOURCE_DATE_EPOCH flattens every other time to one value.
    os.makedirs(f'{root}/timestamps')
    for name in ('nanoseconds', 'before-1970', 'after-2106'):
        write(f'{root}/timestamps/{name}', name.encode())

if __name__ == '__main__':
    main()
