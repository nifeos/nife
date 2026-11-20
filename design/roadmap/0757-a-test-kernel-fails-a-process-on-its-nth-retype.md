---
status: BUILT
built: 2026-10-04
raised: 2026-10-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 757. A test kernel fails a process on its Nth retype

Promoted from its proposal, raised by milestone 745 (count the error paths no test reaches) and
approved by calef on 2026-10-04 (UTC) (#1591). The number 757 is provisional and the integrator
mints it at merge. *(Title and slug are drafts.)*

## Why

Milestone 745 counted 75 cleanup paths, error paths that give something back, all in code no
coverage run reaches; 17 are in `login`
([`notes/untested-error-paths.md`](../../notes/untested-error-paths.md)). A cleanup path that
forgets a `cap_delete` crashes nothing. The only evidence is a capability slot or a region's pages
that stay spent, so nothing finds it until a long-lived service runs out.

## Done when

1. The system-test kernel can fail a named thread's Nth retype exactly as an empty region would,
   and a shipping kernel cannot contain the code that does it.
2. A system test sweeps N over a whole `login` exchange, from 1 until a run makes fewer than N
   retypes, and after every run `login`'s capability table and every region it holds read as before.
3. Both run on all three ISAs, or a scope note records the gap; and each new test carries a
   `Falsification:` record that replays red.
4. A program found mishandling a failure is fixed if cheap, or recorded.

## What is built

**`kernel/src/retype_fault.rs`**, test-kernel only. `arm(tid, n)` names a thread and an N;
`disarm()` returns a `Tally` of how many retypes it saw and whether the Nth came. The hook sits at
the top of the three `memory_region` functions that take pages from a region (`split`, `retype_run`,
`retype_object_page`), before the region lock, and returns `None`, which is what an exhausted region
returns. So `SPLIT`, `RETYPE` and `RETYPE_OBJ` count once each, and a mapping counts once per page
table it builds, which reaches the kernel's own unwinds as well as the program's. The target is the
core's current thread, the one identity the kernel has at those functions without a lock.

**`system_tests/src/user/nth_retype_tests.rs`**, two tests:

- `login_gives_back_everything_when_any_retype_of_a_login_fails`, the sweep. It uses
  `login_tests`' one `login`, because a `login` cannot be torn down and a second one would be a
  permanent charge on the frame ledger. Per N it runs a `LOGOUT` login as `bob` (who holds no durable session there) and compares a
  census of `login`'s table and regions against one taken after a clean login.
- `the_nth_retype_fails_as_an_empty_region_would_and_no_other`, the mechanism with no service in
  the way. For each N the Nth of a `RETYPE`, a `RETYPE_OBJ` and a `SPLIT` is refused and moves
  nothing, and the others succeed. A fault armed on another thread neither fails nor counts this one.

Support: `sched::with_capability_table` (test kernel only) and `login_service::Wiring::tid`.

**Why a shipping kernel cannot contain it.** The module and every call into it are under
`cfg(feature = "system_tests")`. In any other build the module does not exist, so a hook written
without the `cfg` fails to compile, and one written with it compiles to nothing. A kernel *binary*
built with `system_tests` fails to link, on purpose (`kernel/src/lib.rs`, `system_tests_main`).
That is rung one of `CLAUDE.md`'s ladder (no code in the shipping binary that could fire), held up
by rung two (a link failure if the feature reaches a binary). No separate cargo feature: it would
be a second thing to keep out of shipping builds and would buy nothing, because an unarmed hook is
one relaxed load on paths that are never the IPC round trip. At equal cost the choice is the same.

Names are provisional: `retype_fault` and its `arm`, `disarm`, `Tally` and `fails_now`;
`with_capability_table`; `Wiring::tid`; the test module and both tests; `lay_out_child` and
`endow_child` in `supervision_protocol`.

## Measured, 2026-10-04 (UTC), QEMU on a developer machine

| ISA | retypes, first login | retypes, later login | runs failed | result |
|---|---|---|---|---|
| aarch64 | 34 | 27 | 27 | green, under the 5 s reporting floor |
| riscv64 | 35 | 28 | 28 | green, under the 5 s reporting floor |
| x86_64 | | | | skipped, as every `login_tests` test is: no RedoxFS disk on its test boot |

The mechanism test is green on all three. The first login counts more than later ones because it
builds what they reuse (a page table for a scratch window), so the sweep runs from steady state.

## The bug it found, fixed

**`supervision_protocol::build_child_space` leaked the caller's capability slots on failure.**
Recorded in its `BUGS` on 2026-08-26 by milestone 49 (users, login, and attribution) and left, because nothing could reach it. The
sweep reached it at retype 10, `mint`'s caretaker's first frame: each failed login left a stale
`AddressSpace` capability in `login`'s 32-slot table, so a `login` whose caretaker builds kept
failing would stop serving once the table filled. Now the address space, the TCB, a stack frame and
a segment frame are each deleted on the way out of the step that made them, and `build_child` does
the same when `CONFIGURE` refuses. Every caller of the tree's one userspace ELF loader inherits it.

With the fix, `login` gave back everything at every one of the 27 (28) points; no other path in a
login leaked.

## Falsifications

Each replayed on aarch64; both go red.

| patch (`system_tests/falsifications/user.nth_retype_tests.<test>.patch`) | what it puts back | red at |
|---|---|---|
| `login_gives_back_everything_when_any_retype_of_a_login_fails` | `build_child_space` drops the address space's `cap_delete` | retype 10, `slot 18: AddressSpace(..)` |
| `the_nth_retype_fails_as_an_empty_region_would_and_no_other` | `memory_region::split` loses its hook | arming retype 1 of 3: `seen: 2` |

## Follow-on

- **Milestone 786.** Milestone 786 (sweep the other long-lived services under the retype fault). `design/roadmap/0786-sweep-the-other-long-lived-services-under-the-retype-fault.md`:
  `login`'s start-up and schedule path, `system_initializer`'s session build, `swish`.
- **Recorded.** x86_64 does not run the `login` sweep because its test boot attaches no RedoxFS
  disk, the gap every `login_tests` test has; the hook is portable and the mechanism test runs there.
  In `nth_retype_tests`' header.
- **Recorded.** One thread, not a process; only the Nth fails, not every one after; one target at
  a time. In `retype_fault`'s `BUGS`.
- **Recorded.** `MemoryRegion::RETYPE` and `RETYPE_OBJ` keep their pages when the capability
  table is full: the retype has happened and no capability names it, so a full table spends a page
  per call until the region is destroyed. Not reachable by this fault, which fails the retype and
  not the grant. In `kernel/src/system_call.rs` beside `memory_region_retype`.

## Index row

The system-test kernel fails a named thread's Nth retype as an empty region would, and a sweep over
a whole `login` exchange proves `login` gives back every slot and page at every point. BUILT on
aarch64 and riscv64 (x86_64's test boot has no disk for `login`); found and fixed a slot leak in the
tree's one userspace ELF loader.
