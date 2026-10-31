# Fuzzing: the in-guest drivers' rates on aarch64 and riscv64 under emulation, measured

*Name: provisional.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md). It records rates, so that a budget in seeds
turns into hours on the two architectures that run only under full emulation (TCG). Every date is
2026-10-31 (UTC). The same drivers on x86_64 under KVM are in the neighboring note on rates on
x86_64.

## What ran

The kernel-thread driver, milestone 752 (a seeded syscall driver with a shadow model), and the
user-mode confinement driver, milestone 779 (fuzz the surface a confined process can reach), each
ran a 60-second sweep per guest on aarch64 and riscv64. The ladder was 1, 2, 4, 8 and 16 guests at
once, one run per level. The host was a shared multi-core build host with other lanes' load on it.
Each guest ran from its own copy of one prebuilt kernel image, with an initial ramdisk, no disk and
no network. Guest memory was 256 MiB (the aarch64 kernel asserts it) and each guest had 2 vCPUs.
The runs used QEMU 11.1.2 with its default threading. The rates are the guest's own count of seeds
over its own clock.

## Rates, per guest

Mean seeds per second per guest, with the range in brackets where the level had several guests.

| driver | architecture | 1 guest | 2 | 4 | 8 | 16 |
|---|---|---|---|---|---|---|
| kernel-thread | aarch64 | 19.9 | 19.7 | 15.5 | 17.6 | 17.0 |
| kernel-thread | riscv64 | 10.8 | 10.9 | 10.1 | 11.1 | 9.1 |
| user-mode | aarch64 | 9.4 | 9.3 | 8.9 | 8.8 | 8.6 |
| user-mode | riscv64 | 7.3 | 6.7 | 7.4 | 5.7 | 6.3 |

x86_64 under TCG, 1 guest: kernel-thread 17.6 and 15.4, user-mode 9.9 and 8.8. That is a sanity
check only: a first run on a quiet host gave 34 seeds/s for the kernel-thread driver, and the
measurement did not explain the gap.

The ladder reads flat within noise. Repeat runs of the same image differ by up to about 15%. The
worst per-guest rate was 78 to 86% of the single-guest rate, at moments of high host load, and
the loss at 16 guests is about 5 to 15%, which is uncertain.

## False reds

None. Across 62 guest-runs, 31 of each driver on aarch64 and riscv64, no guest panicked and every
guest printed its result line. A TCG guest is slow enough that the 2-second per-call deadline of the
user-mode driver stays far away. The replay path was not exercised on TCG because nothing failed.

## Memory and cores per guest

| driver | peak resident memory per guest | host cores per guest |
|---|---|---|
| kernel-thread | about 126 MB | 1.15 to 1.25 |
| user-mode | 264 to 375 MB | 1.20 to 1.27 |

Sixteen guests of the heavier driver need about 6 GB.

## Determinism

The kernel-thread driver gave byte-identical call censuses over 300 seeds in three runs per
architecture. The user-mode driver prints no census, so its determinism rests on 100-seed passes
and on the design, where the seed is the reproducer.

## Not measured

More than 16 guests, a smaller virtual machine, false reds under real overload, a replay of a
failing seed, and whether QEMU ran multi-thread TCG.

## An earlier figure that this note replaces

An earlier measurement the same day gave 4 seeds/s for the user-mode driver and 9 for the
kernel-thread driver, per guest, on aarch64 under TCG. It used a different, slower setup. This note
does not reconstruct that setup and does not establish why it was slower. Use the table above.

The same earlier session ran the host target `redoxfs_server_session` for 180 seconds on one
process from an empty corpus: 103,159 sessions, 569 runs/s, no crash. Its coverage in libFuzzer
edges was 772 at start, 2,024 after 8,192 sessions, 2,453 after 32,768, 2,660 after 65,536 and
2,785 at the end. It gained about 5% per doubling of inputs near the end, so three minutes reach no
plateau. The measurement above did not repeat it.
