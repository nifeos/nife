# Radon, 2026-10-05 (UTC): milestone 761's run and milestone 766's

*Uncommitted run sheet, written before any boot. One evening answers two outstanding radon runs:
milestone 761 (capability lookup off the global lock), against
[`notes/job-mix/null-system-call-under-load.md`](../../notes/job-mix/null-syscall-under-load.md#the-radon-run-that-decides-it),
and milestone 766 (each core's PerCpu on its own cache line), against
[its roadmap block](../../design/roadmap/0766-each-cores-percpu-on-its-own-cache-line.md).*

## The payloads

All three are built from `c8b5fd09ec68d03eab092679667c77ab088bf824` (`origin/main`, which carries
761, #1663's trap6 fix and 766), with `script/board-image --job-mix --tftp`, the same command the
2026-10-04 payloads used. Release, `riscv64imac-unknown-none-elf`.

| Payload | Features | What it is |
|---|---|---|
| `target/board-main` | `board,job_mix` | the kernel that ships |
| `target/board-main-lock-wait` | `board,job_mix,lock_wait` (`--extra-features lock_wait`) | the same plus the lock-wait instrument |
| `target/board-unaligned` | `board,job_mix` | `main` with 766 undone and `PERCPU` forced to 24 mod 128 |

`target/board-unaligned` was built in `~/projects/nife-worktrees/radon-2026-10-05-unaligned` with
[`unaligned.patch`](unaligned.patch) applied: the last hunk of
`notes/job-mix/null-syscall-hvf-full-mix.scratch.patch` (the `Shifted` wrapper), plus removing
766's `repr(align(128))` on riscv64 and the assertion that would refuse it. Without that removal the
wrapper's 24 bytes of padding would round up to 128 and the control would control nothing. Checked
on the linked ELF: `PERCPU_SHIFTED` at `0xffffffc080253880` (0 mod 128), so the blocks start at
`...898`, 24 mod 128 (and 24 mod 64, which is what matters on the U74's 64-byte line). `main`'s
blocks are 0 mod 128 by construction.

**One confound in the control, carried over from HVF.** The wrapper makes `PERCPU` a reference, so
every per-core access in the unaligned kernel is one extra load. That costs at one task as well as
at four, so the 766 row below reads the *growth* difference, which subtracts it.

**No console line names the payload.** The banner carries no commit or build id, and `main` and
`unaligned` print identical strings. The record of which payload a boot ran is the netboot log
(`serving <root>` and the byte count of `nife-vf2.img` fetched), captured per boot below, checked
against [`payloads.sha256`](payloads.sha256) and these sizes:

| Payload | `nife-vf2.img` sha256 (prefix) |
|---|---|
| `board-main` | `abd3f8487dc2` |
| `board-main-lock-wait` | `96d48d3e27b2` (also prints `job-mix-lock:` lines, which identify it) |
| `board-unaligned` | `044d25c60307` |

Not smoke-booted under QEMU. `main` and `main-lock-wait` are unmodified `main`; the unaligned patch
changes one static's layout and nothing else, and it links.

## Before power (once)

```sh
cd ~/projects/nife-worktrees/radon-2026-10-05
lsof /dev/cu.usbmodem*        # nothing else on the console
lsof -nP -iUDP:69             # nothing else serving TFTP
ls -l target/board-main/nife-vf2.img target/board-unaligned/nife-vf2.img target/board-main-lock-wait/nife-vf2.img
```

The board's card script fetches from `192.168.8.138` (patagonia, `en0`), baked in by this build.
If the board announces another address, `setenv nife_boot_server 192.168.8.138` then
`source ${scriptaddr}` at the U-Boot prompt.

## Each boot, N = 1 to 7

Two terminals, both in `~/projects/nife-worktrees/radon-2026-10-05`. `P` is the payload from the
order below.

Terminal A, the server. Stop the previous one (Ctrl-C) and start it with this boot's root, so the
log names what was served:

```sh
N=1; P=main
script/board-netboot --root target/board-$P 2>&1 | tee bench/radon-2026-10-05/boot$N-$P.netboot.log
```

Terminal B, the console, started before power:

```sh
N=1; P=main
script/board-console --for 30m --until none --log bench/radon-2026-10-05/raw-boot$N-$P.log
```

calef, physically: **power-cycle smart plug 2 only** (radon). Never smart plug 3 (garcia). Never
unplug or touch the USB hub. Wait for `job-mix: done` in terminal B (U-Boot and the fetch are most
of a boot; the timed windows are about 12 seconds), then end the console capture.

After each boot, clean the log (job-mix.md step 5):

```sh
LC_ALL=C tr -cd '\11\12\15\40-\176' < bench/radon-2026-10-05/raw-boot$N-$P.log > bench/radon-2026-10-05/boot$N-$P.log
LC_ALL=C grep -a -E 'one of 7 kinds|median of 21 repeats|job-mix: done|FAILED|REFUSED' bench/radon-2026-10-05/boot$N-$P.log
grep -a 'nife-vf2.img' bench/radon-2026-10-05/boot$N-$P.netboot.log
```

A boot counts only if it prints `one of 7 kinds`, `median of 21 repeats` and `job-mix: done`, and
its netboot log shows this boot's root. Otherwise repeat that boot number with the same payload.

## Boot order

Interleaved, so drift over the evening (temperature, the house network) falls on both sides:

| Boot | `P` |
|---|---|
| 1 | `main` |
| 2 | `unaligned` |
| 3 | `main` |
| 4 | `unaligned` |
| 5 | `main` |
| 6 | `main-lock-wait` |
| 7 | `main-lock-wait` |

## What each outcome means, written before any boot

Ticks are radon's 4 MHz timer. Baselines are `bench/radon-2026-10-04-fix/`: `null_syscall`
`per_job` 99 at one task and 147 at four (146 to 147 over three boots).

### 761: `null_syscall` at 4 tasks, boots 1, 3, 5 (`main`)

| Median of the three | Reading |
|---|---|
| 115 or below | it worked: the remainder is about `compute`'s preemption share (about 6 ticks) over 99. Risk 4's per-crossing cost under load is explained |
| 116 to 135 | partial: the lock was part of the rest. Boots 6 and 7 say whether `current_cap` still waits (it should not) and what the IPC rows still cost the cores |
| 136 or above | the lock was not radon's remaining cause. If boots 6 and 7 show `current_cap` contention near zero and the job did not move, suspect the line traffic no counter sees (BUGS, `PERCPU`) |

**Guard:** `null_syscall` at 1 task above 101 on `main` (99 plus 2%) is a regression to explain
before any row above is claimed.

This table is the 2026-10-04 one, unchanged. One thing differs from when it was written: 766 is now
in `main`, so a GREEN here is 761 and 766 together. Boots 2 and 4 say how much of it is 766.

### 766: the alignment's size on radon, boots 1 to 5

Read the growth difference, so the control's extra load cancels:

`D = [unaligned(4) - unaligned(1)] - [main(4) - main(1)]`, each term the median `null_syscall`
`per_job` over that payload's boots.

The scaling, stated as reasoning and not as a measurement: HVF measured the forced layout at +0.040
ticks a trap of a 24 MHz counter, about 1.7 ns. On radon's 4 MHz counter 1.7 ns is 0.007 ticks a
trap, or 0.43 ticks over the job's 64 traps, which is below the one-tick resolution of `per_job`.
So if a line bounce cost radon what it costs an M3, radon cannot see it. It can see it only if a
coherence miss on the U74 costs several times what it does on Apple's cores, which is plausible (a
slower core, a shared L2, a 64-byte line) but unmeasured here.

| `D` at 4 tasks | Reading |
|---|---|
| 1 tick or less | no effect radon can resolve: 766 is a no-op on radon at this instrument's grain, and 761 owns whatever boots 1, 3, 5 show. The 2026-10-04 boot spread at four tasks was 1 tick |
| 2 to 5 ticks | alignment matters on radon, small: 8 to 20 ns a trap, 5 to 12 times the HVF cost in nanoseconds. Record it as 766's acceptance number |
| 6 ticks or more | alignment is a material share of the rest (an eighth or more of the 48-tick growth from 2026-10-04), well above any plausible scaling of HVF. Check the census before believing it: a placement draw can do this on one boot |

Two guards on this row. If `unaligned(1) - main(1)` is more than 2 ticks, the control's extra load
is bigger than assumed and `D` is the only number to quote. If the two unaligned boots disagree at
four tasks by more than the three `main` boots spread, say the row is unresolved rather than pick
one.

### Lock-wait, boots 6 and 7

The instrumented kernel is slower (2026-10-04: 118 at one task, 342 to 345 at four), so its
`per_job` is not compared with anything. Its counts are.

## What to read, in this order

```sh
cd ~/projects/nife-worktrees/radon-2026-10-05/bench/radon-2026-10-05
LC_ALL=C grep -a -E 'kind=null_syscall' boot[1-5]-*.log | grep -E 'tasks=(1|4) '   # 761 table, 766 row, guard
LC_ALL=C grep -a -E 'kind=compute' boot[1-5]-*.log | grep -E 'tasks=(1|4) '        # preemption share
LC_ALL=C grep -a '^job-mix: tasks=' boot[1-5]-*.log                                 # jpm_median, against 932k at 4 and 998k at 32
LC_ALL=C grep -a '^job-mix-census:' boot[1-5]-*.log                                 # placement, for any outlier boot
LC_ALL=C grep -a 'site=current_cap' boot[67]-*.log | grep 'tasks=4 '                # contended / calls, against 41% on 2026-10-04
LC_ALL=C grep -a 'name=ipc_tables' boot[67]-*.log | grep 'tasks=4 '                 # contended and wait_ticks, against 2,593,130 wait ticks
LC_ALL=C grep -a 'site=reap' boot[67]-*.log | grep 'tasks=4 '                       # stack_free_ticks / reaps, against 167
```

1. `null_syscall` `per_job` at 1 and 4 tasks on boots 1, 3, 5: the 761 table and its guard.
2. The same on boots 2 and 4, then `D`: the 766 row and its guards.
3. Boots 6 and 7, `site=current_cap`: `contended / calls` at 4 tasks. 761 should put it near zero.
4. Boots 6 and 7, `rank=60 name=ipc_tables`: what the IPC rows still cost without the lookups.
5. `jpm_median` per boot at 4 and 32, for the throughput line under each verdict.

## After

- Results go to `notes/job-mix/null-system-call-under-load.md` (761) and 766's roadmap block, and both
  verdicts to fatal risk 4's line, whose colour is calef's.
- Commit the cleaned logs, the netboot logs, `payloads.sha256` and `unaligned.patch` with the
  results; the `raw-*` logs need not be.
- Then prune both worktrees (`radon-2026-10-05`, `radon-2026-10-05-unaligned`); neither holds code
  anyone needs once the logs are committed.
