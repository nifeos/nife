# Call-frame information in hand-written assembly

*Name: provisional.*

Twelve hand-written `.s` files under `kernel/src/arch/` carried zero call-frame information (CFI)
before this milestone: no `.cfi_startproc`, no `.type`, no `.size`. A thirteenth,
`kernel/src/arch/x86_64/fp.s`, was missing from the milestone's own file list but is the same kind
of file and got the same treatment; see the BUGS section. This note records what CFI is, why these
files lacked it, what the directives say on each architecture, the before-and-after evidence, and
what is still not unwindable and why. The evidence it has no room for is in
[`cfi-unwind/`](cfi-unwind/README.md).

## What CFI is, in one paragraph

A compiled function's prologue usually does something like "push these registers, make room for
locals" and its epilogue undoes it. A debugger that wants to print a backtrace, or GDB's `finish`,
or a language's own unwinder, has to reconstruct three things at an arbitrary instruction in the
middle of that function. They are where the caller's stack frame starts (the canonical frame
address, CFA), where each register the function has saved lives relative to it, and where the
return address is. CFI is that description, written once per function, as a small program the debugger replays.
The compiler emits it automatically for every Rust function (that is what let the transcripts below
show line numbers and arguments for `kernel::sched::schedule` without any help from this milestone).
Hand-written assembly gets none of that for free: the assembler has no idea what a `stp x29, x30,
[sp, #-32]!` means about frames unless told, via `.cfi_*` directives.

The output format is `.eh_frame`. The name is inherited from C++ exception handling, which this
kernel does not have and does not want. It is the ordinary ELF section a debugger reads CFI from, and
it is not tied to unwinding actually happening at runtime. `panic = "abort"` means this kernel never
unwinds its own stack, but a debugger attached from outside still can. That is the
whole reason this milestone is about tooling, not correctness.

## Why they lacked it

Nobody had disabled it on purpose in the assembly. `notes/scripts.md` and `xtask`'s `gdb` command
already existed, printing `break kernel_main` / `break _boot` instructions as though a debugger
session were a normal thing to run here. The gap was one line, in all three link scripts, discarding
`.eh_frame*` outright with the comment "we never unwind" -- true about the kernel's own runtime
behavior, and irrelevant to a debugger reading the ELF from outside. That line discarded not only
what this milestone was asked to add, but every CFI record the *compiler* had already been emitting
for ordinary Rust functions the whole time. See "The link-script discovery" below.

## The link-script discovery

The CFI was being thrown away at link time. Adding the directives produced correct `.eh_frame`
content in each object, but the linked kernel's `.eh_frame` was empty on every architecture, because
all three link scripts discarded `*(.eh_frame*)`. Removing that line was not enough: default orphan
placement grew `__image_size` by about 100 KiB, a `(NOLOAD)` section dropped the bytes, and an
`(INFO)` section broke an aarch64 relocation.

What worked is an ordinary output section placed in the script text after `__image_end` and
`__image_size` are computed:

```
.eh_frame : { KEEP(*(.eh_frame)) KEEP(*(.eh_frame_hdr)) }
```

It stays ALLOC, so the bytes survive, and it costs `__image_size` nothing. Verified, not assumed: on
aarch64 the commit before this milestone (`ece6d72c`) and this tree both give
`__image_size = 0x350000`, and their flat `Image` binaries are byte-for-byte identical.
[`link-script-discovery.md`](cfi-unwind/link-script-discovery.md) has each failed placement and the
measurement.

aarch64 also strips `.eh_frame`/`.eh_frame_hdr` back out of that flat binary, in
`helpers/qemu-runner-aarch64.sh` and `xtask/src/inspect.rs`'s `image()`, with
`llvm-objcopy --remove-section`. That binary is what a real bootloader or QEMU's `-kernel Image`
path loads into memory. The full ELF that `gdb <elf>` reads keeps the content either way.
riscv64 and x86_64 do not get the same strip, and that is a real, if minor, gap: see BUGS.

## What the directives describe, per architecture

### The easy case: ordinary functions

`fp_save`/`fp_restore` (all three architectures) and `dispatch_on_interrupt_stack` (all three) are
reached by a real call and return by a real `ret`. Their CFI is the same shape a compiler would
emit: `.cfi_def_cfa_offset` tracking each push/pop, `.cfi_offset` naming where each saved register
lives, `.cfi_restore` undoing it. Nothing architecture-specific to say beyond what the code itself
already does.

