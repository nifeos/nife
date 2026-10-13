# The CPU-model matrix: the canary flake on `thead-c906`

An appendix to [`notes/cpu-models.md`](../cpu-models.md)'s BUGS, under the cap of §212 (a prose
budget). The flake is diagnosed and fixed; the entry is kept for its reasoning.

## The flake, and why the model was innocent

`the_canary_reports_a_byte_that_changed_behind_its_back` flaked on `thead-c906`, and the model was
innocent. The flake was a race in the canary's own single-flight protocol. It was observed
2026-08-15 in four runs of one tree: two failures with the flipped byte uncounted, and two passes.
It was diagnosed and fixed the same day.

The canary's `check()` was single-flight behind an `IN_CHECK` flag and returned silently when it
lost the compare-exchange. Timer ticks on other harts call `check()` too (secondaries are online in
the suite). So the test's decisive check could lose the slot to a tick's pass that had read the
scratch byte *before* the flip. The test's call then checked nothing, and the flip went uncounted.

Nothing in that is c906-specific beyond timing. This model, the matrix's slowest, is merely where
the window landed twice. The suite's logs show the benign half of the same interleaving routinely,
on every model and both ISAs: a tick's pass absorbing the flip (the `0xa5 -> 0x5a` canary line)
before the test's own check runs.

## The fix

The fix is in crates/memory_corruption_canary_gate, and it was loom-searched. The serialization is
one state word with exclusive guards. `check()` reports whether a pass actually ran, the tick
shrugs at a refusal, and the test loops until a pass of its own completes.

The rework also closed a second hole the old spelling permitted and loom falsifies. An `arm()`
could rewrite the watch plan under a checker that had seen `ARMED` but not yet won `IN_CHECK`, a
torn-plan wild read. No observed failure required that fix, but a corruption instrument must not
contain the hole.

## Two CI failures that were not this bug

Two CI failures were briefly attributed to this bug and are NOT it. They are recorded here because
they were sighted through this entry:

- an aarch64 kernel-suite death (PR #204's branch, run 31907966383 attempt 1), read as "same-EL
  data abort just after the canary line";
- a c906 matrix death (the branch of milestone 54 (a network file service a Mac can actually
  mount), run 31910308865 attempt 1) at the riscv trap reporter.

The logs acquit the canary. Both are the thread-stack overflow in the parent's next `BUGS` entry,
right after the user-fault kill report (slot 87 on aarch64, slot 102 on c906). On aarch64 the
canary had disarmed 21 seconds earlier.
