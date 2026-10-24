# The page frame budget: what a test boot spends, and what it gets back

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/frames.md](../frames.md), which summarizes it. This is the receipt for the
failure that misled three milestones. It is written as a budget rather than as a story, because the
number is the point.

## The symptom, and why it always accused the wrong test

The aarch64 test boot failed as `Unmappable(OutOfPageFrames)` about one run in three (measured
2026-08-16). It failed in whatever test happened to spawn last, which was never the test that spent
the memory. It failed disguised: milestone 107 (socket) met it as `time_tests` reporting *"no swish
program in the initrd archive, or no memory to wire one"*, which reads like a packaging bug and is
not one. `notes/net/memory-and-reclamation.md` had recorded it eight separate times as "`virtio::MAX_DEVICES` has asked for
reclamation again".

Two numbers, not one, and the second is the one that fails a boot. Free page frames and the longest
*run* of free page frames are different questions, and `alloc_contiguous` asks the second. Milestone 107
measured 137 page frames free and no run of 128 at the failing allocation and read it as exhaustion.
Both readings now exist: `memory::free_page_frames()` and `memory::largest_free_run()`
(`PageFrameAllocator::largest_free_run`, host-tested in `crates/page_frames/tests/allocator.rs`).

## The instrument: the page frame ledger

`kernel/src/testing.rs` reads free page frames once per test, at the top, so the readings *partition*
the run: what a test is charged is the drop between its own reading and the next one. That shape is
not fussiness. Reading before and after the test body instead charges only what the test spent while
it was running. A test that spawns a service and returns the moment it has its report leaves that
service still mapping its heap. On the first measured boot, before-and-after attribution left
17362 of 29091 page frames landing nowhere at all.

What a run prints:

```text
test kernel::user::dir_capability_tests::a_full_directory_capability_... ... ok
    [that test kept 2284 frames]
...
frames: 29280 free before the first test, 15249 after the last (14031 never returned); longest free run 14080
```

A charge under `PAGE_FRAME_REPORT_MIN` (16 page frames) is silent, so the transcript names the
services and not the arithmetic.

Two ceilings fail the run, and the second is the one that names the bug. `SUITE_PAGE_FRAME_BUDGET`
catches the total residue growing past what is accounted for below, which is how a
new service-shaped test that forgets to hand its memory back is caught in the act. `SUITE_MIN_FREE_RUN`
requires the boot to end with a free run of at least 1024 page frames, and that is the real gate: loading a
program calls `alloc_contiguous`, so what a boot runs out of is contiguity rather than memory. A suite
can pass a residue ceiling and still be fragmented into uselessness, which is exactly the state this
one was in.

## What the boot spent, measured

| | free before the first test | free after the last | never returned | longest free run |
|---|---|---|---|---|
| before | 29307 | **216** | 29091 | **117** |
| after reclaiming init | 29306 | 12504 | 16802 | 12394 |
| after reclaiming init and the net services | 29306 | **15307** | 13999 | **14080** |
| the same, remeasured on the merged tree | 29280 | **15249** | 14031 | **14080** |

216 free page frames and no run longer than 117 is the whole failure. There was no headroom left for
anything, so which test died was decided by scheduling.

The fourth row is the same boot after merging `main` on 2026-08-16. The 32 extra page frames it keeps
are the second act of milestone 54 (a network file service a Mac can actually mount). The SMB test now runs a seeding client through the FS service before
it wires the adapter, and that client is one more process. The number that decides whether a boot
lives is unchanged at 14080, because a client that runs and exits fragments nothing. Read the pair as
the honest shape of this ledger: the residue moves a little every time a test grows a process, and
the free run is what the gate is about. riscv64 came out at 13787 kept and 13733 longest, from
29692 free.

The largest single causes, before:

| page frames | who | what it was |
|---|---|---|
| 12289 | the six `spawn_progenitor` tests | 2048 page frames of building budget each, reserved and never returned |
| 2759 | the ten net tests | a `net_stack`'s 128-page-frame budget, its stack, and its client's, ten times |
| 2284 | the FS service | one block server and one FS server with an 8 MiB heap budget |
| 2146 | the two `authority_tests` | `root_supervisor`'s 1024-page-frame budget, twice |
| 1656 | the credential store | a 6 MiB budget sized by Argon2id's scratch |
| 1362 | the crash-recovery FS test | two more FS servers, deliberately killed and never reclaimed |

## The fix, which is wiring and not mechanism

Everything used here is DECISIONS §16 object revocation, already built and already proved. What was
missing was a handle and an ordering.

- `user::holding::Holding` (name provisional) remembers what the kernel handed a service: its
  threads, the regions to reclaim while those threads still exist, and the regions that may only be
  reclaimed once they are provably gone. `release` spends `sched::kill_thread` and
  `sched::reclaim_region` on them in that order, retrying because the first `reclaim_region` is what
  *arms* §16's kill on a resident. `release_or_fail` is the form tests use, because an instrument that
  reports success either way is not one.