### The interesting case: a context switch is a function that returns somewhere else

`switch_to` (aarch64, riscv64, x86_64) swaps `sp`/`rsp` to a different thread's stack mid-function
and then pops "its" callee-saved registers off that stack. The naive worry is that CFI can only
describe one function's frame, not a jump to an unrelated one. It turns out no special handling
is needed at the swap point, for a reason worth stating precisely: `.cfi_def_cfa_offset N`
defines the CFA as "the CURRENT stack-pointer register's value, plus N" -- a live formula, not a
frozen address. `next_context` is, by construction (`Context::for_kernel_thread`, `for_user_thread`,
or a previous `switch_to` call on that thread), exactly the stack-pointer value that thread's own
identical push sequence left behind. So the same formula, evaluated after the swap, correctly gives
*that thread's* call-site CFA, and the same `.cfi_offset` rules point at the very slots its own
prologue (or the synthetic frame) put values in. One continuous CFI program, no split, is both
simpler and more correct than trying to close and reopen the description at the swap.

`dispatch_on_interrupt_stack` needs a related but distinct trick: it swaps onto a per-CPU interrupt
stack that is not shaped like its own frame, runs a call there, and swaps back. Here the CFA is
restated in terms of a callee-saved register (`x19`/`s0`/`rbp`, via `.cfi_def_cfa_register`). That
register holds the pre-swap value and does not move again until it is restored. So the formula
stays valid on both sides of a stack that briefly isn't the one CFA was originally defined against.

