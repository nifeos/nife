# Prior art

An appendix to [the milestone](../1112-user-mode-runtime-outside-review.md).

How the crate sits against what a Rust reader already knows. Recent downloads are the last 90 days
at the time of the review.

| crate | recent / all-time | what it is | what it does that this crate does not, or does differently |
|---|---|---|---|
| `libc` 1.0 | 479M / 1.96G | the C ABI, syscall numbers and `syscall(2)` | one untyped `syscall` plus typed wrappers per call; this crate has the same two tiers (`raw_syscall`, the wrappers) |
| `rustix` 1.1 | 358M / 1.39G | safe, typed wrappers over Linux syscalls | every return is a `Result<T, Errno>`; this crate returns `i64` and leaves decoding to `abi::Error::from_ret` on 30 of 60 functions |
| `linux-raw-sys` 0.12 | 339M / 1.31G | raw syscall numbers and structs | the `abi` crate's role |
| `syscalls` 0.8 | 486k / 1.56M | `syscall!` over raw numbers, per architecture | this crate's `raw_syscall` |
| `zerocopy` 0.9 | 329M / 1.13G | `FromBytes` and friends for reading a `T` out of bytes | a trait bound that makes "any bit pattern is a valid `T`" a type-level claim; item 1 |
| `bytemuck` 1.25 | 140M / 470M | `Pod` | the same bound, derivable |
| `volatile-register` 0.2 | 2.0M / 12.6M | `RO<T>`, `RW<T>`, `WO<T>` over a typed place | reads take `&self` on a real `T`, so alignment and validity come from the place; item 1 |
| `vcell` 0.1 | 2.3M / 13.8M | `VolatileCell<T: Copy>` | same shape as above |
| `tock-registers` 0.12 | 583k / 2.25M | typed registers over a sealed `UIntLike` | the sealed-trait pattern item 1 recommends |
| `volatile` 0.6 | 1.5M / 7.5M | `VolatilePtr`, `VolatileRef` with access markers | a pointer-shaped window with read-only and write-only variants |
| `cortex-m-rt` 0.7 | 2.4M / 10.6M | `#[entry]`, the reset vector, `link.x` shipped from a build script | the link script lives in the runtime crate and reaches a binary through `OUT_DIR` and `cargo:rustc-link-search`; item 5 |
| `riscv-rt` 0.18 | 772k / 3.0M | the same for RISC-V | same mechanism |
| `fuchsia-zircon` 0.3 | 6.4M / 29.6M | typed handles over Zircon syscalls | one `channel_read` returns bytes and handles together; this crate has ten `receive_*` functions (item 15) |
| `sel4` 0.0.12 | 55 / 18.9k | Rust bindings for seL4 | `seL4_Recv` returns the badge through one out-parameter on every receive; the Reply object is a typed capability, as this crate's `Reply` is |

Where this crate differs: it puts three architectures behind one signature set. It has a six-word
register ABI with the sixth word reserved for a death label. It has a `Reply` newtype that only
`receive_request` can mint. Its windows bounds-check a page the kernel mapped. Its entry macro fixes
the `x86_64` stack alignment with a one-`call` shim. What it costs today: a generic window read that
safe code can misuse (item 1), and two barriers that tell the compiler they are not barriers (item
2). It also costs a manifest and a text that only build and only read inside nife (items 3, 4, 5,
8). Finally, it offers a surface of ten receive variants where the prior art offers one.