- The region's endpoints are now swept before the refusal, not after (`sched::reap_region_objects`).
  This is the load-bearing half. A blocked thread never reaches `schedule()`, so it never spends the
  armed kill. A region holding a server parked in `RECEIVE` was therefore refused forever.
  `the_console_server_builder_brings_up_a_console_that_prints_for_it` builds exactly such a server out of the progenitor's budget, and
  its 2048 page frames were unreclaimable by construction. Sweeping first fixes it because the wake was
  already there: removing an endpoint drains its wait queues, aborts each waiter's IPC and wakes it,
  which is precisely the transition the doomed resident needs. A refused reclaim was already
  destructive (it arms kills; see `reclaim_region`'s BUGS), so this is the same commitment one object
  over.

- `spawn_elf_child_builder` carves a builder's building budget outside the spawned thread and hands
  the caller a holding over it. The region is unchanged; who can name it is not, and that is the whole difference
  between 8 MiB spent and 8 MiB lent.

- A service's endpoints come out of a region of its own (`net_stack`'s do), because that is the
  handle the sweep above needs. From the kernel's shared endpoint chunks there is no such handle and
  no way to end the process short of rebooting.

- Stack pages are retyped from a region instead of allocated, and that region is reclaimed only
  after every thread is gone. A `Spawn::maps` page is not a recorded mapping (see "Three ways a page
  gets into an address space" in [notes/frames.md](../frames.md)), so the revocation of §13 (capability revocation and untyped reclamation) cannot pull it, and freeing a running
  thread's stack is a use-after-free rather than a fault.

## What is still held at the end of a boot, and why

14031 page frames on the merged tree (13999 before that merge; the table above says where the 32
went), and the difference is accounted rather than shrugged at:

- The FS service, ~2284. Wired once (`fs_service::ensure`) and used by every later filesystem
  test. It is a boot service, not a leak.
- The credential store, 1656. Same shape: wired once behind a `DONE` flag and shared.
- `root_supervisor`'s two trees, 2146, and this one is a real limit rather than a choice.
  `root_supervisor` `SPLIT`s the spawner's budget out of its own, and `reclaim_region` refuses a
  region with live children (freeing its whole run would double-free the child's pages). The child is
  destroyed only by *its* owner, which is a process that has been torn down, so the parent can never
  become childless. Reclaiming a split parent whose children's owners are gone is what a capability
  derivation tree buys and this kernel deliberately does not have (notes/object-revocation.md). It
  wants its own lane.
- The crash-recovery FS servers, 1362, and the disk, sink, `c_seam` and shell services below them.
  All are the same shape as the two above: reclaimable in principle by giving their spawn helper a
  holding, and each is a small separate change rather than part of this one.
- One DMA page and one virtio shadow page frame per net service, ~20 page frames. Deliberately
  *not* reclaimed. The NIC keeps whatever receive buffers the dead driver posted, and returning those
  pages to the allocator would let a live device write into memory handed to somebody else. Ending
  that safely means resetting the device at teardown, which is a change to the transport seam and its
  own piece of work. Twenty page frames is not worth the hazard.
- A page per kernel endpoint, carved into chunks by `sched::create_endpoint` and never freed by
  design.
- The login service, ~640 (2026-08-22, milestone 49 (users, login,)). Same shape as the credential store above:
  wired once behind a `DONE` flag (`system_tests/src/user/login_tests.rs`) and shared by every login test.
  `crate::untyped::create` reserves the whole 640-page-frame construction budget the instant the
  service is spawned; splitting pieces of it into a caretaker or a client budget afterwards costs the
  ledger nothing further; only the initial reservation does. See notes/login.md and
  `components/src/login.rs`'s own BUGS on why nothing gives it back: the service serves logins for the life
  of the boot and this slice builds no teardown path.
- A second credential service instance, ~1659 (2026-08-23, milestone 155 (provisioning tool)). The provisioning
  suite (`system_tests/src/user/identity_provisioning_tests.rs`) needs a store *before* anyone has sealed
  it, which the tree's one shared fixture (`credential_tests::provisioned()`) cannot offer: that
  instance is sealed by the time it returns. So this suite wires its own, the same shape as the shared
  one and as permanent for the same reason. The reservation is `credential_service::start`'s own
  1552 page frames: `CRED_BUDGET_PAGES` 1536 plus `CRED_STACK_PAGES` 16. Add the small cost of the two
  `identity_provisioner` invocations this suite runs against it, and of one `fs_subtree_caretaker` its
  headline test builds to prove the created subtree is real. This suite's own tests report their
  charge directly (`[that test kept N frames]`), which is where the 1659 comes from rather than a
  re-derivation here.
- `MappedWindow`'s formatted panic, ~5 (2026-08-25, milestone 139 (drive unsafe) round 4). Found by the
  `toolchain/nightly-bump` PR going red on a plain toolchain bump with no code change of its own.
  It was bisected against CI's own historical `build + test (host + QEMU)` logs. Five independent
  runs read 18621 before `202831a3`/`c94f5d21`, and two read 18626 immediately after; both
  populations were internally exact. `MappedWindow::check` (`crates/user_mode_runtime/src/mapped_window.rs`,
  milestone 139) panics with a *formatted* message, and that alone is enough to pull `core::fmt`'s
  panic-with-arguments machinery into any binary that calls it, even once. Measured directly
  (`llvm-size`, dev profile): `painter`, `window` and `display` each grew their linked text by
  roughly 54 KiB (11–15 KiB to 65–70 KiB) adopting it; `display_terminal`, which already pulled the
  formatter in elsewhere, grew by only ~5 KiB.

  `display` is the standing candidate for where this becomes permanent rather than transient.
  `system_tests/src/user/display_tests.rs` says outright that its driver instance "is a long-lived
  server and never exits" for the rest of that test's boot. A process's `AddressSpace` is sized from
  its own ELF segment page count (`kernel/src/user/loader.rs::load`). So a permanently bigger binary
  should cost permanently more frames by construction.

  This is the fact the account does not have. The per-program page math for `display` alone
  (roughly +14 pages between the same two commits) does not cleanly reduce to the suite's measured
  +5. That specific test's own reported charge moved by only +1 across the same comparison.
  `report_frame_ledger`'s attribution is by whichever test happened to be running when a shared or
  lazily-built resource was first touched, not by the code that spent it. `#[test_case]`
  registration order can also shift when unrelated code size changes elsewhere in the binary. So a
  clean per-test diff was not trustworthy evidence here; only the repeated, internally consistent
  *suite-wide* total is. Recorded as +5, attributed to this migration with confidence; not
  attributed to a specific one of the four migrated programs.

  See `kernel/src/testing.rs`'s `SUITE_PAGE_FRAME_BUDGET` doc comment for the rest of the account.
  It includes the separate ~1-2 frame cross-environment variance this raise had to make room for
  (18621 → 18626 → 18627 local → 18628 the one CI run that failed). This investigation ruled a
  `swish.rs` change and a QEMU version mismatch out of it, and could not otherwise pin it down.
- `printenv`'s four spawns, ~85 (2026-08-26, milestone 47 (navigation naming), DECISIONS §111 (inert configuration)). This is
  `date_tests.rs`'s own shape one program over. `system_tests/src/user/printenv_tests.rs` spawns a
  real `printenv` ELF four times (`spawn_printenv`, `date_tests::spawn_date`'s own helper). Neither
  the child processes nor, in three of the four cases, a by-hand-allocated config-page frame
  (`assembled_page`/`blank_page`) are ever reclaimed. That matches `date_tests.rs`'s own five spawns
  and one `blank` clock page, which are not reclaimed either. Measured directly, twice, zero
  variance: 18717 both times, +85 over the previous budget.

  None of the four tests prints its own `[that test kept N frames]` line, and none of
  `date_tests.rs`'s five spawns does either. A handful of pages per spawn does not cross whatever
  threshold makes a line worth printing, but the frames still count toward the suite total.
  `printenv`'s own linked text is 15,381 bytes (`llvm-size`, dev profile), roughly four pages.
  Four spawns of that, plus a stack page and construction overhead each, is a plausible
  order-of-magnitude match to +85. This is not a page-by-page reconciliation, because of the
  attribution limit of `report_frame_ledger` that the entry above already names. See
  `kernel/src/testing.rs`'s `SUITE_PAGE_FRAME_BUDGET` doc comment for the rest of the account
  (18632 + 85 + 15 headroom = 18732).

## BUGS in the ledger itself

- A charge is attributed to the test that was running, not to the code that spent it. A boot
  service wired lazily by whichever test asks first is charged entirely to that test, which is why
  `a_full_directory_capability_does_everything_inside_and_nothing_outside` appears as the biggest
  spender in the tree while spending almost nothing of its own. Read a large charge as "the service
  this test was first to need", not as an accusation.
- The two ceilings are set from one measurement each, so they are as good as that boot was
  representative. A run where the host resolver does not answer the non-gating DNS check spends a
  little less; that variance is inside the headroom both numbers carry, but neither is a tight bound
  and neither should be read as one.
- **The ledger cannot see memory that never reaches the page frame allocator.** A region reserved and
  unspent costs exactly as much as one filled to its watermark, which is correct for this failure
  (an untyped is a reservation) and means the ledger says nothing about waste *inside* a budget.
