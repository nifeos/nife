---
status: BUILT
raised: 2026-09-21
built: 2026-09-21
---
# 520. Call-frame information in hand-written assembly, and the link-script line that was hiding the compiler's own

Built 2026-09-21: CFI in `kernel/src/arch/`'s hand-written `.s` files.

**The larger finding is not the assembly directives; it is what they exposed.** All three link
scripts (`kernel/link-{aarch64,riscv64,x86_64}.ld`) carried `*(.eh_frame*)` in their `/DISCARD/`
block, commented "stack-unwinding tables; we never unwind" -- true about this kernel's own runtime
(`panic = "abort"` means it never unwinds its own stack) and irrelevant to a debugger reading the
ELF from outside. That one line had been discarding the *compiler's* call-frame information for
every ordinary Rust function in the kernel, on every architecture, since the first commit, not only
whatever this milestone was sent to add. **The debugger could not unwind anything, anywhere, and
nobody knew**, because nothing had ever tried and printed what it got. `script/gdb`'s own
instructions (`break kernel_main`, `break _boot`) work for source-level stepping regardless, which
is exactly the kind of thing that hides a dead unwinder. The hand-written assembly's own missing
directives are what made the defect visible, by giving a reason to go looking at `.eh_frame` at all.

## What was asked, and the twist in the middle of it

Twelve hand-written `.s` files under `kernel/src/arch/` carried zero CFI: no `.cfi_startproc`,
`.type`, or `.size`. A debugger stopped inside a context switch or a trap handler had nothing
describing the frame. The hardest defects this project expects to chase are concurrency bugs on real
silicon where a backtrace at a breakpoint is the main instrument. The brief named twelve files;
there are thirteen -- `kernel/src/arch/x86_64/fp.s` (FXSAVE/FXRSTOR) is the same shape as the other
two `fp.s` files and was simply missing from the original list. It got the same treatment.

Adding the directives compiled clean and produced correct `.eh_frame` content immediately. Checking
that content in the *linked kernel* is what found the discard line: `llvm-objdump --dwarf=frames`
reported empty output on every architecture, for hand-written assembly and for compiler-generated
Rust functions alike. Fixing the link scripts turned out to need its own investigation, recorded in
full in `notes/cfi-unwind.md`; three placements were tried and two were rejected:

1. Left unmentioned. LLD's default orphan placement keeps `.eh_frame*` ALLOC and slots it
   between `.rodata` and `.data` -- before `__image_end`, the point this kernel's own boot code
   reads to fill in the arm64/RISC-V Image header's `image_size` field for a real bootloader.
   Measured: `__image_size` grew by the CFI's own size, about 100 KiB, for a debugging convenience
   that has nothing to do with how much RAM the image needs.
2. An explicit `(NOLOAD)` output section, to move it off ALLOC. This turned it into
   `SHT_NOBITS`, the same representation `.bss` uses: LLD dropped the actual bytes, and
   `--dwarf=frames` went back to empty. Wrong mechanism entirely: NOLOAD is for address space whose
   content does not matter, and CFI content is the whole point.
3. An explicit `(INFO)` output section, to keep the bytes and drop ALLOC. This let LLD place
   the section multiple megabytes from `.text` and broke a real `R_AARCH64_PREL32` relocation
   elsewhere in the image outright: `rust-lld: error: relocation ... out of range`.

What worked: an ordinary output section -- no `NOLOAD`, no `INFO` -- placed in the script text
*after* `__image_end`/`__image_size` are already computed, instead of left to orphan placement.
Stays ALLOC (bytes survive), keeps LLD's normal sequential layout (lands right after `.bss`, close
enough to `.text` that nothing overflows), and costs `__image_size` nothing because that symbol is
assigned before this section exists.

Verified, not assumed. A from-scratch build of the commit immediately before this milestone
(`ece6d72c`) gives `__image_size = 0x350000` on aarch64. This tree, with CFI present throughout,
gives the identical `0x350000`. The flat `Image` binary the two builds' `objcopy -O binary` produce
-- the actual bytes a bootloader or QEMU's `-kernel Image` path loads -- is byte-for-byte
identical, `cmp` confirmed. aarch64's flat-image steps (`helpers/qemu-runner-aarch64.sh`,
`xtask/src/inspect.rs`'s `image()`) now `--remove-section` the CFI back out before boot, since that
is the one place a bootloader actually reads these bytes into memory; the full ELF `gdb <elf>`
reads keeps the content either way. riscv64 and x86_64 have no such strip and none was added (see
BUGS). Neither port has a flat-image step at all, so the CFI rides into QEMU's guest RAM alongside
everything else, harmlessly, in a VM with the usual hundreds of megabytes.

