# The confinement test in detail

*Name: provisional. After the parent's own heading.*

The test [`notes/c-seam.md`](../c-seam.md) calls the milestone: the address-space layout and its two witnesses, the five
assertions and the control, and what the machine printed on both ISAs.

## The layout, and why there are two witnesses

```text
  c_shim (the C component's process)        c_confiner (the checker)
  ----------------------------------       ----------------------------------------
  0x0040_0000  text / rodata / data        0x0040_0000  text / rodata / data
  0x0050_0000  stack                       0x0050_0000  stack
  0x4000_0000  heap (malloc/free)          0x2000_0000  the initrd, read-only
  0x5000_0000  GRANT       1 page, RW <--> 0x5000_0000  the same frame, RW
  0x5000_1000  WITNESS_RO  1 page, RO <--> 0x5000_1000  the same frame, RW
  0x5000_2000  NOTHING (unmapped)          0x5000_2000  a DIFFERENT frame, RW
```

Two witnesses because there are two different claims, and neither implies the other:

- `WITNESS_RO` is the same physical frame, mapped read-only into the component and read/write
  into the confiner. `c_seam_overrun` writes `grant[len]`, which is this page's first byte. When it
  comes back unchanged, that is not "the store landed somewhere else": the page was right there, in
  the offender's own page tables, one byte past a pointer it legitimately held, and **the store did
  not happen.** This is the stronger of the two.
- `WITNESS_FAR` is a different frame at the same virtual address. `c_seam_wild` writes
  `grant[len + 4096]`, an address the component has no mapping for at all and the confiner does.
  When the confiner's page is unchanged, that is the statement **a virtual address means nothing
  outside the address space that owns it**, which is the MMU claim itself, made concrete rather than
  assumed.

Both patterns are position-derived (`i*31+7` and `i*17+3`) and checked byte by byte, which is milestone
29's framebuffer discipline: a partial overwrite is detected, and a `memset` of any single value could
not pass.

## The five assertions, and the control that makes them mean anything

Every one is made from outside the faulting address space, after the component is dead.

1. It faults, rather than silently corrupting and continuing. The death message exists at all,
   carries `EVENT_FAULT`, and carries a non-zero kernel-stamped tid (DECISIONS §26.5: the kernel is the
   only sender on that endpoint, so the tid needs no badge).
2. The fault is the bug we planted. The address the kernel reports equals the address the C code
   computed. Without this the witness checks would be vacuous, because a crash on the way to the bug
   looks identical from the outside.
3. `WITNESS_RO` is intact, every byte.
4. `WITNESS_FAR` is intact, every byte.
5. The restart works, and works means computes. Three instances run in sequence: attempt 0
   overruns, attempt 1 goes wild, attempt 2 runs `c_seam_transform` (uppercase the input, FNV-1a it,
   write both into the grant) and exits cleanly. The confiner reads the output out of the shared
   grant and checks it against an independent Rust implementation of the same definition, so a
   restart that revives a process which merely reports for duty fails. The clean exit arrives as
   `EVENT_EXIT` and is not restarted, which is the other half of §26.3's "both events flow".

The control. Each misbehaving C function stores *inside* its grant first (`grant[0] = 0xC0`),
and that store must be visible in the confiner's view. Without it, every witness assertion could be
satisfied by a process whose stores never worked at all, which would prove nothing. This is the
single most important line in the test.

The verdict is asserted for equality with the expected bitmap, not for containing the interesting
bits: a missing bit is what broken confinement looks like, and a superset would mean the checker started
answering a question nobody asked.

## What the machine actually printed

aarch64:

```
  user thread ... killed: Data abort from a lower EL
    pc 0x402178   far 0x50001000   esr 0x9200004f     <- overrun: permission fault on WITNESS_RO
  user thread ... killed: Data abort from a lower EL
    pc 0x402190   far 0x50002000   esr 0x92000047     <- wild:    translation fault, nothing mapped
```

riscv64:

```
  user thread ... killed: scause 0xf (code 15)
    pc 0x401786   stval 0x50001000                    <- overrun
  user thread ... killed: scause 0xf (code 15)
    pc 0x40179a   stval 0x50002000                    <- wild
```

A cross-ISA difference, found here and worth recording. aarch64's `ESR_EL1` distinguishes the two
bugs in its fault status code (`0x...4f` permission, `0x...47` translation); RISC-V's `scause` reports
both as `0xf`, Store/AMO page fault, with no permission-versus-translation distinction at all. Both
deliver the exact faulting byte address, which is what the test asserts on, so the difference costs
nothing today. It would matter to a userspace pager, which needs to tell "this page is absent, fetch
it" from "this page is protected, that is an error"; on RISC-V that requires reading the page tables
rather than the cause register. That is on the SUSPEND tracker's plate, not this milestone's.

