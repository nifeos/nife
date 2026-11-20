# Cycle counters: the riscv64 half as built and measured

An appendix to [milestone 74 (cycle counters)](../0074-cycle-counters.md), under §212 (a prose budget).
This appendix holds what the riscv64 lane built and left out, the emulator finding it carried out,
and the working behind radon's measurement with the rows it converts.

## What the riscv64 half built, and what it deliberately did not

Built 2026-09-03, lane `milestone/74-cycle-counters-riscv`.

- `SBI_PMU` / `EID_PMU` as an optional row of `machine_discovery::riscv64::SBI_TABLE`, with
  `SBI_REQUIRED` narrowed to the rows that ask for it. Probed by the existing `probe_sbi` loop with
  no new code, and printed on the existing `firmware    :` boot line for the same reason.
- `CounterInfo`, the host-tested decode of `sbi_pmu_counter_get_info`'s packed word. It includes the
  `+ 1` on a width field the specification writes as one less than the width. It returns `None` for
  both CSR and width on a firmware counter. The specification says they "should be ignored", so
  they are not returned rather than returned as numbers a caller could use by accident.
- `kernel/src/arch/riscv64/pmu.rs`: the four calls, the CSR-number dispatch, and one boot line.
  Four kernel tests, each of which asserts plumbing and says in its own doc comment that it is not
  asserting a measurement.
- One `bench-probe: cycles_per_tick` line from `cargo xtask bench --riscv`. It is a probe, not a
  row: it is a rate rather than a duration. So it never enters `bench/baseline-riscv64.txt` and
  `--check` never polices it, which is the existing convention `map_new`'s shootdown probe
  established. One measured ratio converts every existing tick-denominated row at once. That is why
  it is one line at the top rather than a second number on every line.
- notes/riscv-cycle-counters.md, the bench procedure, written in notes/x86-uefi-boot.md's shape: in
  order, with real commands, and a table mapping each observable line to what it means. Step one is
  whether radon's OpenSBI implements the extension at all. That is a fact about somebody else's
  firmware, and every later step is conditional on it.

Deliberately not built, and each is the block's scope note rather than an oversight:

- No second event.
- No per-hart counter record. SBI PMU counters are per-hart and this configures the boot hart's; the
  one consumer is a single-hart probe.
- No user-facing read. The U-mode `rdcycle` path is the grant of milestone 229 (build the cycle-counter grant) and 237's measurement
  build, which already exist.
- No firmware-counter support. Reading one costs an `ecall` per read, and an `ecall` inside a cycle
  measurement measures the `ecall`.

## SBI_REQUIRED, the one thing it forced

The one thing the SBI PMU extension forced was not the `ecall`. `SBI_REQUIRED` was every row of
`SBI_TABLE` by construction, because every extension the kernel had ever called was one it could not
boot without. PMU is the first that is an instrument. A kernel that refused to boot without an
instrument would have confused the measurement with the thing measured. So `SbiRow` grew the
`required` field `Row` already carried one source over. The existing accumulation test failed on the
first run, which is what it was for.

## The emulator was more misleading than predicted

The emulator was more misleading than the block's prediction allowed for. This is the one finding
worth carrying out of the riscv64 half. Under `-icount` the `cycle` CSR and the `time` CSR are
driven off the same virtual clock. So the probe reads `cycles_per_tick 100.00`, exactly the ratio
between the two declared rates, with a rounding wobble and nothing else.

That is not a number that looks wrong. It looks like a clean measurement of a 1 GHz core, and a
reader who did not know what TCG does to these two registers would have every reason to write it
down. The defense is that the probe line prints its inputs (`10000029 cycles over 100000 ticks at
cntfrq 10000000`) rather than only the ratio, so the arithmetic is visible. And
notes/riscv-cycle-counters.md's outcome table names an exact round ratio as the tell rather than as
the answer.

## Why radon's 250.00 is real

A round number is exactly what milestone 16a warned would be an artifact, so the reason it is real
is recorded rather than assumed. 16a's block says QEMU-TCG yields "an implausibly exact 100.00"
because one virtual clock drives both the `cycle` and `time` CSRs, and an exact ratio is the tell.

This one is 250 for a physical reason. `cntfrq` is 4 MHz, the core is running at 1.0 GHz, and both
derive from the same PLL. So the ratio is an integer by construction of the silicon rather than of
the emulator. The evidence that two independent counters are being read is that the measurement is
not exact. 25,000,940 over 100,002 ticks is 250.0044, and a single clock feeding both would give
250.0000 every time. The 0.0044 is the drift an emulator cannot produce.

## What it converts

Every tick-denominated row of a board bench becomes cycles and wall time at 250 cycles per tick.
These are from the same boot, single-hart. `--bench` parks the secondaries, so these are uncontended
costs.

| row | cycles | time |
|---|---|---|
| `null_system_call` | 322 | 0.32 us |
| `yield_switch` | 571 | 0.57 us |
| `ipc_rtt` | 1,046 | 1.05 us |
| `call_reply` | 1,256 | 1.26 us |
| `map_el0` | 1,477 | 1.48 us |
| `ctx_switch` | 1,980 | 1.98 us |
| `relay_rtt` | 2,306 | 2.31 us |
| `broker_rtt` | 2,512 | 2.51 us |
| `ipc_rtt_el0` | 6,222 | 6.22 us |
| `spawn_reap` | 17,086 | 17.09 us |
| `spawn_el0` | 64,820 | 64.82 us |

These are not comparable to `bench/baseline-riscv64.txt`, and the reason is the unit rather than
the machine. That file records QEMU icount, which counts guest instructions, while these are real
cycles on silicon. The icount baseline remains what the tripwire checks; this table is what the
hardware costs. Putting them in one table would be the apples-to-apples failure
`notes/benchmarks.md` exists to prevent.

The aarch64 half was untouched by this when it was measured. It was built three days later and has
not met silicon.
