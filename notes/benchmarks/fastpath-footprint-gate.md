# The fastpath footprint gate

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds `script/fastpath-footprint`: what it
measures, what it prints, the per-architecture numbers and milestone 188's corrections, with the
dates, tables and corrections behind them. Name: ratified 2026-09-24 (calef); [the naming
record](README.md) holds it.*

The target this gate measures against, 4 KiB of IPC fastpath in a 32 KB L1i, is derived in
[the footprint appendix](kernel-footprint-and-caches.md), under "The target".

### Tracking it: `script/fastpath-footprint`, built 2026-08-18

*Milestone 132 owns this gate; design/roadmap/0132-the-fastpath-footprint.md carries the reasoning,
the BUGS and the trigger that would turn the gap below into scheduled work.*

An earlier paragraph proposed a gate and named what blocked it: which symbols the hot path is, given
assembly with no symbol sizes and inlined callees with no symbols at all. The answer is below, and
the gate exists.

#### The mechanism

`script/fastpath-footprint` walks the call graph out of the disassembly, as
`script/stack-depth-check` already does for stack chains. It reports these per ISA:

- `ipc_send_receive`: the transitive closure of non-cold calls from the SEND/RECEIVE roots (`ipc_send`,
  `ipc_receive`, `schedule`, `finish_switch`, `current_capability`).
