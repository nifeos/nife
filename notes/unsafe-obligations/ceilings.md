# The unsafe census: what each number is held to, and why the answers differ

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/unsafe-obligations.md](../unsafe-obligations.md), which summarizes it. The
measurement behind these numbers is in [census.md](census.md). Both are the work of milestone 134
(the register of measures).

## The density outside `arch/`

At most 52 <!--count-at-most:unsafe-density-outside-arch--> unsafe blocks per 10,000 lines
outside `kernel/src/arch/`. The direction is down, because unsafe outside `arch/` is not paying
for hardware access. It is a raw syscall, a shared page, or a hand-rolled data structure, and each
of those has a safe wrapper somebody could write. The ceiling was first written at a threshold the
tree crossed the day before, rather than at slack. Every sample before 2026-08-18 would have failed
it, 2026-08-16 included at 111.7. That is what makes it a ratchet instead of decoration.

Every move of this ceiling came behind a measured reduction. The rule each move follows is milestone
139's block, `design/roadmap/0139-drive-down-unsafe.md`.

| Round | Date | Ceiling | Density after | What collapsed |
|---|---|---|---|---|
| 1 | 2026-08-23 | 100 to 97 | 90.8 | seven programs' volatile accessors, onto `MappedWindow` |
| 2 | 2026-08-24 | 97 to 96 | 89 | `user_mode_runtime`'s twelve `asm!` traps; nine FS page-copy loops |
| 3 | 2026-08-24 | 96 to 95 | 88 | `swish`, `disk_surveyor` and `net_stack` windows |
| 4 | 2026-08-24 | 95 to 94 | 87 | the framebuffer and graphics windows |
| 5 | 2026-08-24 | unchanged | 87 | device register blocks onto `tock_registers` |
| 8 | 2026-09-01 | 94 to 88 | 78.8 | 37 page-zeroing sites, 5 device-tree parses |
| 9 | 2026-10-07 | 88 to 72 | 65.3 | the revocation log walk, address-space installs, code pages |
| 10 | 2026-10-08 | 72 to 66 | 59.7 | the scheduler queue ownership token, `Unqueued<T>` |
| none | 2026-10-12 | 66 to 61 | 61.09 | nothing; the ceiling caught up to the tree |
| 891 | 2026-10-12 | 61 to 60 | 60.83 | `static mut` buffers into atomics and a firmware allocation |
| 11 | 2026-10-19 | 60 to 52 | 52.74 | raw `invoke` calls onto the runtime's wrappers; four test suites' repeated contracts into one type or helper each |
| 12 | 2026-10-19 | 52 to 51 | 51.48 | six more test suites' repeated contracts into one helper, token or fixture method each |
| 784(g) | 2026-10-21 | 51 to 52 | 52.17 | nothing; 818 safe lines left the denominator and no unsafe was added (calef, on #2497) |

## Round 1 chose the headroom

Its reasoning is the one later rounds argue from. The new ceiling kept 7 points of headroom above
the density that reduction actually reached (90.8, truncated to 90). That is the same absolute
headroom the original 100-vs-93 ceiling carried. It was not written at the exact new value the way
`unsafe-thread-safety-claims` and `agents-md-lines` were.

Those two were populations small enough, or additions rare enough, that every single one deserves a
stop. This measurement moved on 38 non-merge commits in 14 days before it was first gated. That is
ordinary lane traffic, not a population worth stopping on every member. A zero-headroom density
ceiling would fail the next lane that adds one legitimate unsafe block outside `arch/` without
growing the line count to match. That is exactly the "only ever rejects legitimate work" signature
this script has already deleted three checks for.

Headroom here is not slack given back. The ceiling fell by the same 3 points the density fell from
its pre-reduction reading (100 to 97, against 93.4 to 90.8). So the full gain that lane won is
locked in, and nobody can silently spend it back up to 100.

## Round 8: the kernel outside `arch/`

Lowered a sixth time, 94 to 88, by milestone 139 (drive the unsafe count down, and cinch the ratchet behind it) round 8 (2026-09-01). The six-point step is the
first one this measurement's own history argues for, rather than the convention. Rounds 6 and 7
worked `user/`, the userspace crate that milestone 175 (split `user/`) divided into `components/`
and `fixtures/` on 2026-09-13, and left the ceiling at 94. Round 8 is the first round to work `kernel/src` outside
`arch/`. That had been the largest unworked pool in the tree: 242 blocks, against `user/`'s 162
after round 7. It is also the part DECISIONS §14 (the project's direction) calls verified. Two collapses, both the §94 (what may live in a userspace library, and what must be per-binary) shape
and both measured from the diff against this round's own base commit (`8fc30efb`):

*Page zeroing, thirty-seven sites.* Every service, driver and test fixture that allocated a frame
went on to zero it by hand. Each had its own `// SAFETY:` comment over `core::ptr::write_bytes`,
asserting the same two facts. A frame just handed back by `memory::alloc` is exclusively the
caller's, and the direct map reaches it. That is one fact about what the allocator returns, and the
allocator is the only thing that can check it. Two of the copies had already noticed they were
copies (`kernel/src/user/rmle_service.rs`: *"a second copy of three lines"*;
`kernel/src/user/session_reviver_service.rs`, retired 2026-09-27 and gone as of 2026-10-11:
*"matches `fs_service::frame`'s own shape"*) without anyone lifting it out. That is the same tell
`user/src/ntp.rs` (now `components/src/network_time_client.rs`) carried for round 1's cluster and
`user/src/timetable.rs` (now `components/src/timetable.rs`) for round 6's. `memory::alloc_zeroed` and
`memory::alloc_contiguous_zeroed` (both new; ratified by calef 2026-09-01) hold it once, in the
module that owns the allocator. Every migrated call site is now ordinary safe code.

*The device tree, five sites.* `memory.rs`, `console.rs`, `pci.rs` and `smp.rs` (twice) each took
the boot pointer, from `crate::DTB` or as an argument that was always that same value. Each handed
it to `dtb::Dtb::from_ptr` under a hand-written comment rewording the same two facts. It is the
pointer firmware put in `x0`/`a1` and `kernel_main` stashed before anything else ran. And it is
physical, so the direct map names it. `crate::device_tree` (new; ratified by calef 2026-09-01)
holds that once, beside the static it is a fact about. Three functions lost a `dtb_ptr` parameter
that was always `crate::DTB` in the bargain, which is the same one-source-of-truth gain one level
out.

Measured from the diff: 42 `unsafe {` blocks removed, 2 added, net -40. That took `kernel/src`
outside `arch/` from 242 to 202, and the tree-wide count from 738 to 698. Density 83 to 78
(truncated; 78.8 exactly).

## Why 88 and not something tighter

Ratified by calef, 2026-09-01. An earlier draft of this paragraph argued against a "seven-point
convention", and there is no such convention. The actual headroom on record is six points at round 1
(100 against a density of 90.8) and seventeen at round 7 (94 against 77). Round 6's own "the 7-point
cushion every prior round preserved" was true of the run of rounds it was written about, and not of
the milestone. What the rounds have really held to is this block's own stated rule. The ceiling
falls whenever a real reduction lands, and the headroom is argued beside the marker rather than read
off a table. So 88 is not a departure; it is that argument, made here.

The argument is a measurement none of the earlier rounds had. Round 7 reached density 77 on
2026-08-26. This round found 83 on 2026-09-01: six points of unrelated growth in six days, the
steepest stretch on record by some distance. A ceiling seven points over the current density is
therefore about one week of ordinary lane traffic before it fires on somebody's honest work. That
is not a ratchet. It is the exact "only ever rejects legitimate work" signature `script/lint` has
already had three checks deleted for. 88 keeps ten points over the 78 this round reached, which at
the observed rate is roughly ten days. And it still cinches six of the eleven points that were
standing above the tree when this round started.

The gain is more than kept, and in a note whose whole subject is a measurement the arithmetic is
worth stating exactly. The density fell five points (83 to 78, truncated) and the ceiling fell six
(94 to 88), one point further than it gained.

## Rounds 9 and 10, and the 2026-10-12 cinch

Lowered again, 88 to 72, by milestone 139 round 9 (2026-10-07 UTC). Five weeks of lane traffic
had taken the density from 78 to 66 with no round working it; the ceiling stood 22 points over
the tree. The round removed 25 blocks and added 2 (1,147 to 1,124,
density 66.6 to 65.3, from the diff against base `3dfbf1fd4`).

Lowered again, 72 to 66, by milestone 139 round 10 (2026-10-08 UTC), the scheduler queue ownership
token, counted from the merged tree: 1,029 blocks over 172,506 lines, density 59.7. 66
keeps round 1's seven points.

Lowered again, 66 to 61, on 2026-10-12 (UTC), with no reduction behind it. In four days ordinary
lane traffic took the tree from 59.7 to 61.09 (1,103 blocks over 180,545 lines at base
`0e72ca92a`), so the ceiling stood five points over a tree nobody had cut. Milestone 139's ratchet
rule says a ceiling above the true number is a budget, so 61 gives up round 1's headroom on
purpose. Truncation still leaves some: about 16 blocks at that day's line count before the density
reads 62. At the rate of those four days (about 0.35 a day) that is a few days of traffic, and
then the gate stops a lane on honest work. That lane finds a reduction, or calef raises the
ceiling with the reason written here. This is the trade round 8 argued against; it is recorded so
the first lane it stops finds the argument beside the marker. calef ruled on it the same day
(UTC): "Keep 61."

Lowered again, 61 to 60, by milestone 891 (a mutable static becomes an atomic, a lock or a
recorded cell) on 2026-10-12 (UTC). Moving `static mut` buffers into atomics and an owned firmware
allocation removed 4 blocks, and the merged tree reads 60.83 (1,099 blocks over 180,675 lines).
The ceiling follows it down, by the rule the paragraph above sets.

## Thread-safety claims

At most 23 `unsafe impl Send`/`Sync` claims <!--count-at-most:unsafe-thread-safety-claims-->,
and this one has no headroom at all. Each is a hand-written assertion that the compiler is wrong
about a type, which is the most consequential unsafe in the tree: a wrong one is a data race that
no test reliably reproduces. The population moved twice in three weeks, so a zero-slack ceiling
costs a lane one line and buys a written reason for every addition. That is the same trade
`bench/baseline-aarch64.txt` makes and this tree already respects.

Raised from 17 to 18 by milestone 134's Tier A lane (2026-08-22). `kernel/src/bench.rs`'s
`Racy<T>` (E4, application working-set displacement) is a second instance of `sched.rs`'s existing
corruption-canary idiom. It is one `unsafe impl<T> Sync for Racy<T> {}`, guarded by the same
argument that one already carries: a scratch buffer one thread at a time touches, serialized by the
caller rather than by a lock. Same shape, same reasoning, a different file.

Raised from 18 to 20 by milestone 47's environment-variable fork (2026-08-23, DECISIONS §111):
`crates/environment_contract::ConfigPage`'s `unsafe impl Send`/`Sync`. That is the exact pair
`clock_contract::ClockPage` already carries, for the same argument, restated for a type with a
plainer contract. The config page is shared across address spaces by construction; that is the
whole point of a page-shaped endowment. Every access goes through the same immutable byte reads,
whichever process is reading. So there is no non-atomic mutable aliasing for either trait to protect
against. `ClockPage` needs the same two impls despite carrying a seqlock, precisely because its
*writer* uses atomics too. `ConfigPage` needs them for the simpler reason that it has no writer at
all once it is mapped (see `environment_contract`'s own docs on why it needs no seqlock).

Raised from 20 to 22 by milestone 161's x86_64 timebase-page work (2026-08-25):
`crates/counter_frequency_contract::TimebasePage`'s `unsafe impl Send`/`Sync`, the same pair
`ConfigPage` already carries and for the identical argument. The page is computed once by the
kernel at boot (`kernel::user::x86_timebase_page_phys`) and mapped read-only into every x86_64
process; it has no writer once mapped.

Raised from 22 to 23 by milestone 161's x86_64 SMP item (2026-08-25): `kernel::cpu::X86TrapPerCpu`'s
`unsafe impl Sync`. It is a single claim over a struct of three plain `u64`s reached through
`PerCpu` (`x86_trap`), via the `gs`-relative addressing `IA32_GS_BASE` already gives every core to
its own block. It carries no lock because it needs none. `trap.s`'s `isr_restore` and
`x86_syscall_entry` are the only readers or writers, both running on the core whose own slot they
touch. `IA32_GS_BASE` is an MSR no context switch saves or restores and no other core's write can
name. So two cores can never reach the same instance. That is the argument `PerCpu` itself already
carries (`unsafe impl Sync for PerCpu`, cpu.rs's own comment: "no two cores ever reach the same
block").

Raised from 23 to 24 by milestone 835 (a C library, stage 1: files, clock and memory), 2026-10-10
(UTC): `OneThread`, the C library's descriptor table, sound because a stage-1 C process has one
thread. Milestone 836 (a C library, stage 2: threads) takes it back out.

Lowered from 24 to 23 by milestone 836 (a C library, stage 2: threads), 2026-10-12 (UTC). The
descriptor table is a `std::sync::Mutex` now. The thread record a `pthread_t` names is built from
atomics and a lock, so it needs no claim of its own.

## Other unsafe traits

Other than `Send`/`Sync`, at most 13 <!--count-at-most:unsafe-trait-claims--> `unsafe impl`s of
an unsafe trait, at the tree's exact value for the reason the line above
gives (milestone 139 round 9, 2026-10-07 UTC; the marker's name is provisional). The census once
counted `unsafe impl` as one number and the gate watched only its `Send`/`Sync` half. This is the
rest (`GlobalAlloc`, `intrusive_fifo::Node`, `ns16550::RegisterSpace`, `virtio_drivers::Hal`).
It read 7, 9, 9, 9, 11 and 12 from 2026-08-18 to 2026-10-10, the last for the futex host tests.
The thirteenth is
`confined_virtio::WindowHal`, milestone 947 (the virtio input device runs on `virtio-drivers`):
a driver built on that crate has to claim its `Hal` contract, and the module's `BUGS` say which
part this claim does not meet.

## Mutable statics

At most 26 `static mut` declarations <!--count-at-most:static-mut-declarations--> in the Rust that
runs on nife, outside the system tests and the fixtures. The marker's name is provisional, and it is
a ratchet ceiling (`RATCHET_CEILINGS` in `script/lint`): it fails when the count rises and when it
falls, so a converted static is banked by lowering this number in the same commit. Milestone 891
(a mutable static becomes an atomic, a lock or a recorded cell) added it on 2026-10-12 (UTC). The
census then read 52 in all tracked Rust and 42 in this scope, and the milestone took the scope to
27, all in `components/`, which milestone 812 (real threads in one address space) converts. A
`static mut` is not unsafe code, but every access to one is, and the argument that
only one writer exists lives in a comment. The test harnesses are out because the census the
milestone was raised from left them out; their single-threaded shape is each test's own premise.

## What has no ceiling, and why

No target for `kernel/src/arch/`, which is 139 blocks and rising. Driving that number down means
either writing assembly wrong or moving it out of `arch/`, and DECISIONS rule 1 says arch code
belongs there. So a ceiling would be a gate pushing against the architecture. An honest census with
no direction is the right answer. It is not left as prose, though, because prose is where numbers go
stale: `script/lint` prints it on every run, asserted never.

No second `unsafe fn` count. The `==> unsafe fn contracts` check already derives one and prints it,
at 53 declarations on 2026-08-18. The parent's own "the shape of the 33" heading has been wrong for
days with nothing to say so. Adding a second count on a slightly different scope would be the exact
drift this milestone exists to stop, so the register cites that line instead. The 33 heading is left
standing. Its table of eleven `unsafe fn`s with no unsafe operation is still the finding, and
renumbering a heading to chase a moving count is the maintenance tax the whole convention refuses.

And no ceiling on `unsafe fn` either, re-decided with data by milestone 139 round 9 (2026-10-07
UTC). With `unsafe_op_in_unsafe_fn` on, every call of one is a block the density already counts,
so a second ceiling would price one hazard twice. And the count is not monotone: per 10,000 lines
outside `arch/` it read 6.0, 10.1, 11.2 and 8.8 between 2026-08-18 and 2026-10-07. A ceiling
written in August would have failed honest work in September.
