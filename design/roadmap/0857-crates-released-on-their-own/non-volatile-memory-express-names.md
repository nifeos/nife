# The names in `non_volatile_memory_express`

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md),
for the crate of milestone 814 (proven NVMe queue logic, released on its own). This file is the record
of every name in the crate, which `helpers/name_release.py` reads in place of `Name:` blocks: the
crate's shipped files carry none.

Ratified: 2026-10-25 (#2720).

The ruling of that date ratified every name below but the two marked provisional. Each row with a
refusal names the spelling the ruling turned down and why. nife's process plan sits outside the
crate: the client windows, `client_window`, `MAX_DSTRD` (the doorbell stride one mapped page of BAR0
can serve), `ESCAPE_MAGIC`, `Handoff` with `pack` and `unpack`, and the `confinement_attackers`
feature are in the `non_volatile_memory_express` module of the nife-internal crate
`driver_process_contract`, which does not publish (ruled 2026-10-26 UTC). `can_write` sits in nife's
NVMe server. Those names follow nife's ordinary naming and
have no row here. A new name in this crate takes a row here, `provisional` until a ruling, and
`script/lint` fails a module or `pub` item without one.

The column `outside` says whether a caller outside the crate reaches the name. Some names have no row
because `script/names` does not count them: the variants of `Doorbell` (`SubmissionTail`,
`CompletionHead`), the fields of `Completion` and `IdentifyNamespace`, and the inline modules
`regs`, `tests` and `verification`. The same ruling covers them; the opcodes and register names keep the specification's spelling.

| name | kind | path | status | outside |
|---|---|---|---|---|
| `non_volatile_memory_express` | crate | `src/lib.rs` | ratified 2026-09-17 (#1806). NVMe spelled out, as the acronym test asks; permanent once published. | yes |
| `ACQ` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `AQA` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `ASQ` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `CAP` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `CC` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `CSTS` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `DOORBELL_BASE` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | no |
| `CSTS_RDY` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `CSTS_CFS` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `Cap` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `max_queue_entries` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `doorbell_stride` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `timeout_500ms` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | no |
| `min_page_size` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `enabled_controller_configuration` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). Refused `cc_enabled` (it returns the Controller Configuration value that enables the controller, which `cc_enabled` did not say). | yes |
| `Doorbell` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `doorbell` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `ADMIN_IDENTIFY` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | no |
| `ADMIN_CREATE_IO_CQ` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | no |
| `ADMIN_CREATE_IO_SQ` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | no |
| `NVM_READ` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `NVM_WRITE` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | no |
| `NVM_FLUSH` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | no |
| `CNS_NAMESPACE` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `CNS_CONTROLLER` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | no |
| `Command` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `identify` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `create_io_completion_queue` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). Refused `create_io_cq` (spelled out, as the acronym test asks). | yes |
| `create_io_submission_queue` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). Refused `create_io_sq` (spelled out, as the acronym test asks). | yes |
| `read` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `flush` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `write` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `prp_pair` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `holds_block` | item | `src/lib.rs` | provisional. No ruling covers the name. | yes |
| `transfer_command` | item | `src/lib.rs` | provisional. No ruling covers the name. | yes |
| `Completion` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `from_dwords` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `SubmissionQueue` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). Refused `SqState` (an abbreviation, and the type is the queue, not a state beside it). | yes |
| `new` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `push` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `tail` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `record_head` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). Refused `note_head` (it records the head the controller reported). | yes |
| `CompletionQueue` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). Refused `CqState` (the same reason as `SqState`). | yes |
| `new` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `head` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `is_owned` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `pop` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `parse_identify_namespace` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `lba_format_shift` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `IdentifyNamespace` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). | yes |
| `capacity_bytes` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). Refused `bytes` (it is the namespace's capacity in bytes, not its bytes). | yes |
| `blocks_per_transfer` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). Refused `blocks_per` (it did not say per what). | yes |
