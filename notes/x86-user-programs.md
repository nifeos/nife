# The hand-assembled x86_64 user programs

`kernel/src/user/x86_programs.rs` holds the user programs the `x86_64` kernel runs without a
compiler. This note carries their design. Each builder's doc comment says what its program does and
where it faults.

## Why they are written by hand

Every other architecture in this tree runs real ELF binaries out of the initial ramdisk.
`x86_64-unknown-none` has none, because `crates/user_mode_runtime` has no arms for this ISA and
`fixtures/build.rs` cannot compile its C components for it ([the x86 port](x86-port.md)). So the
programs the kernel needs before it has a userspace are machine code, the way the other ports' test
fixtures still are (`user/supervision_tests.rs`, `user/force_kill_tests.rs`). A four-instruction
program that faults on purpose is not worth a compiler.

The module is not test-only. The boot tour needs a real ring-3 process to show, and a `#[cfg(test)]`
fixture cannot be one. One copy lives in the kernel and the test modules alias it, so the ABI these
programs speak is written down once.

## Why a program is a `&[u32]`

`sched`'s child builders take a stub as `&[u32]`, one word per instruction. That is right on the two
fixed-width ISAs and means nothing on x86. Changing the type would turn the aarch64 and RISC-V
programs into byte soup and lose their one-instruction-per-line shape. So each x86 program is a byte
listing, packed into little-endian words at compile time by the `packer!` functions. The bytes reach
memory in the same order either way, which is all the CPU cares about.

The listings are what an assembler produced, checked against `clang -target x86_64-unknown-none`
rather than recalled. The core-reading programs were checked with
`llvm-mc -triple x86_64-unknown-none -show-encoding` on 2026-10-03. Each byte line in the source
carries its instruction as a trailing comment.

A program is padded to a word boundary with `0x90`, the one-byte `nop`. Zero would not do: `00 00`
is `add [rax], al`, which faults rather than falling off the end quietly.

## The ABI

`rax` carries the syscall number, and `rdi`, `rsi`, `rdx`, `r10`, `r8` and `r9` the arguments
(`arch::x86_64::exceptions::TrapFrame::syscall_nr`). That is DECISIONS §124 (ratify the x86_64 syscall ABI). These programs were the
first things in the tree to speak it from ring 3 through a scheduler.

Immediates use the 32-bit forms. Writing `eax` zeroes the top half of `rax`, so `mov eax, imm32`
sets the whole register. The 64-bit forms are five bytes longer and buy nothing today.

The programs that SEND encode `rendezvous::SEND` as `xor esi, esi`. A `const` assert in each fails
the build if `SEND` is ever given a nonzero value.

## The fault offsets

A test that asserts on the faulting pc asks the module for the offset. x86 instruction lengths
differ from the fixed four bytes of the other ports, so a constant added by hand would be wrong.

| program | faulting instruction | offset past the entry |
|---|---|---|
| `fault` | the load from `addr` | 5 |
| `port_out`, `port_out_then_exit` | `out dx, al` | 6 |
| `port_out_on_cpu_then_exit` | `out dx, al`, on core `cpu` | 19 |
| `port_out_on_cpu_then_exit` | the load from 0, on another core | 29 |
| `cap_delete_then_port_out` | `out dx, al` | 18 |
| `port_in_then_exit` | `in al, dx` | 4 |
| `port_in_then_out` | `out dx, al` | 7 |
| `two_capability_deletes_then_port_out` | `out dx, al` | 30 |
| `receive_then_port_out` | `out dx, al` | 31 |

## A fixture whose escape would block must exit instead

A port fixture's interesting case is the one where the `out` is wrongly permitted. If the program
then SENDs on a rendezvous the test is not receiving, the child parks forever and so does the test.
A real escape that blocks instead of reporting is not evidence: the run hangs rather than going
red, and no falsification record can be attached to it. That is row 26 of
[the confinement claims](confinement-claims.md), one object over.

So the fixtures whose `out` is expected to fault end in `SYS_EXIT` rather than in a report:
`port_out_then_exit`, `port_out_on_cpu_then_exit`, `cap_delete_then_port_out`,
`two_capability_deletes_then_port_out` and `receive_then_port_out`. A wrongly permitted `out` then
arrives as `EVENT_EXIT` on the supervision endpoint. The test wants `EVENT_FAULT` in that same
message, so the assertion that states the claim is the one that fires.

`port_out` and `port_out_reporting_cpu` do report, because their callers expect the `out` to
succeed and receive the word.

## The delete-it-yourself fixtures

`cap_delete_then_port_out` runs `SYS_CAP_DELETE` on the slot holding its `PortRange` capability,
then executes the `out`. The port grant is enforced by a cached field the context switch reads, and
the delete must clear it: authority the thread gave up by its own hand must be gone at once.
`SYS_CAP_DELETE` is a plain syscall number, not an `INVOKE` method, so the slot rides in `edi` and
nothing else is read. `dx` is loaded after the syscall, so no register the syscall clobbers matters.

`two_capability_deletes_then_port_out` holds one range in two slots. Deleting one must leave the
`out` permitted, and deleting both must revoke it.

`receive_then_port_out` parks in RECEIVE on slot 1 holding the range. The test revokes the range
while the child is parked, then wakes it. The RECEIVE is what lets the test pick the instant between
"holds the port" and "does the `out`", so the fault is the revocation's doing and not a race.

## Knowing the core

The port grant is installed in a per-core TSS I/O bitmap, and the hand-off test is about that
bitmap. On a machine with more than one core, a test that does not know which core its child ran on
is not testing it. At `NIFE_SMP=2` a non-holder that ran on the other core faulted for a reason
unrelated to the hand-off.

`port_out_reporting_cpu` reads its core from the current-CPU page (`current_cpu_contract`, written
by the context switch on the core doing the switch) once before the `out` and once after. It sends
`[word, core after, core before]`, and equal values bracket the `out` on one core. The word's
address is under 2 GiB (`address_space_map::CODE_MODEL_CEILING`), so the 32-bit absolute form of
`mov` reaches it without a sign-extension surprise.

`port_out_on_cpu_then_exit` checks the core first and runs the `out` only on core `cpu`. On any
other core it loads from address 0, which `address_space_map::NULL_GUARD` keeps unmapped. The two
faults differ by pc, so "landed on the wrong core, try again" is never read as "the `out` faulted".

## The blocking residents

`receive` and `call` are the two shapes of a resident that blocks forever, for the force-kill and
supervision tests. DECISIONS §16 (object revocation)'s kill is armed by a refusal and spent by `schedule()`, which a
thread parked in RECEIVE never reaches. A `CALL` caller whose request a server has collected is on
no queue at all: it left the sender queue, and only the one-shot `Reply` capability the server holds
can wake it. Its teardown has to sweep that `Reply` out of the server's capability table, or a
forgeable answer is left behind (milestone 133 (ending a permanently blocked thread)).

## BUGS

- Every immediate is a 32-bit one, so a report word or a syscall number above `u32::MAX` would be
  silently truncated. Nothing here is near that.
- No program checks a syscall's return value. These are fixtures: each proves it reached the kernel
  and came back, and the caller asserts on the kernel's side of the transaction.
- `port_out_reporting_cpu`'s bracket does not catch a thread that migrated away and back between
  its two reads. That takes two preemptions and two steals inside three instructions, and the
  caller says so.
