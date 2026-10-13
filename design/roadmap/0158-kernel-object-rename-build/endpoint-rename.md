# The `Endpoint` rename, as built

An appendix to [milestone 158 (build the eleven kernel object and identifier
renames)](../0158-kernel-object-rename-build.md). It holds the lane's account of
`Endpoint`/`EpId`/`EpFail` becoming `Rendezvous`/`RendezvousId`/`RendezvousFailure`: its real
surface, its scope call and how it was verified.

## The surface

This was the biggest, most-cited name. It was done to completion first, per the block's own
instruction to do that one first and measure before committing to the rest.

The real surface was much larger than the block's own table, once re-measured fresh rather than
trusted from 2026-08-23's count. It was not just the 82 occurrences of the type name across 22 `.rs`
files. It also covered:

- a whole ABI module, `abi::endpoint` -> `abi::rendezvous`. This is the
  `SEND`/`RECV`/`SEND_CAP`/`RECV_CAP`/`CALL`/`REAP`/`SURVEY` method-number namespace every user
  program and the kernel's `syscall.rs` dispatch on;
- an `objtype::ENDPOINT` -> `objtype::RENDEZVOUS` object-kind discriminant;
- `MAX_ENDPOINTS` -> `MAX_RENDEZVOUS`;
- a family of lowercase function and field names spelling the same concept: `create_endpoint` ->
  `create_rendezvous`, `endpoint_cap` -> `rendezvous_cap`, `endpoint_of` -> `rendezvous_of`, the
  `endpoints` field on `IpcTables` -> `rendezvous_table`, and several test-only helpers and test
  names.

`EpId` alone was 216 occurrences across 43 files once the type alias's own call sites were counted.
That is well past the milestone doc's "follows Endpoint's rename" note.

## The scope call

It was scoped deliberately to the identifier surface: types, functions, constants, module paths,
struct fields, test names, and backtick-quoted doc-comment citations of those identifiers. That is
the same rung `IpcTables` rename of milestone 98 (the scheduler that stopped scheduling) drew.

Informal lowercase prose that uses "endpoint" as a descriptive English word, without naming a
specific identifier, was left alone in files outside the object's own implementation. The bulk of it
is in `kernel/src/user.rs` and in the consumer crates and programs that merely narrate "an endpoint
capability". This matches 98's own recorded scope call rather than sweeping every prose sentence
tree-wide.

`crates/glob`'s unrelated "endpoint" (a character-range boundary) was checked and correctly left
untouched. It is a real in-tree homonym, not a hit.

## Verified

- Every host-testable crate builds, and its full test suite (including doctests) passes.
- The kernel builds clean for both `aarch64-unknown-none-softfloat` and
  `riscv64imac-unknown-none-elf`.
- `user`, `fs_server` and `xtask` build clean.
- The two Kani-proof-bearing crates that reference the renamed type both verify successfully
  post-rename: `crates/inter_process_communication` (6 harnesses) and `crates/capability` (12
  harnesses). That satisfies the proof obligation of milestone 69 (split `kernel/src/user.rs` by service).

`notes/ipc-naming.md` was updated to describe `Rendezvous` as the current name, with a provenance
line for a reader who remembers `Endpoint`. Its "Family resemblance" section was left untouched,
because it never named the old identifier. It compares the underlying *model* to Mach's port and
QNX's channel, not the type name.
