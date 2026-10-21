# `no_std` and bare metal

## The standard library is three crates, not one

| Crate | Needs | Gives you |
|---|---|---|
| **`core`** | nothing at all | `Option`, `Result`, iterators, slices, arithmetic, atomics, `core::fmt` |
| **`alloc`** | a heap | `Vec`, `String`, `Box`, `BTreeMap` |
| **`std`** | **an operating system** | `File`, `TcpStream`, `thread::spawn`, `Instant::now`, `println!` |

Everything in `std` is ultimately implemented by making a syscall into Linux, macOS,
or Windows.

## Why the kernel can't use `std`

Because there is nobody to call.

`File::open("foo.txt")` compiles down to "execute a syscall instruction and let the
operating system handle it." In our kernel, we are the operating system. There is
nothing beneath us to answer. The call would go into the void.

`#![no_std]` means: *do not link the standard library, link only `core`.* That is exactly
what the `none` in `aarch64-unknown-none-softfloat` is telling the compiler.

## The four things you feel

**`#![no_std]`**: no `Vec`, `String`, `Box`, `HashMap`, `println!`, `File`.

**`#![no_main]`**: In a normal program `main` is *not* the first thing to run. The C
runtime (`crt0`) runs first: sets up the stack, initializes libc, builds `argc`/`argv`,
*then* calls `main`. There is no libc here and nobody has set up a stack. So there can be
no `main`. We write our own entry point, `_start`, in assembly, and it sets up the stack
itself.

`#[panic_handler]`: `std` provides one, so you've never had to think about it.
Without `std` you must write it, and it forces a real question: what *should* happen when
a kernel panics? There's no process to kill, no stderr, no shell to return to. It's your
call. In nife: print to the serial port, then halt the CPU forever.

No `println!`, so we write our own. The good part: the hard bit of `println!` (the
whole `{:?}` / `{:x}` / width-and-padding formatting engine) lives in `core::fmt`,
which we still have. All `std` contributed was *where the bytes go*. Implement one trait,
`core::fmt::Write`, with one method that pushes bytes at the UART, and the entire
formatting machinery comes along for free.

## The point

`no_std` is not Rust with the training wheels off. It is Rust with the assumption of an
operating system removed. And we are the ones building the operating system.

So every missing piece of `std` is something we earn back by implementing the thing
`std` assumed existed:

| Missing | Because we haven't built | Earned back at |
|---|---|---|
| `println!` | somewhere for bytes to go | milestone 1, and again at 8 when the console driver left the kernel |
| `thread::spawn` | a scheduler | the scheduler is milestone 6; `thread::spawn` itself is **still `Unsupported`**, and the reason is a design fork rather than a missing service ([thread-spawn-fork.md](thread-spawn-fork.md)) |
| `Vec`, `String`, `Box` | a heap allocator | milestone 4 (MMU on: page tables, the kernel heap, the high half) in the kernel, then **taken back out**: milestone 14 (kernel objects from untyped: remove the kernel heap) made the kernel allocate nothing after boot, and the heap moved to userspace as `memory_region_heap`, on `talc` |
| `File::open` | a filesystem | milestone 32 (RedoxFS behind a capability FS server), bound into `std::fs` by milestone 27 phase two |

The heap row is the one worth reading twice, and this table said the opposite until
2026-09-05. At milestone 4 the kernel did get a `#[global_allocator]`, `extern crate alloc;`,
and a working `Vec`. Milestone 14 then argued the kernel should not have a heap at all and
deleted it, so `kernel/src/` has no global allocator today. Earning a piece of `std` back is
what this note claims and it is still true; what it did not anticipate is that the kernel would
give one back on purpose. See [heap.md](heap.md) and [collections.md](collections.md).

---

*Add to this file as new `no_std` friction comes up.*
