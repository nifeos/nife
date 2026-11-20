#!/usr/bin/env python3
"""Write the seeds for the `memory_region_heap_sequence` fuzz target.

Each seed is the target's header (arena pages, page budget; two little-endian bytes each) and a
list of operations in the format src/sequence.rs reads. Each seed names one shape a program's heap
meets, so the fuzzer starts past the first initialize. The script writes beside itself, from any
directory: python3 fuzz/seeds/make-seeds.py from the crate's root
"""
import pathlib
import struct

OUT = pathlib.Path(__file__).resolve().parent / "memory_region_heap_sequence"
UNLIMITED = 0xFFFF
ALIGN_64 = 0x80


def header(arena_pages, budget=UNLIMITED):
    return struct.pack("<HH", arena_pages - 1, budget)


def initialize(maximum_pages, slot=0, extra=0):
    return struct.pack("<BBHH", 0, slot, maximum_pages, extra)


def commit_all():
    return bytes([1])


def allocate(size_class, raw, align):
    return struct.pack("<BBIB", 2, size_class, raw, align)


def small(size, align=0):
    return allocate(7, size, align)


def deallocate(index):
    return struct.pack("<BH", 3, index)


def reallocate(index, size, size_class=7):
    return struct.pack("<BHBI", 4, index, size_class, size)


def full_check():
    return bytes([5])


def set_budget(pages):
    return struct.pack("<BH", 6, pages)


def allocate_at_top(align=0):
    return struct.pack("<BB", 7, align)


SEEDS = {
    # A program's first allocations: initialize, then a few small blocks, freed out of order.
    "first-allocations": header(64) + initialize(64)
    + small(24) + small(100, 3) + small(7) + deallocate(1) + small(48) + full_check(),
    # Cache-line blocks, as per-CPU data asks for.
    "aligned-64": header(64) + initialize(64)
    + b"".join(small(64 * n, ALIGN_64) for n in (1, 2, 3, 5)) + deallocate(0) + small(64, ALIGN_64),
    # One block grown in place, then past what in-place growth can give, then shrunk.
    "grow-and-shrink": header(256) + initialize(256)
    + small(32) + reallocate(0, 4000) + reallocate(0, 300000) + reallocate(0, 16) + full_check(),
    # The cap: a block of nearly the whole arena, then one byte too many.
    "near-the-cap": header(128) + initialize(128)
    + allocate(4, 4096 - 64, 0) + allocate(4, 4096 + 4096, 0) + deallocate(0),
    # A block that ends at the top of the committed pages, beside the arena's end.
    "at-the-top": header(32) + initialize(32) + small(10)
    + allocate_at_top() + allocate_at_top(ALIGN_64) + deallocate(1) + allocate_at_top(),
    # A budget that runs out partway through a grow, then comes back.
    "budget-runs-out": header(512, 12) + initialize(512)
    + small(40000) + small(40000) + set_budget(UNLIMITED) + small(40000),
    # commit_all maps every page up front, as a wired heap does, then allocates without growing.
    "wired": header(64) + initialize(64) + commit_all()
    + small(1000) + small(200000) + allocate_at_top() + full_check(),
    # A second initialize, with a larger cap, which the heap ignores once it holds pages.
    "initialize-twice": header(64) + initialize(16) + small(100) + initialize(64, slot=3)
    + small(100000),
    # No region: initialize with the sentinel slot, so every grow refuses, then a real one.
    "no-region": header(16) + initialize(16, slot=0xFF) + small(8) + initialize(16) + small(8),
    # A block asked to grow past the arena's end and to nearly `isize::MAX`: the in-place check
    # must refuse both without computing an address past the arena.
    "reallocate-past-the-end": header(64) + initialize(64) + small(100)
    + reallocate(0, 8000, size_class=4) + reallocate(0, 0, size_class=6) + full_check(),
    # Empty and oversized layouts, and a large alignment.
    "odd-layouts": header(64) + initialize(64)
    + allocate(0, 0, 0) + allocate(6, 0, 0) + allocate(5, 20, 16) + reallocate(0, 33),
}


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for name, data in SEEDS.items():
        (OUT / f"{name}.bin").write_bytes(data)


if __name__ == "__main__":
    main()
