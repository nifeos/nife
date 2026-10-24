# The spawn job's `Gone`: a miscompiled yield, not a race

*Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `spawn-destroy-gone`.*

Under HVF on aarch64, `script/job-mix --hvf --release --smp 4` failed every sweep. Risk 4's lane
(#1661) found it on 2026-10-05: nine runs out of nine, seven refused with `-11` (`Gone`) in a spawn
job and two wedged. It tagged the refusal to the spawn job's `DESTROY` of its child region, saw a
build that counted `DESTROY`'s retries pass, and read the shape as fatal risk 5's (multicore
reliability). Lane `lane/hvf-spawn-destroy-gone` took it on 2026-10-05 (UTC).

**It was not a multicore defect in the kernel.** `user_mode_runtime::yield_now` and `cap_delete`
told the compiler that no register changes across their trap, the kernel writes `x0` on the way
out of every syscall, and an optimized loop in the spawn job kept `DESTROY`'s arguments in `x0..x4`
across a yield.

## What each experiment asked

| Question | Experiment | Answer |
|---|---|---|
| Q1. Does it reproduce here? | one unmodified `--hvf --release --smp 4` run of `f7dec0e65` | yes, `-11` in a spawn job after 20 s |
| Q2. Can `DESTROY` return `Gone` at all? | read `memory_region_destroy` and `invoke`'s lookup | no. It maps every refusal to `NotPermitted`, and an empty slot is `NoSuchSlot`. `Gone` comes from an IPC abort (`take_ipc_aborted`) and nowhere on this path |
| Q3. Then what did the task send? | disassemble `spawn_job` in the release `job_mix_task` | after a refused `DESTROY` it traps `invoke(0, 4)`, below |
| Q4. Is that the whole cause? | 20 runs of each side, alone on the host | 0 of 20 complete before, 20 of 20 after |
| Q5. Why did riscv64 and radon never show it? | disassemble the riscv64 release build | its register allocator reloads the arguments after the yield |
| Q6. Does TCG hide it on aarch64? | three TCG runs of each side | no. Without the fix it wedges 3 of 3; with it, 3 of 3 complete |

## The interleaving, which is in one thread

`fixtures/src/job_mix_task.rs` `spawn_job` retries `DESTROY` while the child is still exiting:

```rust
loop {
    let r = user_mode_runtime::destroy_region(child);
    if r == 0 { break; }
    if r != abi::Error::NotPermitted as i64 { return Err(r); }
    yield_now();
}
```

`yield_now` was `asm!("svc #0", in("x8") abi::SYS_YIELD, options(nostack, nomem))`, with no output.
That is a promise that `x0..x4` survive the trap, so LLVM hoisted the next iteration's argument
setup above the yield (release `job_mix_task`, `f7dec0e65`):

```text
60000ec0:  mov  w8, #0x1       // SYS_YIELD
60000ec4:  mov  x0, x9         // x0 = child, DESTROY's slot
60000ec8:  mov  w1, #0x4       // x1 = DESTROY
60000ecc:  mov  x2, xzr
60000ed0:  mov  x3, xzr
60000ed4:  mov  x4, xzr
60000ed8:  svc  #0             // yield: the kernel writes x0 = 0 on the way out
60000edc:  mrs  x12, CNTVCT_EL0
60000ee0:  mov  w8, #0x2       // SYS_INVOKE
60000ee4:  svc  #0             // invoke(slot 0, method 4)
```

`kernel/src/syscall.rs` `dispatch` ends every syscall with `frame.set_arg(0, result)`, and a yield's
result is 0. So the retry invoked slot 0, `job_mix::SLOT_REPORT`, with method 4, which on an
endpoint is `CALL`. The supervisor (`kernel/src/job_mix.rs` `subrun`) waits on that endpoint in a
plain `RECEIVE`, and since §246 (a plain `RECEIVE` never takes a capability) a `CALL` that meets a
plain `RECEIVE` is answered `Gone` (`sched::call_meets_plain_receive`). The task reported the
`-11` as `DESTROY`'s, because to its source code it was. The supervisor also took the `CALL`'s two
words as a report, which is the likely source of the wedges: a subrun that counted one report too
many or too few, and nothing left to send the rest. Before §246 the caller stayed parked, which is
a wedge by another route.

Why it depends on timing. The bad call happens only on the second pass of the loop, so only
when the first `DESTROY` found the child still on its way out. Why the instrumented build passed.
Counting retries changed the loop, and the new allocation happened not to hoist. Neither is evidence
about the kernel.

## Why riscv64 never showed it

The same source compiled for riscv64 (`job_mix_task`, release, from the risk 4 lane's tree) reloads
`a0` after the yield's `ecall` (`mv a0, t1`). The declaration was just as wrong there; the register
allocator did not happen to exploit it. riscv64 TCG and radon run that build, so neither could see
it. `x86_64`'s build was not inspected.

The claim that TCG hides it was true of riscv64 only. On aarch64 TCG, with the fix reverted by
the falsification patch below, `script/job-mix --release --smp 4` went quiet at `tasks=1` in 3 runs
of 3, and completed 3 of 3 with the fix. That is the wedge `null-syscall-under-load.md`'s BUGS
recorded on 2026-10-04 as a multicore hang wanting a bisect.

## Before and after

`script/job-mix --hvf --release --smp 4`, patagonia, one QEMU at a time, 2026-10-05 (UTC). "Before"
is base `f7dec0e65`, "after" is this lane's tree.

| | completed | refused `-11` | wedged |
|---|---|---|---|
| before | 0 of 20 | 19 | 1 |
| after | 20 of 20 | 0 | 0 |
| falsification patch applied | 0 of 5 | 4 | 1 |

A passing sweep takes about 16 s on this host.

## The fix

- One trap per architecture in `crates/user_mode_runtime` (`trap6`), declaring all six ABI
  words as outputs (the sixth is milestone 105 (the two forks)'s label register). `invoke6`,
  `invoke5`, `yield_now`, `cap_delete` and `exit` all go through it, so no
  wrapper can declare fewer registers than the kernel writes. `yield_now` also lost `nomem`: a
  yield is when other threads write the memory this one shares with them.
- The std overlay had the same declarations (`patches/std-nife/overlay/std/src/sys/pal/nife/rt.rs`:
  `yield_now` with no output, `invoke` and `call` with `x1..x4` input-only). It gets the same single
  trap. No std program is known to have been miscompiled, which is luck rather than a property.
  `thread::sleep`'s yield loop was exposed.
- A gate, `script/lint` check 15 (`helpers/syscall_asm.py`, provisional name): every
  `svc`/`ecall`/`syscall` in an `asm!` block outside `kernel/` must declare all six words
  (and `rcx`, `r11` on `x86_64`). Against base `f7dec0e65` it names 22 blocks in three files.
  `os_primitives_benchmarker`'s `null_syscall` gained four `lateout`s to pass it. That function is
  `#[inline(never)]` and the registers are caller-saved, so its measured body stays
  `mov w8, #0xffff; svc #0; ret`.
- Falsification, replayable: [`spawn-destroy-gone.falsification.patch`](spawn-destroy-gone.falsification.patch)
  restores the old `yield_now`. With it applied the gate fails on three blocks and the HVF sweep
  fails as in the table.

No kernel change, and nothing about `DESTROY`'s semantics or the syscall surface: §10 (process
model: capability-based, microkernel) and §16 (object revocation) are untouched.

## What this says about fatal risk 5

For the maintainer; the verdict is calef's. This removes one item of risk 5's evidence rather than
adding one. The HVF `Gone` and the aarch64 TCG wedge were the same userspace miscompilation, and
the kernel's region teardown, reap ordering and barriers were not involved. With the fix the full
mix completes on four real Apple cores, 20 sweeps of 20, which risk 4's appendix could not do
(it stubbed the spawn job). That is positive evidence for multicore reliability under real
concurrency, but HVF only, and a sweep is minutes of load, not a soak.

## BUGS

- **The gate reads text.** A trap whose template is built by a macro, or one in `global_asm!` or a
  `.s` file, is invisible to it. None exists outside the kernel today; a hand-written assembly stub
  allocates its own registers, so the compiler hazard does not reach it.
- **The std overlay change is gated by CI, not here.** Building it locally takes the account-wide
  `nife-dev` link, which another lane held when this was written.
- **Twenty runs is a rate, not a proof.** The proof is the disassembly and the gate; the runs show
  nothing else was hiding behind it on this host.
