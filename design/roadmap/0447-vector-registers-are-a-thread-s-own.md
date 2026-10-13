---
status: BUILT
raised: 2026-09-20
built: 2026-09-20
---
# 447. A thread's vector registers are its own

Built 2026-09-20. This is Route 2 of milestone 164 (x86_64 userspace can't build `aes`: no SSE, no
scalar fallback), opened by calef; that milestone's [block](0164-x86-64-fs-server-aes.md) prices
the route.

This block is complete on its own. Four appendices hold the working
([index](0447-vector-registers-are-a-thread-s-own/README.md)):

- [`riscv.md`](0447-vector-registers-are-a-thread-s-own/riscv.md): why the RISC-V handler retries
  rather than decodes, why it sets `sstatus` in the trap frame, and how the missing `arch::init`
  call was found.
- [`costs.md`](0447-vector-registers-are-a-thread-s-own/costs.md): the fastpath, benchmark and
  icount tables, the memory argument, and the stack-frame gate's numbers.
- [`proof.md`](0447-vector-registers-are-a-thread-s-own/proof.md): what each of the five tests
  catches, and why they are kernel threads.
- [`target-flip.md`](0447-vector-registers-are-a-thread-s-own/target-flip.md): what flipping the
  targets to hard float would take, buy and cost.

## What was true before this, and it was true on purpose

The kernel saved no floating-point or vector state anywhere, on any of the three architectures. Not
a gap: a decision, recorded three times and enforced by construction.

- Every target in `targets/` is soft-float. `x86_64-unknown-nife.json` carries
  `"features": "-mmx,-sse,-sse2,...,-avx,-avx2,+soft-float"` and `"rustc-abi": "softfloat"`; the
  other two are the `-softfloat`/`-neon` equivalents.
- Milestone 184 (extend the `std` port to x86_64), in [its own block](0184-std-x86-64.md), calls
  that feature string "a correctness requirement" in its target table. The reason is beside it:
  "`kernel/src/arch/x86_64/` saves no FPU or SSE state on a context switch."
- §31 (the foreign-language seam: C holds no capabilities and makes no syscalls), in
  [its own section](../decisions/0031-foreign-language-seam.md), says the same about the C boundary.
  "The kernel never enables FP/SIMD for EL0, and the context switch" saves nothing. So a vector
  register in a confined component would be a trap or a corruption, depending on which bit first.
- 164's own refusal priced exactly this milestone and declined to build it: an SSE-enabled x86
  userspace "would mean an `FXSAVE` area per thread and save/restore in the context-switch path,
  and none of that is needed to compile `aes`." That was the right call then. It is what calef
  reopened.

So the interesting thing here is not that a kernel can save registers. It is that a kernel built
this far on the assumption that it never had to now does, without slowing the switch for threads
that do not use it. And the three ISAs' mechanisms are different enough to be worth writing down.

## The rule, which is one sentence

> The FP/SIMD register file holds the running thread's data, or it holds the initial state. It
> never holds a thread's data while a different thread runs.

That is `kernel/src/fp.rs`'s header, and everything else is that sentence made true at the one
instant it could stop being true. `crate::fp::hand_over` is called by the outgoing thread, on the
outgoing thread's stack, with `IPC_TABLES` already released and interrupts masked, immediately
before `arch::switch_to`. Four cases:

| outgoing | incoming | what happens |
|---|---|---|
| not live | not live | nothing. Two loads and a branch. Every switch in this tree today. |
| live | live | save, restore |
| not live | live | enable, restore |
| live | not live | save, scrub to the initial state, disable |

The switch itself is untouched. `context.s` still saves what a function call may destroy, on all
three architectures, and this runs beside it rather than inside it. That was a deliberate choice
over growing `Context` and letting `switch_to` do the whole job. The register file is not a calling
convention's callee-saved set; it is 512 bytes that cross a *thread* boundary. Keeping it in Rust
keeps it typed, testable, and out of three files of delicate hand-computed offsets.

