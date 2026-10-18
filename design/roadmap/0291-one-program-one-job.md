---
status: BUILT
raised: 2026-09-14
built: 2026-09-14
---
# 291. `fixtures/src/hello.rs` was thirty-one programs wearing one name

Built for twenty-two of the thirty-one roles; the remaining nine are proposed as a follow-on
below.

calef, 2026-09-14, shown the role table: **"31 role binary is not the right shape. If there is
anything left then we can consider a name for what remains."** It follows his ruling earlier that
day on `components/src/ntp.rs`, three roles to three programs, where the reason was: *"Part of the beauty of Unix
that I think we want to retain is small programs with specific functions."*

## The principle, which the tree had enacted five times and never written down

**A program does one thing, and a role is an exception that has to say why.**

The tree had been dissolving this one binary a role at a time for two months, each departure
written up in its own file as a local fix and nowhere as a rule. The least-authority demo left at
19f.2, the console at 19f.3 (`components/src/console.rs`), the input driver at 19f.4, the shell at
19f.5, `init_boot` at 266 (`components/src/progenitor.rs`). Five departures, five explanations, no
principle. That is rung zero: the next person to add a behavior to a fixture had nothing to read
that said not to add it as a role.

`fixtures/src/hello.rs`'s own provenance block had gone further and declared the problem closed:

> *"The limitation that used to be recorded here is closed (milestone 266). It read that the name
> had outlived the description..."*

266 moved one role out. Thirty-one remained, so that sentence was false the day it was written
and stayed false for three weeks. Correcting it is part of this milestone, and it is the reason the
principle is now stated in the file a reader meets rather than in a roadmap block they will not.

## The inventory, which is what decided the work

Thirty-one roles, and they fell into three groups that want three different treatments. The
disposition column is what this milestone did.

