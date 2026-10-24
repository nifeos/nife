# Fuzzing: the three bugs

*Name: provisional. Named after the main page's own heading.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md), which states the conclusion.

The first two were found in the crate when it was named `dtb` and its type `Dtb`, and the headings
keep those names as the account of where each bug was. Both are `device_tree_blob` and
`DeviceTreeBlob` since 2026-09-18.

## 1. `dtb::Dtb::node_reg` indexed past its cell stack (out-of-bounds panic)

`node_reg` carries `#address-cells`/`#size-cells` down a 16-entry per-depth array, and matched a
node at any depth `>= 2`. When the matched node's `reg` property arrived, it read `acells[depth - 1]`
with no upper guard, so a device tree nested 17 levels deep with a matching name at the bottom
indexed `acells[16]` on a 16-element array. That is a panic in the parser the kernel runs against a
blob it did not write, before there is any way to report a failure.

Fixed by refusing to match past the stack, `(2..MAX_DEPTH).contains(&depth)`, which is what the
sibling `node_reg_compatible` already did. Clamping the index would have been worse: past `MAX_DEPTH`
the walk has stopped tracking cell counts, so the region it decoded would be arithmetic on the wrong
widths. Not finding the node is honest; finding it and reporting the wrong address is not.

**Found by reading the code while writing the fuzz target. Ten minutes of fuzzing did not find it**,
and that is recorded here rather than smoothed over. See the main page's BUGS.

## 2. `dtb::Region::end` overflowed on a hostile memory map

`end()` was `self.start + self.size` on two `u64`s straight out of the blob.
`kernel/src/memory.rs`'s `place_bitmap` calls it on every RAM region the device tree declares, and
the kernel's own test builds are dev-profile builds with overflow checks on, so a `/memory` node
claiming `start = size = u64::MAX` is a panic on the boot path. In a release build it wraps instead,
which is worse: the frame allocator gets a memory map that is quietly wrong.

Fixed twice over, at the source and at the type. `decode_reg` now refuses a wrapping pair outright
(`Error::RegionOverflow`), because a boot path should be told its memory map is impossible rather
than work around it. And `end()` saturates, because `Region` is `pub` with `pub` fields and a type
anyone can construct has to hold on its own. That second argument is not new here;
`elf::Segment::page_range` already carries it in a comment.

Found by the fuzzer, in about ten minutes from a cold corpus, starting from a mutated copy of the
committed `qemu-aarch64-virt.dtb`. Nobody had thought to look at `end()`.

## 3. `nifefs::write_image` accepted a name it could not store

A name containing a NUL byte was written into the entry's NUL-padded name field, where every reader
stops at the first NUL. So `"a\0b"` was written and read back as `"a"`, and `read("a\0b")` answered
`None`: data written, not readable, nothing panicking, no test noticing. Two names agreeing up to
their first NUL become one entry, which is exactly the collision the `NameTooLong` refusal was
introduced to stop on 2026-08-01. That fix missed this case because nobody thought to write a name
with a NUL in it.

Fixed with `Error::NameHasNul`, the same shape as `NameTooLong`.

Found by `nifefs_roundtrip` in under a minute, minimal input `[("\0", [])]`. No totality proof
could have found it, because nothing panicked: it is a *property* violation, and the property had
never been written down.