`script/fastpath-footprint` is byte-identical before and after on every reported number
(`ipc_send_recv` 6300, `ipc_call_reply` 8234, `ipc_fastpath` 8234, `system_call_entry` 1701): CFI lives
in `.eh_frame`, a section distinct from `.text`, and the measurement confirms it moved nothing. That
and the `__image_size`/flat-image evidence above are the proof. Annotating every hand-written
assembly file in the tree, plus un-hiding the compiler's own CFI for every Rust function, changed
nothing else about what this kernel boots or how fast its IPC path runs.

## What the directives describe

Ordinary leaf functions (`fp_save`/`fp_restore`, `dispatch_on_interrupt_stack`) get the same CFI a
compiler would emit: nothing architecture-specific to say. The two cases worth naming:

A context switch is a function that returns somewhere else, and needs no special CFI at the
swap. `switch_to` (all three ISAs) swaps its stack-pointer register to a *different thread's*
stack mid-function. `.cfi_def_cfa_offset N` defines the CFA as "the CURRENT stack-pointer register
plus N" -- a live formula, not a frozen address -- and `next_context` is, by construction, exactly
the value that thread's own identical prologue (or a synthetic `Context::for_*_thread` frame) left
behind. So one continuous CFI program, never split, correctly describes both sides of the swap. Fake
first-run trampolines (`thread_trampoline` and its user-mode and x86_64 twins) mark the return
column `.cfi_undefined`: there is no real caller, and `context.rs` already says so in prose ("no
caller: the backtrace ends here") -- the directive says the same thing to the unwinder.

**A trap is not a call, and two of the three architectures cannot fully describe it.** Vector/trap
entries mark `.cfi_signal_frame` and recover every general-purpose register correctly everywhere.
Whether they can *also* recover the interrupted PC -- and so keep an unwind going past the trap
boundary, into whatever kernel code took the interrupt -- is where the three architectures diverge.
This is recorded here rather than only in a note nobody reads twice:

- x86_64 can, in full. The hardware trap frame puts the interrupted RIP and RSP at fixed
  offsets from the software frame's own CFA. The SysV DWARF register mapping already treats
  register 16 (RIP) as the ordinary return-address column and register 7 (RSP) as an ordinary
  describable register. `isr_common` states `.cfi_offset 16, 16` / `.cfi_offset 7, 40` and the
  unwind genuinely continues past the trap.
- **AArch64 has a spec-correct answer that the tool does not honor.** The interrupted PC lives in
  `elr_el1`, and AArch64's own DWARF register mapping anticipates exactly this case: register 33 is
  `ELR_mode`, defined for describing an asynchronously-created frame. `vectors.s` states
  `.cfi_return_column 33` / `.cfi_offset 33, -24` -- correct per the ARM DWARF spec, and inert with
  the tool this project actually runs: GDB hardcodes column 30 (`x30`) as the AArch64 return-address
  column and does not consult `.cfi_return_column`
  ([sourceware.org/pipermail/gdb/2023-January/050488.html](https://sourceware.org/pipermail/gdb/2023-January/050488.html)).
  The honest choice, made here, is `.cfi_undefined x30` at the boundary rather than repurposing a
  column GDB does consult to smuggle `elr_el1` through it. That would make the unwind work today at
  the cost of `p $lr` in that frame silently lying about the real `x30` register.
- **RISC-V cannot, and has no spec-level answer to reach for.** The interrupted PC lives in `sepc`,
  and unlike AArch64, the base RISC-V DWARF register mapping has no reserved pseudo-register for a
  CSR at all. `trap.s`'s `.cfi_undefined ra` at the trap boundary is not a workaround for a tooling
  gap the way AArch64's is. It is the only currently-describable answer, and closing it would need a
  convention this project cannot mint unilaterally -- a project-local DWARF register number would
  mean nothing to a debugger built against the standard mapping.

**`image_header.s` is data, not code, and stays that way on purpose.** It is the arm64 Image header:
a 64-byte struct a bootloader reads, with one instruction (`b _boot`) grafted onto its front so the
entry point can also be byte 0 of it. It carries no CFI, `.type`, or `.size` -- function directives
would claim it is a function with a frame, which is false, and "wrong CFI is worse than none" is the
standing rule this milestone was built under. A `CFI-EXEMPT:` comment says so in the file itself,
and `script/lint`'s new check (below) honors that marker rather than special-casing the filename.
riscv64's `_start` carries the identical Linux Image header shape and gets the identical treatment.

## The proof: before and after, under a real debugger

Captured with GDB 17.2 against two builds of the identical source tree -- commit `ece6d72c`
(immediately before this milestone) and this branch's tip -- on aarch64, the only architecture
`cargo xtask gdb` currently drives. Full transcripts are in `notes/cfi-unwind.md`'s appendix
"Before and after: the GDB transcripts".

Before, breaking on `switch_to` and asking for a backtrace. GDB's own frame-pointer fallback
(there was no CFI at all to consult, the compiler's included, because of the discard line above)
correctly walked four real Rust frames using the AAPCS64 `x29` chain ordinary functions still build
regardless of CFI. Then it walked into `exception_vectors`, which builds a raw trap frame rather
than a conventional `x29` link -- and looped, printing the identical bogus frame past #9976, three
minutes and forty-five seconds before being killed by hand. This is the sharpest evidence in the
whole milestone that "no CFI" is not merely "less information": GDB's own fallback trusted a
hand-written function that looked frame-pointer-shaped and was not, and had nothing telling it when
to stop.

After, the same breakpoint, on a kernel thread resumed out of `sched::ipc_recv` by a timer
preemption: the backtrace walks seven real Rust frames -- `switch_to -> schedule -> ipc_recv ->
system_call::invoke -> system_call::dispatch -> exception_body -> exception_dispatch` -- GDB labels the trap
frame `<signal handler called>` (recognizing `.cfi_signal_frame`), and stops there with "frame did
not save the PC". That stop is the honest limit the AArch64 section above describes, not a shortfall
this milestone left in: the directive that would let GDB go one frame further exists in `vectors.s`
and GDB does not read it.

## The gate, and what it does and does not prove

`script/lint` gained a check (`==> hand-written asm carries CFI`): a `.s` file under
`kernel/src/arch/` that exports a `.global` symbol and contains no `.cfi_startproc` fails the build,
unless it carries a `CFI-EXEMPT:` marker. File-granularity, matching the reach `script/citations`
and the roadmap's own `## Revisit` check are honest about for their own claims: it proves CFI is
*present*, never that it is *correct*. The evidence for correctness in this milestone is the GDB
transcripts above, not the gate.

## BUGS

- **RISC-V's trap boundary has no forward-looking directive to offer**, unlike AArch64's dormant-but-
  correct `ELR_mode` one. Not this milestone's to fix; it would need an ecosystem-level DWARF
  convention, not a project-local one.
- AArch64's spec-correct `ELR_mode` column is unconsumed by GDB today. Re-test if GDB ever gains
  AArch64 return-column support (cited above); the directive costs nothing to leave in place.
- **riscv64 and x86_64 do not strip CFI from what QEMU actually boots.** Neither port has a
  flat-image/objcopy step at all; QEMU's `-kernel` loads each ELF's `PT_LOAD` segments directly. The
  CFI (a few hundred bytes to ~30 KiB depending on the build) rides into guest RAM harmlessly. If
  either port grows a real flat-image path this is the thing to revisit.
- `cargo xtask gdb` is aarch64-only, so the live-GDB evidence above is aarch64-only too. x86_64's
  and riscv64's CFI is built and inspected with `llvm-objdump --dwarf=frames` but not exercised
  against a live GDB session. The FDE listings are not in `notes/cfi-unwind.md`; its BUGS section
  says the inspection is recorded in this milestone's pull request.

- The gate is a presence check, at file granularity. That is stated above and repeated here, because
  the limit is easy to forget once the check is green. It cannot confirm every function in a
  multi-function file has its own directives, or that an offset is right rather than merely present.

## Follow-on

- **Recorded.** A `cargo xtask gdb --arch riscv64|x86_64` (or equivalent) would let the live-GDB
  evidence this milestone gathered for aarch64 be gathered for the other two ports without a
  hand-rolled QEMU `-s -S` invocation each time. In `notes/cfi-unwind.md`'s BUGS section, beside the
  finding that today's tooling only reaches aarch64.

## Index row

Twelve hand-written `.s` files under `kernel/src/arch/` (thirteen counting `x86_64/fp.s`, missing
from the original list) get real call-frame information. It is `.cfi_undefined` where a function
genuinely has no caller rather than an invented frame, `.cfi_signal_frame` at trap entries, and a
`switch_to` on every architecture whose CFA formula needs no special handling at the stack swap.
The formula is expressed in terms of the live stack-pointer register. The bigger find was underneath
it: all three link scripts were discarding `*(.eh_frame*)` outright, which had been hiding the
*compiler's* CFI for every ordinary Rust function, on every architecture, since the first commit,
not only this milestone's additions. Fixed after two rejected placements (`NOLOAD` drops the bytes,
`INFO` breaks a relocation) by moving `.eh_frame` past where `__image_size` is already computed,
verified byte-for-byte against a build of the prior commit on both `__image_size` and the flat
aarch64 boot image. `script/fastpath-footprint` is byte-identical before and after. Proved live: a
GDB backtrace that used to loop past 9976 frames of `exception_vectors ()` now walks seven real Rust
frames through a resumed thread and a trap. It stops exactly where AArch64's DWARF gap and RISC-V's
absent one say it honestly must.
