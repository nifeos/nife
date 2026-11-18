---
status: BUILT
built: 2026-11-18
raised: 2026-11-16
---
# 1119. The filesystem_contract falsification patches are checked, and each turns its harness red

The rule in force: each of the eleven Kani harnesses in `crates/filesystem_contract` carries a
replayable falsification patch, every patch applies to the current tree, and each turns its own
harness red on the assertion the patch names. The branch is
`milestone/1119-filesystem-contract-falsifications-repaired`; the number is provisional.

What drives it: the outside review of milestone 1090 added six harnesses and six patches, and its
pull request left the crate's patches for a separate lane to check. A survivor was suspected in
`proofs.a_resolved_request_never_widens`: a patch that changes only `Rights::resolve` is absorbed by
the `&` in `attenuate` when the harness checks the child alone.

## What is in force

- `a_resolved_request_never_widens` carries two assertions. The first, `child.bits() & !parent.bits()
  == 0`, cannot see `resolve` widen, since `attenuate` masks the answer. The second,
  `parent.resolve(dir::PARENT_RIGHTS) == parent.bits()`, can, and the patch's `Expected to fail`
  line names it. The patch stays as it is; swapping it for a patch on `attenuate` would repeat
  `attenuate_never_widens` and leave the `resolve` half of the harness without evidence.
- `script/falsifications --sweep filesystem_contract` reports 11 swept, 0 survivors, 0 stale or
  broken. Each patch was also replayed by hand with `cargo kani --harness <name>`, and the failed
  check of each is the one its `Expected to fail` line names. The two reply patches and the two
  `attenuate` patches fail inside `assert_eq!` with a message, which Kani reports as its placeholder
  message from `core::panicking::assert_failed_inner`; the harness line is the `assert_eq!` named.

## Index row

Every falsification patch of `filesystem_contract` applies and turns its own harness red on the assertion it names, so none of its eleven harnesses is a survivor.
