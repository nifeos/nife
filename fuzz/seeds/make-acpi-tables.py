#!/usr/bin/env python3
"""Rebuild fuzz/seeds/acpi_firmware_tables/*.bin, the seeds of the `acpi_firmware_tables` target.

Each seed is one selector byte (which reader: 0 IVRS, 1 GTDT, 2 SPCR, 3 FADT Arm boot flags, 4 FADT
reset register) followed by a table body, the bytes after the 36-byte SDT header. The bodies mirror
the tables the unit tests in crates/machine_discovery/src/acpi/ build (ivrs.rs, gtdt.rs, spcr.rs,
fadt.rs), so a seed is past every length check and the fuzzer's mutations land on the walk.
crates/machine_discovery/tests/fuzz_seed.rs holds each seed to decoding as described here.

Run from the repository root: python3 fuzz/seeds/make-acpi-tables.py
"""
import struct

OUT = 'fuzz/seeds/acpi_firmware_tables'
SDT_HEADER = 36


def entry4(kind, ident):
    return bytes([kind, ident & 255, ident >> 8, 0])


def entry8(kind, ident, other):
    return bytes([kind, ident & 255, ident >> 8, 0, 0, other & 255, other >> 8, 0])


def ivhd(kind, base, entries):
    header = 24 if kind == 0x10 else 40
    b = bytearray(header)
    b[0], b[1] = kind, 0xD1
    b[2:4] = struct.pack('<H', header + len(entries))
    b[4:6] = struct.pack('<H', 0x10)
    b[6:8] = struct.pack('<H', 0x40)
    b[8:16] = struct.pack('<Q', base)
    if kind != 0x10:
        b[24:32] = struct.pack('<Q', 0x29D3)
    return bytes(b) + entries


def ivmd(kind, flags, first, last, start, length):
    b = bytearray(32)
    b[0], b[1] = kind, flags
    b[2:4] = struct.pack('<H', 32)
    b[4:6] = struct.pack('<H', first)
    b[6:8] = struct.pack('<H', last)
    b[16:24] = struct.pack('<Q', start)
    b[24:32] = struct.pack('<Q', length)
    return bytes(b)


def ivrs(blocks):
    # IVinfo as QEMU writes it (EFRSup, PASize 40), then the eight reserved bytes.
    return struct.pack('<I', 1 | (40 << 8)) + bytes(8) + b''.join(blocks)


selects = b''.join(entry4(2, i) for i in [0, 8, 0x10, 0x18, 0x20, 0xF8, 0xFA, 0xFB])
selects += struct.pack('<Q', 0x0100A00000000048)  # the IOAPIC special entry QEMU adds
q35 = ivrs([ivhd(0x10, 0xFED80000, selects), ivhd(0x11, 0xFED80000, selects)])

ranges = ivrs([ivhd(0x10, 0xFEB80000, b''.join([
    entry4(1, 0), entry4(3, 0x100), entry4(4, 0x1FF), entry8(0x43, 0x200, 0x18),
    entry4(4, 0x2FF), entry8(0x42, 0x300, 0x28)]))])

uid = bytearray(22)
uid[0], uid[21] = 0xF0, 3
acpi_hid = ivrs([ivhd(0x10, 0xFED80000, bytes(uid) + b'uid' + entry4(2, 0xA0))])

memory = ivrs([
    ivhd(0x11, 0xFED80000, entry4(2, 8)),
    ivmd(0x21, 7, 8, 0, 0x7F000800, 0x1000),
    ivmd(0x22, 6, 0x100, 0x1FF, 0x7E000000, 0x2000),
    ivmd(0x20, 0, 0, 0, 0x7D000000, 0x1000),
    ivmd(0x21, 7, 8, 0, 0x7C000000, 0),
])

gtdt = bytearray(60)
for at, gsiv in [(12, 29), (20, 30), (28, 27), (36, 26)]:
    gtdt[at:at + 4] = struct.pack('<I', gsiv)
gtdt[32:36] = struct.pack('<I', 3)

spcr = bytearray(22)
spcr[0] = 3
spcr[8:16] = struct.pack('<Q', 0x09000000)
spcr[16] = 8
spcr[18:22] = struct.pack('<I', 33)

arm_at = 129 - SDT_HEADER
arm = bytearray(arm_at + 2)
arm[arm_at] = 3

reset = bytearray(129 - SDT_HEADER)
reset[112 - SDT_HEADER:112 - SDT_HEADER + 4] = struct.pack('<I', 1 << 10)
reg = 116 - SDT_HEADER
reset[reg], reset[reg + 1] = 1, 8
reset[reg + 4:reg + 12] = struct.pack('<Q', 0xCF9)
reset[128 - SDT_HEADER] = 0x0F

seeds = {
    'ivrs_q35_amd_iommu': (0, q35),
    'ivrs_ranges_and_aliases': (0, ranges),
    'ivrs_acpi_hid_entry': (0, acpi_hid),
    'ivrs_memory_definitions': (0, memory),
    'gtdt_qemu_virt': (1, bytes(gtdt)),
    'spcr_pl011': (2, bytes(spcr)),
    'fadt_arm_boot_psci_hvc': (3, bytes(arm)),
    'fadt_reset_register_q35': (4, bytes(reset)),
}
for name, (selector, body) in seeds.items():
    with open(f'{OUT}/{name}.bin', 'wb') as f:
        f.write(bytes([selector]) + body)
