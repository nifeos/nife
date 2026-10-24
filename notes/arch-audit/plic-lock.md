# The arch audit: how the PLIC lock was closed

*Name: provisional.*

An appendix to [`notes/arch-audit.md`](../arch-audit.md). It records the three things decided while
closing finding 3, the PLIC enable-bit race, and what the tests can and cannot prove about it.

## Three things decided along the way

`enable` and `disable` now share one helper that holds an `IrqSafeMutex` across the
read-modify-write.

**It must be `IrqSafeMutex`, and that is not belt-and-braces.** `disable` is called *from the
external-interrupt handler* and `enable` from thread context with interrupts on. A plain spinlock
would let a thread take the word on hart H, take an external interrupt on H, and spin in the handler
forever on a lock only the interrupted code can release. One hart, no SMP needed, permanent. This is the
opening paragraph of §9 (locking: IrqSafeMutex, plus a discipline), arrived at in a new place.

The rank is `rank::IRQ_CONTROLLER`, one name for both controllers rather than a duplicate.
The two drivers are mutually exclusive at *compile* time (`drivers/mod.rs` gates each to its ISA).
So they are not two locks needing an order between them. They are one lock role with two
implementations, and two names at rank 20 would invite the reader to work out a precedence that does
not exist. (INBOX/MAPPINGS/KMEM share rank 59 for the opposite reason: those coexist and are declared
never-nested.)

The placement is a leaf with slack in both directions. The body is two MMIO accesses and no calls,
so nothing is taken beneath it. It is always taken holding nothing. The handler holds nothing by
§9's record-and-defer rule, and every `enable` caller has already dropped `SCHED` (`bind_irq` and
`create_endpoint` take and release it first).

The lock is scoped to the one register that needs it, verified register by register rather than
assumed. The per-source priority word is unshared. The threshold write is a whole-word store of the
constant `0`, so it is idempotent rather than an RMW, even though the affinity work made it
reachable cross-hart. Claim/complete is per-context and therefore hart-local, so it stays lock-free.
It also does not share a word with the enable bits (the 0x2000 and 0x20_0000 blocks), so
serializing one did not drag in the other. Both icount baselines are byte-identical, which is the
confirmation that nothing hot was locked.

## On proving it, honestly

There is no test here that would have caught the original bug, and the test module says so. The
window is two MMIO accesses. Widening it to catch the race would mean shipping instrumentation inside
the critical section and then testing the instrumented version. A loop of two harts hammering the
bits passes with the lock and passes without it.

What is pinned instead is the half a test can reach: that the read-modify-write preserves the
neighbors sharing its word. That is exactly the invariant a lost update violates. It is also the
regression *this change* risked by folding both directions into one helper. It fails on demand (drop
the `read` and it reports "the read-modify-write dropped it").

A second test pins the irqsave/irqrestore behavior at the two real call-site shapes, because turning
the fix into a hang is the more likely way to get this wrong later. The serialization itself is
attested by the suite it runs inside rather than by an assertion. Every riscv virtio test calls
`enable` from thread context and `disable` from the handler. A misplaced rank would panic with LOCK
ORDER VIOLATION, and a non-IRQ-safe lock would hang.
