---
status: BUILT
raised: 2026-10-20
built: 2026-10-24
promoted_from: the-self-test-proves-supervision-and-teardown
---
# 944. The self-test proves a supervised death and a teardown that leaks nothing

*(Title and slug provisional.)*

Reuse: the TCB spawn path (`create_thread_control_block`, `CAP_INSERT`, `CONFIGURE`, `START`) and
the supervision fixtures `system_tests` already had, moved into the kernel so both use one copy.
`user::run` was the proposal's candidate and is not used: supervision is fixed at `START` from the
reserved fault slot and never afterward (DECISIONS §26 (the fault endpoint)), and a thread `run`
enters never passes `START`. Nothing new is taken.

Raised 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`. calef ruled on 2026-10-19
(UTC), on #2434, that every claim the per-port tours made becomes a check in `self_test::run`.
Two of `x86_64`'s did not, and milestone 890 (riscv64 and `x86_64` join the shared boot path)
recorded them in its BUGS.

## Index row

Two of the per-port tours' claims are checks in the boot self-test on all three ports: a faulting
thread's death reaches its supervisor, and tearing a process down returns every frame. Both spawn
through one helper that takes a supervision endpoint.

## The two claims

- A faulting user-level thread's death reaches its supervisor as a message, naming the thread,
  the faulting pc and the address.
- Tearing down a process returns every frame it cost. A teardown measured once, before a spawn and
  after its teardown, also counts first-use carves and whatever else allocated in the window, so
  the claim is about a second round.

## What was built

- `kernel/src/user/supervised_child.rs` (name provisional): `spawn(region, stub, capabilities, fault)`
  builds a child's address space, code page, stack page and TCB out of one untyped region, puts
  `capabilities` in slots 0, 1, ... and `fault` in `abi::fault::FAULT_EP_SLOT`, and starts it. It refuses
  by naming the step rather than panicking. Beside it are the fault and report stubs for aarch64,
  riscv64 and `x86_64` (`x86_64`'s from `user::x86_programs`), `code_page`,
  `supervision_endpoint` and the addresses. `CODE_VA` and `STACK_VA` are spelled out as
  `CODE_ADDRESS` and `STACK_ADDRESS` there; the suite imports the first under its old name.
- `system_tests`' `supervision_tests` and `code_pages` use that module, so the suite and the boot
  build the same child.
- `self_test::run` gains two checks after `budget`, and `boot_ladder::SELF_TEST_CHECKS` lists
  them. `SELF_TEST_CHECKS_EARLIER` gains 13, so a log from a 13-check kernel still reads green.
  - `supervised`: one child, born supervised, faults on a load from an unmapped address. The
    message must be `EVENT_FAULT` with the child's thread id, the load's pc and the address; the
    corpse is then reaped with its region.
  - `teardown`: the same round twice, judged on the second: the used-frame count must not move.
- `budget` waits for its helper thread's reap, so no teardown is in flight when `teardown`
  brackets the count.

notes/self-test-checks.md has each check's design.

## Evidence

`script/boot-check`, all three under QEMU, 2026-10-24 (UTC):

| port | `supervised` | `teardown` (first, second round) | verdict |
|---|---|---|---|
| aarch64 | pc `0x60000004`, addr `0xa50000` | 0, 0 frames | 10 of 15 passed, 5 absent |
| riscv64 | pc `0x60000004`, addr `0xa50000` | 0, 0 frames | 10 of 15 passed, 5 absent |
| `x86_64` | pc `0x60000005`, addr `0xa50000` | 0, 0 frames | 12 of 15 passed, 3 absent |

Falsification: one frame allocated and dropped after the second round turns the aarch64 verdict
red, `teardown FAILED ... the first cost 0 frame(s), the second 1`. `script/test` on all three
runs the suite's supervision tests through the shared helper; the pull request has the gate table.

## Done when

1. A spawn helper takes a supervision endpoint on aarch64, riscv64 and `x86_64`.
2. `self_test::run` has `supervised` and `teardown`, and both pass under QEMU on all three.
3. Milestone 890's BUGS line and follow-on bullet point here.

## Exit criteria, as built

| criterion | state |
|---|---|
| 1 | Done: `user::supervised_child::spawn`, one function on all three. |
| 2 | Done: the table above. |
| 3 | Done. |

## BUGS

- `teardown`, like `budget`, brackets the machine's used-frame count, so it is sound only while
  nothing else on any core allocates or frees in its window. Each earlier check waits for its own
  reaps; nothing enforces that a later-added check does.
- Neither check has run on a board yet. Both name claims that hold on every machine, so neither
  is ever absent, and the next radon or xenon boot reads them.
- The stubs are hand-assembled per port; a wrong encoding shows only as a fault at an unexpected
  pc, which `supervised` asserts.

## Follow-on

- **Recorded.** The frame-count bracket's soundness is a BUGS entry in `kernel/src/self_test.rs`.
- **Recorded.** The first board boot that reads both checks is the next radon or xenon session's,
  in this block's BUGS.