| # | Role | What spawns it | What it proves | Disposition |
|---|---|---|---|---|
| 0 | `SELF_CHECK` | `tests::a_real_elf_from_the_initrd_runs_at_el0_and_verifies_itself`, bare | A real ELF loaded: `.rodata` readable, `.data` copied, `.bss` zeroed, stack deep enough | split: `image_self_checker` |
| 2 | `PRINTING` | `tests::a_user_client_moves_data_through_shared_memory` | §10's split: bytes through shared memory, length through IPC, kernel never in the data path | split: `console_test_client` |
| 3 | `VIRTIO_BLK` | `virtio_service::start`, the tour, `tests` | A userspace driver reads a file off a virtio disk over DMA | **deleted**: `block_driver` |
| 7 | `UNTYPED_DEMO` | `memory_region_service::start`, the tour | A process maps pages until its own region is spent and the kernel allocates nothing | split: `memory_region_depleter` |
| 8 | `VIRTIO_ATTACK` | `virtio_service::start_attacker` | The kernel refuses a DMA descriptor aimed outside the driver's region | **deleted**: `block_driver` |
| 9 | `GRANTER` | `delegation_service::wire` | A process hands a capability on, narrowed | split: `delegation_granter` |
| 10 | `RECEIVER` | `delegation_service::wire` | The delegated capability works, and cannot be re-delegated without `GRANT` | split: `delegation_receiver` |
| 11 | `PAGE_FRAME_PRODUCER` | `page_frame_service::wire` | Two processes compose shared memory themselves, out of a capability | split: `page_frame_producer` |
| 12 | `PAGE_FRAME_CONSUMER` | `page_frame_service::wire` | The shared page is genuinely shared, and `READ` alone cannot map it writable | split: `page_frame_consumer` |
| 13 | `VIRTIO_ATTACK_INDIRECT` | `virtio_service::start_attacker_indirect` | The indirect-descriptor escape is refused too | **deleted**: `block_driver` |
| 14 | `CALL_SERVER` | `call_service::wire` | A reply capability is one-shot: a second reply is refused | split: `call_server` |
| 15 | `CALL_CLIENT` | `call_service::wire` | A process calls a server it was never individually wired to | split: `call_client` |
| 16 | `REVOKE_DEMO` | `revoke_service::wire` | `REVOKE` deletes every capability to a frame, the revoker's own included | split: `frame_revoker` |
| 17 | `EP_MAKER` | `retype_ep_service::wire` | A process mints an IPC object out of its own memory and delegates it | split: `rendezvous_minter` |
| 18 | `EP_USER` | `retype_ep_service::wire` | A peer listens on a rendezvous that reached it entirely by delegation | split: `rendezvous_peer` |
| 19 | `ADDRESS_SPACE_BUILDER` | `address_space_service::wire` | A process builds an address space from EL0, and break-before-make holds inside it | split: `address_space_builder` |
| 20 | `INIT` | `spawn_progenitor`, role 20 | A userspace loader parses an ELF and starts a child; the kernel never touches its bytes | **kept as a role** (see below) |
| 21 | `CHILD` | built by role 20 out of `hello`'s own ELF | The built process runs and reports | **kept as a role** |
| 22 | `DEV_CHILD` | built by role 23 | Delegated device MMIO is a real view of the UART (`0xB105F00D`) | **kept as a role** |
| 23 | `INIT_DEV` | `spawn_progenitor`, role 23 | A parent delegates a device's registers to a driver it built | **kept as a role** |
| 24 | `INIT_CONSOLE` | `spawn_progenitor`, role 24 | A parent builds the real console server, wires it, and drives it as a client | **kept as a role** |
| 25 | `INIT_IRQ` | `spawn_progenitor`, role 25 | A parent delegates an interrupt capability to a driver it built | **kept as a role** |
| 26 | `IRQ_CHILD` | built by role 25 | The interrupt arrives through the delegated capability | **kept as a role** |
| 28 | `INIT_LEAST_AUTHORITY_DEMO` | `spawn_progenitor`, role 28 | `START` carries data, not just identity: the argument reaches a fresh EL0 thread intact | **kept as a role** |
| 29 | `INIT_COREMARK` | `spawn_progenitor`, role 29 | A real compute workload runs correctly against the native ABI | **kept as a role** |
| 30 | `VIRTIO_BLK_WRITE` | `virtio_service::start_writer` | The write path: write, read back, re-check the filesystem around it | **deleted**: `block_driver` |
| 31 | `VIRTIO_BLK_WRITE_ABANDON` | `virtio_service::start_write_abandoner` | A driver killed mid-write leaves the device and transport sane | **deleted**: `block_driver` |
| 32 | `VIRTIO_BLK_SERVER` | `fs_service::blk_server_image` and every filesystem test | Blocks served over IPC to the FS server | **deleted**: `block_driver` |
| 40 | `VIRTIO_NET` | `virtio_service::start_net` | A DHCP round trip over virtio-net | **deleted**: `block_driver` |
| 42 | `CYCLE_COUNTER_CHILD` | `tests`, directly, both granted and not | A granted thread reads the counter; an ungranted one is killed for it | split: `cycle_counter_reader` |
| `_` | the catch-all | nothing | nothing | **deleted**: it is a trap now |

## Seven roles deleted, and the duplicate nobody had noticed

**`components/src/block_driver.rs` already was the virtio driver.** Same seven role numbers, same
dispatch, same `crates/virtio` behind it, and RISC-V and `x86_64` had been using it since parity C.
aarch64 reached the identical code through `hello` only because `initrd_aarch64`'s table never
packed `block_driver`. The cost of that was a `cfg` fork every caller had to be routed around:
`fs_service::blk_server_image` and `ripgrep_tests::block_server_image` each carried three arms whose
entire content was "which binary carries this role on this board".

Packing `block_driver` on aarch64 too **deleted seven roles and needed no new name at all**, which
made it the first thing to do rather than the last. Both of those functions are one line now, and
twenty-four test comments that read *"a second copy through hello's roles would double the suite's
slowest tests"* were corrected: the two legs differ by instruction set now and by nothing else.

## Fourteen roles split, under provisional names

Every name below is **provisional**. calef names programs; each file's own header says what it does
and which alternatives were refused, which is where a ruling should be read from rather than from a
list. Gathered here so they can be ruled in one pass:

`image_self_checker`, `console_test_client`, `memory_region_depleter`, `delegation_granter`,
`delegation_receiver`, `page_frame_producer`, `page_frame_consumer`, `call_server`, `call_client`,
`frame_revoker`, `rendezvous_minter`, `rendezvous_peer`, `address_space_builder`,
`cycle_counter_reader`. Plus two crates, `capability_demo_proto` and `loaded_image_check`.

