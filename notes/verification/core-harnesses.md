# The core harnesses: capabilities, memory, paging and IPC

An appendix to [notes/verification.md](../verification.md), which keeps one line per crate. This
file holds the harness tables and the reasoning for the capability core, the untyped-region
accounting, the paging arithmetic, the frame allocator, the device-tree leaves, the IPC rendezvous,
the generational table, the intrusive queue and the ASID allocator.

## `crates/capability`

14 harnesses in `crates/capability/src/lib.rs`, under `#[cfg(kani)]` <!--count:capability-harnesses-->;
the table names the eight this appendix adjudicated:

| Harness | Property |
|---|---|
| `subset_is_reflexive` | every capability is a subset of itself |
| `subset_is_transitive` | rights cannot be laundered through a derivation chain (why a *flat* subset check suffices, with no tree walk) |
| `from_bits_cannot_forge_a_right` | an attacker-controlled syscall register cannot conjure an undefined right |
| `subset_matches_allows` | the two phrasings of the order agree, so a bug in one shows against the other |
| `derive_never_widens_rights` | the central theorem, on the real `CapabilityTable::derive` |
| `split_never_widens_rights` | authority never widens at the *other* mint site: `Cap::mint_child` (the inheriting mint `MemoryRegion::SPLIT` now uses) hands a child no more than the parent held (milestone 35 (Prove the DMA-confinement boundary), below) |
| `a_deleted_capability_stays_deleted` | for every table state, once `delete` succeeds the slot answers `NoSuchSlot` to both `get` and a second `delete` (the consume-on-use mechanism behind the one-shot Reply) |
| `delete_touches_only_its_slot` | deleting any slot leaves every other slot exactly as it was (consuming one caller's Reply cannot orphan another's) |

The last two run over a *symbolic* table: every slot is independently empty or holds a capability
with symbolic object and rights. So "no state exists in which a consumed slot works again" is
quantified over table states, not sampled.

## `crates/memory_regions`

Four in `crates/memory_regions/src/proofs.rs` <!--count:memory-regions-harnesses-->, the
untyped-region accounting behind object revocation
(DECISIONS §16 (object revocation)). Two when this was written, and in `src/lib.rs`, where they no
longer live; the table names those two. The scary property there is no double-free:

| Harness | Property |
|---|---|
| `split_stays_within_budget_and_progresses` | a successful carve advances the watermark by exactly `want` without overflow and never past the parent's budget, and strictly progresses, so consecutive carves are disjoint runs within the parent |
| `destroy_never_frees_a_child_to_the_allocator` | a pinned or parent region refuses; a **root** frees to the allocator; a **child** *never* does, its pages return to the parent. So a page reaches the allocator only through the one root that owns it, exactly once |

The second is the no-double-free crux. The kernel (`untyped::split`, `untyped::destroy`) *calls*
`memory_regions::split_new_watermark` and `memory_regions::destroy_outcome` rather than keeping a
parallel copy. So the proved arithmetic is the arithmetic that runs. It is a Phase-2-style
extraction. The pure page accounting is here (address-agnostic, in page units). The byte arithmetic
and the I/O (freeing frames, un-bumping the parent) stay in the kernel around it.

## `crates/paging`

37 across `crates/paging` <!--count:paging-harnesses-->, in `src/verification.rs` and one
module per architecture (`aarch64.rs`, `sv39.rs`, `x86_64.rs`, `domain.rs`); the per-architecture
parity series those modules hold is the arch gate's record, not this appendix's. The eight below
are the originals this appendix adjudicated, in `src/lib.rs` at the writing: the address arithmetic
under the four-level walk, and the MMU
isolation invariants. The last three closed the MMU step of milestone 18 (verify the capability core, then spread
inward).

| Harness | Property |
|---|---|
| `index_is_always_in_bounds` | every extracted table index is < 512, so the walk never reads past a table (memory safety) |
| `the_indices_and_offset_tile_the_address` | the four 9-bit indices and the 12-bit offset reassemble the low 48 bits exactly, no bit lost or shared (the `39 - 9*level` shift math is correct) |
| `the_offset_does_not_change_the_walk` | changing only the page offset leaves all four indices fixed: a whole 4 KiB page shares one leaf (page granularity) |
| `distinct_pages_take_distinct_paths` | two page-aligned addresses with the same four indices are the same page (the arithmetic core of isolation) |
| `the_two_halves_are_disjoint` | no address is in both `TTBR0` (low) and `TTBR1` (high) |
| `the_user_va_gate_admits_only_the_aligned_low_half` | `is_user_page_va` equals the bit test the syscall layer used to hand-roll, and admits no address in the kernel's half |
| `the_leaf_descriptor_keeps_address_and_permissions_apart` | the L3 descriptor `map` writes decomposes back into exactly the address and exactly the flags, for every representable physical page and every `Flags` constructor: no permission bit can redirect the address, no address bit can grant a permission |
| `the_low_half_mapper_rejects_the_high_half_untouched` | for every address outside the low half (every kernel address included), `map`/`unmap`/`translate` on a `TTBR0` mapper reject before touching any memory (the harness gives the mapper a null root and a panicking frame source, so a touch is a proof failure) |

