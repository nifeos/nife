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

Name: ratified 2026-11-11 (UTC). The architect ruled each of the thirteen spellings below as the audit recommended.

## The problem

design/naming.md says a name this tree authors spells its words out. The ruling of 2026-11-11 (UTC) found `MemoryRegion::RETYPE_OBJ` in breach. It became `RETYPE_OBJECT` and kept method number 2, so no encoding and no `ABI_VERSION` changed. The audit that followed read every method of the kernel objects and found thirteen more names with the same fault. Each was a two-way door before the first release, and each touches the syscall surface, so each was the architect's call.

## The work

All thirteen are ruled (2026-11-11 (UTC)) and renamed, each with every live use: the kernel, `user_mode_runtime`, `process_builder`, components, fixtures, system tests, the std overlay, the falsification patches, the notes that describe the surface as it is, the NOT-STARTED roadmap blocks and the present-tense passages of the decisions. Every number stays, so no caller's encoding and no `ABI_VERSION` changes. Each renamed item carries a ratified `Name:` block that refuses the old spelling, and none carries a `doc(alias)` of it.

| # | Was | Now |
|---|---|---|
| 1 | `process_builder::retype_obj_from` (parameters `ut`, `objtype`) and `retype_page_frame_from` | `retype_into_kernel_object_from` (parameters `memory_region`, `object_type`) and `retype_into_page_frame_from`; the local `as retype_obj` aliases are gone, and the kernel test helper is `retype_into_kernel_object` |
| 2 | `memory_region_retype_obj` and its notification test | `memory_region_retype_object`; `the_rights_are_101s_and_retype_object_mints_a_full_rights_notification` |
| 3 | `abi::objtype` | `abi::object_type` |
| 4 | `address_space::MAP_RO` | `MAP_READ_ONLY` |
| 5 | `address_space::MAP_RW` | `MAP_READ_WRITE` |
| 6 | `interrupt::ACK` | `ACKNOWLEDGE` |
| 7 | `fault::FAULT_EP_SLOT` | `FAULT_RENDEZVOUS_SLOT`; the ABI docs that called the supervision object an endpoint say rendezvous |
| 8 to 12 | `SYS_EXIT`, `SYS_YIELD`, `SYS_INVOKE`, `SYS_CAPABILITY_DELETE`, `SYS_EXIT_THREAD` | `SYSCALL_EXIT`, `SYSCALL_YIELD`, `SYSCALL_INVOKE`, `SYSCALL_CAPABILITY_DELETE`, `SYSCALL_EXIT_THREAD` |
| 13 | `run_end_va` | `run_end_virtual_address` |

What remains:

- `FAULT_RENDEZVOUS_SLOT`'s own doc comment in `crates/abi/src/fault.rs` still says "supervision endpoint" and "a working endpoint". `helpers/format_values_check.py` reads a doc comment that changes under a rename as a changed meaning, so those two words change in a doc-only pull request with no rename beside it.
- Two spellings the audit did not list, for the architect if wanted: the kernel test `an_interrupt_raised_while_its_line_is_masked_is_delivered_at_the_ack` (and its falsification patch), and the generic IPC sentences in `crates/abi/src/rendezvous.rs` that still call the object an endpoint.

## What drives it

Nothing consumes it yet; the driver is design/naming.md's own rule, and the cost of delay is that every new caller copies the abbreviation before the first release.
