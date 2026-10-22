# Boolean predicates: the worklist

*An appendix to [`design/naming.md`](../naming.md), which is the rule, and a companion to
[functions-that-answer-yes-or-no.md](functions-that-answer-yes-or-no.md), which argues it. This file is the census the pass
took on 2026-09-24 and what became of each name. It is a dated record: the counts are from that
day's tree, and a later reader should expect them to drift. The file's stem is a provisional name; naming is an architect's.*

## How the census was taken

Every function returning `bool` in every tracked `.rs` file outside `vendor/`, found by a scanner
that strips comments and strings, follows braces, and reads each signature to its body. Test
functions and helpers inside `#[cfg(test)]` or `#[cfg(kani)]` modules were set aside, since test
names are sentences here. The std overlay under `patches/std-nife/` was added by hand after a grep,
because the scanner skipped `patches/`.

**454 functions returned `bool`. 434 were not test code, and 316 of those did not begin with
`is_`, `has_`, `can_` or a std relation name.** Reading each doc comment sorted the 316:

| Class | Scanned | Std overlay | What happens |
|---|---|---|---|
| Renamed, the rule applied as written | 87 | 3 | the table below; 734 sites in all |
| A word needs choosing, so an architect's | 9 | 1 | [What needs an architect](#what-needs-an-architect) |
| Fits already: a finite verb phrase or a quantifier | 57 | 0 | [Fits as written](#fits-as-written) |
| Exempt: an action that reports whether it worked | 162 | 1 | [Actions](#actions) |
| Exempt for another reason | 1 | 1 | [Other exemptions](#other-exemptions) |
| **Total** | **316** | **6** | |

The counts are functions, so a name defined once per architecture counts three times. "Sites" is
every occurrence the rename rewrote: definitions, calls, doc links, falsification patches and notes
that describe current code. Roadmap blocks, decisions and mutation-testing measurements keep the
names they were written with, per the rename procedure.

## Renamed

| Was | Defined in | Visibility | Sites | Now | Landed in | Notes |
|---|---|---|---|---|---|---|
| `tick_pending` | `kernel/src/arch/{aarch64,riscv64,x86_64}/timer.rs` | pub | 18 | `is_tick_pending` | this pull request | the ruling's own example; doc comments say ratified |
| `timer_pending` | `kernel/src/arch/x86_64/irq.rs` | pub | 3 | `is_timer_pending` | this pull request | doc comment says ratified |
| `enabled` | `kernel/src/arch/*/interrupts.rs` | pub | 21 | `is_enabled` | kernel follow-up |  |
| `active` | `kernel/src/iommu.rs, kernel/src/arch/*/iommu.rs` | pub | 15 | `is_active` | kernel follow-up |  |
| `live` | `kernel/src/arch/*/fp.rs` | pub | 7 | `is_live` | kernel follow-up | the `live` flag it reads keeps its name |
| `cycle_counter_grantable` | `kernel/src/arch/*/timer.rs` | pub | 9 | `is_cycle_counter_grantable` | kernel follow-up |  |
| `from_lower_el` | `kernel/src/arch/aarch64/exceptions.rs` | private | 7 | `is_from_lower_el` | kernel follow-up | notes/stack/interrupt-stack.md's measurement keeps the old name, with the new one beside it |
| `v3` | `kernel/src/arch/aarch64/irq.rs` | private | 7 | `is_v3` | kernel follow-up |  |
| `pmuv3_present` | `kernel/src/arch/aarch64/pmu.rs` | pub(super) | 6 | `is_pmuv3_present` | kernel follow-up |  |
| `local_apic_ready` | `kernel/src/arch/x86_64/irq.rs` | pub | 10 | `is_local_apic_ready` | kernel follow-up |  |
| `reachable` | `kernel/src/arch/x86_64/machine.rs` | private | 5 | `is_reachable` | kernel follow-up |  |
| `update_in_progress` | `kernel/src/arch/x86_64/rtc.rs` | private | 3 | `is_update_in_progress` | kernel follow-up |  |
| `rx_waiting` | `kernel/src/console.rs, drivers/{ns16550,pl011}.rs` | pub | 18 | `is_byte_waiting` | kernel follow-up | first renamed `is_rx_waiting`; calef's review of the crates follow-up (2026-09-24) asked what `rx` stands for, so both UART predicates became `is_byte_waiting` |
| `full`, `done` | `kernel/src/ipc_stack_depth.rs` | private | 7 | `is_full`, `is_done` | kernel follow-up |  |
| `host_bridge_present` | `kernel/src/pci.rs` | private | 13 | `is_host_bridge_present` | kernel follow-up |  |
| `thread_present` | `kernel/src/sched.rs` | pub | 46 | `is_thread_present` | kernel follow-up | seven notes point at it and move; roadmap blocks keep it |
| `intact` | `kernel/src/stack.rs` | pub | 5 | `is_intact` | kernel follow-up |  |
| `expired` | `kernel/src/testing.rs` | pub | 4 | `is_expired` | kernel follow-up | takes `&mut self` (it re-anchors), which clippy allows for `is_` |
| `instruction_backend_available` | `kernel/src/user/entropy_service.rs` | private | 7 | `is_instruction_backend_available` | kernel follow-up | three per-architecture copies |
| `crash_disk_present` | `kernel/src/user/fs_service.rs` | pub | 6 | `is_crash_disk_present` | kernel follow-up |  |
| `doorbell_ready` | `kernel/src/virtio.rs` | private | 11 | `is_doorbell_ready` | kernel follow-up |  |
| `known`, `plausible` | `crates/clock_contract` | pub | 23 | `is_known`, `is_plausible` | crates follow-up | the std overlay's clock calls `state::is_known` too |
| `authenticated` | `crates/credential_contract` | pub | 19 | `is_authenticated` | crates follow-up | falsification patch prose moves |
| `writable`, `valid_name` | `crates/filesystem_contract` | pub | 18 | `is_writable`, `is_valid_name` | crates follow-up | consumers in two other workspaces (redoxfs_server, tools/redoxfs_host) |
| `recognized` | `crates/environment_contract` | private | 4 | `is_recognized` | crates follow-up |  |
| `in_region` | `crates/dma_validator` | pub | 24 | `is_in_region` | crates follow-up | four patches move; the harness `in_region_is_sound` keeps its name |
| `deasserted`, `clock_enabled` | `crates/jh7110_clock_and_reset` | pub | 21 | `is_deasserted`, `is_clock_enabled` | crates follow-up |  |
| `clocks_running` | `crates/jh7110_clock_and_reset` | pub | 8 | `has_clocks_running` | crates follow-up | plural subject: Rust has no `are_` |
| `checksum_ok` | `crates/machine_discovery/src/acpi.rs` | pub | 11 | `is_checksum_ok` | crates follow-up | one patch moves |
| `startable` | `crates/machine_discovery/src/cpu_list.rs` | pub | 16 | `is_startable` | crates follow-up |  |
| `screen_hold` | `crates/machine_discovery/src/framebuffer.rs` | pub | 9 | `has_screen_hold` | crates follow-up | the `screen_hold` Cargo feature keeps its name |
| `answered` | `crates/machine_discovery/src/riscv64.rs` | pub | 11 | `has_answered` | crates follow-up |  |
| `heterogeneous` | `crates/machine_discovery/src/riscv64.rs` | pub | 6 | `is_heterogeneous` | crates follow-up |  |
| `rdseed` | `crates/machine_discovery/src/x86_64.rs` | pub | 11 | `has_random_seed_instruction` | crates follow-up | first renamed `has_rdseed`; calef's review asked what `rdseed` is. Intel's mnemonic `RDSEED` stays where it names the instruction, the CPUID bit, or Linux's `rdseed` flag; `draw_rdseed` became `draw_random_seed` in the same change |
| `armed_hint` | `crates/memory_corruption_canary_gate` | pub | 8 | `is_armed_hint` | crates follow-up |  |
| `owned` | `crates/non_volatile_memory_express` | pub | 13 | `is_owned` | crates follow-up |  |
| `in_half` | `crates/paging (trait `PageFormat`)` | trait | 52 | `is_in_half` | crates follow-up | an in-tree trait, so every impl moves; seven patches move |
| `delivered` | `crates/thread_wake_handshake` | pub | 8 | `is_delivered` | crates follow-up | `entropy_contract::delivered` returns `Option` and keeps its name |
| `quoted`, `spent`, `from_root`, `interruptible`, `open` | `crates/grant_plan` | pub | 39 | `is_quoted`, `is_spent`, `is_from_root`, `is_interruptible`, `is_open` | crates follow-up |  |
| `ok` | `crates/swish (`Status`)` | pub | 6 | `is_ok` | crates follow-up | matches `Result::is_ok` |
| `refused`, `complete` | `crates/ps, crates/pmap` | pub | 46 | `is_refused`, `is_complete` | crates follow-up |  |
| `truncated` | `crates/documentation (x2), crates/machine_discovery` | pub | 21 | `is_truncated` | crates follow-up |  |
| `unclosed_fence` | `crates/documentation` | pub | 5 | `has_unclosed_fence` | crates follow-up |  |
| `ink` | `crates/bitmap_font (and its example)` | pub | 25 | `is_ink` | crates follow-up |  |
| `seen_in`, `relocated`, `finished` | `crates/serial_console` | pub | 23 | `is_seen_in`, `is_relocated`, `is_finished` | crates follow-up |  |
| `bootable` | `crates/boot_slot` | pub | 10 | `is_bootable` | crates follow-up |  |
| `block_size_ok` | `crates/globally_unique_identifier_partition_table` | pub | 12 | `is_block_size_ok` | crates follow-up |  |
| `considered` | `crates/stick_maker` | private | 2 | `is_considered` | crates follow-up |  |
| `granted` | `crates/user_mode_runtime, components/src/printenv.rs` | pub | 28 | `is_granted` | crates follow-up | five programs and `system_initializer` import it |
| `granted`, `config_granted`, `reachable` | `patches/std-nife/overlay (time, env, fs)` | private | 19 | `is_granted`, `is_config_granted`, `is_reachable` | crates follow-up | compiled only in the patched std build |
| `rx_pending` | `components/src/input.rs` | pub (in a program) | 4 | `is_byte_waiting` | crates follow-up | first renamed `is_rx_pending`; see `rx_waiting` above |
| `output_correct`, `absent` | `fixtures (c_confiner, login_test_client)` | private | 4 | `is_output_correct`, `is_absent` | crates follow-up |  |

## What needs an architect

Each of these answers a question, so the rule covers it, but adding `is_` to the existing words
does not produce a name that reads. Choosing the words is a naming decision. All are function
names inside one crate or program, so each is cheap to change later.

| Now | Defined in | Visibility | Recommended | Why a word changes |
|---|---|---|---|---|
| `check_descriptor` | `crates/dma_validator` | pub | `is_descriptor_allowed` | `check_` reads as an action; the function only answers |
| `one_queue_invariant` | `crates/inter_process_communication` | pub | `one_queue_invariant_holds` | a noun phrase; the verb form keeps the invariant's name visible in Kani harnesses and two patches |
| `same_bytes` | `crates/grant_plan` | private, `const` | `bytes_eq` | std's `*_eq` shape (`ptr_eq`, `addr_eq`); a `const` helper because `==` on slices is not `const` |
| `shift` | `crates/video_terminal` (`Modifiers`) | pub | `is_shift_held` | `is_shift` does not parse |
| `reverse` | `crates/video_terminal` (`Attributes`) | pub | `is_reverse_video` | `is_reverse` reads as a direction |
| `translate_as_el0` | `kernel/src/arch/aarch64/mmu.rs` | private, `unsafe` | `can_el0_access` | named for the `AT` instruction it issues; the answer is about access |
| `aarch64`, `riscv64`, `x86_64` | `xtask/src/suite.rs` (`ArchLegs`) | pub(crate) | `includes_aarch64` and so on | `is_aarch64` would be false for `All`, which also answers yes |
| `no_capability` | `patches/std-nife/overlay/std/src/sys/fs/nife.rs` | private | `is_missing_capability` | `is_no_capability` does not parse |

If calef says no to any of these, the name stays as it is and this table records the refusal.

Each of these, and `removed` and `asked` below, carries a `/// Name: provisional` marker naming the
recommendation, so it queues on `script/names --unratified` with every other unratified name
(calef's standing direction, 2026-09-24). The std overlay's `no_capability` carries one too and
does not show: `script/names` skips `patches/` on purpose, so that entry is visible only here.

## `rx` and `tx`

The pass renamed `is_rx_waiting` to `is_byte_waiting` after calef's review of #1255 asked what
`rx` stands for, and left every other public `rx` or `tx` name alone because none is a predicate.
They are listed here, 2026-09-25, so the abbreviation is ruled once rather than name by name. Each
definition carries a `/// Name: provisional` marker naming the recommendation, so all of them queue
on `script/names --unratified`. None is a wire format: every one is a function or constant inside
one program, and `UART_RX_INTID`'s value is what other code agrees on, not its spelling.

| Now | Defined in | Visibility | Sites | Recommended | Why |
|---|---|---|---|---|---|
| `rx_get` | `components/src/input.rs` (three architectures) | pub in a private `mod uart` | 5 | `read_byte` | it reads the byte `is_byte_waiting` reports, so the pair reads as one vocabulary |
| `arm_rx_interrupt` | `components/src/input.rs` (aarch64, riscv64) | pub in a private `mod uart` | 4 | `arm_receive_interrupt` | the word both UART manuals spell out |
| `discard_rx` | `kernel/src/console.rs`, `kernel/src/drivers/{ns16550,pl011}.rs` | pub | 16 | `discard_waiting_bytes` | it drops the bytes `is_byte_waiting` would have reported |
| `rx_enable` | `kernel/src/console.rs` | pub | 3 | `enable_receive_interrupt` | it enables an interrupt, which the current name does not say; matches the driver method it calls |
| `enable_rx_interrupt` | `kernel/src/drivers/ns16550.rs` | pub | 3 | `enable_receive_interrupt` | as above |
| `enable_tx_interrupt` | `kernel/src/drivers/ns16550.rs` | pub, `#[cfg(test)]` | 5 | `enable_transmit_interrupt` | the transmit twin of the row above |
| `tx_bytes` | `kernel/src/console.rs` | pub | 1 | `bytes_written` | it counts what the kernel wrote to the console; the private static `TX_BYTES` becomes `BYTES_WRITTEN` with it |
| `UART_RX_INTID` | `kernel/src/user.rs` (three architectures) | pub | 12 | `UART_RECEIVE_INTID` | `INTID` is the GIC's own term and stays |
| `VnetRxToken`, `VnetTxToken` | `components/src/virtio_net_transport.rs` | pub | 13 | keep | they implement smoltcp's `phy::RxToken` and `phy::TxToken`, and the upstream word is what a reader of smoltcp looks for |

Sites are `git grep -w` hits on 2026-09-25 outside `design/roadmap/` and this directory, code and
notes together; roadmap blocks keep the old names under the rename procedure.

Private names this table leaves out, because the review question was about public ones: the
virtio-net queue constants (`NET_RX_*`, `NET_TX_*` in `crates/virtio`, `RX_*`, `TX_*` in
`components/src/virtio_net_transport.rs`) and the helpers beside them (`post_rx`, `wait_rx`,
`rx_take`, `tx_send`, `rx_buf`, `tx_buf`). They follow the virtio specification's own
`receiveq`/`transmitq` shorthand. `tools/redoxfs_host`'s `ls_tx`, `put_tx` and the rest are a
different word: there `tx` means a RedoxFS transaction, which is the best argument that the
abbreviation does not decode itself. If calef rules `rx` out, these are the next pass, and none of
them needs a marker to be found again: `git grep -wiE '[a-z_]*(rx|tx)[a-z_]*'` is the census.

## Fits as written

A finite verb or a quantifier already says "this is a question" (see
[functions-that-answer-yes-or-no.md](functions-that-answer-yes-or-no.md#why-a-verb-phrase-is-allowed)). The 57, by name:

`allows` (twice), `survey_includes`, `denies_all`, `carries_len`, `takes_name`, `mutates`, `fits`
(twice), `is`, `matches_with`, `opens_with_a_literal_dot`, `overlaps` (twice), `overlaps_ram`.
`all_distinct_ids`, `all_distinct_names`, `any_core_running_real_work`, `file_name_fits`,
`component_fits`, `was_already_up`, `status_says_okay`, `repeats_newest`, `stack_works`,
`holds_block`, `holds`, `holds_the_system`, `selects`. `kernel_refuses`, `progenitor_refuses`,
`grant_allows`, `udp_grant_allows`, `runs_after`, `page_is_clean`, `pwd_is`, `userspace_ran`,
`reached_the_goal`, `user_can_read` and `user_can_write` (twice each), `asid_tagging_is_trusted`,
`calibration_has_converged`, `would_violate`, `machine_has_no_device_page_for_the_console`,
`machine_has_no_rtc`, `machine_has_no_entropy`, `had_step`, `page_contains` and `has`. In `xtask`,
`the_disk_agrees` (twice), `kernel_wrote_during_boot`, `redoxfs_reads_back`,
`shell_navigation_landed`, `redoxfs_subtree_was_confined` and
`redoxfs_glob_grant_took_exactly_the_match`.

## Actions

These do something and return whether it worked, which is std's `HashSet::insert` shape, so the
rule does not apply and the verb stays. By place:

- **`xtask`, 71**: every build, boot and gate entry point (`test`, `bench`, `uefi_boot`,
  `swish_check`, `mkredoxfs` and the rest) returns whether it passed.
- **kernel, 36**: `disable` (three architectures), `claim`, `register_space`, `record_mapping`,
  `take_ipc_aborted`, `take_need_resched`, `check`, `check_armed`, `kill_thread`,
  `mark_current_fp_live`, `strand_reply_caller`, `bind_tick_routes`, `spawn_into`,
  `arm_for_start`, `enable_for_current`, `place_bars`, `adopt`, `reclaim`, `release`, `asked`,
  `serve_shootdown_nmi`. The trap bodies and `dispatch_on_interrupt_stack` (three architectures
  each, and also `extern` symbols), and the test drivers `until_the_tick_is_raised`,
  `wait_for_report`, `reclaim_within_two_seconds`, `pool_came_back`, `real_single_hart_or_skip` and
  `spawn_os_primitives_benchmarker`.
- **components, 22**: block and entropy I/O (`read_at`, `write_at`, `read_span`, `fill`,
  `refill`, `read_primary`, `reseed_and_wait`), process building (`build`, `fire`, `reap`,
  `spawn_stage`, `reclaim`, `map_page_frame`), and `push`, `page`, `handle_byte`, `removed`,
  `rederive_one` and `wait_for_fault`.
- **crates, 23**: `push` (twice), `keep_smallest`, `descend`, `ascend`, `step`, `encode`,
  `sector`, the `Pages::page` trait method, `remove_sender`, `remove_receiver`, `take_aborted`,
  `claim`, `grow`, `map_page_frame`, `memory_region_destroy`, `start_child`, `fill_entropy`, and
  virtio's `init_with_features`, `wait_rx`, `blk_flush` and `complete_flush`.
- **elsewhere, 10, and one in the std overlay**: `fixtures` (`start_child`, `write_marker`, `destroy_with_retry`),
  `uefi_loader` (`write_back`, `write`, `flush`, `paint_handoff`), `std_exerciser` (`udp_ok`,
  `tcp_echo_ok`, which perform the round trip they name), `redoxfs_server`'s `grow`, and the std
  overlay's `grow`.

**`removed` and `asked` are the two worth a second look.** Both read as participles and both act:
`removed` sends an unlink and `asked` prompts and reads a reply. A reader could take either for a
question. They are named for what they do, which the rule allows, and a name like `remove` or
`ask_to_confirm` would say so more plainly. That is a naming choice this pass did not make.

## Other exemptions

- `page_frames`'s private `get(i)` on its bitmap follows C-GETTER's `get`, like `Cell::get`.
- The std overlay's `Permissions::readonly` is std's own method name, which the overlay implements.
- `exception_body`, `riscv_trap_body` and `x86_trap_body` are counted as actions above, and are
  `extern "C"` symbols that assembly calls by name, which would hold them even if they were not.

## BUGS

- **The class counts are a reading, not a measurement.** The scanner found the 316; sorting them
  into question or action meant reading each doc comment, and a function whose comment is wrong
  about what it does is sorted wrong.
- **The action counts are approximate by a few**: some helpers straddle (a check that prints its
  verdict is both), and each was put where its doc comment's verb pointed.
- **Nothing keeps this file current.** It records one pass. A new predicate is held to the rule by
  review, not by this table.