The user-VA gate is a Phase-2-style extraction in miniature. `untyped::MAP` and `frame::MAP` both
hand-rolled `va & 0xfff != 0 || (va >> 48) != 0`. Both now call `paging::is_user_page_va`, so the
gate the kernel runs is the gate that is proved.

The descriptor harness leans on one assumption worth recording. `pa` is taken as representable
(bits 47:12), which is the architecture's own descriptor format. It is true of every `pa` the kernel
maps, since the frame allocator and untyped regions are bounded by RAM, far below 2^48.
`Mapper::map` masks a wider `pa` silently. Nothing can hand it one today. If that ever changes, the
mask is where to add the check.

Deliberately not proved: the `Mapper` round-trip (map a page, translate it back). This was
considered and declined, not skipped. Kani only pays off on *symbolic* inputs, and here both ends
are dead. A concrete-address round-trip is a unit test Kani happens to execute, with no gain over
the tests already present. A symbolic-address round-trip reasons over a built four-level page table.
That is the "BMC over real memory" case that walls the same way the ELF parser did. And the
invariants of the walk that actually matter are *already* proved in the `paging` arithmetic
harnesses above: index-in-bounds, distinct pages take distinct paths, the lossless address split. So
the round-trip would burn the solver to re-cover proved ground, or hit the wall. It stays covered by
the host and kernel tests.

## `crates/page_frames`

Five in `crates/page_frames/src/lib.rs` <!--count:page-frames-harnesses-->, the physical frame allocator:

| Harness | Property |
|---|---|
| `two_allocations_are_distinct` | over any bitmap, two back-to-back `alloc`s never return the same frame (the property isolation rests on: one physical page is never handed to two owners) |
| `an_allocated_frame_is_aligned_and_in_range` | an allocated frame is frame-aligned and within `[base, base + total*FRAME_SIZE)` |
| `index_of_inverts_frame_addressing` | frame address and bitmap index are inverses, so naming is unambiguous |
| `containing_rounds_down_within_a_frame` | `PageFrame::containing` returns an aligned frame that holds the address |
| `bitmap_bytes_covers_every_frame` | the bitmap is always sized to hold one bit per frame (no out-of-bounds in `get`/`set`) |

The allocator harnesses build a small allocator over a *symbolic* bitmap directly. The
`#[cfg(kani)]` module is inside the crate, so it can reach the private fields. They do not go
through `new`, which fills the bitmap all-used. The scan loops are bounded by pinning `total = 8`, so
`unwind(9)` suffices.

## `crates/device_tree_blob`

Four in `crates/device_tree_blob/src/lib.rs` <!--count:device-tree-blob-harnesses-->, the device-tree parser's leaf readers. The whole-parse
token loop is the same BMC wall as ELF, so the leaves are what get proved.

| Harness | Property |
|---|---|
| `be32_is_total` / `be64_is_total` | the big-endian readers never panic for any offset, even `usize::MAX` |
| `be32_reads_big_endian_when_in_bounds` | an in-bounds read is exactly `bytes[at..at+4]`, MSB first |
| `align4_rounds_up_to_a_multiple_of_four` | the padding helper rounds up correctly for any realistic length |

`be32`/`be64` were *hardened* to reach totality. Their `at + 4` / `at + 8` is now a checked add, so
a near-`usize::MAX` offset from a corrupt blob returns `Truncated` instead of panicking. The 12
integration tests against a real QEMU device tree are unchanged, so the hardening is faithful. This
is the ELF lesson reused: prove (and here, harden) the loopless leaves; the walk stays on the tests.

## `crates/inter_process_communication`

Fifteen in `crates/inter_process_communication` <!--count:inter-process-communication-harnesses-->,
the synchronous-rendezvous state machine: the rendezvous six the table names, in
`src/verification.rs` (`src/lib.rs` at the writing), plus the notification and timer harnesses
those later subsystems added to the same crate (four and three).
It is the decision core of `sched`'s `Rendezvous`, extracted as pure logic. It was restated over
the intrusive queues at milestone 14 (kernel objects from untyped) phase A.3, so the rewire did not demote proved code back to
argued code. The same six properties now hold over real `intrusive::Queue`s with TCB-shaped nodes,
composing with the `Queue`'s own FIFO proof below.

| Harness | Property |
|---|---|
| `send_preserves_the_invariant` / `receive_...` / `signal_...` | every operation preserves "at most one wait queue is ever non-empty," the invariant the whole IPC design rests on |
| `send_rendezvous_iff_a_receiver_waited` | a send rendezvouses exactly when a receiver was waiting, else blocks (no dropped message, no spurious block) |
| `receive_drains_a_pending_signal_first` | a receive takes a pending async signal before a blocked sender, so a signal is never lost |
| `a_collected_sender_is_forgotten` | once a receive collects a blocked sender, the endpoint holds no name for it in either queue and no later receive can produce it again (the endpoint half of the one-shot Reply) |

