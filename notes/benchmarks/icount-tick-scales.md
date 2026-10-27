# icount tick scales: a tick is not an instruction

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the per-architecture tick scales under
`-icount`, the in-kernel ground-truth measurement that fixed them, and the correction of the x86_64
"17x". Name: provisional (the icount-attribution lane, 2026-09-30); [the naming record](README.md)
holds the ratified ones.*

## 2026-09-30: the x86_64 "17x" was units; the kernel paths are within 7 percent

The question that opened this, calef's on 2026-09-30: swish-check's x86_64 first boot spends 15 s per
transcript line where aarch64 spends 0.2 s, and CI's 38-minute job is 84 percent that leg. The
baselines appear to show x86_64 executing ~17x the guest instructions for the same work
(`yield_switch` 1,124,419 against 19,300,949; `ipc_rtt` 1,034,642 against 17,254,582). The lane's
brief was to attribute the 17x to paths. The premise fails first, and measurably: the harnesses are
the same work, but the ticks are not the same unit.

The premise half that holds: `kernel/src/bench.rs` is one file compiled three ways, and `yield_switch`
and `ipc_rtt` run the same source, the same iteration counts, the same scheduler on every leg. Nothing
in the harness inflates x86_64.

### The unit: one instruction is one nanosecond; one tick is not one instruction

Under `-icount shift=0` QEMU advances virtual time exactly one nanosecond per retired guest
instruction and by nothing else ([`xtask/src/icount.rs`](../../xtask/src/icount.rs), and
[the x86 instruments appendix](x86-instruments.md) for the x86_64 leg's byte-identical boots). Each
leg's counter reads that clock **at the counter's own frequency**:

| leg | counter | rate on this machine | ticks per guest instruction | guest instructions per tick |
|---|---|---|---|---|
| aarch64 | `CNTVCT_EL0` | 62,500,000 (`bench: cntfrq`) | 0.0625 | 16 |
| x86_64 | `rdtsc` | 999,935,600 (PIT-calibrated; [the x86 instruments appendix](x86-instruments.md)) | 1.0000 | 1 |
| riscv64 | `rdtime` | 10,000,000 (`bench: cntfrq`) | 0.0100 | 100 |

So a baseline row's "ticks" are `counter_frequency / 1e9` of an instruction, and a raw tick ratio
between two legs mixes that scale factor into the comparison. The aarch64/x86_64 raw ratio on
`yield_switch` is 17.17; the scale factor between the legs is 16.0; the real instruction ratio is
1.07.

### The ground truth, measured inside the kernel

A temporary probe (the patch is described at the end, and reverted after these boots) timed a loop of
exactly 400,001 guest instructions with the same `arch::timer::now()` every bench row reads:

| leg | ticks for 400,001 instructions | ticks per instruction |
|---|---|---|
| aarch64 | 25,001 | 0.0625 |
| x86_64 | 400,020 | 1.0000 |
| riscv64 | 4,001 | 0.0100 |

The x86_64 surplus of 19 is the handful of instructions between the loop's last jump and the second
`rdtsc`. The three scales match each leg's printed frequency exactly, which is the check: the
counter, not the instruction stream, is what differs between legs.

### The corrected cross-arch table

Baseline ticks per iteration, converted to guest instructions per iteration by each leg's scale:

| row | aarch64 (x16) | x86_64 (x1) | riscv64 (x100) | x86_64 / aarch64 |
|---|---|---|---|---|
| `yield_switch` | 8,995 | 9,650 | 9,345 | 1.07 |
| `ipc_rtt` | 16,554 | 17,255 | 17,044 | 1.04 |
| `relay_rtt` | 32,896 | 34,439 | 34,242 | 1.05 |
| `call_reply` | 16,926 | 17,789 | 17,709 | 1.05 |
| `broker_rtt` | 33,804 | 35,530 | 35,372 | 1.05 |
| `spawn_reap` | 54,203 | 44,709 | 53,890 | 0.83 |
| `map_new` | 3,841 | 2,758 | 3,701 | 0.72 |
| `coremark` | 1,307,131 | 1,196,353 | 1,427,525 | 0.92 |

Every shared row is between 0.72x and 1.07x. Three ISAs running the same scheduler and IPC Rust agree
within a few percent, which is what sharing the source should produce. **There is no 17x anywhere in
the kernel paths.** Rows from this lane's own boots (same evening, one tree) give the same ratios:
`yield_switch` 9,296 against 9,930, ratio 1.068.

The residual, attributed as far as measurement goes. x86_64 runs 4 to 7 percent more instructions on
the switch and IPC rows. One measured component is the port-grant install: the x86_64 switch installs
the incoming thread's port grant on every switch-in (`install_port_grant`, `kernel/src/sched.rs`;
§152 (the port-range capability: object and method semantics on the syscall surface) is the
mechanism). `tss_iomap_lazy_nop` prices that call at ~130 debug-build instructions. Two
switch-ins per `yield_switch` iteration is ~260 instructions of the 655 gap; the port-grant field
read under the lock and the arch's own switch code account for the rest at this build's debug cost.
On `spawn_reap`, `map_new` and `coremark` x86_64 executes fewer instructions than aarch64; `coremark`
is soft-float on both legs (the aarch64 userspace target is `-neon` soft-float too,
`targets/aarch64-unknown-nife.json`), so no emulation gap exists between them.

