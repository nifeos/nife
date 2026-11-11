# Prior art for the counter_frequency_contract crate

An appendix to [the milestone](../1073-counter-frequency-contract-outside-review.md).

How other systems hand a process the rate of its free-running counter. The details marked
unverified are from memory of those sources, not from reading them during this review.

| system | mechanism | rate or scale | update discipline | discovery of the page |
|---|---|---|---|---|
| Arm, aarch64 | `CNTFRQ_EL0`, an architected register firmware writes | hertz, 32 bits | none needed; constant by architecture | a register, no page |
| RISC-V | `/cpus/timebase-frequency` in the device tree; no CSR states it | hertz | the kernel reads it once | no architected way to reach a process; each kernel invents one |
| Linux vDSO | the `vvar` data page beside the vDSO | a multiplier and shift per clock source, not hertz (unverified in detail) | a sequence counter; readers retry across an update | `AT_SYSINFO_EHDR` in the auxiliary vector, not a fixed address |
| FreeBSD | the shared page's `vdso_timekeep` and `vdso_timehands` | a scale factor; a version field (`tk_ver`) and a generation per hand (unverified in detail) | a generation counter, seqlock style | the shared page's address through the auxiliary vector |
| OpenBSD | the `timekeep` page, read-only in every process | a version constant the kernel and libc compare (unverified in detail) | a generation counter | ELF auxiliary vector |
| Fuchsia | a vDSO call returning ticks per second (unverified) | hertz | constant for the boot | the vDSO |
| Rust crates (`quanta`, `fugit`, `x86`, `riscv`, `aarch64-cpu`) | `quanta` calibrates the counter in user space itself; `fugit` carries the rate as a const type parameter; the register crates read the counter and expose no rate on the two architectures without one | hertz, or a compile-time constant | none | not applicable |

This crate differs from those designs on purpose, in four ways.

- It carries the rate itself in hertz, not a scale pair, so every reader divides the same way.
- It has one writer before any reader exists, so it needs no sequence counter and a torn read is
  impossible by construction.
- The page sits at a fixed virtual address every program compiles in. There is no auxiliary vector
  to parse, and a process with no page faults rather than reading a wrong number.
- An unknown rate is a zeroed frame that reads as `None`, where the vDSO designs always have a
  number.

It costs two things. The contract cannot express a rate that changes after boot (every design above
shares that for an invariant counter). The fixed address becomes part of the contract's identity the
moment a published binary compiles it in.

The concrete example the band exists for: a firmware that leaves `CNTFRQ_EL0` at its reset value
reports 0 or 1 hertz. A reader checking `> 0` believes 1 hertz and reports every second as a
billion. `is_plausible(1)` refuses it at the point it is learned, and the three kernel boot paths
assert through that one function.
