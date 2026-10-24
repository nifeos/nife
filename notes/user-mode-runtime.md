# The user-mode runtime

*Name: provisional, minted 2026-10-23 (UTC) by comments/929-crates-09.*

`crates/user_mode_runtime` is what every nife userspace program links, on all three architectures.
User mode is `EL0` on aarch64, U-mode on riscv64 and ring 3 on `x86_64`. The crate holds one
syscall wrapper, `invoke`, and the calls every program builds on it: `send`, `receive`, `call`,
`exit` and `require`. Each wrapper discharges `invoke`'s safety obligation once, so a call site
does not repeat it. This note carries the design the crate's doc comments link to.

## What lives here, and what does not

The opt-in modules (`mapped_window`, `initial_ramdisk`, `virtio`, `entropy`, `socket_frame`,
`child_stub`) each hold one invariant that several programs had copied verbatim. That is the shape
§94 (what may live in a userspace library, and what must be per-binary) allows in a library: a body
copied into every binary is not per-binary, only its declaration is. They are modules rather than
crate-root items because only a program that owns the matching capability has a use for them.

A `#[panic_handler]` cannot live here. Exactly one may exist per linked binary, a library that
defined one would force it on every binary that links it, and `hello` wants its own. The trap
underneath it can live here, so `trap` is in the crate and `panic_handler!` expands to a handler in
each binary that asks. A program with its own handler does not invoke the macro. Device helpers
stay in the drivers that own them.

Nothing here runs on the host. Every function traps to the kernel, and `script/test`'s host pass
excludes this crate and its dependents; `script/lint` derives and checks that set. The doc examples
are `no_run` and only type-checked. The wire contracts layered over this crate carry runnable ones.

## Three ABIs, one surface

aarch64 uses `svc #0` with the number in `x8` and arguments in `x0..x5`. RISC-V uses `ecall` with
`a7` and `a0..a5`. `x86_64` uses `syscall` with `rax` and `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`,
as §124 (ratify the x86_64 syscall ABI) records. All three return in the first argument register,
the kernel reconciles them in `TrapFrame`, and every signature and `abi` constant is identical.

`x86_64` costs more in three places. `syscall` clobbers `rcx` and `r11` at every site, `rdtsc`
answers in two halves, and there is no architected counter frequency, so `cntfrq` reads a page the
kernel fills and panics when the page is zeroed or unrecognized.

## The trap

`trap6` is the one place the trap instruction appears. It passes the number and six words in, and
reads six words back. The kernel writes its result into the first register on the way out of every
syscall, and some methods write up to the sixth, so no caller may declare a register as surviving
the trap. A yield comes back with `x0 = 0`, and a retry loop that kept its arguments live across
one sent the wrong call (notes/job-mix/spawn-job-miscompiled-yield.md). `helpers/syscall_asm.py`
gates every userspace trap.

For `SYS_INVOKE`, the capability, the method and three arguments go in the first five registers, and
the reply comes back in the same five. The sixth register is an output and goes in as zero. The
kernel writes argument register 5 (`x5`, `a5`, `r9`) with a dead child's label when it delivers a
death message to a plain `RECEIVE`, and leaves it alone otherwise (§148 (resolves by asking the
kernel), as amended 2026-10-04). Declared as an input only, LLVM would keep a live value in it
across the trap. Zero in makes `receive_fault` read `0` for anything the kernel did not stamp.

On `x86_64`, two operands have no counterpart on the other architectures, and both are forced by
the instruction. `syscall` writes the return address into `rcx` and the caller's `RFLAGS` into
`r11`, so both are declared clobbered. A version without them compiles, then corrupts whichever
local the register allocator had put there. `r10` carries the fourth word in place of the C ABI's
`rcx` for the same reason, which is why §124 records that substitution as forced. `options(nostack)`
still holds, because `syscall` does not push. That is also why the kernel's entry path parks `rsp`
by hand (`arch/x86_64/trap.s`).

`raw_syscall` is `trap6` made public for one caller, `fixtures/src/confined_syscall_fuzzer.rs`.
It draws the number and all six words at random, unknown numbers included, which no typed wrapper
can express.

## Entering a process