### Where the wall clock went

The same bench boots, timestamped per line on the host (machine shared with another lane; ratios
informative, magnitudes approximate):

| | aarch64 | x86_64 |
|---|---|---|
| whole suite, rows only | 1.77 s | 0.65 s |
| `coremark` row | 0.229 s | 0.252 s |
| `yield_switch` row | 0.027 s | 0.028 s |

Per-instruction host cost: 1.0x on scheduler code (689 against 709 MIPS per leg), 1.2x on pure
compute (1.46 against 1.22 GIPS). The "2-3.5x TCG amplifier" behind the lane's framing is not
visible on kernel code, and dividing the swish wall ratio by the spurious 17x is the likeliest way it
was derived.

The expensive kind of code is stores. `tss_iomap_switch` and `tss_iomap_lazy_switch` retire nearly
the same instruction count (24.62M against 24.36M) and the bitmap row takes 3.8x the host time (0.122
s against 0.032 s): the 8,192-byte write per switch-in is 31 MiB of stores across the row. Store-heavy
code through this TCG runs ~3.8x slower per instruction than arithmetic.

The swish-check x86_64 leg is store-heavy by construction and is already attributed
(`xtask/src/swish_check/verdict.rs`, the `SWISH_CHECK_X86_LINE_SECONDS` table). The console server blocks on
every screen paint. `display_terminal` repaints the whole 924x344 surface per scroll, and
`framebuffer_driver` copies into an uncacheable aperture one word at a time, in debug builds, every
store through TCG. Measured there: 321.1 s for the leg against 6.9 s (aarch64) and 7.3 s (riscv64),
slowest lines 24.7 s and 16.7 s, CI 1.5x to 1.8x slower than patagonia. The 15 s per line and the 84
percent of CI are that path and the OVMF boot, not scheduler instructions.

### The two adjacent facts, and what they do not explain

- The x86_64 kernel and the initrd userspace build for one target, `x86_64-unknown-none`, whose spec
  is `-mmx,-sse,+soft-float`; `xtask/src/archive.rs` records it (it is why `aes` cannot legalize, and
  why `aes_force_soft` exists in `.cargo/config.toml`). The std-farm target `targets/x86_64-unknown-nife.json` carries the
  full `-mmx,-sse,-sse2,-sse3,-ssse3,-sse4.1,-sse4.2,-avx,-avx2,+soft-float`. Nothing on the leg has
  a vector unit.
