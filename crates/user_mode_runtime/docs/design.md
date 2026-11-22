# The user-mode runtime: design

`user_mode_runtime` is what every user-mode program of the kernel links, on all three
architectures. User mode is `EL0` on aarch64, U-mode on riscv64 and ring 3 on `x86_64`. The crate
holds one system call wrapper, `invoke`, and the calls every program builds on it: `send`,
`receive`, `call`, `exit` and `require`. Each wrapper discharges `invoke`'s safety obligation once,
so a call site does not repeat it. This page carries the design the crate's doc comments link to,
and [time.md](time.md) covers the clock.

## What lives here, and what does not

The opt-in modules (`mapped_window`, `initial_ramdisk`, `virtual_io`, `entropy`, `socket_frame`,
`child_stub`) each hold one invariant that several programs had copied verbatim. A library may hold
a body that every binary would otherwise copy; only what must differ per binary stays out. They are
modules rather than crate-root items because only a program that owns the matching capability has a
use for them.

A `#[panic_handler]` cannot live here. Exactly one may exist per linked binary, and a library that
defined one would force it on every binary that links it. The trap underneath it can live here, so
`trap` is in the crate and `panic_handler!` expands to a handler in each binary that asks. A program
with its own handler does not invoke the macro. Device helpers stay in the drivers that own them.

