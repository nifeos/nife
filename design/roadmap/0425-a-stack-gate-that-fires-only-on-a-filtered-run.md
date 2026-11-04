---
status: NOT-STARTED
raised: 2026-09-17
promoted_from: a-stack-gate-that-fires-only-on-a-filtered-run
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 425. A stack gate that fires only on a filtered run

Promoted from the proposal `a-stack-gate-that-fires-only-on-a-filtered-run`,
filed 2026-09-17 from milestone 313's security audit while gating (finding 7), outside its lens.
*(Number provisional until the merge queue lands it.)*

It is a measurement to explain and then either a limit to move with a reason or a
chain to shorten.

**Premise re-checked 2026-09-19 and still true on the record side.** `kernel/src/stack.rs` still
asserts `used <= 61440` for the boot stack, and `notes/stack-high-water.md` still carries one `boot`
row at that limit with no x86_64 row beside it. The 9 KiB measurement itself was not re-run here.
It needs an x86_64 kernel leg, which this pass deliberately did not build.

## What was measured

On patagonia, on the base commit `52da4ae4`'s own `kernel/` and `crates/` with nothing of the
audit's applied, `cargo xtask test --arch x86_64 --test a_refusal_and_a_success` (one test,
`language_tests::a_refusal_and_a_success_report_different_numbers`) ends with

```
stack high-water: boot   62456/65504 bytes (95%), paint floor 4864
[PANIC] panicked at kernel/src/stack.rs:621:5:
boot stack high-water 62456 exceeded 61440
```

four times out of four, byte-identical. The **full** `x86_64` suite on the same tree, both boot
modes, ends green with the boot stack at 53144 (PVH, 243 passed) and 50272 (UEFI, 214
passed). So the same test drives the boot stack about 9 KiB deeper when it is the only test selected
than when it runs inside the suite. The gate that would catch real growth trips on the filtered
run and not on the one CI runs.

## Why it matters

Two reasons, and the second is the one that touches something this tree relies on.

A filtered run is how a person reproduces one failure, and it is how milestone 210's `--test
<substring>` was built to be used. A gate that fires only there teaches whoever hits it that the gate
is noise, which is the wrong lesson about a stack gate.

And `script/falsifications --sweep` replays every kernel record as a filtered single-test run. A
record whose test happens to sit on a deep chain would report error ("the kernel never reached
its test runner" is not the shape; it would be a red at the `used <= 61440` assertion in `report_high_water` in `kernel/src/stack.rs` rather than at the assertion
the patch names). And the sweep's whole discipline is that a red for the wrong reason is not
evidence. No record today names that test, so nothing is wrong yet.

## What would close it

Find what a filtered run does 9 KiB deeper than the suite does on the same test. The likely shape is
something that runs once per boot before the first test and is on the boot stack when the first
selected test begins. So in a full run it has been and gone by the time this test is reached, and
in a filtered run it is still there. Then either an `x86_64` row in `notes/stack-high-water.md` with
a limit that has a reason, or a shorter chain.

## Index row

On one commit's own tree, `cargo xtask test --arch x86_64 --test a_refusal_and_a_success` panics
four times out of four at the `report_high_water` assertion in `kernel/src/stack.rs` with the boot stack at 62456 of 65504 bytes. The full
x86_64 suite on the same tree ends green at 53144 and 50272 on the two boot modes. The same test
drives the boot stack about 9 KiB deeper when it is the only test selected than when it runs inside
the suite. So the gate that would catch real growth trips on the filtered run and not on the one CI
runs. That matters twice. A filtered run is how a person reproduces one failure and how milestone
210's `--test <substring>` is meant to be used, and a gate that fires only there teaches the wrong
lesson about a stack gate. And `script/falsifications --sweep` replays every kernel record as a
filtered single-test run, where a red at the `report_high_water` assertion in `kernel/src/stack.rs` rather than at the named assertion is a red
for the wrong reason and therefore not evidence. No record names that test today, so nothing is
wrong yet. What closes it is finding what a filtered run does 9 KiB deeper, and then either an
x86_64 row with a limit that has a reason or a shorter chain.
