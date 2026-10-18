# The DMA-confinement validator: harnesses, bounds and non-vacuity

An appendix to [notes/verification.md](../verification.md). It holds milestone 35 (prove the
DMA-confinement boundary): the seven harnesses on the validator, the bounds each one uses and why
each is adequate, and `kani::cover!`. The IOMMU half, and what the proof does not establish, is in
[iommu-domain.md](iommu-domain.md).

## What the validator is

Seven harnesses live in `crates/direct_memory_access_validator/src/lib.rs`. This was the last
isolation boundary in the system that was attacker-tested but never proved. It confines a userspace
virtio driver's DMA. On every `NOTIFY` the kernel walks the driver's descriptors. It refuses any
whose buffer escapes the driver's granted region, or is indirect. It copies the validated ones into a
kernel-private shadow ring the device reads, so the driver cannot touch what the device acts on.

The logic was lifted out of `kernel/src/virtio.rs::validate_and_shadow`, which now calls it, so it
could be proved. That is the same Phase-2 move `memory_regions` and `inter_process_communication`
made. The kernel's QEMU attacker suite is unchanged and green: the DMA-escape and indirect-escape
end-to-end tests, on both ISAs. So the extraction is faithful.

| Harness | Property |
|---|---|
| `in_region_is_sound` | the confinement predicate is sound and total: for every base/size/addr/len, if `is_in_region` accepts then `base <= addr` and `addr + len <= base + size` with no overflow (direction-agnostic, so it underwrites both TX device-reads and RX device-writes) |
| `an_accepted_descriptor_is_confined` | for every descriptor bit pattern (flags fully symbolic, so the device-writable RX bit is covered) and every region, an accepted descriptor is not indirect and its whole buffer is in-region |
| `validate_and_shadow_confines_every_chain` | **the main theorem**: over a fully symbolic driver descriptor table and region, no descriptor the walk copies into the shadow is ever out-of-region or indirect, so the device only ever reads confined descriptors. Symbolic-index-bounded, so it also proves the walk never reads or writes past a ring |
| `an_oversized_batch_is_refused` | a batch claiming more than `qsize` new entries is refused before a single descriptor is read or written (the DoS bound on the outer loop; the memory closures panic if called) |
| `a_descriptor_mutated_after_validation_cannot_reach_the_device` | the shadow ring closes the time-of-check/time-of-use race: after a validated copy, the driver aiming its own descriptor at any address cannot change what the device reads from the shadow |
| `the_outer_walk_stays_inside_the_rings_and_terminates` | the loop that feeds the chain walk: for **every** `(from_idx, to_idx)` pair, wraparound included, every ring access lands inside its own ring and the loop terminates |
| `distinct_queues_occupy_disjoint_blocks` | multi-queue isolation (milestone 30): for any two distinct in-range queues, one queue's whole ring area ends before the other's block begins, and a queue's descriptor table ends before its own available ring begins |

## The main theorem is per head, and that is a decomposition

The main theorem proves the confinement core, `shadow_one_head`. The write closure instruments it, so
it asserts "in-region and not indirect" the instant each descriptor lands in the shadow. It is proved
for one newly-published head, not the whole ring, and that is a decomposition, not a sample. The
invariant is checked on *every* shadow write. One head's chain already writes up to `qsize` fully
symbolic descriptors. So the per-write property is quantified over arbitrary descriptor content and
position. The outer loop only repeats that validated processing for each further head, reaching no
new descriptor state; its bound is the separate `an_oversized_batch_is_refused`. Batching the whole
ring pushed the SAT formula to `qsize * qsize` symbolic reads. That was out of a practical
`script/verify` budget (three minutes) for no added coverage. The single-head form verifies in ~20s.

## The bounds, and why each one is adequate

Bounded model checking means somebody chose the bounds. A proof whose bounds hide the interesting
case reads as stronger than a test while being worth less. So here is each bound in the DMA
harnesses, stated with its justification:

