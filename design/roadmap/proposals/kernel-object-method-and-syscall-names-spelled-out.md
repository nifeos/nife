---
status: PROPOSED
raised: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# The remaining kernel-object method and syscall names are spelled out

Raised by lane/record-open-follow-ups. Pull request #3483 (`RETYPE_OBJ` becomes `RETYPE_OBJECT`) listed these thirteen names in its "What I need from you" section and held them for the architect.

Reuse: the rename discipline of design/naming/rename-what-moves.md and the `Name:` blocks, and the ratified `retype_into_kernel_object`. No new tool or dependency.

Name: provisional. The recommended spellings below are the lane's, not rulings.

## The problem

design/naming.md says a name this tree authors spells its words out. The ruling of 2026-11-11 (UTC) found `MemoryRegion::RETYPE_OBJ` in breach. It became `RETYPE_OBJECT` and kept method number 2, so no encoding and no `ABI_VERSION` changed. The audit that followed read every method of the kernel objects and found thirteen more names with the same fault. None has changed. Each is a two-way door before the first release. Each also touches the syscall surface, so each is the architect's call, one at a time, recommendation first.

On 2026-10-08 the architect refused `retype_object` for the runtime function and ratified `retype_into_kernel_object`. Items 1 and 2 derive from that.

## The work

For each item the architect rules, one lane renames it and every live use in the same commit. Live uses are the kernel, `user_mode_runtime`, `process_builder`, components, fixtures, system tests, the std overlay, the notes that describe the surface as it is and the NOT-STARTED roadmap blocks. The number stays, so no caller's encoding changes. Each rename carries a `Name:` block and no `doc(alias)` of the old spelling, and the present-tense passages of the decisions move with it. A refused name stays out.

| # | Name | Where | Recommended |
|---|---|---|---|
| 1 | `retype_obj_from` | `process_builder`, the helper every builder calls; local aliases in `login`, `user_timetable_keeper`, `system_initializer`; the `retype_obj` helper in `confined_fuzzer_tests.rs` | `retype_into_kernel_object_from`, pairing with `retype_into_kernel_object` as `retype_page_frame_from` pairs with `retype_into_page_frame` |
| 2 | `memory_region_retype_obj` | kernel handler in `kernel/src/syscall/retype.rs`; one test name | `memory_region_retype_object`, matching `memory_region_retype` and `memory_region_split` |
| 3 | `abi::objtype` | the module of kernel object kinds, and the argument word in every `RETYPE_OBJECT` doc | `abi::object_type`; "objtype" joins two abbreviations |
| 4 | `address_space::MAP_RO` | `MAP_INTO`'s read-only mapping word | `MAP_READ_ONLY` |
| 5 | `address_space::MAP_RW` | the read-write word | `MAP_READ_WRITE` |
| 6 | `interrupt::ACK` | method 1 on an interrupt capability | `ACKNOWLEDGE`; the runtime wrapper is already `interrupt_acknowledge` |
| 7 | `fault::FAULT_EP_SLOT` | the last capability slot, naming a thread's fault rendezvous | `FAULT_RENDEZVOUS_SLOT`; `EP` abbreviates "endpoint" and the object is now a rendezvous |
| 8 | `SYS_EXIT` | syscall 0 | `SYSCALL_EXIT`; `SYS` abbreviates "syscall", which design/naming.md keeps |
| 9 | `SYS_YIELD` | syscall 1 | `SYSCALL_YIELD` |
| 10 | `SYS_INVOKE` | syscall 2 | `SYSCALL_INVOKE` |
| 11 | `SYS_CAPABILITY_DELETE` | syscall 3 | `SYSCALL_CAPABILITY_DELETE` |
| 12 | `SYS_EXIT_THREAD` | syscall 4 | `SYSCALL_EXIT_THREAD` |
| 13 | `run_end_va` | a helper of the address-space handlers in `kernel/src/syscall/address_space.rs`, not a method; a falsification patch names it too | `run_end_virtual_address`; `va` is on the spelled-out list |

Items 8 to 12 share one prefix, so the architect may rule them together.

The audit read and passed the methods of `MemoryRegion` (`MAP`, `RETYPE`, `SPLIT`, `DESTROY`, `USAGE`), `Rendezvous`, `Reply`, `Notification`, `Timer`, `ThreadControlBlock`, `AddressSpace` (`MAP_INTO`, `LIST`, `UNMAP`, `WAIT`, `WAKE`), `PageFrame`, `PortRange`, `Process`, `Reboot`, `Console`, `Virtio` and `Usage`, the survey words and the `Error` variants. `futex` is a term of art said as a word, as `syscall` is, and `virtio` keeps its specification's spelling.

## What drives it

Nothing consumes it yet; the driver is design/naming.md's own rule, and the cost of delay is that every new caller copies the abbreviation before the first release.
