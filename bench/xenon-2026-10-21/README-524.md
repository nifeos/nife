# xenon, 2026-10-21 (UTC): milestone 524's invariant-TSC bit on silicon

The boot that milestone 524 (the three x86_64 boot gates: NX, SYSCALL, and the invariant TSC)
waited for: a real x86_64 part reporting `CPUID.80000007H:EDX[8]`. `main` at `5ee9b4afb`, built
with `cargo xtask uefi-image`, booted from a stick at about 23:03 UTC. xenon is an Intel Core
i5-7500T.

| file | what it shows |
|---|---|
| `boot-to-prompt.log` | the whole boot; milestone 890 (riscv64 and x86_64 join the shared boot path)'s README describes the capture |

The lines that answer 524, printed by `isa::init` on the boot core and again by the tour:

```
  cpu         : x86_64, vendor GenuineIntel, cpuid leaves 0..0x16 and 0x80000000..0x80000008
  features    : nx syscall invariant-tsc rdseed
```

No `unpromised` line is printed: the `Gate::Warn` row found its bit set. The TSC's measured rate
on the same boot was `tsc 2713 MHz (PIT calibration, best of 2 windows, worst 2713 MHz)`.
