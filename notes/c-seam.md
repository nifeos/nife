# Running a foreign language: the C seam

Milestone 36, DECISIONS §31. Built 2026-07-29, both ISAs, in QEMU.

The question that started it: **can a userspace service be written in another language, the way a
monolith would have put a C FAT32 driver in the kernel?** The answer is yes, and this milestone is the
smallest thing that proves it. A memory-unsafe C component, compiled by bare-metal clang, running
confined, faulting on a deliberate out-of-bounds write, and restarted by its supervisor.

The component is throwaway on purpose. What is being de-risked is the seam: the toolchain, the
linkage shape, the libc, and the confinement proof. That has to happen before a real foreign component
depends on it (libghostty-vt, the display ladder's later rung). If the seam has a problem, you want
to find it with 150 lines of disposable C rather than half way into a port. The detail this page has
no room for is in [`c-seam/`](c-seam/README.md).

## Why memory-unsafe C is the best demonstration, not the worst

It looks backwards at first. The project's thesis (DECISIONS §14) is a verified-Rust capability
microkernel, and here we are linking C.

But read the thesis again: *a verified core that confines unverified workloads.* The workload was
always going to be unverified. C is simply the most unverified workload available: no bounds checks, no
borrow checker, nothing at all between a bad index and a store to whatever address it computed. The
more unverified the component, the more the confinement has to prove. So this is the sharpest
available test of the claim rather than a dilution of it.

And the contrast with a monolith stops being rhetorical and becomes arithmetic:

| | in-kernel C (a monolith) | confined C (here) |
|---|---|---|
| what a bad index reaches | any physical memory, any device register | one page it was granted |
| who notices | nobody, until something else breaks | the kernel, at the instruction |
| what recovers | reboot | its supervisor, in userspace |
| what is trusted | the C | the page tables |

The peer project Atom keeps FAT32, AHCI, and xHCI in the kernel today, which is what the left column
looks like in practice.

The other half of the argument is that isolation here does not know what a language is. Page
tables, unforgeable capabilities, the DMA validator, and the IOMMU are all language-agnostic
mechanisms. None of them has an opinion about which compiler produced the instructions they are
confining. That is exactly why a language boundary is cheap here and would be expensive in a system
whose safety came from the language.

## The shape: a Rust shell that holds everything

```text
   kernel  <--- svc / ecall --->  c_shim (Rust)  <--- C ABI: (u8*, usize) --->  c_seam.c
                                  ^^^^^^^^^^^^
                                  every capability and every syscall stops here
```

`fixtures/src/c_shim.rs` is the whole design, and what it does *not* do is the point:

- The C makes no syscalls. Not because we asked nicely, but because a syscall needs a capability
  slot number and the C has never seen one. There is no `svc`, no `ecall`, and no inline asm anywhere
  in `c_seam.c`, and there could not usefully be: an `svc` with a made-up slot number gets
  `NoSuchSlot`.
- The C holds no capabilities. It is handed a pointer and a length.
- So the foreign component **cannot widen the kernel's syscall surface** (DECISIONS §4 rule 3). A
  vendor who wanted one more syscall to make their component work would have to change the Rust shell,
  in this repository, in review.

That makes the confinement claim precisely as narrow as it should be: **the C can corrupt memory
inside a grant the shell already had, and nothing else.**

It is also the same sans-IO shape RedoxFS's `Disk` trait already uses (DECISIONS §27): a component
with the logic and none of the I/O, wired up from outside. Here the boundary is a language rather than
a trait, and the shape did not have to change.

### What crosses, and what cannot

Crossing:

```c
uint32_t c_seam_transform(unsigned char *grant, size_t len);   /* honest work */
void     c_seam_overrun  (unsigned char *grant, size_t len);   /* the bug, one byte past */
void     c_seam_wild     (unsigned char *grant, size_t len);   /* the bug, a page past */
```

Scalars and buffers. Not crossing, and each of these is a seam decision this spike did not have to
make: structs by value, callbacks from C into Rust, ownership transfer, an error type, varargs, C++
(name mangling, exceptions, static initializers), bitfields, enum widths.

The layout of the shared page is agreed by a comment in both languages (`fixtures/c/c_seam.c`'s
`C_SEAM_*` defines and `crates/c_seam`'s constants) rather than by generated bindings. For one page
of bytes that is the right trade; for a real API it would not be, and the honest reason to say so here
is that "we will generate bindings when there is an API worth generating" is a plan and "we forgot"
is not.

## The libc question, answered by tier

The roadmap's milestone-36 file records three tiers of C dependency (design/roadmap/0036-foreign-component.md, "The line
this does not cross"):

1. Freestanding. No libc. littlefs allocates nothing; libghostty-vt needs a
   supplied allocator
   ([the graphical terminal runs full-screen programs](../design/roadmap/0705-the-graphical-terminal-runs-full-screen-programs.md)).
2. A handful of symbols. Shim what the component actually references. This is what this spike
   proves.
3. **Full POSIX.** `open`, `fork`, `socket`, threads. Needs a real libc port, which is DECISIONS §15's
   "later, if ever" road and the one Redox took with relibc. Not walked here, and saying so is what
   keeps this from becoming that project.

### What the object demands (five), and what the linker demands (two)

Discovered by letting the build fail and reading the error, which is the only honest way to learn what
a foreign object file needs. `llvm-nm --undefined-only` on the compiled object:

```
$ llvm-nm --undefined-only c_seam.o          # identical for aarch64 and riscv64
                 U free
                 U malloc
                 U memcpy
                 U memset
                 U strlen
```

Five, and *only* five. Worth stating explicitly what is absent, because each would have been a
finding: no compiler-rt helper (`__muldi3` and friends), no `memmove` conjured out of a struct copy,
no `__stack_chk_fail`. Checked at `-O0`, `-Os`, `-O2`, `-O3`, with and without `-fno-builtin`, and
even with `-fstack-protector-strong` (this component has no stack arrays, so no canary was inserted;
`build.rs` passes `-fno-stack-protector` anyway, so that stays a decision rather than luck).

Then delete all five shims and let the linker speak:

```
rust-lld: error: undefined symbol: malloc
rust-lld: error: undefined symbol: free
```

Two. `memcpy`, `memset`, and `strlen` are already there, weakly, from Rust's own
`compiler_builtins` for the bare-metal targets, which is what lets a `no_std` Rust binary link at all.
The C component's needs and the Rust runtime's needs overlap almost exactly. That is a reusable
finding: tier two is smaller than it looks, because the freestanding Rust runtime has already paid for
most of it.

### The trap: a Rust `memcpy` shim calls itself

Do not define the three symbols the runtime already owns. Here is why, and it cost a debugging session.

The obvious Rust implementation of `memcpy` is `core::ptr::copy_nonoverlapping`. That lowers to a
call to `memcpy`. So the shim calls itself, forever.

The symptom does not look like recursion. It looks like this:

```
  user thread ... killed: Data abort from a lower EL
    pc 0x4003a8   far 0x4fcff0   user sp 0x4fcff0   esr 0x92000047
```

A store fault at exactly `sp`, sixteen bytes below the bottom of the stack, in
`core::ptr::const_ptr::is_aligned_to` (a debug build spends a frame per `core::ptr` helper, so the
backtrace is all allocator plumbing). That reads unmistakably like "the stack is too small". It is
not: quadrupling the stack from 16 KiB to 64 KiB reproduced it exactly, sixteen bytes below the new
bottom, which is the tell. `compiler_builtins` avoids the trap by compiling with `#[no_builtins]`; a
program crate cannot.

The fix is also the smaller answer: shim `malloc` and `free`, and let the runtime keep the rest.

### Where `malloc` comes from

Milestone 27's untyped-backed `GlobalAlloc` (`memory_region_heap::MemoryRegionHeap`, DECISIONS §22), wired to the
untyped region the instance was built in. Three consequences, all of them the point:

- The C heap is the process's own memory budget. There is no ambient allocator to leak into.
- A C leak exhausts that instance, visibly, and reaches no other process's memory.
- The single `MemoryRegion::DESTROY` that reaps the corpse reclaims the heap along with everything else, so
  a restart loop is not a leak.

One real cost of the C ABI shows up here and cannot be shimmed away: `free(p)` carries no size,
while `GlobalAlloc::dealloc` requires the original `Layout`. So `malloc` stores the size in a 16-byte
header in front of the pointer it returns (16 because that is also the alignment `malloc` must
guarantee for any C type, so the payload stays aligned for free). Every C-to-Rust allocator bridge pays
this; it is worth knowing it is a property of `free`'s signature rather than a shortcut.

## The toolchain: bare-metal clang, one compiler for two ISAs

`fixtures/build.rs` compiles `fixtures/c/c_seam.c` and hands the one object to the linker for the
`c_shim` binary only, so every other program links exactly as before. clang is found from a known
list: `$CRICKER_CC`, then Homebrew's llvm keg by path, then `clang` on `PATH`. Each candidate must
have both the AArch64 and RISC-V backends (`clang -print-targets`), so Apple's clang is rejected on
purpose: the parity gate of §19 (architectural parity), applied to the toolchain. `script/bootstrap` installs one, and without
one `fixtures/build.rs` panics with installation instructions.

The load-bearing flag is `-mgeneral-regs-only` on aarch64. Without it clang vectorizes the byte
loops, and FP/SIMD registers are unusable at EL0 here and unsaved across a switch. On riscv64 the
ABI is `lp64`, not `lp64d`, to match `riscv64imac`. [`the-toolchain.md`](c-seam/the-toolchain.md)
has the full flag list and the reasons.

## The confinement test, which is the milestone

`kernel/src/user::c_seam_tests`, both ISAs. Three processes are involved:

```text
  c_confiner  budget + initrd + report + the C component's supervision endpoint
    |        holds GRANT, WITNESS_RO, and WITNESS_FAR mapped read/write in ITS OWN space
    |
    +-- c_shim (instance N)   report endpoint, its own region's untyped, GRANT rw, WITNESS_RO ro
          |
          +-- c_seam.c        a pointer and a length
```

`c_confiner` is builder, supervisor, and checker at once. The checker role has to be a separate
address space from the C component, because a checker *inside* the faulting address space could only
report what that address space could see, which is exactly the thing under suspicion.

The address-space layout gives the checker two witnesses. `WITNESS_RO` is the same physical
frame, mapped read-only into the component and read/write into the confiner. `WITNESS_FAR` is a
different frame at the virtual address one page further on. An overrun into the first proves the
store did not happen; a wild write to the second proves a virtual address means nothing outside the
address space that owns it.

Five assertions are made from outside, after the component is dead. It faults. The fault address
equals the address the C code computed. Both witnesses are intact, byte by byte. And a restart
computes the right answer, checked against an independent Rust implementation. The control makes
them mean something: each misbehaving function stores inside its grant first, and that store must
be visible to the confiner. Both ISAs report the exact faulting address. aarch64's `ESR_EL1` also
tells a permission fault from a translation fault; RISC-V's `scause` does not, which would matter to
a userspace pager. [`the-confinement-test.md`](c-seam/the-confinement-test.md) has the layout, the
assertions in full and the transcripts.

## What the supervisor had to hold, and what it would rather have held

This is the part that feeds an open design fork, so it is written down precisely.

`c_confiner` restarts its child, and to restart it must first reap it: the corpse is
dead-until-reaped (§26.4), and its region stays pinned until somebody says otherwise. Reaping is
`MemoryRegion::DESTROY`, which needs `WRITE` on the region. **`WRITE` on a region is also exactly what
builds a process from it** (`RETYPE`, `RETYPE_OBJ`, `SPLIT`). There is no narrower right.

- What it had to hold: a full-rights untyped budget, for its whole life. From that it can split
  regions, make frames, make address spaces, make threads, make endpoints, and destroy regions. It
  needs the last one. Everything else came attached.
- What it would have preferred: a reap-only right over one region it did not create. That is
  enough to collect a corpse and return its pages, and not enough to build anything.
- The alternative that exists today, and why this milestone did not use it: milestone 22 phase
  B.2's proxy. Its supervisor (`sub_server_supervisor`) holds no memory at all and asks a construction sub-server
  (`spawner`) to reap on its behalf, so policy and authority sit either side of an IPC boundary. That
  is the right answer for a system's the progenitor, where the whole point is that progenitor can no longer build. It
  is the wrong answer *here*, because it moves the requirement behind an IPC hop and the requirement is
  the interesting part. A spike should make its requirements visible.

Nothing was invented to work around this: no new capability, no new method, no new syscall. The
concrete requirement, for whoever decides the fork, is one sentence: **a supervisor needs exactly
`DESTROY` on one region it did not create.** Whether that is a rights bit split out of `WRITE` or a
distinct `Untyped::REAP` changes the rights model and the syscall surface, which makes it a decision
rather than an implementation detail. DECISIONS §26's phase-B block already records it as one.

## Honest caveats: what a spike does not prove

The whole value of doing this cheaply is that it fails early. The corollary is that it does not prove
much about the expensive thing.

- A throwaway component is not a vendor component. Untested here: a real build system (this is one
  `clang -c`, not autotools, CMake, or `build.zig`), multiple translation units and their link order,
  headers we do not control, and API churn across upstream versions. Milestone 29's libghostty-vt is a
  tier-one (freestanding) component by design, which is the cheapest possible next step up from
  here, and that sequencing is deliberate rather than lucky.
- The libc list is this component's list. A component wanting `errno`, `assert`, `stdio`, locales,
  `setjmp`, floating point, or thread-local storage asks a different question, and some of those
  (`stdio`, TLS) are tier-three questions wearing a tier-two coat. The rule from DECISIONS §31 stands:
  if a symbol cannot be answered without POSIX semantics, that is a finding, and the answer is to
  choose a component that does not need it.
- The C ABI surface is one function shape. See "what crosses" above.
- **Nothing here is verified.** DECISIONS §18's proof toolchain does not reach C and never will. The C
  is confined, not correct. That is the whole point, and it is also the limit of the claim: this
  milestone says nothing about the component's behavior, only about its blast radius.
- The grant is one page and the workload is trivial. No claim is made about the cost of the seam at
  volume: no benchmark, no copy-avoidance story, no measurement of what a C component costs versus a
  Rust one. A real component gets that treatment when there is a real component.

## See also

- [Rust `std` on the native ABI](std.md): the heap under this `malloc`.
- [Milestone 36 (a foreign-language component)](../design/roadmap/0036-foreign-component.md): the
  libc tiers.
- [Supervision](supervision.md): the fault endpoint this leans on, and the five-word message.
- [Trusted init](trusted-init.md): milestone 22 phase B.2's proxy shape, the alternative to the
  authority this confiner holds directly.
- [The native ABI](abi.md): the capability contract the shell speaks and the C cannot.
- [The framebuffer contract](framebuffer-contract.md): the two-witness discipline this test borrows.
