# Capability, spelled out: the worklist

*Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `capability-worklist`.*

*An appendix to [`design/naming.md`](../naming.md), which is the rule, and the worklist for the
ruling recorded in [spelled-out-rulings.md](spelled-out-rulings.md#the-tree-spells-capability-never-cap).
It is a dated census of the tree on 2026-10-06 (UTC), taken before any rename. The counts will
drift as lanes land; `python3 helpers/cap_abbreviation.py --list` prints the live list. The file's
stem is a provisional name; naming is an
architect's.*

Every spelled-out name below is a proposal for calef. None has been ruled on.

## The count

On 2026-10-06, 2,890 identifier occurrences in tracked `.rs` files abbreviate capability as `cap`
or `caps`. That is the ceiling `helpers/cap_abbreviation.py` holds, and the sweep lowers it with
`--bank`. Occurrences, not definitions: a name used fifty times counts fifty.

| File group | Occurrences | The largest names |
|---|---|---|
| `kernel/` | 993 | `cap` 293, `rendezvous_cap` 200, `Cap` 58, `memory_region_cap` 43 |
| `system_tests/` | 810 | `cap` 314, `rendezvous_cap` 124, `delete_current_cap` 54 |
| `crates/` other than the ABI | 754 | `caps` 176, `cap_delete` 104, `cap` 93, `CapNeed` 42 |
| `components/` | 212 | `cap_delete` 99, `caps` 50, `send_cap` 17 |
| `fixtures/` | 88 | `cap_delete` 22, `receive_cap` 17, `caps` 16 |
| `crates/abi/`, `crates/user_mode_runtime/` | 33 | the system call names below |
| **Total** | **2,890** | |

The kernel module `cap` (`kernel/src/cap.rs`) is the single biggest source: 364 of the 730
bare `cap` hits are its path, `crate::cap::` or `super::cap::`.

## What the gate does not count

Comments and string literals are skipped, so "a cap on lane count" is never read. The gate keeps
two kinds of identifier, listed by file in `KEPT` in the helper. Both are proposals here too:

- Ceilings, where `cap` is the English word for a limit or `CAP` a buffer's size. `heap_cap`,
  `HIT_CAP` and `cap_mib` in `redoxfs_server/src/bin/second_mount.rs`, `PRINT_CAP` in
  `kernel/src/sched.rs`, `CAP` in `components/src/printenv.rs` and `kernel/src/stack.rs`,
  `FMT_CAP` in `crates/calendar`, `CALIBRATION_WINDOW_CAP` in the x86_64 timer, `cap_given` in
  `xtask/src/board.rs`, and four test names that use the word.
- Register and field names a specification coined, after the `operation` ruling's rule that an
  identifier mirroring an external API keeps its spelling. VT-d's `CAP` and its fields, the RISC-V
  IOMMU's `CAPS`, NVMe's `CAP` and its `Cap` decoder, IEEE 802.3's `*_CAPS` ability bits in
  `crates/e1000e`, PCI's `CAP_PTR`, `STATUS_CAP_LIST` and `CAP_ID_*`, and virtio's `VIRTIO_CAP_*`.

If calef rules a kept name should be spelled out after all, it leaves `KEPT` and joins the sweep.

Not counted, and on the list by hand: `ncaps` (15 sites, in `crates/capability_plan` and
`crates/system_initializer`) has no separator, so the gate reads it as one word. Propose
`capability_count`.

## Proposed names

The pattern is mechanical, and the sweep applies it everywhere unless a row says otherwise: `cap`
becomes `capability`, `caps` becomes `capabilities`, `Cap` becomes `Capability`, `CAP` becomes
`CAPABILITY`. The rows below are the public names and the ones where the pattern needs a word chosen.

### Kernel

| Today | Proposed | Note |
|---|---|---|
| module `cap` (`kernel/src/cap.rs`) | `capability_object` | `capability` would shadow the `capability` crate the module imports. The file's own header calls it "the set of nouns" a capability points at. |
| `rendezvous_cap`, `memory_region_cap`, `page_frame_cap`, `page_frame_run_cap`, `device_frame_cap`, `port_range_cap`, `irq_cap`, `timer_cap`, `notification_cap`, `address_space_cap`, `thread_control_block_cap`, `virtio_cap`, `reply_cap`, `memory_region_root_cap` | `rendezvous_capability` and so on | the constructors in the module |
| `rendezvous_cap_badged`, `irq_cap_rights`, `virtio_cap_rights`, `memory_region_cap_rights` | `rendezvous_capability_badged` and so on | |
| `current_cap`, `delete_current_cap`, `enter_current_cap`, `leave_current_cap`, `current_cap_totals` | `current_capability` and so on | |
| `thread_control_block_insert_cap`, `thread_control_block_cap_insert`, `thread_control_block_delegate_cap` | `thread_control_block_insert_capability` and so on | |
| `ipc_send_cap`, `ipc_send_cap_from`, `ipc_receive_cap`, `ipc_delegate_cap` | `ipc_send_capability` and so on | |
| `delete_page_frame_caps`, `delete_port_range_caps` and their `_where`, `_overlapping`, `_from_others`, `_impl` forms, `delete_reply_caps_naming` | `delete_page_frame_capabilities` and so on | |
| fields `outgoing_cap`, `receiving_cap`, `cap_delivered`, `fault_cap` | `outgoing_capability` and so on | |
| `in_current_cap`, `cap_calls`, `cap_contended`, `cap_wait_ticks` (`kernel/src/lock_wait.rs`) | `in_current_capability`, `current_capability_calls`, `current_capability_contended`, `current_capability_wait_ticks` | they count `current_cap`'s lock, so the name says which |
| `K_CR_SERVER_RECEIVE_CAP`, `cap_delete_then_port_out`, `CAP_DELETE_THEN_PORT_OUT_PC_OFFSET` | follow `RECEIVE_CAPABILITY` and `capability_delete` | |

### Crates

| Today | Proposed | Note |
|---|---|---|
| `CapNeed` (`crates/capability_plan`) | `CapabilityNeed` | |
| `MAX_CAPS`, `TooManyCaps`, `FULL_CAPS`, `OVER_CAPS`, `CAP_TWICE`, `caps()` | `MAX_CAPABILITIES`, `TooManyCapabilities` and so on | |
| `Command::Caps` (`crates/grant_plan`) | `Command::Capabilities` | see the swish builtin below. `CapKind` is done: it became `CapabilityKind` on 2026-10-11 (UTC), #1913 |
| `CAP_TAG` (`crates/grant_plan/src/spawnproto.rs`) | `CAPABILITY_TAG` | the value `0x6361_705f` stays: it is a wire marker |
| `CAP_WRITE`, `CAP_USER`, `CAP_USER_EXEC`, `CAP_KERNEL_EXEC`, `CAP_DEVICE`, `CAP_GLOBAL`, `CAP_WRITE_COMBINE`, `from_caps` (`crates/paging`) | `ACCESS_WRITE` and so on, `from_access` | private bits for what a mapping grants; `CAPABILITY_WRITE` would read as an object capability, so propose `ACCESS_`. A word for calef. |
| `MsiCap`, `MsixCap`, `VirtioCap`, `msi_cap`, `msix_cap`, `virtio_caps`, `cap_offset`, `cap_id` (`crates/pci`, `kernel/src/pci.rs`) | `MsiCapability`, `MsixCapability` and so on | the tree's own names for PCI capabilities; the spec's register names are kept |
| `GraphicalTerminalCaps`, `graphical_terminal_caps`, `drop_caps`, `n_caps`, `opt_cap`, `sh_caps`, `con_caps`, `in_caps` (`crates/system_initializer`) | `GraphicalTerminalCapabilities`, `drop_capabilities`, `capability_count` for `n_caps`, and so on | |
| `write_caps`, `write_image_caps`, `caps_image` (`crates/swish`, `components/src/swish.rs`) | `write_capabilities` and so on | |
| `cap_slot` (`components/src/net_stack.rs`, `crates/user_mode_runtime`) | `capability_slot` | |

### Tests and fixtures

`MCap` and `SendCap` in `system_tests/src/user/syscall_fuzzer_tests.rs` become `ModelCapability`
and `SendCapability`. `with_cap`, `WITH_CAP`, `report_cap`, `cap_theirs`, `cap_mine`, `real_cap`
and `abstract_cap` follow the pattern. The file `receive_cap_attack_tests.rs` and the test
functions named after `send_cap`, `receive_cap` and `cap_insert` follow the ABI batch, since their
names quote it.

### The last batch: the ABI and the `Cap` type

| Today | Proposed |
|---|---|
| `SYS_CAP_DELETE` | `SYS_CAPABILITY_DELETE` |
| `CapSlot` | `CapabilitySlot` |
| `SEND_CAP`, `RECEIVE_CAP`, `NO_CAP`, `CAP_INSERT` | `SEND_CAPABILITY`, `RECEIVE_CAPABILITY`, `NO_CAPABILITY`, `CAPABILITY_INSERT` |
| `cap_delete`, `receive_cap`, `receive_cap_badged`, `receive_cap_bound`, `try_receive_cap` (`crates/user_mode_runtime`) | `capability_delete`, `receive_capability` and so on |
| `Cap<O>` (`crates/capability`), `type Cap` (`kernel/src/cap.rs`) | `Capability<O>`, `Capability` |

No method number, constant value or wire layout changes. These names cross every program and the
kernel at once, so they go in one batch, after the rest has landed.

Two left the batch early. calef ratified `send_capability` (was `send_cap`) and
`thread_control_block_capability_insert` (was `tcb_cap_insert`) on 2026-10-08 (UTC), #1842, and
milestone 139 (drive the unsafe count down) applied both on 2026-10-12.

## Sweep order

1. Locals, fields and private items, one file group at a time: `kernel/`, `crates/`,
   `components/`, `fixtures/`, `system_tests/`. Each batch is mechanical and banks the ceiling.
2. The kernel's public functions and the `cap` module.
3. The crates' public types (`CapNeed`, the PCI and paging names; `CapKind` is done, #1913).
4. Last, as one batch: the ABI names and the `Cap` type, with the tests named after them.

Closed records (`BUILT` roadmap blocks, `design/decisions/`, dated reports) keep the name they used,
as the `operation` ruling did, so a grep for the old name still finds them. Live notes that cite a
renamed identifier move with it; [rename-what-moves.md](rename-what-moves.md) is the procedure.

## Open, outside the identifier sweep

The swish builtin a person types as `caps` is a command name, not an identifier, so neither this
ruling's gate nor its sweep touches it. Whether it becomes `capabilities` is a separate naming
question for calef, and its twelve `caps_*` test names follow whichever word he picks.