Three of them stopped carrying a word that had become false. `EP_MAKER` and `EP_USER` were an
abbreviation needing a decoder over an object this tree calls `RENDEZVOUS` everywhere else, and
`user` already means two other things here. `CYCLE_COUNTER_CHILD` was never anybody's child: the
kernel's own test starts it directly, because milestone 229 shipped the grant mechanism without a
syscall method to set it.

Every split program ignores `x0`. A one-job program has no role selector, so the role numbers
simply stop existing for these fourteen rather than being renumbered. `hello`'s remaining nine keep
theirs unchanged, gaps and all: a role number is a word the kernel puts in `x0`, so it is a value
the kernel's wiring and that file agree on.
[The progenitor's grant order](0301-one-grant-order-for-the-progenitor.md) recorded six
`spawn_progenitor` tests that name them. Renumbering would be an edit to a wire value bought with
tidiness.

## Two crates, because rule 7 admits no other answer

`fixtures/src/hello.rs` said it out loud: *"One binary, so one constant serves both roles."* That
sentence is what the split made false.

`capability_demo_proto` holds the three words two compilation units must now agree on:
`PAGE_FRAME_SENTINEL` (producer and consumer), and `USED_WORD` (the delegation receiver and
`kernel/src/user/delegation_service.rs`, which had its own copy with a "must match" comment beside
it). `CYCLE_COUNTER_WORD` (the reader and the kernel test, likewise duplicated).

`loaded_image_check` holds the self-check and its `.rodata`/`.data`/`.bss` markers, which two
fixtures need. It takes its `fail` as a `fn() -> !` parameter rather than calling `user_rt`, and
that is worth a sentence. A crate that reaches `user_rt` reaches EL0 syscall `asm!` and compiles for
aarch64 or riscv64 only, which `script/lint` catches and which would have put this crate in four
separate host-pass exclusion lists to buy one function call. As a parameter it costs the two callers
one argument, stays host-buildable, and carries two host tests.

Keeping it in one program and dropping it from the other was considered and refused.
The printing client's self-check is load-bearing to
`a_user_client_moves_data_through_shared_memory`, which asserts on the *absence of a fault* as well
as on the bytes. And after a split no other test covers that binary's own image.

Refused: passing the sentinel in `arg1` instead of sharing a constant, which would have removed
the agreement rather than relocating it, since the spawner could hand the same word to both halves.
It loses because it moves a fixture's own invariant into the kernel's wiring, where a reader of
either program can no longer see what value is expected or why. It also loses because AGENTS.md rule 7 is
written as an absolute, not as a preference to be traded against.

## The archive grew, and the directory ceiling was in the way

Fifteen more entries on aarch64 and fourteen on the other two boards put both archives past
`nifefs::MAX_FILES`, which was 76. `DIR_BLOCKS` is 10 now, up from 6, so the ceiling is 127. No
magic bump: `start_block` is absolute and `ENTRIES_IN_FIRST_BLOCK` is a function of `NAME_LEN`, so
no reader can tell, which is the rule `MAGIC`'s own block records for milestone 24's 4-to-6 move.
10 rather than the 8 that would have sufficed, because `MAX_FILES`' note says this ceiling is
crossed by lanes that cannot see each other and it has now been crossed that way three times; the
headroom costs 2 KB once.

The aarch64 archive went from 9.0 MB to 11.1 MB in a debug build, about 140 KB per added
program, which is debug information rather than code. Recorded rather than defended: it is a real
cost and the number should be re-taken if anyone ever measures boot time against archive size.

`initrd_aarch64` now prints *why* a pack failed, which `initrd_riscv` has done since somebody lost
an afternoon to the silent version. This milestone lost a shorter one to the same message.

**The one cost of the move that is not 2 KB was measured by CI, not by this lane.**
`a_short_image_is_refused_not_indexed` proves its boundary with a `kani::any()` array of
`DIR_BLOCKS * BLOCK - 1` bytes, so that symbolic input grew from 3071 to 5119. This lane's
environment had no Kani in it, so `script/verify` was the one gate it could not run, and the block
said so before CI answered. It answered: both `prove` shards passed on the tree that made the
change, in 14:59 and 15:20. That is the suite's ordinary shape and it is **not** a measurement of
the harness itself, so the caveat beside the constant now says which number to watch if anybody
raises `DIR_BLOCKS` again.

## What was not done, and why

The nine `INIT`/child roles stay in `hello`, and the split of those is a separate milestone.
Two reasons, and the second is honest about being partly effort:

1. **It is a boot-path change, not a fixtures change.** `kernel::user::spawn_progenitor` picks the
   archive entry from the role: `progenitor` for the boot role, `hello` for everything else. Six
   roles becoming six programs turns that choice into a table, and the three children each need
   their parent to name their archive entry instead of re-entering its own image
   (`ROLES_ENTRY`). That is exactly where milestone 268 was rebuilding the boot sequence on all
   three architectures while this lane ran, and two lanes in `spawn_progenitor` is the collision
   this tree already knows how to avoid.
2. **It would not have fitted this lane honestly.** Nine more programs, a kernel spawn change and a
   third full re-gate, on top of a change that already touches two archive tables and the
   filesystem's directory geometry. Landing two thirds of the split, verified, beats landing all of
   it unverified.

See the proposal below.

## Follow-on

- **Milestone 405.** The nine roles this milestone left in `hello`. Numbered on 2026-09-19 by
  milestone 433's drain of the pile. `spawn_progenitor` (`spawn_hello` since milestone
  166) takes the archive entry as a parameter (or a role-to-entry table), the three children
  become their own programs, and the six parents name their child's entry instead of re-entering
  their own image. Gated on milestone 268, which was rebuilding that function beside this lane. It
  is also what would settle the name: after it, either `hello` is deleted or there is something
  left that a name can describe.