And it lives in the free space of the thread's own TCB page, not in the `Thread` struct and not on
the kernel stack. That was the second shape rather than the first, and `script/stack-frame-check`
is the reason (see [the cost summary](#what-it-costs-measured) below).

## Why eager, and why "lazy" is a warning word here

The cheap-looking scheme is to leave the outgoing thread's registers in the hardware, trap when the
incoming thread touches them, and only then swap. Several kernels did that until 2018, when LazyFP,
CVE-2018-3665, showed that on x86 the trap is not a boundary. Speculative execution past the `#NM`
reads the registers the trap was supposed to protect, and one thread recovers another's AES round
keys. The mechanism was `CR0.TS`, which is the very bit this tree's x86 half now uses.

So the three control registers here answer "has this thread ever wanted FP", which is a performance
question. They never answer "whose data is in the registers", which is a confidentiality one. The
second question is answered by the registers always being right, which is what the fourth row of
the table above is for. When a thread that used the unit hands off to one that has not, the file is
scrubbed to the architectural initial state *before* the trap is re-armed. There is nothing behind
the trap to speculate at.

## Three architectures, three mechanisms, one rule

| | gate | first-use trap | what moves | bytes |
|---|---|---|---|---|
| aarch64 | `CPACR_EL1.FPEN`, `0b00` (traps EL0 and EL1) against `0b11` | its own exception class, `ESR_EL1.EC == 0x07` | `q0`-`q31`, `FPCR`, `FPSR` | 512 + 16 |
| riscv64 | `sstatus.FS`, Off against anything else | an ordinary illegal instruction; nothing distinguishes it | `f0`-`f31`, `fcsr` | 256 + 8 |
| x86_64 | `CR0.TS`, with `CR4.OSFXSR`/`OSXMMEXCPT` set and `CR0.EM` clear | `#NM`, vector 7 | the `FXSAVE` area: x87, `MXCSR`, `xmm0`-`xmm15` | 512 |

The RISC-V row cost this milestone real time, twice ([`riscv.md`](0447-vector-registers-are-a-thread-s-own/riscv.md)
has both). RISC-V does not say why an instruction was illegal, so the handler retries rather than
decodes: a genuinely illegal instruction traps again with `FS` no longer Off and takes its fault.
And `sstatus` travels in RISC-V's trap frame, so the first-use handler sets the frame's copy too.

## What it found: one architecture's boot path does not run `arch::init`

`arch::fp::init` shuts the unit for a core before any thread exists. It went first in `arch::init`,
which was green on aarch64 and x86_64 and silently wrong on RISC-V. `main`'s RISC-V tour calls
`arch::exceptions::init()` directly and never passes through `arch::init`. OpenSBI hands over a
hart with `sstatus.FS` already set, so no thread took the first-use trap and two threads shared a
register file with nothing saying so.

The fix is not a third call site. The invariant is about threads, so it lives where threads begin
to exist: `sched::init` and `sched::adopt_secondary_idle`, both portable. The test that caught it,
`the_first_floating_point_instruction_takes_a_trap`, was written to catch exactly this. The full
account is in [`riscv.md`](0447-vector-registers-are-a-thread-s-own/riscv.md#what-it-found-one-architectures-boot-path-does-not-run-archinit).

## What it costs, measured

The tables and working are in [`costs.md`](0447-vector-registers-are-a-thread-s-own/costs.md).

- IPC fastpath: `script/fastpath-footprint` is within the 5% bound on every ISA; the largest
  increase is riscv64's `syscall_entry`, +2.2%. `hand_over`'s register-touching half and
  `enable_for_current` are `#[cold]` and out of line, which keeps riscv64's `ipc_send_recv` inside
  the bound. `crate::fp::ENABLES` checks the claim: it counts threads that ever asked for the unit,
  and it is zero on every shipping boot.
- Benchmarks: `script/bench` shows a context switch costing 2-3% more instructions, about seventeen
  per switch on aarch64. Every figure is inside the 10% tripwire and `coremark` did not move.
- Memory: none. The register file (544 bytes on aarch64, 528 on x86_64, 272 on riscv64) lives in
  the 2,944 idle bytes of each thread's 4096-byte TCB page, and a `const` assertion fails the build
  if it ever stops fitting.
- Stack frames: unchanged. `FpState` lives in the TCB page, not inline in `Thread`, because inline
  `script/stack-frame-check` fails `Thread::spawn::<sched::init>` at 5152 bytes against the
  4096-byte guard page: an unoptimized build copies the `Thread`, so each byte added costs two of
  frame.

- Instruction clock: `script/icount` is identical before and after, as it should be. The timer
  handler does not switch threads.

## The proof, and it fails against a kernel without this

Five `#[test_case]`s in `kernel/src/fp.rs`, running on all three architectures;
[`proof.md`](0447-vector-registers-are-a-thread-s-own/proof.md) says what each one catches.
`two_threads_doing_vector_work_do_not_see_each_others_registers` is the milestone. Two threads on
one core fill the whole register file with distinct patterns, yield two hundred times each, and
check their own values after every turn. Falsified on purpose: with `save` removed from
`hand_over`, it fails with "a thread found another thread's values in its own vector registers".
The others check the scrub (the fourth row, and CVE-2018-3665's case), every register by name, the
first-use trap, and that threads which never used the unit leave it shut.

The tests are kernel threads, because every target in `targets/` is soft-float and no userspace
binary can execute an FP instruction. So all three first-use handlers serve the kernel's own
exception level as well as userspace, and §31's C seam is untouched.

## What this does NOT do: the target flip is not taken

No target JSON is changed and no force-soft flag is removed. Turning `+soft-float` off changes the
calling convention for every userspace binary and every `std` crate built against it. That is an
ABI two programs agree on, which AGENTS.md's *move fast on what can be undone* tenet puts in the
irreversible category. §22 (Rust `std` on the native ABI, the Hermit way) chose it deliberately,
specifying nife's target JSON as "softfloat, and `singlethread = true`".

This milestone makes the flip possible, and it is an architect's to make, on evidence.
[`target-flip.md`](0447-vector-registers-are-a-thread-s-own/target-flip.md) prices it: four edits
and a rebuild, buying less than the framing suggests (one force-soft flag, `aes_force_soft`), and
costing a save on most switches once most threads are live. Milestone 534 (the soft-float targets
could now be flipped) carries it now.

## BUGS

- `live` never clears. A thread that used FP once saves and restores the whole file on every
  switch for the rest of its life. RISC-V's `sstatus.FS` could answer the narrower question in
  hardware and this does not use it. A uniform rule across three ISAs was judged worth more than
  one ISA's optimization while no workload exists to measure the difference on. Recorded in
  `kernel/src/fp.rs` and in `arch/riscv64/fp.rs`, which has the four-state table it declines to use.
- x86_64 uses `fxsave`, not `xsave`. A thread using AVX would have `ymm` upper halves this does
  not move. Safe only because `CR4.OSXSAVE` is clear, so every VEX-encoded instruction raises `#UD`
  and no thread can get into that state. The day this kernel sets `XCR0`, `arch/x86_64/fp.rs` has
  to grow an `xsave` path with it, and nothing enforces that coupling. It is stated in that file's
  `BUGS` where the next reader meets it. `xsave`'s init optimization (skipping components in their
  initial configuration) is left on the table with it.
- aarch64 does not disable SVE or SME. `CPACR_EL1.ZEN` and `SMEN` are left at their reset values,
  which on every machine this kernel has run on means trapped. A part that reset them open would
  let a thread keep vector state this file does not move. Nothing in this tree emits SVE and the
  check belongs with `arch::isa`'s feature reading; recorded, not built.
- A RISC-V hart without the D extension refuses rather than loops, and nothing tests it.
  `sstatus.FS` is hardwired to zero on such a part, so the enable does not take, `is_enabled` says
  so, and the first-use trap becomes an ordinary fault. Every machine in this tree's matrix
  (`qemu-system-riscv64 -cpu rv64`, the JH7110's U74) has D. F without D is worse: 32-bit
  registers where `fp.s` writes 64, so the save would be wrong rather than refused.
- A migrating thread carries its register file through memory, saved on the core it leaves and
  restored on the core it arrives at. Correct, and a kilobyte of traffic a same-core switch does not
  pay. Nothing measures it.
- This is proved under QEMU only. No board has run it. The one line most likely to be wrong on
  silicon is the one QEMU cannot falsify. `arch::fp::init` writes `CPACR_EL1` to a value QEMU's
  reset already supplies, so on the emulator it could be deleted without a test noticing. The
  architecture says the reset value is UNKNOWN.

## Follow-on

- **Milestone 534.** Milestone 534 (the soft-float targets could now be flipped),
  [`0534-the-soft-float-targets-could-now-be-flipped.md`](0534-the-soft-float-targets-could-now-be-flipped.md),
  holds the target flip: what it would take, buy and cost, and the two measurements that should
  come before it. It is written up rather than recommended, because it is an ABI two programs agree
  on and §22 chose the current one deliberately. The *fork reaches calef with its questions already
  answered* tenet asks for options and costs on an irreversible fork, and explicitly not for a
  winner. A `design/decisions/` section is owed when calef rules on it; this lane does not write
  one, per its brief. First on that block's list: re-measure the soft-float x86_64 failures of
  milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets) against nife's
  own target specifications on the pinned nightly. 442's block says it owes that in its own first
  item. It is a prerequisite for pricing the flip rather than a
  consequence of it, and it is cheap.
- **Recorded.** `arch::fp::init` is called from `sched::init` and `sched::adopt_secondary_idle`
  rather than from `arch::init`, because RISC-V's boot hart never calls the latter. The reason is at
  the call site in `kernel/src/sched.rs`, and all three `arch/*/fp.rs` point at it.
- **Recorded.** The `xsave`/`XCR0` coupling above, in `arch/x86_64/fp.rs`'s `BUGS`.
- **Refused.** Building a `Thread` in place, field by field, instead of writing a struct literal
  through a pointer. Priced under the gate, which raised the question
  ([`costs.md`](0447-vector-registers-are-a-thread-s-own/costs.md#what-the-gate-caught-a-struct-that-gets-copied-through-a-callers-frame)).
  It would take `Thread::spawn_into`'s frame well below the 3552 bytes it has carried since
  milestone 124 (a thread is born where it lives: the spawn path's copies). What it costs is the
  thing the ladder ranks highest. A struct literal is checked for completeness by the compiler and twenty-five
  `addr_of_mut!` writes are not, so a forgotten field is uninitialized memory in a TCB with nothing
  to catch it. 124 stopped one hop short of this for the same reason. It is the right next move if
  the frame ever needs more headroom. It was not needed here: the register file moved out of the
  struct instead and the numbers went back to 124's exactly.
- **Refused.** Growing `Context` so that `switch_to` saves the register file with the callee-saved
  set. It looks tidier and keeps `thread.rs`'s line that a thread's whole saved state is one stack
  pointer. It is worse: an uninitialized region in every thread's kernel stack frame, and a new
  frame-size constant leaking into portable code. It also means three files of hand-computed
  offsets to keep in step, and a save/restore that Rust cannot test without an emulator. The cost
  of the chosen shape is two field reads in `schedule`, measured above.
- **Refused.** Using RISC-V's `sstatus.FS` dirty bit to skip a save whose registers are unchanged.
  It is the one place any of the three ISAs offers something better than "has this thread ever
  asked". Taking it would mean three different policies to reason about, for a saving no workload
  can currently demonstrate. First thing to try when one exists; in `crate::fp`'s `BUGS`.
- **Refused.** Decoding the faulting instruction on RISC-V to tell an FP trap from a bad opcode.
  Above: `stval` may be zero for that cause, so it would mean reading user memory at `sepc` and
  decoding seven major opcodes plus the compressed forms, to answer a question one retry answers.

## Index row

Milestone 164's refused Route 2, reopened by calef. The kernel saved no FP or vector state anywhere,
which is why every target in `targets/` is soft-float as a correctness requirement rather than a
preference; it now saves the whole register file across a context switch on all three architectures,
under one rule: the file holds the running thread's data or the initial state, never a stranger's.
Eager and not lazy, because the trap this uses on x86 is `CR0.TS` and deferring the restore behind
it is CVE-2018-3665. The expensive half is `#[cold]` and off the IPC fastpath, so a soft-float
thread pays two loads and a branch, and the register file costs no memory: it lives in the 2,944
bytes of a thread's TCB page that were already allocated and idle, which is where
`script/stack-frame-check` sent it after the inline shape put `Thread::spawn` 1,056 bytes over the
guard page. `script/fastpath-footprint` within bound on every ISA, a context switch 2-3% more
instructions, `coremark` and `script/icount` unmoved. Proved by two threads
doing vector work on one core, falsified against a kernel with the save removed. The target flip is
**not** taken: it is an ABI and calef's, and the block ends with what it would take, buy and cost.
