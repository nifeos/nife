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
# 752. A seeded syscall driver with a shadow model

Part (b) of the proposal `fuzz-the-surface-a-confined-process-can-reach` (#1592), ruled yes by calef
on 2026-10-04 (UTC). The number 752 is provisional (749 to 751 are held by open pull requests) and
the integrator mints it at merge. The proposal file still holds part (a); moving (b) out of it is
the integrator's once #1592 lands. *(Title and slug are drafts.)*

## What is built

`system_tests/src/user/syscall_fuzzer_tests.rs`, one test,
`seeded_syscalls_agree_with_the_shadow_model`. From a seed, seven kernel threads issue random
capability operations through `syscall::invoke`, the function the trap handler calls for
`SYS_INVOKE`. The conductor predicts every answer, every wake, where every parked thread is parked,
and every slot of every capability table, and fails on the first difference with the seed, the step,
the call and a one-line replay. The seventh thread is the witness: it holds nothing, everything it
tries must be `NoSuchSlot`, and its table must stay empty.

Reached: two rendezvous, one notification, two frames and the Replies `CALL` mints; `SEND`,
`RECEIVE`, `SEND_CAP`, `RECEIVE_CAP`, `CALL`, `BADGE`, `REPLY`, `SIGNAL`, `WAIT`, `POLL`, `BIND`
(refusals only), `PageFrame::REVOKE`, an unknown method on each, and `SYS_CAP_DELETE`. Slots
include empty and out-of-range ones, rights are random subsets, and both arrival orders happen at
every rendezvous. The rest is in the module's `BUGS`.

**Kernel threads, not an EL0 program.** The proposal said EL0. The oracle needs to know which side of
a rendezvous parked first, which only the kernel can say without guessing at timing, and an EL0
helper needs a TCB and an address space each. What is not exercised is the trap entry's register
marshaling, which every EL0 test already crosses. At equal cost the choice would be the same,
because the ordering knowledge is what makes the oracle exact.

Names are provisional: the module, the test, and `NIFE_SYSCALL_FUZZ_SEEDS`.

## Measured, 2026-10-04 (UTC), QEMU on a developer machine

The suite runs seeds 0 to 31 and the corpus seed 180: 33 seeds, 1,517 calls (800 refused, 338
parked, 40 delegations, 15 replies, 42 revokes). All three ISAs ran the same 1,517 calls with the same
census, which is the determinism claim measured.

| ISA | wall time | seeds/s |
|---|---|---|
| aarch64 | 338 ms | 97 |
| riscv64 | 612 ms | 53 |
| x86_64 | 329 ms | 100 |

CI cost per merge is under a second of guest time per ISA plus the compile of one test module.

## The six falsifications

Replayed on aarch64 with the committed seeds, each patch applied alone. All six turn it red.

| patch (`system_tests/falsifications/user.<module>.<test>.patch`) | seed | step | the mismatching call |
|---|---|---|---|
| `receive_cap_attack_tests.a_call_server_tells_its_reply_from_a_delegation_on_both_arrival_orders` | 0 | 17 | `CALL` woke a parked `RECEIVE_CAP` with `x4 = 0`; the model says `REPLY_DELIVERED` |
| `receive_cap_attack_tests.a_plain_send_to_receive_cap_delivers_no_cap_whichever_side_parks_first` | 0 | 18 | `SEND` woke a parked `RECEIVE_CAP` with `x1 = 1` (the sender's word); the model says `NO_CAP` |
| `receive_cap_attack_tests.a_death_message_received_by_receive_cap_delivers_no_cap_whichever_side_parks_first` | 0 | 18 | the same hunk, so the same call |
| `receive_cap_attack_tests.an_interrupt_signal_received_by_receive_cap_delivers_no_cap_whichever_side_parks_first` | 0 | 18 | the same hunk, so the same call |
| `receive_cap_attack_tests.a_send_cap_collected_by_a_plain_receive_stages_nothing_for_a_later_plain_send` | 19 | 63 | `RECEIVE_CAP` returned `x1 = 2`, a stale delegation; the model says `NO_CAP` |
| `revocation_in_flight_tests.a_capability_revoked_while_it_is_in_flight_does_not_reach_the_receiver` | 559 | 14 | `RECEIVE_CAP` returned `x1 = 6`, a revoked frame; the model says `NO_CAP` |

The last two rows were re-measured on 2026-10-04 (UTC), on aarch64, riscv64 and x86_64, after
§246 (a plain `RECEIVE` never takes a capability) changed the generated sequence; they were 180
at step 75 and 5 at step 39. Seed 342 joined the corpus in commit f7dcd3dca, and the recheck
is commit 134923b38 (PR #1611). Seed 342 rotted between 2026-10-04 and 2026-10-08 (UTC), found by
milestone 139 (drive the unsafe count down) round 10's sweep: its record survived its own defect on
the claim commit too. A
5,000-seed sweep under the same patch re-found the defect at seed 559, step 14, red on aarch64,
riscv64 and x86_64, and the row above carries 559.

Three patches are byte-for-byte the same hunk (milestone 634 (a plain SEND received by
RECEIVE_CAP never hands the receiver a sender-chosen slot)), so four defects are distinct.

The first cut missed two. With four actors and random rights, 24 seeds went green under the
patches for milestone 633 (an outside agent attacks the confinement claim) and for revocation in
flight; a 5,000-seed sweep found both (seeds 751 and 271). After the endowment was biased toward
whole rights and the actors went from four to six, a second sweep found every patch by seed 180,
and seed 180 became the corpus. The weekly sweep is what finds the next one.

## The weekly sweep

`.github/workflows/falsifications.yml`, job `syscall-fuzz`, on milestone 616 (constrained hardware
fuzzing for the boot path)'s cadence. It starts at the ISO week times a million, runs seeds until
60 s of guest time have passed (under the 90 s per-test budget), and prints the range it covered.
At the aarch64 rate above that is about 6,000 seeds a week. A finding is a red scheduled run, never
a red trunk. Not yet run on a runner; the first scheduled run is its measurement.

## A real finding, and it is an architect's call

**A capability reaches a plain `RECEIVE` on one arrival order only.** A `SEND_CAP` or `CALL` that
finds a plain `RECEIVE` parked installs its capability in the receiver's table and returns the slot
in `x1`. In the other order the receiver gets no capability (633's fix), and a `CALL` caller
collected that way waits forever for a Reply nobody holds. The suite reached it 12 times in 33 seeds
and the model pins today's behavior, so the test is green and any change is seen. Recorded at
`abi::rendezvous::RECEIVE`'s `BUGS`.

It is exploitable today. A std program, including an unvouched one, is spawned holding its own
memory region with `WRITE` in slot 0 (`crates/system_initializer/src/lib.rs:3756`, the `StdLayout`
caps). `MemoryRegion::RETYPE` checks only `WRITE` and returns a frame with `Rights::ALL`
(`kernel/src/syscall.rs`, `memory_region_retype`), so the program holds a `GRANT` capability it can
`SEND_CAP` again and again. Its stdout is a rendezvous that a plain-`RECEIVE` server drains. Every
delegation that finds that server already parked costs it one of its 32 slots, and none of these
servers reads `x1`, so none is ever freed. Native programs given `--mem` are in the same position;
native programs without it hold no `GRANT` capability.

| server (plain `RECEIVE`) | endpoint | holders of `WRITE` | a holder with a `GRANT` capability? |
|---|---|---|---|
| swish, reading a child's output (`components/src/swish.rs:3220`, also 2594, 3195, 4522) | the child's `out` | every spawned child | yes: std children, `--mem` children |
| swish, draining diagnostics (`swish.rs:3367`) | the diagnostics rendezvous | stages that declare diagnostics | yes, if the stage is std or `--mem` |
| `terminal_sink_caretaker` (`:80`) | the terminal sink | children given the default diagnostics or a screen tail | yes: a std child on that path |
| `wc`, `mdr` (`wc.rs:117`, `mdr.rs:155`) | the pipe source | the upstream stage | yes if upstream is std or `--mem` (not traced end to end) |
| console (`console.rs:199`) | requests | `line_editor` only | trusted |
| `system_log` (`:151`) | intake | no untrusted holder at boot | no |
| supervisors, `job_undertaker`, `timetable`, `swapper`, report endpoints | fault, death, note, report | the kernel or trusted components | no |

So a confined program can fill swish's table today, and the shell is the process that spawns
everything else. Read from the code by a sub-audit on 2026-10-04 (UTC), with the key sites checked by
hand; not driven end to end under QEMU.

The seven questions:

1. Options. (A) Plain `RECEIVE` never takes a capability: a `SEND_CAP` arriving second delivers its
   data and drops the copy, as the sender-first order already does, and a `CALL` that reaches a plain
   `RECEIVE` is answered `Gone`. (B) Plain `RECEIVE` always takes one, reverting 633's sender-first
   drop; lost, because a receiver that did not ask still gains authority. (C) Refuse a `SEND_CAP` at
   a receiver that did not ask; lost to (A), because the sender-first order cannot refuse (the sender
   has already parked and been told nothing) so the two orders would still differ. (D) seL4's
   sender-side gate, below: `SEND_CAP` needs `GRANT` on the endpoint capability itself. A further
   rung, not a substitute: it closes this only where every `out` is handed out without `GRANT`, which
   is a sweep of the grant sites, and it does not make the two orders agree. (E) Leave it recorded;
   lost, because it is exploitable.
2. This tree. The sender-first order is (A) for `SEND_CAP` (`sched::ipc_receive` drops
   `outgoing_cap`). Of the plain-`RECEIVE` sites in `components/`, `crates/`, `fixtures/` and `src/`,
   none reads `x1` as a slot and none `REPLY`s afterwards: `user_mode_runtime::reply` accepts only a
   typed Reply, and only `receive_request` (a `RECEIVE_CAP`) builds one. Every `CALL` server in the
   tree receives with `RECEIVE_CAP` (the caretakers, the drivers, `line_editor`, `credentialer`,
   `entropy`, `net_stack`, the compositor, `swapper`, the fixtures), and every `CALL` client targets
   one of them. The std port makes no `RECEIVE` at all. So `CALL` answered `Gone` at a plain `RECEIVE`
   breaks no program in the tree.
3. Prior art, read: the seL4 Reference Manual, `manual/parts/ipc.tex` at seL4 commit `c6ce4d2a0c`.
   Section "Capability Transfer" copies capabilities "provided that the endpoint capability invoked
   by the sending thread has Grant rights". Without it the send is "a transfer of the raw message,
   without any capability transfer". The receiver names the one slot it will accept a capability
   into (`receiveCNode`, `receiveIndex`, `receiveDepth`). If that slot cannot be looked up, the
   transfer ends and "no error message will be returned to the receiving thread". Section "Calling
   and Replying" puts a `Call`'s reply capability "in a specific slot of the receiver TCB", outside
   any CSpace, so a plain `seL4_Recv` can always answer a `Call`. So seL4 has the receiver opt in and
   the message arrive regardless, which is (A) for `SEND_CAP`, and it gates the sender as (D) does.
   Its Reply needs no receive slot because it lives outside the capability space. Nife's lives in
   the table, so a `CALL` at a plain `RECEIVE` has nowhere to put it, and `Gone` is the honest
   answer.
4. Premise. Verified on three ISAs (12 occurrences in the committed seeds); the code is
   `sched::ipc_send_cap` and `ipc_call_badged`'s receiver-first arms.
5. Cost, measured on a throwaway branch (never pushed, deleted) carrying (A): a `receiving_cap` flag
   set at each receive's park, and a branch in the two receiver-first arms.
   `script/fastpath-footprint`, `ipc_call_reply`: aarch64 7,044 to 7,116 B (+72, +1.0%), riscv64 6,106
   to 6,166 (+60), x86_64 8,227 to 8,339 (+112, +1.4%). `ipc_send_receive` moved 4 B or less, and all
   are inside the 5% band. `script/bench` (aarch64, TCG and icount): `call_reply` 1,072,267 to 1,072,517 ticks over
   1,000 iterations (+0.02%), `ipc_rtt` 1,046 to 1,061 ticks per iteration (+1.4%), `ipc_rtt_el0`
   +0.1%, `null_syscall` unchanged. The sender-first `CALL` arm (answer the stranded caller `Gone`) was
   not in the measured patch and adds a branch off the fastpath.
6. Reversibility. A change to what `RECEIVE`, `SEND_CAP` and `CALL` return: §10 (the
   capability-based microkernel process model)'s surface. Nobody has acted on today's behavior (the
   audit above), so the change is cheap now and gets dearer with every program written against it.
7. At equal cost. (A) either way; the measured cost is small, so this is not an effort argument.

Recommendation: (A), with `CALL` at a plain `RECEIVE` answered `Gone`, on both orders. It closes a
hole reachable today, breaks nothing in the tree, makes both orders say the same thing, matches
seL4's receiver-opts-in rule, and costs about 1% of the `CALL` closure. (D) is worth a later look as
a second rung. When (A) lands, the driver's model changes in the two receiver-first arms and the
`cap_to_plain_receive` census becomes a count that must stay zero.

## Follow-on

- **Recorded.** The plain-`RECEIVE` finding above waits on an architect (a `design/decisions/` section is the integrator's to mint), and is recorded meanwhile in `abi::rendezvous::RECEIVE`'s `BUGS` and the driver's `BUGS`.
- **Recorded.** What the generator does not reach (timers, `MemoryRegion`, TCBs, a bound
  notification, death messages, `PageFrame::MAP`), in the driver's `BUGS`. A Nth-retype test can
  reuse `Model`, `Stage` and the actor loop by adding an `Obj::Region` and its methods.

## BUGS

- Both seeded drivers, this one and the confined driver of milestone 779 (fuzz the surface a
  confined process can reach), wait with `testing::wait_on_every_core`: two seconds of ticks on
  every online core, then a 20 s backstop on the counter. A vCPU the host stops takes no ticks, so
  it holds a wait open and does not fail it. A host that stops a vCPU for longer than the backstop
  still fails a run, and the message then reads "(the backstop)" beside a core with few ticks.
- Side by side under KVM, two-second deadlines on the counter failed up to half of the
  guests at 8 and 16, and every failing seed passed alone. With the every-core wait, 4 guests ran
  green 32 of 32 times, beside busy loops too. Nobody has yet run 8 or 16 guests with it, so a run
  wider than 4 guests may still report false reds. The table is in `notes/fuzzing/rates-on-x86-64-under-kvm.md`.
- What stops a vCPU on that host is not known. Instrumented runs saw single stalls of up to 1.45 s
  of the counter that ended without a fault, and four guests pinned to four host CPUs beside eight
  busy loops saw none, so CPU contention alone is not it.

## Index row

A seeded driver issues random capability operations through the real syscall layer and predicts
every answer and every capability table from a shadow model, so a confinement defect that crashes
nothing still fails. BUILT: runs on all three ISAs, red under all six recorded kernel
falsifications, and found that a capability reaches a plain `RECEIVE` on one arrival order only.
