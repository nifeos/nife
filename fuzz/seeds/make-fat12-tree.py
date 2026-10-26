#!/usr/bin/env python3
"""Write fuzz/seeds/file_allocation_table_boot_sector/small-tree.img, a 6 KiB FAT12 volume.

The FAT reader's fuzz target walks a volume's whole tree, and the dosfstools boot sectors that also
seed it hold no tree at all. This volume is the smallest that reaches every path of the walk: a long
name, a subdirectory holding another, a file chained over three clusters, a free slot, the volume
label, and free clusters. Every byte is placed here by the layout of Microsoft's "FAT: General
Overview of On-Disk Format", version 1.03, so the file rebuilds byte for byte, and `fsck.fat -n`
accepts it.

    python3 fuzz/seeds/make-fat12-tree.py
"""

import os
import struct

SECTOR = 512
CLUSTERS = 8  # clusters 2 to 9
TOTAL = 1 + 2 + 1 + CLUSTERS  # reserved, two tables, root, data
EOC = 0xFFF


def short(name, attributes, cluster, size):
    """A short directory entry: an 11-byte name, attributes, first cluster and size."""
    assert len(name) == 11
    return name + bytes([attributes]) + bytes(14) + struct.pack('<HI', cluster, size)


def checksum(name):
    """The short-name checksum a long name's pieces carry."""
    total = 0
    for byte in name:
        total = (((total & 1) << 7) + (total >> 1) + byte) & 0xFF
    return total


def long_piece(text, name):
    """The one long-name piece `text` needs, for the short name `name`."""
    units = [ord(c) for c in text] + [0]
    units += [0xFFFF] * (13 - len(units))
    raw = b''.join(struct.pack('<H', u) for u in units)
    return (bytes([0x41]) + raw[0:10] + bytes([0x0F, 0, checksum(name)]) + raw[10:22]
            + bytes(2) + raw[22:26])


def table():
    """Both tables' one sector: the directories end at once, the file runs over 3, 4 and 5."""
    entries = [0xFF8, 0xFFF, EOC, 4, 5, EOC, EOC, EOC, 0, 0]
    out = bytearray(SECTOR)
    for cluster, value in enumerate(entries):
        at = cluster * 3 // 2
        pair = struct.unpack_from('<H', out, at)[0]
        pair = (pair & 0x000F) | (value << 4) if cluster & 1 else (pair & 0xF000) | value
        struct.pack_into('<H', out, at, pair)
    return bytes(out)


def boot():
    """The boot sector and its BPB."""
    out = bytearray(SECTOR)
    out[0:3] = b'\xEB\x3C\x90'
    out[3:11] = b'MSWIN4.1'
    struct.pack_into('<HBHBHHBHHHII', out, 11, SECTOR, 1, 1, 2, 16, TOTAL, 0xF8, 1, 1, 1, 0, 0)
    out[36] = 0x80
    out[38] = 0x29
    struct.pack_into('<I', out, 39, 0x1234ABCD)
    out[43:54] = b'NIFETEST   '
    out[54:62] = b'FAT12   '
    out[510:512] = b'\x55\xAA'
    return bytes(out)


def sector(*entries):
    """One directory sector holding `entries`, the rest zero, which ends the directory."""
    data = b''.join(entries)
    return data + bytes(SECTOR - len(data))


def main():
    alias = b'ALONGN~1   '
    gone = bytearray(short(b'GONE    TXT', 0x20, 0, 0))
    gone[0] = 0xE5
    root = sector(
        short(b'NIFETEST   ', 0x08, 0, 0),
        long_piece('A Long Name', alias),
        short(alias, 0x10, 2, 0),
        bytes(gone),
        short(b'HELLO   TXT', 0x20, 3, 1300),
    )
    outer = sector(
        short(b'.          ', 0x10, 2, 0),
        short(b'..         ', 0x10, 0, 0),
        short(b'INNER      ', 0x10, 6, 0),
    )
    inner = sector(
        short(b'.          ', 0x10, 6, 0),
        short(b'..         ', 0x10, 2, 0),
        short(b'DEEP    BIN', 0x20, 7, 512),
    )
    text = bytes((i * 7 + 1) & 0xFF for i in range(3 * SECTOR))
    clusters = [outer, text[0:512], text[512:1024], text[1024:1536], inner,
                bytes(range(256)) * 2, bytes(SECTOR), bytes(SECTOR)]
    image = boot() + table() + table() + root + b''.join(clusters)
    assert len(image) == TOTAL * SECTOR
    here = os.path.dirname(os.path.abspath(__file__))
    path = os.path.join(here, 'file_allocation_table_boot_sector', 'small-tree.img')
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'wb') as f:
        f.write(image)


if __name__ == '__main__':
    main()
