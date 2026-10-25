# The names in `non_volatile_memory_express`, for ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for the crate of milestone 814 (proven NVMe queue logic, released on its own). A release-track crate's names take their ratification before it leaves this tree (ruled 2026-10-25 UTC). This file lists every name a stranger meets in it, in the record format `helpers/name_release.py` reads.

This file ratifies nothing. Until the line below carries a date, the crate's own `Name:` blocks stay the record, and the status column copies them as of 2026-10-25 (UTC). `python3 helpers/name_release.py --draft non_volatile_memory_express` prints a fresh copy. At ratification each row's status takes the ruling and any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

64 names: 57 no block, 6 provisional, 1 ratified. A status of `unrecorded` means the crate holds no block for the name. The column `outside` says whether a caller outside the crate can reach the name; 0 sit in private modules without a re-export. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them.

The last column reads each name against design/naming.md's tests and 857's Fork 5, which asks for names that stand on their own because the crate may outlive nife. 21 rows say something other than `clear` or `term`. This file renames nothing.

- `clear`: a stranger reads it right.
- `term`: the specification or databook owns it, and it keeps that spelling, as design/naming.md asks of a name across somebody else's interface.
- `nife`: it names nife's own process plan (address-space windows, the handoff a server receives). A stranger has no such plan, and the crate may shed the name before it leaves.
- `scope`: it belongs to something other than the crate's subject.
- `generic`: a word that could name almost anything, design/naming.md's second failure mode.
- `unclear`: what a stranger would trip on, said in the cell.

| name | kind | path | status | outside | a stranger reads it |
|---|---|---|---|---|---|
| `non_volatile_memory_express` | crate | `src/lib.rs` | ratified 2026-09-17. | yes | clear |
| `ACQ` | item | `src/lib.rs` | unrecorded. | yes | term |
| `ADMIN_CREATE_IO_CQ` | item | `src/lib.rs` | unrecorded. | yes | term |
| `ADMIN_CREATE_IO_SQ` | item | `src/lib.rs` | unrecorded. | yes | term |
| `ADMIN_IDENTIFY` | item | `src/lib.rs` | unrecorded. | yes | term |
| `AQA` | item | `src/lib.rs` | unrecorded. | yes | term |
| `ASQ` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CAP` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CC` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CLIENT_WINDOWS` | item | `src/lib.rs` | provisional. | yes | nife: nife's own process plan |
| `CNS_CONTROLLER` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CNS_NAMESPACE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CSTS` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CSTS_CFS` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CSTS_RDY` | item | `src/lib.rs` | unrecorded. | yes | term |
| `Cap` | item | `src/lib.rs` | unrecorded. | yes | term |
| `Command` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `Completion` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `CqState` | item | `src/lib.rs` | unrecorded. | yes | unclear: completion queue is a phrase people say (the acronym test) |
| `DISK_TREE_WINDOW` | item | `src/lib.rs` | provisional. | yes | nife: nife's own process plan |
| `DOORBELL_BASE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `Doorbell` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `ESCAPE_MAGIC` | item | `src/lib.rs` | unrecorded. | yes | nife: nife's own process plan |
| `FILESYSTEM_WINDOW` | item | `src/lib.rs` | provisional. | yes | nife: nife's own process plan |
| `Handoff` | item | `src/lib.rs` | unrecorded. | yes | nife: nife's own process plan |
| `INSTALLER_WINDOW` | item | `src/lib.rs` | provisional. | yes | nife: nife's own process plan |
| `IdentifyNamespace` | item | `src/lib.rs` | unrecorded. | yes | term |
| `MAX_DSTRD` | item | `src/lib.rs` | unrecorded. | yes | term |
| `NVM_FLUSH` | item | `src/lib.rs` | unrecorded. | yes | term |
| `NVM_READ` | item | `src/lib.rs` | unrecorded. | yes | term |
| `NVM_WRITE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `SqState` | item | `src/lib.rs` | unrecorded. | yes | unclear: submission queue is a phrase people say (the acronym test) |
| `blocks_per` | item | `src/lib.rs` | unrecorded. | yes | unclear: the phrase stops before its noun |
| `bytes` | item | `src/lib.rs` | unrecorded. | yes | generic |
| `can_write` | item | `src/lib.rs` | provisional. | yes | nife: nife's own process plan |
| `cc_enabled` | item | `src/lib.rs` | unrecorded. | yes | unclear: an abbreviation of the CC register's name in a function name |
| `client_window` | item | `src/lib.rs` | provisional. | yes | nife: nife's own process plan |
| `create_io_cq` | item | `src/lib.rs` | unrecorded. | yes | unclear: `cq` needs the specification to decode |
| `create_io_sq` | item | `src/lib.rs` | unrecorded. | yes | unclear: `sq` needs the specification to decode |
| `doorbell` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `doorbell_stride` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `flush` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `from_dwords` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `head` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `holds_block` | item | `src/lib.rs` | unrecorded. | yes | nife: nife's own process plan |
| `identify` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `is_owned` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `lba_format_shift` | item | `src/lib.rs` | unrecorded. | yes | term |
| `max_queue_entries` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `min_page_size` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `new` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `note_head` | item | `src/lib.rs` | unrecorded. | yes | unclear: `note` hides what it does to the head |
| `pack` | item | `src/lib.rs` | unrecorded. | yes | nife: nife's own process plan |
| `parse_identify_namespace` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `pop` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `prp_pair` | item | `src/lib.rs` | unrecorded. | yes | term |
| `push` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `read` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `tail` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `timeout_500ms` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `transfer_command` | item | `src/lib.rs` | unrecorded. | yes | nife: nife's own process plan |
| `unpack` | item | `src/lib.rs` | unrecorded. | yes | nife: nife's own process plan |
| `write` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `confinement_attackers` | feature | `Cargo.toml` | unrecorded. | yes | nife: nife's own process plan |

Inline modules a stranger also meets, which `script/names` does not count as names of their own: `regs`.
