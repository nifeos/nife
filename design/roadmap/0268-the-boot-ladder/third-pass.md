# The third pass, 2026-09-19: the top rung, gated

An appendix of [milestone 268 (every architecture boots the same way)](../0268-the-boot-ladder.md).
The main page keeps what changed, the cost, why the gate boots the PVH image, and both findings, and
is complete without this file. This one keeps the transcript, the cost table and the full reasoning.

The last outstanding item. `script/boot-check` asserts the `swish` prompt on all three
architectures, which is the rung this milestone named as the terminal state of a default boot and
the one thing its own gate could not read.

## What changed, and it is small

Each leg packs its architecture's archive before building the kernel, hands the runner
`NIFE_INITIAL_RAMDISK`, and watches to `Stage::Prompt` instead of `Stage::SelfTest`. The packer has to run
first rather than beside: it writes the measurement manifest `kernel/build.rs` reads, and a kernel
built against a stale one refuses to load the progenitor and reaches no prompt at all.

```
$ script/boot-check
boot-check (aarch64): reached shell prompt (3867 bytes in 2.9s)
boot-check (aarch64): nife machine: aarch64, 4 processor(s), 256 MiB, 100 Hz
boot-check (aarch64): nife self-test: 5 of 5 passed
boot-check (riscv64): reached shell prompt (7732 bytes in 2.8s)
boot-check (riscv64): nife machine: riscv64, 4 processor(s), 256 MiB, 100 Hz
boot-check (riscv64): nife self-test: 5 of 5 passed
boot-check (x86_64): reached shell prompt (7332 bytes in 2.7s)
boot-check (x86_64): nife machine: x86_64, 1 processor(s), 254 MiB, 100 Hz
boot-check (x86_64): nife self-test: 5 of 5 passed
boot-check: every architecture climbed the ladder to a shell prompt, and the self-test verdict on
the way was green
$ echo $?
0
```

`--inject` still comes back RED on all three, and it stops at the red verdict rather than reaching
a prompt, so its summary line says that instead of claiming a rung it did not climb.

## The cost, measured before it was chosen

The brief was to price this rather than make a pre-push gate slow by accident. Patagonia, warm
target directory, 2026-09-19:

| | verdict only | prompt |
|---|---|---|
| aarch64, boot | 2.8 s | 2.9 s |
| riscv64, boot | 2.6 s | 2.8 s |
| `x86_64`, boot | 2.9 s | 2.7 s |
| whole gate, wall | 19.4 s | 25.6 s |

The boot costs nothing and the archive costs six seconds. The prompt follows the verdict within a
few hundred milliseconds on every architecture, because the progenitor loads and measures its
programs while the emulator is already running. The whole difference is packing three archives, and
two of the three packers are a no-op against a tree `script/test` has already built.

## The PVH boot, not the UEFI image

The `x86_64` leg is the PVH `-kernel` boot, not the UEFI image. That choice is worth recording
because the number that prompted the question points the other way. Milestone 182 (x86_64's own
interactive-boot entry point)'s `script/swish-check` leg boots `BOOTX64.EFI` under OVMF and measures
about six minutes, because under firmware the console server waits for every byte to be painted on
the screen. That is milestone 400 (the shell on the firmware's screen). Six minutes on a gate a
person runs before every push is not a cost to absorb quietly. The fidelity it buys (the loader, the
firmware memory map, OVMF's BAR placement, the screen tee) is fidelity for *typing*, which this gate
does not do. The prompt arrives on COM1 either way. So the two gates boot different images on
purpose, the slow one is the one that types, and this took 2.7 seconds.

Would the PVH image still win if both cost the same? No, and saying so is the point: at equal cost
the customer's image is the better thing to boot. This is a decision made on cost, and it is
written down as one.

## Two things found on the way

- `host::cargo` sets `NIFE_DISK` and `NIFE_NET` in xtask's own environment, for every cargo
  invocation it makes, and the archive packers go through it. The aarch64 and `x86_64` runners
  treat a set-but-missing `NIFE_DISK` as fatal, and the riscv64 one ignores it. So the gate went
  red on two of three legs the first time it packed an archive, with a runner error and a zero-byte
  transcript. The leg now sets the child's environment explicitly instead of inheriting it, which
  also means an exported `NIFE_DISK` in a developer's shell cannot change what the gate boots.
- `Stage::Prompt`'s own documentation still said the rung was unreachable on `x86_64`. Fixed
  where a reader meets it, in `crates/board_console/src/progress.rs`.
