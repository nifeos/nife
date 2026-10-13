# The `Tcb` rename, as built

An appendix to [milestone 158 (build the eleven kernel object and identifier
renames)](../0158-kernel-object-rename-build.md). It holds the lane's account of `Tcb`/`Tid`
becoming `ThreadControlBlock`/`ThreadId`, with their companions: its real surface, what it left
alone and how it was verified.

## The surface

`Tcb`/`Tid` -> `ThreadControlBlock`/`ThreadId`, with companions `TcbPtr` ->
`ThreadControlBlockPointer` and `TidSet` -> `ThreadIdSet`. It was done to completion in a separate
lane (milestone/158-tcb-rename).

The strict PascalCase count landed close to the block's own stale table for once. `Tcb` was 12
files and 33 occurrences against an estimated 12 and 28. `Tid` was 13 files and 82 against an
estimated 12 and 76. But that comparison undersells the real surface the same way it did for
`Endpoint`: neither of those counts saw the lowercase identifier family at all.

The real surface, re-measured:

- an ABI module, `abi::tcb` -> `abi::thread_control_block`. This is the
  `CONFIGURE`/`CAP_INSERT`/`START` method-number namespace every spawn path in the kernel and every
  child-building user program dispatches through;
- an `objtype::TCB` -> `objtype::THREAD_CONTROL_BLOCK` object-kind discriminant;
- a family of lowercase function and field names spelling the same concept: `tcb_cap` ->
  `thread_control_block_cap`, `create_tcb` -> `create_thread_control_block`, `configure_tcb` ->
  `configure_thread_control_block`, `tcb_insert_cap` -> `thread_control_block_insert_cap`,
  `start_tcb` -> `start_thread_control_block`, and `tcb_ptr` -> `thread_control_block_ptr`;
- `tcb_configure`/`tcb_cap_insert` in `syscall.rs`'s dispatch, `tcb_start` in `supervision_proto`
  (now `supervision_protocol`) and `system_initializer`, and the `Thread` struct's own
  `tcb_kmem`/`tcb_region` fields;
- the `Tid` half's own compound names: `current_tid` -> `current_thread_id`, `set_current_tid` ->
  `set_current_thread_id`, `write_tid` -> `write_thread_id` in `crates/ps`, and five test names
  built the same way.

47 files changed, 385 insertions, 376 deletions.

## Left alone

This matched `Endpoint`'s own scope call.

- Bare short local variable and parameter names that are exactly `tid` or `tcb`, with no further
  word attached. This is the same restraint the `Endpoint` rename used, keeping `ep` unchanged
  throughout `kernel/src/sched.rs`.
- Informal lowercase "tcb"/"TCB" prose describing the concept in English rather than naming the
  identifier. `fixtures/src/hello.rs`'s "(endpoint | aspace | tcb)" list stayed lowercase, matching
  how "endpoint" stayed lowercase there after that rename.
- One historical citation in `crates/abi/src/lib.rs`'s own crate-naming rationale, which names
  `Tcb`/`Aspace`/`Untyped` as the abbreviations a naming review "sank". Renaming that citation to
  `ThreadControlBlock` would have made the sentence describe the winning name as the one that lost.
  So it was reverted to the old name it is actually citing.

`notes/tcb.md` (the object's own note) got a provenance line the same way `notes/ipc-naming.md` did.
Its "acronym collision" section (TCB as Thread Control Block vs. Trusted Computing Base) is
unaffected either way, since both senses are already informal usage.

## Verified

The same four ways as `Endpoint`:

- the full host test suite green, including doctests;
- both kernel targets build clean;
- `user`/`fs_server`/`xtask` build clean;
- `crates/inter_process_communication` (6 harnesses) plus `crates/capability` (12 harnesses) both
  re-verify under Kani post-rename.
