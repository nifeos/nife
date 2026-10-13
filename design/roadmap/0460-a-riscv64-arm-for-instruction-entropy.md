---
status: REFUSED
raised: 2026-09-20
refused_by: 162, 448
---
# 460. A riscv64 arm for the CPU-instruction entropy source

Refused by milestone 162 (design/roadmap/0162-cpu-instruction-entropy.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '162. Real hardware entropy on x86_64 and aarch64: RDSEED and RNDRRS', under `## Follow-on`:

> A riscv64 arm. Neither `RDSEED` nor `RNDR`/`RNDRRS` exists on that ISA, so there is no
> instruction to wrap; milestone 159's JH7110 TRNG is the real hardware source there, through its
> own driver, and pretending otherwise would be a parity claim with nothing behind it.
>
> -- design/roadmap/0162-cpu-instruction-entropy.md

## Why it is here rather than only there

The instruction-entropy source wraps `RDSEED` on x86_64 and `RNDR`/`RNDRRS` on aarch64. The refusal
is that riscv64 has no instruction to wrap, so a riscv64 arm would be a parity claim with nothing
behind it, and the real hardware source on that architecture is milestone 159 (a real hardware entropy source: the JH7110's TRNG), through its own driver.

## Revisit

- **Condition.** A riscv64 target this tree runs gaining an architectural entropy instruction, and a
  toolchain that will emit it. The refusal rests on the absence of an instruction rather than on the
  idea being wrong, so it is a fact about an ISA at a date and facts about ISAs change.

## Index row

This is a parity gap that is not a parity defect: two architectures have an entropy instruction and
one does not, so the third gets a driver instead. The refusal is a statement about what the ISA
offers, which makes it the shape of refusal most likely to erode without anyone noticing.
