# Interleavings: the BUGS list in full

An appendix to [notes/interleaving.md](../interleaving.md), whose BUGS section lists each entry in
one line.

- Loom models C11, not aarch64 and not riscv64. Said three times in the parent note on purpose. A
  failure it reports is real; a clean run is not a proof about the silicon. Milestone 81's HVF leg
  is the complementary evidence, and it is a sample rather than a search.
- Not in `script/test` or `script/ci-build`'s table, and not a `script/*` table entry. A loom model's search cost is exponential in threads and protocol length, so a harness
  added later can take minutes unintended. The `interleavings` job in `ci.yml` (milestone 746 (Loom
  runs in CI), provisional) runs it per pull request as a required status (calef, 2026-10-04 UTC), with a 20-minute budget tripwire.
- It cannot see the reschedule interrupt. `work_steal_slot`'s liveness claim is that a poked victim
  eventually reaches a scheduler entry and clears its slot, and until it does, every other idle core
  is locked out of that victim. That is outside the model in both directions: loom does not know
  about the SGI, and it does not know about the timer tick that makes the thief retry.
- The harnesses are small on purpose, and small is a bound. Two thieves, one victim, two polls;
  two writers, one reader, one publish each; one waker, one victim core, one thief in the
  block/wake models; two destroyers, or one destroyer against one retype, split or parent, in the
  region models. The protocols are symmetric enough that a third
  participant explores no new state *in these cases*, and that is an argument, not a proof. Every
  harness carries reachability flags (the `Reached` type) so a bound that quietly empties the
  interesting branch fails loudly, which is `kani::cover!`'s job done by hand.
- The region model checks the claim, not the free loop. What loom searches in `crates/memory_regions`
  is who wins the right to reclaim a region. That the winner then frees the *right* pages, exactly
  once, is `destroy_outcome`'s Kani proof plus the kernel's own tests, and the two arguments meet
  only in the reader's head. Covering both would need the frame allocator lifted too.
- The region model's lock is not the kernel's lock. `IrqSafeMutex` masks interrupts and carries
  a rank for the deadlock order; `loom::sync::Mutex` has neither. So the model says the protocol is
  correct *given* mutual exclusion, and says nothing about whether `IrqSafeMutex` provides it or
  whether the rank is right. That is `script/lint`'s rank check and [locking.md](../locking.md), and
  it is the same division `thread_wake_handshake` records for `SCHED`.
- The gate on `untyped.rs` is narrower than the property it protects. Milestone 136 (one decision
  path) closes the wider hole (see [lift-and-caller.md](lift-and-caller.md)), and what it buys is
  bounded. The free-site pin covers `kernel/src/memory_region.rs` only, so
  region pages freed from another module are not caught. The warrant is line order rather than
  dataflow. And nothing checks that a newly pinned public method is modeled at all, so a lane can
  widen the surface, pin it, and never write a harness. That last one is the same gap one level up,
  and it is rung four: the failure message asks in words.
- `crates/user_mode_runtime`'s spin lock and the interrupt-routing lottery are unmodelled. Both are named
  in the parent's survey with the reason: one does not compile for the host, and the other lives under
  `arch/` where rule 1 keeps it. Neither is a small retrofit.
- The `#[cfg(loom)]` code is invisible to `script/lint`, exactly as the Kani harnesses were before
  milestone 113 built a shim for them. Here it costs one flag instead of a shim:
  `script/interleaving-check` compiles the harnesses with `-D warnings`, so it lints them itself. If
  a third tool ever gets its own cfg, the shim question comes back.