`thread_trampoline`/`user_entry_trampoline` (and x86_64's pair) are the other half of the same
trick: a fake switch frame (`Context::for_kernel_thread`, etc.) whose "return address" is the
trampoline's own entry, so the very first `ret`/resume of a brand-new thread lands there. The link
register at that instant is self-referential, not a real caller -- `context.rs` already says so in
prose ("no caller: the backtrace ends here"); `.cfi_undefined` on the return-address register says
the same thing to the unwinder.

### The hard case: a trap is not a call, and two architectures cannot fully describe it

A vector entry (aarch64's `vectors.s`), a trap entry (RISC-V's `trap.s`), and an interrupt/exception
stub (x86_64's `trap.s`) all build a signal frame. That is the interrupted context's register file,
landed on the stack by hardware plus a macro, not by a `push` a debugger can walk backwards through
by convention. `.cfi_signal_frame` marks the FDE as one of these (GDB does not decrement the PC by
one when symbolizing it, and other tools know not to assume an ordinary call convention). The GP
registers are describable everywhere with plain arithmetic on the macro's own stores, so all three
recover x0-x29/a0-a7,s\*/rax-r15 correctly from any PC in the handler. The interrupted PC is the
part that differs by architecture:

- x86_64 can describe it fully, and this is the pleasant surprise of the three. The hardware
  frame puts the interrupted RIP and RSP at fixed offsets from the software frame's own CFA. The
  SysV DWARF register mapping already treats register 16 (RIP) as the ordinary return-address
  column and register 7 (RSP) as an ordinary describable register. So `isr_common` states
  `.cfi_offset 16, 16` / `.cfi_offset 7, 40`, and by the FDE the unwind continues past the trap,
  into whatever Rust function was interrupted and from there into *its* caller. That is read from
  `llvm-objdump --dwarf=frames`, not from a live GDB session: no x86_64 transcript was captured (see
  BUGS).
- aarch64 cannot, today, though the DWARF spec has an answer. The interrupted PC lives in
  `elr_el1`, a system register the hardware does not restore into `x30`/`lr` the way a `bl` does.
  AArch64's own DWARF register mapping anticipates exactly this. Register 33 is `ELR_mode`, defined
  for describing an asynchronously-created (signal/exception) frame (`aadwarf64.rst`, ARM's DWARF
  for the ARM 64-bit Architecture). `vectors.s` states `.cfi_return_column 33` and
  `.cfi_offset 33, -24` -- spec-correct, and there for whichever tool honors it. GDB is not one
  of them, as of this writing: it hardcodes column 30 (`x30`) as the AArch64 return-address column
  and does not consult `.cfi_return_column`
  ([sourceware.org/pipermail/gdb/2023-January/050488.html](https://sourceware.org/pipermail/gdb/2023-January/050488.html)).
  So the honest choice, and the one this milestone makes, is `.cfi_undefined x30` at the trap
  boundary. Repurposing column 30 to smuggle `elr_el1` through it would make GDB's unwind *work*
  today, at the cost of lying about where the real `x30` register lives: a query like `p $lr` in
  that frame would then read `elr_el1`'s value instead. Wrong CFI is worse than none;
  leaving the column undefined tells GDB, correctly, "stop here", which is exactly what the "after"
  transcript below shows it doing.
- RISC-V cannot, and has no spec-level answer to reach for either. The interrupted PC lives in
  `sepc`, and unlike AArch64, the base RISC-V DWARF register mapping has no reserved pseudo-register
  for a CSR the way `ELR_mode` exists. `trap.s`'s `.cfi_undefined ra` is therefore not a workaround
  for a tooling gap (as aarch64's is); it is the only currently-describable answer.

### `switch_to`'s epilogue restoring `x2`/`sp` itself (RISC-V)

One more RISC-V-specific note: `trap_return`'s very last register load, `ld x2, 2*8(sp)`, changes
the register the CFA formula is defined in terms of to an unrelated value (the interrupted context's
own `sp`), the same move `switch_to` makes deliberately. `.cfi_restore x2` there is correct (it
describes recovering the *value*, independent of the separate CFA-tracking mechanism), and it is the
last instruction before `sret`, so nothing after it needed a further rule.

### `image_header.s` is data, not code

Per the brief's own warning: `kernel/src/arch/aarch64/image_header.s` is the arm64 Image header, a
64-byte data structure the bootloader reads, with one instruction (`b _boot`) grafted onto its front
so the entry point can also be byte 0 of it. It carries no CFI, `.type`, or `.size`, and a
`CFI-EXEMPT:` comment says why, in the file, where a reader (or `script/lint`'s new check) meets it.
riscv64's `_start` has the identical shape (a Linux Image header with one leading `j`), for the same
reason and the same treatment.

### 16-bit and 32-bit code (x86_64 only)

`x86_64/boot.s`'s trampolines run in real mode and 32-bit protected mode for most of their length,
before the far jump into 64-bit mode. Those stretches carry no CFI, for two reasons. They have no
valid stack for most of it (no caller, nothing worth describing as a frame). And this ELF's own
`.eh_frame` is emitted and read as 64-bit DWARF throughout, so annotating a 16-/32-bit instruction
stream inside it would not obviously mean what the rest of the file's CFI means. `long_mode_entry`/
`_start_high` and `ap_long_mode_entry` (all `.code64`) get real CFI once execution is running in the
width the rest of the file is.

## Before and after: the evidence

Captured on aarch64 (the only architecture `cargo xtask gdb` currently drives), with GDB 17.2,
against the commit before this milestone and this branch's tip.
[`gdb-transcripts.md`](cfi-unwind/gdb-transcripts.md) has both cases in full. What they show:

- A brand-new thread's first run stops at `thread_trampoline` in both builds, which is correct:
  that frame is fabricated and has no caller. The after build stops there on purpose
  (`.cfi_undefined lr`), so GDB no longer calls the stack corrupt. The case shows no gain in reach,
  and is reported because claiming one would be invented CFI.
- Resuming an already-running thread is the case that matters. Before, the link script had discarded
  the compiler's CFI too, and GDB's frame-pointer fallback walked into `exception_vectors` and
  looped. It was killed at frame #9976, nearly four minutes after the `bt`. After, the backtrace
  shows seven real Rust frames past `switch_to`, then `<signal handler called>` for the vector
  entry, then stops cleanly at the trap boundary that the hard case above describes.

## Footprint

`script/fastpath-footprint` measures ELF symbol sizes in `.text`; CFI lives in `.eh_frame`, a
different section. Run before and after, on the same machine, same profile (release, which is what
the gate builds): byte-identical on every reported number (`ipc_send_receive` 6300, `ipc_call_reply`
8234, `ipc_fastpath` 8234, `syscall_entry` 1701). The gate's own printed "+N% against baseline" lines
are unchanged too, which makes sense: they compare against a stored reference figure from an earlier
point in the project's history that has nothing to do with this branch, and this branch moves
neither. The kernel's `__image_size` numbers (a different, and initially surprising, place this
milestone risked moving something) are covered in "The link-script discovery" above: verified
byte-for-byte, not merely argued.

## BUGS

- **The task that started this milestone named twelve files; there are thirteen.**
  `kernel/src/arch/x86_64/fp.s` (FXSAVE/FXRSTOR for the FPU/SSE register file) was omitted from the
  original enumeration. It is a real hand-written `.s` file under `kernel/src/arch/`, of the exact
  same shape as the other two `fp.s` files, and got the same CFI. Parity (DECISIONS's rule 5, cited
  in AGENTS.md) would have required it eventually regardless; this just does it now rather than
  leaving a gap the new `script/lint` check would otherwise have had to special-case.

- **riscv64 and x86_64 do not strip `.eh_frame` out of the image they boot**, unlike aarch64.
  Under QEMU neither port has a flat-Image/objcopy step: QEMU's `-kernel` loads each ELF's
  `PT_LOAD` segments directly (see each port's own `helpers/qemu-runner-*.sh` header comment). So
  the CFI rides along into guest RAM. It is a few hundred bytes to ~30 KiB, depending on how much
  the compiler emits for a given build. That is harmless in a QEMU VM with the usual hundreds of
  megabytes, and a new strip-then-boot step was not judged worth it at the time. The note said to
  revisit this if either port grew a real flat-image path, and riscv64 has one now:
  `script/board-image` runs `llvm-objcopy -O binary` for the VisionFive 2 with no
  `--remove-section`. What that costs the board image has not been measured.

- **GDB does not honor AArch64's spec-correct `ELR_mode` return column** (DWARF register 33;
  `.cfi_return_column 33` in `vectors.s`). So a backtrace on this architecture stops one frame short
  of x86_64's FDE, which describes the interrupted RIP and lets an unwind continue into the
  interrupted kernel code. This is a fact about the tool this project actually uses, cited to its
  own bug report
  ([sourceware.org/pipermail/gdb/2023-January/050488.html](https://sourceware.org/pipermail/gdb/2023-January/050488.html)).
  It is not a limitation this milestone introduced or could fix from the assembly side. Re-test if
  GDB ever gains AArch64 return-column support; the directive is already there, doing nothing useful
  today and no harm either.

- RISC-V has no spec-level answer at all for the trap-PC case, unlike AArch64's (merely
  unconsumed) `ELR_mode`. The base RISC-V DWARF register mapping has no reserved pseudo-register for
  `sepc`. `trap.s`'s trap boundary is therefore `.cfi_undefined ra` with no forward-looking
  alternative to offer. Closing this gap would need a convention this project cannot mint
  unilaterally. It would want to match whatever, if anything, the RISC-V toolchain ecosystem settles
  on, since a project-local DWARF register number would not mean anything to a debugger built
  against the standard mapping.

- **This is a presence check, not a correctness check** (the same limit `script/citations` and the
  roadmap's `## Revisit` check are honest about for their own claims). `script/lint`'s new "hand-
  written asm carries CFI" check confirms a `.s` file with an exported symbol also contains at least
  one `.cfi_startproc`, at file granularity. It cannot confirm the CFI is correct, that every
  function in a multi-function file has its own directives, or that a numeric offset is right rather
  than merely present. The evidence for correctness in this milestone is the transcripts, not
  the gate; a future file that adds a `.cfi_startproc` and then gets the offsets wrong would pass
  this check and fail a debugger, silently.

- Only one architecture's transcripts are captured. `cargo xtask gdb` currently only drives
  aarch64 (`xtask/src/main.rs`'s `RUNNER`/`TARGET` are aarch64-only). x86_64 and riscv64's CFI is
  built and inspected via `llvm-objdump --dwarf=frames` (see the design work this milestone's pull
  request records) but not exercised against a live GDB session the way aarch64's is.
  A `cargo xtask gdb --arch riscv64|x86_64` (or the equivalent) is a reasonable follow-on if this
  becomes a recurring need. Today it would have to be a hand-rolled QEMU `-s -S` invocation per
  port, which is exactly the kind of thing worth a lane rather than a footnote.