A system call runs only in a user-mode process under the kernel, so the doc examples that make one
are `no_run` and only type-checked. The logic that runs anywhere (the window's bounds, alignment
and copies, the frame accessors, the reply decoder, the link script's base address and the child
stubs' encodings) has host tests.

## Three system call conventions, one surface

aarch64 uses `svc #0` with the number in `x8` and arguments in `x0..x5`. RISC-V uses `ecall` with
`a7` and `a0..a5`. `x86_64` uses `syscall` with `rax` and `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`. All
three return in the first argument register, and every signature and `moho_abi` constant is identical.

`x86_64` costs more in three places. `syscall` clobbers `rcx` and `r11` at every site, `rdtsc`
answers in two halves, and there is no architected counter frequency, so `counter_hertz` reads a
page the kernel fills and panics when the page is zeroed or unrecognized.

## The trap

`trap6` is the one place the trap instruction appears. It passes the number and six words in, and
reads six words back. The kernel writes its result into the first register on the way out of every
system call, and some methods write up to the sixth, so no caller may declare a register as
surviving the trap. A yield comes back with `x0 = 0`, and a retry loop that kept its arguments live
across one would send the wrong call.

For `SYSTEM_CALL_INVOKE`, the capability, the method and three arguments go in the first five
registers, and the reply comes back in the same five. The sixth register is an output and goes in
as zero. The kernel writes argument register 5 (`x5`, `a5`, `r9`) with a dead child's label when it
delivers a death message to a plain `RECEIVE`, and leaves it alone otherwise. Declared as an input
only, the register would let the compiler keep a live value in it across the trap. Zero in makes
`receive_message` read `0` for anything the kernel did not stamp.

On `x86_64`, two operands have no counterpart on the other architectures, and the instruction
forces both. `syscall` writes the return address into `rcx` and the caller's `RFLAGS` into `r11`,
so both are declared clobbered. A version without them compiles, then corrupts whichever local the
register allocator had put there. `r10` carries the fourth word in place of the `rcx` the C calling convention uses, for
the same reason. `options(nostack)` still holds, because `syscall` does not push, which is also why
the kernel's entry path parks `rsp` by hand.

`raw_system_call` is `trap6` made public for one kind of caller: a fuzzer of the surface a confined
process can reach. It draws the number and all six words at random, unknown numbers included, which
no typed wrapper can express.

## Entering a process

The kernel enters a new thread with the stack pointer at the top of its stack page, 16-byte aligned.
On aarch64 and riscv64 that is what a function expects. On `x86_64` the psABI expects
`rsp % 16 == 8` at a function's first instruction, because a `call` has just pushed the return
address. A plain `extern "C" fn _start` therefore runs every frame eight bytes off. Without vector registers in
use nothing faults, so the mistake stays latent until a local needs 16-byte alignment and lands eight
bytes off.

`program_entry!(start)` writes `_start` for a program: a one-`call` shim on `x86_64`, as the
standard library's entry for the target has, and a plain function elsewhere. Every `no_std` program
uses it rather than writing `_start` by hand. A program can check the alignment from inside itself,
and on `x86_64` that check fails without the shim.

## Ending a process

`exit` and `trap` are the two ways a program ends, and they report differently. `exit` reports
`EVENT_EXIT` to a supervisor and `trap` reports `EVENT_FAULT` on the fault endpoint. A supervised
child that traps is therefore legible as having failed, and one that exits is not. A panic must take
the trap path, or it misreports what happened.

The trap instruction is `brk #0` on aarch64, `ebreak` on riscv64 and `ud2` on `x86_64`. The
`x86_64` arm's `SAFETY` comment says why `int3` cannot be used from ring 3 here. Both functions end
in a spin that never runs and exists only to satisfy `-> !`. `trap` spins rather than calling
`exit`, because `exit` would turn an impossible return into a clean-looking death.

`require` is the one way a program says no. There is no error value to return, and limping past a
failed check turns a detected mistake into an undetected one. A failed check must look like a
broken program, because it is one, so `require` traps at the call. The panic handler ends in `trap`
as well.

Every trap in a program prints the same fault line. The kernel prints the faulting `pc`, and the
`pc` of a trap is always the one breakpoint instruction, shared by every caller. The return address
is in `x30` (aarch64), `ra` (riscv64) or on the stack (`x86_64`) when the kernel takes the fault.
Printing it beside `pc` would name the failing call, and that is a change to the kernel's fault
printers rather than to this crate. Until then, a program can make the failure a data abort at an
address derived from the return address, because the kernel prints the faulting address too.

## Memory a process was handed

### Mapped windows

`MappedWindow` is one page at a fixed virtual address, read and written by volatile access. The page
is either a direct memory access (DMA) page a driver's wiring maps before `_start` runs, or a shared frame the program maps
itself with `PageFrame::MAP`. The invariant every hand-written copy asserted was that an offset lies
inside the one page the kernel mapped. Nothing checked it, so a wrong offset constant was a silent
out-of-bounds access. The type takes the trust once, at construction, and turns staying inside the
page into an `assert!` at each access.

`read_volatile::<T>` and `write_volatile::<T>` take the sealed `WindowInteger` types and panic on a
misaligned address. All are native-endian, which is little-endian on every architecture this crate
targets.

`copy_within` serves a caller moving a block of its own pixels, a terminal's scroll. A
word-at-a-time loop at opt-level 0 costs a call, a bounds check and a volatile access per word:
349,440 of them for one scroll of a 1120x624 scanout. The copy is not volatile and is safe anyway,
because no reference is formed. What a peer or device reads afterwards is ordered by the fence and
the call the caller makes to publish the bytes, as for its volatile writes.

`copy_to_slice` and `copy_from_slice` move a block between the window and ordinary memory, for the
same reason. A network stack moves every received byte through a DMA buffer and a socket's frame,
and at opt-level 0 a byte loop through single-byte accessors set its rate. Each checks the range
once, then copies without volatile access, with a compiler fence on both sides so the copy stays
between the caller's ownership read, its barrier and its doorbell.

`as_slice` and `as_mut_slice` serve a caller that hands the whole page to a parser or serializer.
They return an ordinary reference, not a volatile access. The callers get exclusive access from the
protocol's turn-taking: one side stages the page, then hands it off by a `SEND` or `receive`. A
process can have threads, so the claim covers this process's own threads too, which is why both are
`unsafe`.

### Barriers

`virtual_io_ring_barrier` orders a virtio driver's accesses to a virtqueue against the device's.
aarch64 and riscv64 are weakly ordered, so both the machine and the compiler need a real
instruction. `dmb ish` orders the inner shareable domain, where a coherent DMA agent observes.
RISC-V takes one full `fence`, the conservative choice.

`x86_64` is total store order (TSO). The orderings this barrier is asked for are store-store (descriptor before index,
index before notify) and load-load (used index before payload). The machine already guarantees both
for the write-back memory a coherent device shares. The store-load case TSO does not give is not
reachable here, because every notify and register access in these drivers leaves through a system
call instruction, which is serializing. So on `x86_64` the compiler is the whole exposure, and
`compiler_fence` is the whole answer.

The function exists once because the copies it replaced each had an aarch64 and a riscv64 arm and
no third. On `x86_64-unknown-none` each compiled to an empty body, and an empty function raises no
warning. A fourth architecture now fails to build, ending in `compile_error!` rather than a
fallback.

`doorbell_barrier` is the stronger barrier, for a driver whose doorbell is a direct store to a
device-typed page in its own address space with no system call in between. It is `dmb sy`, `fence`
and `mfence`. It must not be merged with `virtual_io_ring_barrier`, whose drivers notify through a
system call.

### The initial ramdisk

An initial ramdisk is a filesystem image the bootloader leaves in RAM before the kernel starts. It
breaks a circle: the first user-mode program needs a file, a file needs a file system, a file
system needs a block driver, and the block driver is itself a user-mode program. The kernel finds
the region by reading `linux,initrd-start` from the device tree. Those spellings stop at the
boundary.

The kernel maps the archive read-only at `INITIAL_RAMDISK_VIRTUAL_ADDRESS` before `_start`. `bytes`
keeps an `unsafe` call site, unlike `MappedWindow`, because it returns a whole `'static` slice. No
type can validate its length, so the obligation stays a sentence rather than an `assert!`.

### The socket frame

`socket_frame::Frame` reads `socket_contract`'s layout from one mapped page. Each address space maps
its own copy wherever its image leaves room; the two sides share the layout, never the address. So
the type is constructed with its base. It lives here rather than beside the offsets in
`socket_contract` because it is built on `MappedWindow`, and a contract crate never links the
runtime: the dependency runs from this crate to that one.

## Clients of services

`entropy::fill_random_bytes` is the client half of `entropy_contract`. Entropy is a capability: a
program reads random bytes only through an endpoint it was granted. The contract holds the request
and reply encodings, pure and dependency-free. A fill function has to `CALL` an endpoint, so putting
it in the contract would make the contract depend on this runtime. The loop, where the refusals
live, is `entropy_contract::fill_with`, host-tested there, and this module supplies the `CALL` as a
closure. The standard library's own fill cannot be shared: it lives inside `std`, these callers are
`no_std`, and it panics where they must refuse quietly.

The `virtual_io` module wraps the four `VirtualIo` capability methods. The `SAFETY` argument at
each raw call site was `invoke`'s own contract and nothing more. `moho_abi::virtual_io` states what the
kernel checks: `WRITE_REGISTER` refuses the queue-address and notify registers, and `SETUP_QUEUE`
and `NOTIFY` are bounds-checked against the DMA region and its high-water mark.

## A child's whole program

`child_stub::SEND_THEN_EXIT` is the smallest whole program, hand-assembled per architecture. It
sends one word on slot 0, then exits. A parent copies it into a frame it maps writable in its own
space, then aliases that frame as `MAP_CODE` into each child, and the kernel makes the instruction
cache coherent at that map. Benchmarks that spawn many children share the one copy.

The `x86_64` stub is bytes, because x86 instructions are not a fixed width. Each immediate uses the
32-bit form, `mov r32, imm32`, which zeroes the upper half of the register and is the only encoding
that keeps a small constant to five bytes. `r10` and `r8` need a register-extension (REX) prefix (`0x41`) to be named at
all, so those rows are six bytes. The fourth and fifth argument registers did not exist on the 386
whose opcode map this still is.

## Address spaces

`map_page_frame` replaces a page the kernel wired in at spawn. A spawn-time mapping has no
capability behind it, so nobody can narrow it, hand it on or take it back. A frame the process
mapped itself is in the revocation database, and `PageFrame::REVOKE` can pull it out from under the
process.

`address_space_layout_into` carries the same obligation one address space over. It can change an
address space under code that assumed a mapping was fixed, but `map_page_frame` already accepts
that risk for the caller's own space. The kernel makes the same checks: the address-space
capability's `WRITE`, the frame's rights against the mode, and the address. A caller racing what
the call changes has a correctness problem in its own code, not a Rust-safety obligation a wrapper
could check.