- `switch_to` saves six callee-saved registers and the return address, 56 bytes, no FP state
  (`kernel/src/arch/x86_64/context.rs`). The FP half of a switch is `fp::hand_over`: two loads and a
  branch when neither thread is FP-live, the register-file machinery `#[cold]`
  (`kernel/src/fp.rs`), reached today only by that module's own tests because everything is
  soft-float.

Connection to the hot segments: none of the rows above is FP, and the kernel's copies are
word-wide. The two 8,192-byte TSS writes per iteration are ~2,120 instructions together
(`tss_iomap_switch` less `tss_iomap_lazy_nop`), eight bytes per store, not a byte-wise
soft path, so soft-float inflates no number in the table. The XSAVE-aware-switch-plus-SSE candidate
is not what the "17x" accused the kernel of. It is a lever for the swish screen copy (SSE stores
would cut that copy's instruction and store count ~4x) and for userspace SIMD, and it lives on the
console path, priced there (`design/roadmap/0400-the-shell-on-the-firmware-screen.md` owns that
design).

### What the numbers imply, per segment

- Scheduler and IPC: nothing to fix on x86_64 for instruction count. The port-grant hook is the one
  measured x86_64 surcharge and is §152's (the port-range capability) price for ports; the rest is
  debug-build noise that release already strips.
- CI's 84 percent: the levers are the console path (damage instead of whole-surface repaint, a
  cheaper scroll copy) and release builds for the leg's heavy programs. Kernel-side switch or IPC
  work cannot move that leg by more than its instruction share, which this table puts near zero.
- Cross-arch reading of baselines: scale before comparing. `instructions = ticks * 1e9 /
  counter_frequency`, per leg, or read the probe method below.

### Corrections this forces in existing records

- [Counter frequency and calibration](counter-frequency-and-calibration.md), 2026-09-21, first
  section, said `bench/baseline-riscv64.txt` "holds guest instruction counts, which no counter
  frequency enters". Wrong: the file holds ticks at the counter's rate, one tick per 100 guest
  instructions on that leg. Corrected in place, with this appendix as the record.
- [`design/fatal-risks/the-crossing-cost.md`](../../design/fatal-risks/the-crossing-cost.md) called
  its aarch64 floor rows "in guest instructions". They are ticks; multiply by 16. The floors stand,
  the unit label was wrong. Corrected in place.
- The three baseline files' headers and the template that writes them (`xtask/src/bench.rs`) said
  "icount counts guest instructions" where a reader compares files. They now say ticks are per-arch
  units and warn against comparing raw ticks across architectures.

### Method: the probe, so it can be replayed

A block at the top of `bench::run()`, since reverted: an `asm!` loop of `subs`/`b.ne` (aarch64),
`sub`/`jne` (x86_64) or `addi`/`bne` (riscv64), 200,000 iterations, two instructions per iteration
plus one, bracketed by two `arch::timer::now()` reads, printing
`bench-probe: instr_loop <ticks> for 400001 instructions`. Patched and unpatched boots of the same
tree printed the same row values on aarch64 and x86_64, and riscv64 moved 0.1 percent; adding the
probe shifted no row past the documented drift. It is not permanent because any added live code
shifts baselines ([drift and provenance](icount-drift-and-provenance.md)), and a permanent probe
would re-save every floor to buy a number each `bench: cntfrq` line already implies.

### BUGS

- The wall-clock magnitudes were taken beside another lane's measurement run. Both legs ran under the
  same load, which is what the ratios rest on; the magnitudes are approximate.
- The eight-bytes-per-store reading of the TSS write is arithmetic from ticks (two 8,192-byte writes
  per iteration, ~2,120 instructions together), not disassembly.
- 62.5 MHz, 10 MHz and ~1 GHz are this QEMU's (11.1.1, `virt`/`q35`) rates. A machine property can
  change any of them, which is why the method, not the constants alone, goes on record.
- The swish leg's own wall numbers come from `xtask/src/swish_check.rs`'s 2026-09-19 table, not
  re-measured here; re-running that leg is half an hour of CI-shaped wall clock and the recorded
  attribution already answers this lane's question.