The kernel enters a new thread with the stack pointer at the top of its stack page, 16-byte aligned.
On aarch64 and riscv64 that is what a function expects. On `x86_64` the psABI expects `rsp % 16 ==
8` at a function's first instruction, because a `call` has just pushed the return address. A plain
`extern "C" fn _start` therefore runs every frame eight bytes off. Without SSE nothing faults, so the
mistake stays latent until a local needs 16-byte alignment and lands eight bytes off.

`program_entry!(start)` writes `_start` for a program: a one-`call` shim on `x86_64`, as std's entry
has (`patches/std-nife/overlay/std/src/sys/pal/nife/entry.rs`), and a plain function elsewhere.
Every `no_std` program uses it, and `script/lint` refuses an `extern "C" fn _start` written by hand
under `components/`, `fixtures/` or `redoxfs_server/`. `loaded_image_check`'s alignment check is the
proof from inside a process: `image_self_checker` fails it on `x86_64` without the shim.

## Ending a process

`exit` and `trap` are the two ways a program ends, and they report differently. `exit` reports
`EVENT_EXIT` to a supervisor and `trap` reports `EVENT_FAULT` (`kernel/src/sched.rs`, §26 (the fault
endpoint)). A supervised child that traps is therefore legible as having failed, and one that exits
is not. A panic must take the trap path, or it misreports what happened.

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
Printing it beside `pc` would name the failing call, and that is a change to the three
`arch/*/exceptions.rs` fault printers rather than to this crate. Until then, a program can make the
failure a data abort at an address derived from the return address, because `far` is printed.

## Time

The counter, its frequency, monotonic nanoseconds, the cycle counter and the current CPU each
read a register or a kernel-filled page with no syscall. Why each target differs, and what each
refuses, is in [the time appendix](user-mode-runtime/time.md).

## Memory a process was handed

### Mapped windows

`MappedWindow` is one page at a fixed virtual address, read and written by volatile access. The page
is either a DMA page a driver's wiring maps before `_start` runs, or a shared IPC frame the program
maps itself with `PageFrame::MAP`. The invariant every hand-written copy asserted was that an offset
lies inside the one page the kernel mapped. Nothing checked it, so a wrong offset constant was a
silent out-of-bounds access. The type takes the trust once, at construction, and turns staying
inside the page into an `assert!` at each access.

The named accessors (`r8`, `w8`, `r16`, `w16`, `r32`, `w32`, `r64`, `w64`) keep the vocabulary the
migrated callers used, so migrating a file changed only where they are defined. All are
native-endian, which is little-endian on every architecture this tree targets.

`copy_within` serves a caller moving a block of its own pixels, the terminal's scroll. A
word-at-a-time loop at opt-level 0 costs a call, a bounds check and a volatile access per word:
349,440 of them for one scroll of the 1120x624 scanout. The copy is not volatile and is safe anyway,
because no reference is formed. What a peer or device reads afterwards is ordered by the fence and
the call the caller makes to publish the bytes, as for its volatile writes.

`as_slice` and `as_mut_slice` serve a caller that hands the whole page to a parser or serializer.
They return an ordinary reference, not a volatile access, as every migrated call site already did.
The callers get exclusive access from the protocol's turn-taking: one side stages the page, then
hands it off by a `SEND` or `receive`. Since milestone 812 (`std::thread::spawn` runs real threads)
a process can have threads, so the claim covers this process's own threads too.

### Barriers

`virtio_ring_barrier` orders a virtio driver's accesses to a virtqueue against the device's. aarch64
and riscv64 are weakly ordered, so both the machine and the compiler need a real instruction.
`dmb ish` orders the inner shareable domain, where a coherent DMA agent observes. RISC-V takes one
full `fence`, the same conservative choice `kernel::arch::direct_memory_access_write_barrier` makes.

`x86_64` is TSO. The orderings this barrier is asked for are store-store (descriptor before index,
index before notify) and load-load (used index before payload). The machine already guarantees both
for the write-back memory a coherent device shares. The store-load case TSO does not give is not
reachable here, because every notify and register access in these drivers leaves through a syscall
instruction, which is serializing. So on `x86_64` the compiler is the whole exposure, and
`compiler_fence` is the whole answer.

The function exists once because the copies it replaced each had an aarch64 and a riscv64 arm and
no third. On `x86_64-unknown-none` each compiled to an empty body, and an empty function raises no
warning, so `script/lint`'s `x86_64` user pass could not catch it (notes/architecture-list-sweep.md,
finding 9). A fourth architecture now fails to build. `entropy_backend`'s backend ladder is the
in-tree precedent for ending in `compile_error!` rather than a fallback.

