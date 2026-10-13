# How extended attributes are proven

An appendix to [`notes/xattr.md`](../xattr.md). It holds the host tests, the crash-consistency
sweep, the on-device witness, and the three caretaker witnesses of milestone 61 (the caretakers),
each with the control that must fail.

## The layer and the engine

Host, milliseconds, no emulator. The whole of the layer's semantics is pure functions over a
byte slice (`filesystem_protocol::xattr::store`). So what replaces what, which ceiling refuses, and
what a truncated blob means are all tested without a filesystem. There are seven tests in
`filesystem_protocol`, including a sweep that cuts a blob at every byte and asserts it reads back
*short rather than wrong*. Ten more in `redoxfs_server` drive the real engine against a
`DiskMemory` image. They cover:

- persistence across a mount the attribute was not written in;
- the rename property;
- the purge with a provoked node-id reuse, and the replaced-destination purge;
- the store's invisibility to seven verbs and to a listing;
- both rights directions and every ceiling;
- and the shrink-without-a-tail.

Crash-consistent, measured rather than argued (milestone 57 (partitioning and formatting a real
drive, and extended attributes)). Every mutation runs inside one `fs.tx`, and the sweep of milestone
37 (prove RedoxFS's crash consistency) proves prefix consistency for whatever a transaction
contains, so the property holds by construction, and it is also in the sweep.
`redoxfs_server/tests/crash_consistency.rs` reads each name's attributes as part of the filesystem's
state. The workload has four attribute operations covering the three shapes the store has: creating
it (two node creations in one commit), growing a blob, and shrinking one, which is the path that
must truncate afterwards. They are interleaved with a write to the same file, so the claim being
decided is the interesting one. An attribute lives in a *different file* from the data it describes,
and a recovery holding the new bytes without the new attribute (or the reverse) is a state that
never existed and fails the sweep.

The workload's own sanity check does double duty here. An attribute operation changes nothing
observable unless the snapshot reads attributes, so a harness that quietly stopped looking fails at
the fixture rather than passing everywhere below it.

On device, both ISAs (DECISIONS §19 (architectural parity is a tenet)). `fixtures/src/fs_test_client/proof.rs`, the proof role, carries a witness that
reports a bitmap, and the kernel test asserts an exact set, so a client that could do nothing
and one that could do everything both fail. It checks eight claims:

1. an attribute is set and read back *with the type code*;
2. it is listed, and nothing else is;
3. it survives a rename;
4. it is gone after a remove;
5. it is gone after an unlink and remake;
6. an over-long value is refused with `E2BIG`;
7. the store is unnameable;
8. the store is absent from a full cursor-driven enumeration of the root.

It rides the existing FS-service test's third report word
rather than booting the three-process stack a second time.

## The caretakers (milestone 61)

How it is proven, both ISAs, three witnesses, each with a control that must fail:

- A read-only per-file grant lists and gets (so the verbs reach the store) and its `SETXATTR` is
  refused. A writable grant of the same shape sets, reads back with the type code, and removes.
  Without the second run the first is equally consistent with a caretaker that refuses everything.
- A read-only subtree grant reads attributes and cannot set one; the full and
  append-only runs (both carrying `dir::WRITE`) can. Same three configurations milestone 47's
  rights ladder already used, one bit wider.
- A name-set grant of exactly one name reads and writes that file's attributes and still cannot
  open the entry beside it, which is the naming property asked with `READ` rather than with `rm`'s
  `REMOVE`.