These are inductive-step proofs: assume a valid state, apply one operation, check the invariant
holds. A non-empty queue is modeled with a single waiter. The decision and the invariant depend only
on whether a queue is empty, never its length, and that keeps the `VecDeque` reasoning tractable.

### Phase 2: the kernel calls the proved logic

Phase 2 is done. The IPC functions in `kernel/src/sched/rendezvous.rs` no longer hand-roll the
rendezvous branch. They call `inter_process_communication::Rendezvous<Thread>` for the *decision*. It is
the same generic type, so the queues are the kernel's real endpoint state, not a model kept in sync.
The functions spend their own code only on the bookkeeping the queues cannot express: mailboxes,
waking a thread onto a run queue, and the one-shot Reply that leaves a caller blocked.

The full QEMU suite (102 tests, including the Call/Reply, frame-delegation, and revocation tests)
passes unchanged, so the rewire is faithful. The kernel's IPC path *is* the proved logic now, not a
parallel copy of it. This is the first place a proof reaches all the way into the running kernel
rather than staying in a host crate.

### Phase 3: the one-shot Reply, which needed no rewire

"One reply, to this caller, exactly once" (DECISIONS §12 (call/reply IPC)) decomposes into three
legs. It is worth recording which kind of evidence each one rests on.

1. The endpoint forgets a collected caller: `a_collected_sender_is_forgotten` in
   `crates/inter_process_communication`. A `CALL`er queues as a sender and blocks. The server's
   receive pops it destructively. From that moment the kernel-minted Reply capability is the *only*
   name for the blocked caller anywhere in the system. (The caller is never in the receiver queue:
   `ipc_call` does not `receive`, and a blocked thread cannot run to enqueue itself again.)
2. Consume-on-use is final: `a_deleted_capability_stays_deleted` and `delete_touches_only_its_slot`
   in `crates/capability`. The syscall layer deletes the Reply capability the instant it is invoked.
   The proofs say no table state exists in which the consumed slot can be invoked again, and that
   consuming one caller's Reply cannot disturb another's.
3. The capability cannot be duplicated or delegated. This leg is structural, not a harness. There is
   no syscall that copies a capability within a capability table (`CapabilityTable::derive` is
   kernel-internal). The only cap-moving syscall, `SEND_CAP`, requires `GRANT`, which `reply_cap`
   deliberately never mints. This leg lives in the shape of the syscall surface (§4 (kernel shape): narrow and
   explicit), so it is an inspection argument. It is backed end to end by the QEMU test in which the
   call server invokes its Reply twice and the kernel refuses the second (`fixtures/src/hello.rs`,
   `call_server`).

There was no rewire, because `capability::CapabilityTable` and
`inter_process_communication::Rendezvous` already *are* the kernel's capability table and endpoint
state. The proofs landed on code the kernel was running all along.

## `crates/generational_table`

Four in `crates/generational_table/src/lib.rs` <!--count:generational-table-harnesses-->, the
generational thread table (milestone 14 phase
A; see notes/generational-names.md); the table names the three this appendix adjudicated:

| Harness | Property |
|---|---|
| `a_removed_name_never_resolves_again` | once removed, a name fails `get`/`get_mut`/`remove` forever, even after its slot is reused (the stale-ThreadId safety that capability payloads will lean on) |
| `live_names_are_distinct_and_resolve_to_their_own_entry` | the `(generation, slot)` packing cannot alias two live entries |
| `a_name_the_table_never_minted_resolves_to_nothing` | for any u64, resolution succeeds only on exactly a name the table issued |

## `crates/intrusive_queue`

One in `crates/intrusive_queue/src/lib.rs` <!--count:intrusive-fifo-harnesses-->, the scheduler's
queue structure (milestone 14 phase A.2; see notes/intrusive-queues.md):

| Harness | Property |
|---|---|
| `any_push_pop_interleaving_is_queue_ordered_and_lossless` | the real `Queue`, driven by a six-step *symbolic* operation sequence over three nodes, agrees with a trivially-correct model at every step: FIFO order, no node lost or invented, lengths agree, and no stale link is dereferenced |

One harness rather than several, because the operation-sequence shape subsumes the single-step
properties: a push-preserves-X proof is the sequence of length one.

## `crates/address_space_identifier`

Three in `crates/address_space_identifier/src/lib.rs` <!--count:address-space-identifier-harnesses-->, the TLB tag allocator (milestone 15 (tagged address spaces); see
notes/address-space-identifiers.md, including which half of the ASID contract stays on a hardware
witness test rather than a proof):

| Harness | Property |
|---|---|
| `the_kernel_asid_is_never_allocated` | no reachable state hands a user space ASID 0, the kernel's tag |
| `two_live_asids_are_distinct` | live allocations never alias, from any symbolic state |
| `free_releases_exactly_its_own_asid` | free clears its own bit and no other |