`doorbell_barrier` is the stronger barrier, for a driver whose doorbell is a direct store to a
device-typed page in its own address space with no syscall in between. It is `dmb sy`, `fence` and
`mfence`. It must not be merged with `virtio_ring_barrier`, whose drivers notify through a syscall.
`components/src/non_volatile_memory_express.rs` was its first caller, and the `e1000e` data plane
its second.

### The initial ramdisk

An initial ramdisk is a filesystem image the bootloader leaves in RAM before the kernel starts. It
breaks a circle: the first userspace program needs a file, a file needs a filesystem, a filesystem
needs a block driver, and the block driver is itself a userspace program. The kernel finds the
region by reading `linux,initrd-start` from the device tree, and every run passes QEMU's `-initrd`.
Those spellings stop at the boundary, as design/naming.md records.

The kernel maps the archive read-only at `INITIAL_RAMDISK_VA` before `_start`, through
`kernel::user::boot_progenitor` at boot and `spawn_elf_child_builder` in the tests. `bytes` keeps an
`unsafe` call site, unlike `MappedWindow`, because it returns a whole `'static` slice. No type can
validate its `len`, so the obligation stays a sentence rather than an `assert!`.
notes/unsafe-obligations.md names the same case for `switch_user_root`.

### The socket frame

`socket_frame::Frame` reads `socket_contract`'s layout from one mapped page. Each address space maps
its own copy wherever its image leaves room; the two sides share the layout, never the address. So
the type is constructed with its base. It lives here rather than beside the offsets in
`crates/socket_contract` because it is built on `MappedWindow`, which cannot compile for the host.
`socket_contract`'s wire format is host-tested, and a dependency on this crate would drag it out of
the host pass.

## Clients of services

`entropy::fill` is the client half of `entropy_contract` (§44 (entropy is a capability),
notes/entropy.md). The contract holds the request and reply encodings, pure and dependency-free, and
the kernel's entropy service and the std PAL build on it. A fill function has to `CALL` an endpoint,
so putting it in the contract would make the contract depend on this runtime. The loop, where the
refusals live, is `entropy_contract::fill_with`, host-tested there, and this module supplies the
`CALL` as a closure. The std PAL's `fill_bytes` cannot be shared: it lives inside `std`, these
callers are `no_std`, and it panics where they must refuse quietly.

The `virtio` module wraps the four `Virtio` capability methods. The `SAFETY` argument at each raw
call site was `invoke`'s own contract and nothing more. `abi::virtio` states what the kernel checks:
`WRITE_REGISTER` refuses the queue-address and notify registers, and `SETUP_QUEUE` and `NOTIFY` are
bounds-checked against the DMA region and its high-water mark.

## A child's whole program

`child_stub::SEND_THEN_EXIT` is the smallest whole program, hand-assembled per architecture. It
sends one word on slot 0, then exits. A parent copies it into a frame it maps writable in its own
space, then aliases that frame as `MAP_CODE` into each child, and the kernel makes the I-cache
coherent at that map. `fixtures/src/os_primitives_benchmarker.rs` and milestone 168 (a multi-tasking
workload benchmark)'s `SPAWN` job share the one copy.

The `x86_64` stub is bytes, because x86 instructions are not a fixed width. Each immediate uses the
32-bit form, `mov r32, imm32`, which zeroes the upper half of the register and is the only encoding
that keeps a small constant to five bytes. `r10` and `r8` need a REX prefix (`0x41`) to be named at
all, so those rows are six bytes. The registers the `syscall` ABI's fourth and fifth arguments use
did not exist on the 386 whose opcode map this still is.

## Address spaces

`map_page_frame` replaces a page the kernel wired in at spawn. A spawn-time mapping has no
capability behind it, so nobody can narrow it, hand it on or take it back. A frame the process
mapped itself is in the revocation database, and `PageFrame::REVOKE` can pull it out from under the
process (notes/frames.md).

`address_space_map_into` carries the same obligation one address space over. It can change an
address space under code that assumed a mapping was fixed, but `map_page_frame` already accepts
that risk for the caller's own space. The kernel makes the same checks: the address-space
capability's `WRITE`, the frame's rights against the mode, and the address. A caller racing what
the call changes has a correctness problem in its own code, not a Rust-safety obligation a wrapper
could check.