| Bound | Value | Why it is adequate |
|---|---|---|
| queue size (`QS`) | 8 | **It is the system's own bound, not a proof convenience.** `direct_memory_access_validator::LAYOUT_QSIZE` is the kernel's `QSIZE`, `setup_queue` refuses `num > QSIZE`, and the kernel now *aliases* the crate's constant rather than keeping a copy. So the proof is over the shipping configuration, and no larger ring can exist to be unproved. |
| chain length | ≤ `qsize` = 8 | The walk is `for _ in 0..qsize`, and a chain cannot usefully be longer: there are only 8 descriptors, so any longer walk is revisiting one. A **cycle** is therefore covered rather than excluded: `next` is fully symbolic, so `0 → 1 → 0 → …` is among the proved inputs, and the loop bound is what makes it terminate instead of hanging. |
| loop unrolling | `unwind(10)` / `unwind(11)` | One more than each loop can need, so Kani's *unwinding assertion* is part of the proof: if any input could drive a loop longer, verification fails. That turns the bound from an assumption into the **termination proof**. Checked by falsification: delete `validate_and_shadow`'s batch-size guard and the unwinding assertion fails at iteration 11. |
| batch size | ≤ `qsize` | Proved as a property (`an_oversized_batch_is_refused`), not assumed: a claim of more than `qsize` new entries is refused before a single read. |
| queue count | `MAX_QUEUES` = 2 | Compile-time asserted in the kernel (`MAX_QUEUES * RING_BLOCK <= FRAME_SIZE`, one shadow frame per device) and enforced at runtime (`setup_queue`/`notify` refuse `queue >= MAX_QUEUES`). |
| region base/size, descriptor `addr`/`len`/`flags`/`next`, ring indices | **unbounded** | Fully symbolic `u64`/`u16`. Every attacker-controlled value is unconstrained, which is the point: the bounds above are all structural (how many slots a ring has), never a restriction on what an attacker may write into one. |

The one place the composition is an argument rather than a single harness, said plainly: "the whole
batch is confined" follows from four legs taken together. They are the per-head theorem, the
per-write invariant, the outer-loop bound, and the ring-bounds harness. Each leg is proved. Joining
them is a reading of four harnesses, not a fifth harness. That is the same shape as the one-shot
Reply's three legs ([core-harnesses.md](core-harnesses.md)), and it is recorded for the same reason.

## Non-vacuity: `kani::cover!`, and why a bound needs it

A bound raises a question an assertion cannot answer. Suppose the harness's assumptions turned out to
be jointly unsatisfiable, or a bound quietly excluded the interesting shape. Every assertion would
pass, and the harness would prove nothing while reporting `SUCCESSFUL`. The parent note has always
named that as the main failure mode ("it proves what you asserted, not what you meant"). Until
milestone 35 the only defense was reading the harness carefully.

`kani::cover!(condition)` inverts the question: it fails when the condition is unreachable. So it
turns "this harness really does exercise the case I claim" from a reading into a result. Milestone 35
introduced it, with four cover properties where the risk was real:

- `the_outer_walk_stays_inside_the_rings_and_terminates` covers four things. A wrapped batch
  (`to < from`) is reachable. A wrapped batch is *walked to completion* rather than merely refused.
  Some batch is accepted, so the harness is not vacuously refusing everything. And some batch is
  refused, so the guards are reachable. Without those, "wraparound is covered because `from` and `to`
  are unconstrained" would be an inference about the code rather than a checked fact.
- The two domain harnesses that stack assumptions cover that a multi-page grant satisfies them. So
  neither is quantifying over an empty set, and neither has `i` pinned to zero.

The cheap general rule this suggests: any harness with more than a couple of `kani::assume`s, or any
harness whose interesting case is a corner of an unconstrained input, should carry a `cover!` for
that case. It costs no solver time worth measuring, and it is the only thing that catches a vacuous
proof.
