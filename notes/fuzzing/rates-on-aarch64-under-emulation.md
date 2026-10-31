# Fuzzing: the in-guest drivers and a host target under emulation, measured

*Name: provisional.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md). It keeps the first rates measured for the
confinement fuzzers, taken on 2026-10-31 (UTC) before the x86_64 runs under KVM.

What was measured. One QEMU guest at a time, on a shared many-core x86_64 build host with other
lanes' load on it, under full emulation (TCG) of aarch64. Not a census.

| What | Inputs | Wall time | Rate | Result |
|---|---|---|---|---|
| User-mode driver, seeds `900000000..900000277` | 278 seeds | 60,047 ms | 4 seeds/s per guest | clean |
| Kernel-thread driver, seeds from `900000000` | 542 seeds, 24,066 calls | 60,043 ms | 9 seeds/s per guest | clean |
| User-mode driver, seed `202642000408` (the 2026-10-12 sweep red) | 1 seed | 444 ms | | green |
| `redoxfs_server_session`, empty corpus | 103,159 sessions | 180 s | 569 runs/s on one process | no crash |

The hosted runner's 2026-10-26 sweeps ran the same two drivers at 13 and 40 seeds/s, three to four
times faster than this host. The measurement did not establish why. A cold build took 8 min 9 s for
60 s of fuzzing.

The replayed seed settled that the 2026-10-12 red was the referee's, not the kernel's: the later fix
for the missing `SLICE` arm covered it.

The host target's coverage, in libFuzzer edges: 772 at start, 2,024 after 8,192 sessions, 2,453
after 32,768, 2,660 after 65,536 and 2,785 at the end. It gained about 5% per doubling of inputs
near the end, so three minutes reach no plateau.

For the same drivers under KVM, see the neighbouring note on rates on x86_64.