- `ipc_call_reply`: the same closure from the CALL/RECEIVE_CAP/REPLY roots. This is the shape the
  system actually runs, and it is 25 to 29% larger; see
  [the wrong-shape correction](#the-gate-measured-the-wrong-shape-and-the-aarch64-entry-figure-counted-a-table-it-never-fetched).
- `ipc_fastpath`: derived, the worse of those two, which is what one round trip costs. It keeps its
  old name because a dozen places in the tree cite it. It is not gated on its own; both shapes are.
- `syscall_entry`: the trap vector plus the exception dispatcher plus `syscall::dispatch`, summed
  flat with no closure. A syscall traverses one path through a decoder. Closing over `dispatch`
  would pull in every object and method in the ABI and measure the syscall surface rather than this
  path. Its own bytes are on every syscall, so they count; its other arms are not, so they do not.

The script gates it at 5% growth against `bench/fastpath-<arch>.txt`. That is tighter than the
icount tripwire's 10% because these numbers are static: icount drifts when the compiler remakes
inlining decisions for unrelated reasons, where a symbol size moves only when the code moves.
DECISIONS §144 replaces that stored baseline with a delta against `main` plus an absolute 16 KiB
ceiling per architecture; that decision is made and not yet built.

### What the gate prints: distance, not drift (2026-09-21)

The gate used to report `+1.4% against baseline (7028)` and nothing else. That is movement away from
whatever the binary was the last time somebody re-recorded it. Two problems follow. The number means
something different after a compiler change, since symbol sizes move when the optimizer does. And
it says nothing about the constraint in the script's own first line, *"the IPC fastpath must stay
small enough to live in L1i"*: the 32 KB and the 4 KiB lived in this note, nowhere near the tool.

So each figure now prints as bytes, as a multiple of the 4 KiB target, and as a share of the 32 KB
L1i that binds. `total` is also given as a fraction of the 16 KiB ceiling that §144 (a delta and a
ceiling) decides and nothing yet enforces. riscv64, on `nightly-2026-09-20`:

```
    budget: 4096 B target, 32768 B L1i (radon's SiFive U74, the smallest we run on)
    ipc_send_receive    4734 B   1.16x target  14.4% of L1i  over 8 symbols
    ipc_call_reply   6038 B   1.47x target  18.4% of L1i  over 10 symbols  <- the shape the system runs
    ipc_fastpath     6038 B   1.47x target  18.4% of L1i  the worse of the two shapes
    syscall_entry    1914 B                  5.8% of L1i  over 5 symbols (flat, no closure)
    total            7952 B   1.94x target  24.3% of L1i  (7.77 KiB), an upper bound
    49% of the 16 KiB ceiling §144 (a delta and a ceiling) decides and does not yet enforce
    drift: ipc_send_receive +2.2% against baseline (4632), within the 5% band
```

The stored riscv64 baseline is 4,632 / 5,936 / 1,828, so the printed figures sit +2.2%, +1.7% and
+4.7% above it.

Drift is kept, demoted and relabelled. It is still the thing that fails, because a 5% jump inside
one pull request is a mistake somebody just made. It is no longer what the output is about.

Neither budget is gated, by calef's call on 2026-09-21. The tree is 1.5x to 2x over the 4 KiB
target, so a gate on it would fail on the day it was written. Shrinking the fastpath to pass one
would optimize against a target whose value nobody here can currently measure. Milestone 370 (a
layout control) exists because the perturbation experiments that would price a byte of footprint
cannot yet tell footprint from addresses. Whether 4 KiB is still the right number belongs with
milestone 132 (the fast path's footprint) and milestone 188 (the IPC fastpath), and waits on 370.
The footprint baselines were deliberately not re-saved in the same change, for the same reason.

The footprint baselines are eroding the way the icount ones did. They carry no record of the nightly
behind their figures. On 2026-09-20's pin every figure on every ISA sits above its baseline, with
riscv64's `syscall_entry` at +4.7% against a 5% band. One more compiler bump can fail this gate for
a reason no commit is responsible for. `bench/baseline-*.txt` got the toolchain stamp and
`script/lint`'s check on 2026-09-21. `bench/fastpath-*.txt` did not, because stamping it means
re-saving it. The script's own `BUGS` records it.

#### What "non-cold" means, since 2026-09-04 (milestone 188 phase 3)

Two families. The panic and formatting family is libcore's, and a regex in the script matches it,
because it carries no attribute anyone can read. Everything else comes from `#[cold]` in this
workspace's own source: the script greps for the attribute and excludes exactly those functions.
Outlining a cold arm is otherwise invisible to a closure walk, which simply follows the new call and
counts the same bytes under a new name. It also means the exclusion cannot be widened by editing the
gate, only by writing a claim in the code that a reviewer sees in a diff.

### The numbers, on `main` at 2026-08-18

| | aarch64 | riscv64 |
|---|---|---|
| `ipc_fastpath` (closure, 9 symbols each) | 5,780 | 5,074 |
| `syscall_entry` (flat) | 4,168 | 2,692 |
| **total, an upper bound** | **9,948 (9.71 KiB)** | **7,766 (7.58 KiB)** |

*A dated reading, not the current numbers. Two architectures because that is what the gate measured
that day; x86_64 is below. The current stored baselines are the milestone 188 "after" rows further
down, which are what `bench/fastpath-*.txt` holds.*

The closure's aarch64 members: `ipc_receive`, `schedule`, `ipc_send`, `finish_switch`, `wake`,
`current_capability`, `kmem::recycle`, `memcpy`, `switch_to`. Every one is defensible as something an IPC
round trip actually runs, which is the test the root list has to pass.

Closing naively from `finish_switch` returned 11.2 KiB. That function's reap branch drags in
`KernelStack::drop`, `untyped::destroy`, `revoke_region`, `delete_frame_caps` and the unmap path,
which run when a thread exits and never during an IPC. Classifying the teardown family as cold took
the figure to 5.6 KiB. A gate shipped at 11.2 would have measured thread death and called it IPC,
and would have been quiet about a doubling of the real path.

### x86_64, added 2026-08-27, and why one of its two numbers is not comparable

The gate measured two of the three architectures for nine days. `script/stack-frame-check` carried
the same silent omission until the architecture-list sweep of the same week found both. x86_64 was
then gated at `ipc_fastpath` 6,639 and `syscall_entry` 1,637, total 8,276 (8.08 KiB), in
`bench/fastpath-x86_64.txt`. *(Milestone 188 (the IPC fastpath) re-baselined it: the current x86_64 baseline is
`ipc_call_reply` 8,122 and `syscall_entry` 1,637, in the table below.)*

`ipc_fastpath` is comparable across all three, and x86_64 is the largest. The closure is the same
eight functions as aarch64's list above, minus `memcpy`, which LLVM inlines on x86_64 (the symbol
exists in the image and nothing references it). So the +15% over aarch64 is the same portable Rust
in a different ISA's encodings, not a different path.

`syscall_entry` is not comparable, and a reader looking at three numbers will assume it is. On
aarch64 and riscv64 a syscall is an exception. `svc` enters the same vector table as a page fault,
and `ecall` the same `stvec` handler as a timer interrupt, so both entry figures carry the whole
vector and cause decoder. On x86_64 a ring-3 `syscall` reads `IA32_LSTAR` and jumps, consulting no
IDT entry at all. Its entry set is four symbols: `x86_syscall_entry`, `x86_syscall_handler`,
`isr_restore` (the shared return path, the twin of riscv64's `trap_return`) and `syscall::dispatch`.
Excluded, because no syscall fetches them: the 256 IDT stubs (2,412 bytes), `isr_common`, and
`x86_trap_dispatch` / `x86_trap_body` (918 bytes). Those last two are the twins of the `riscv_trap_*`
pair that riscv64's list does include. x86_64's entry figure being the smallest of the three is that
architectural fact, not a leaner decoder. The per-symbol reasoning is in the script beside the
`ENTRY` table, where the next person to add an ISA meets it.

Nothing here is a cache result on x86_64 any more than on the other two; "What cannot be measured
yet" below applies to all three.

We are over the target on every architecture, and milestone 188 moved every number above; the next
section has the current figures. `syscall::dispatch` has not been the largest single item since
milestone 156 (`syscall_entry`'s measured size is every method combined) took 864 bytes out of it.

### The gate measured the wrong shape, and the aarch64 entry figure counted a table it never fetched

*Milestone 188 phases 1 to 3, 2026-09-04. `design/roadmap/0188-ipc-fastpath.md` carries the full
argument and the phase-4 recommendation.*

#### Phase 1: the roots were the wrong shape

The roots were `ipc_send` and `ipc_receive`, which is the shape of `ipc_rtt_el0` and of essentially no
service in this tree. A service is a client `CALL` and a server `RECEIVE_CAP` then `REPLY`.
`kernel/src/bench.rs`'s own `call_reply` doc already calls that "the one-endpoint shape real services
use". Measured on the same binaries, the shape userspace runs is larger everywhere. `ipc_receive_capability`
and `ipc_call` carry the one-shot Reply mint, the capability-table insert into the server and the
`WaitRole::Reply` parking DECISIONS §12 (call/Reply IPC) requires, and none of that is on a bare `SEND` or `RECEIVE`.
The gate now reports and checks both shapes.

The syscall count is corrected with it, which [the calibration appendix](calibration-against-sel4.md)
used to get wrong. Our round trip is three syscalls to seL4's two (client `CALL`, server `RECEIVE_CAP`,
server `REPLY`), not four to two. Four is `ipc_rtt_el0`'s count and no service issues it. The
residual one is the `ReplyRecv` fusion this tree does not have, which is a syscall-surface question
and calef's.

#### Phase 2: aarch64's `syscall_entry` counted all sixteen exception vector entries

`exception_vectors` is 2,020 bytes of sixteen self-contained 128-byte slots, because the hardware
requires that layout. An `svc` from EL0 lands in exactly one of them (index 8) and fetches at most
128 bytes. The other 1,892 are page faults, interrupts, kernel-mode traps and the AArch32 entries
this
kernel will never support. `exception_restore` (92 bytes), which runs on the return leg of every
syscall and is the twin of riscv64's `trap_return`, was in neither figure and is now in this one.
The riscv64 port needs no equivalent fix (`stvec` is in direct mode, one handler, no table), and neither does
x86_64 (no IDT entry at all). Nothing got faster: this is an accounting fix, and the old figure was
gameable by 1,892 bytes without changing one fetched instruction.

#### Phase 3: outlining alone made the number bigger

Milestone 156's `#[inline(never)]` method works on `syscall_entry` because that half is flat. On a
closure it is a no-op: the walk follows the new call and counts the same bytes under a new name. The
first attempt moved aarch64's `ipc_send_receive` from 5,888 to 6,220. What made it work was teaching
the walk to read `#[cold]` out of the source (above). Four arms went out of line: `finish_switch`'s
reap, `schedule`'s killed-thread conversion and its self-pop heal, and `set_ipc_aborted`, whose four
call sites are all on these closures.

| | `ipc_send_receive` | `ipc_call_reply` | `syscall_entry` | total |
|---|---|---|---|---|
| aarch64, before 188 | 5,888 | (7,576) | 3,304 | 9,192 |
| aarch64, after | **5,356** | **7,028** | **1,508** | **8,536 (8.34 KiB)** |
| riscv64, before 188 | 5,122 | (6,390) | 1,828 | 6,950 |
| riscv64, after | **4,632** | **5,936** | **1,828** | **7,764 (7.58 KiB)** |
| x86_64, before 188 | 6,767 | (8,657) | 1,637 | 8,404 |
| x86_64, after | **6,236** | **8,122** | **1,637** | **9,759 (9.53 KiB)** |

Parenthesized figures were not measured before this milestone; they are what the same binary would
have reported. The "after" rows are the baselines `bench/fastpath-*.txt` still holds. The totals move
in both directions because two effects run against each other. Counting the shape the system runs
adds 1,300 to 1,900 bytes, and aarch64's vector-table fix removes 1,796.

`kmem::recycle` left the closure on every architecture, which settles a question the milestone had
listed as open. It was reached only through `finish_switch`'s reap arm, so a successful rendezvous
cannot reach it, and phase 3 established that structurally rather than by assertion.

The cost of phase 3, measured: +0.17% retired instructions on `ipc_rtt` and +0.24% on `call_reply`
(aarch64, icount, against the same tree without the extraction). That is the outlined calls. It buys
6 to 10% off the footprint, and no cache effect anyone here can observe.

Where that leaves the 4 KiB target: `ipc_call_reply` is 5,936 to 8,122 bytes, so the shape the
system runs is still 48% to 103% over, after the cheap method has been applied. Milestone 188's own
recommendation reads that as the case for eventually hearing phase 4, and the block says so with the
numbers.

### BUGS

- It is an upper bound, not a footprint. The script sums whole symbol sizes, so a cold tail parked
  at the end of a hot function counts even though its cache lines are never fetched. The direction
  is deliberate: a tripwire that over-counts consistently still catches growth.
- Indirect calls are invisible, the same blind spot `script/stack-depth-check` records. A call
  through a function pointer contributes nothing to the closure.
- The script assumes the riscv64 tail instruction is 4 bytes. That ISA mixes 2- and 4-byte
  instructions, so the last instruction of each symbol may be over-counted by two bytes.
  Conservative, and lost in the noise at this scale.
- The same 4 is a guess in both directions on x86_64, whose instructions are 1 to 15 bytes, so it is
  not strictly an upper bound there. Measured: summing exact symbol sizes instead gives 6,619 against
  6,639 for the closure and 1,632 against 1,637 for the entry set, a 0.3% over-count. The `int3`
  padding LLVM parks after most x86 functions is already counted and roughly cancels the under-count
  on a symbol ending in a 5-byte tail `jmp`. That is a fifteenth of the 5% tolerance, so the formula
  was left alone rather than made per-ISA.
- Conditional branches to another symbol are not followed on any ISA. `b.ne`, `bne` and `jne` all
  fail the call pattern. Checked on x86_64: every conditional branch in the five root symbols targets
  an offset inside its own symbol, so nothing is missed today. A compiler that started emitting a
  conditional tail call would drop that callee from the closure silently.
- The cold list is a judgment, and a wrong entry is silent. If a symbol an IPC really does reach
  ever matches the cold pattern, it drops out of the number with no warning. The list is in the
  script with a reason per family for this reason.
- It is not a cache measurement. See below.

### What cannot be measured yet, and the machine that changes it

Every number above is static footprint. It bounds the problem and does not measure it. The icount
instrument models no caches at all ([notes/benchmarks.md](../benchmarks.md) says so under its two
instruments). The HVF runs are on the one machine whose L1 is large enough to hide the effect
entirely.

The TX1 is the machine where this becomes measurable: 48 KB L1i, a real PMU, and the platform whose
published seL4 numbers milestone 25 (cross-OS performance comparison) compares against. The cycle counters of milestone 74 (cycle counters), plus PMU
cache-miss events on that board would turn this from arithmetic into a measurement, taken next to
the kernel under comparison.