- **Milestone 408.** `fn check(ok: bool)` now has nine copies across `fixtures/` and `components/`,
  seven of them added here. All of them are `if !ok { user_rt::trap() }` or a panic reaching the same
  instruction. `user_rt` is the obvious home; the decision is the public function name, which is
  calef's, so the block carries a `DECISION` gate.
- **Milestone 405.** The nine roles left here, a name that did not describe them, and a role
  fault that left a spawner waiting on its watchdog. Milestone 405 (the nine `INIT` roles) made the nine roles programs,
  and the kernel names the entry it enters.
- **Done.** The Kani cost of the `DIR_BLOCKS` raise, which this lane could not measure, was
  answered by CI: both `prove` shards green on the tree that made the change. What that does not
  settle, and what `crates/nifefs/src/lib.rs`'s `DIR_BLOCKS` block now names, is the harness's own
  share of those fifteen minutes, which is the number to take if this constant is raised again.
- **Recorded.** `notes/adding-a-program.md` gained the principle and the archive ceiling, because
  that is the page the next person adding a fixture reads and a roadmap block is not.
- **Done.** The archive-size and directory-ceiling costs are measured and stated in this block
  rather than left for somebody to rediscover.

## Index row

calef, 2026-09-14: *"31 role binary is not the right shape. If there is anything left then we can consider a name for what remains"*. Twenty-two of thirty-one roles gone. Seven deleted outright: `components/src/block_driver.rs` was already the identical virtio driver over the identical `crates/virtio`, used by the other two boards since parity C, and aarch64 reached the same code through `hello` only because its archive table never packed it. Packing it collapsed three `cfg` forks to one line each and corrected 24 test comments. **Fourteen split into programs**, all names provisional, plus two crates (`capability_demo_proto`, `loaded_image_check`) because rule 7 admits no `#[path]` module for what a split pair must agree on. **Nine `INIT`/child roles kept**, proposed as a follow-on: splitting them changes `spawn_progenitor`'s role-to-entry choice, which is the boot path milestone 268 (every architecture boots the same way) was rebuilding beside this lane. The principle the tree had enacted five times and never written down is now stated where a reader meets the last multiplexer: a program does one thing, and a role is an exception that has to say why. `hello`'s own block claimed the problem closed by 266, which had moved one role out of thirty-two. `nifefs::DIR_BLOCKS` 6 -> 10 (ceiling 76 -> 127) because both archives crossed `MAX_FILES` on the same commit.
