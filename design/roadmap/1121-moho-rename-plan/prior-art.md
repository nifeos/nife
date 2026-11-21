# Prior art

An appendix to [the milestone](../1121-moho-rename-plan.md).

The lane read each entry on 2026-11-21 at the source named.

| system | what it renamed or kept | how | source |
|---|---|---|---|
| Rust's WASI target | the triple `wasm32-wasi` became `wasm32-wasip1` | the new name arrived in 1.78, warnings for the old one in 1.81, and the old target stopped shipping in 1.84, so users had two releases with both | the Rust blog's post on its WASI targets, 2024-04-09 |
| Hermit, in rustc | the `hermitkernel` targets left rustc | rust-lang/rust#95299, merged for 1.61: "RustyHermit now maintains custom json targets, which are distributed with the kernel" | the pull request |
| Hermit, on crates.io | the build crate `hermit-sys` became `hermit` | a new crate; `hermit-sys` keeps its last version (0.6.0, 57,659 downloads) and `hermit` is at 0.6 | the crates.io API; the `hermit` README in `hermit-os/hermit-rs` |
| Hermit, in `std` | the ABI crate `hermit-abi` is a dependency of `std` | `library/std/Cargo.toml` takes `hermit-abi` 0.5.3 from crates.io with `rustc-dep-of-std` under `cfg(target_os = "hermit")`, so an upstream `std` port needs its ABI crate published | the pinned nightly-2026-11-20's `rust-src` |
| Hermit, in getrandom | the backend calls `sys_read_entropy` and `sys_secure_rand32` and `64` as `extern "C"` symbols | no crate dependency | getrandom 0.3.4's `src/backends/hermit.rs` |
| Hermit, in LLVM | the OS is `HermitCore` in LLVM and `hermit` in rustc | the two names need not match | LLVM's `Triple.h` on main |
| Redox | the crates.io package is `redox_syscall`, and its library name is `syscall` | `[lib] name = "syscall"` in the manifest of 0.9.5 | the published crate |
| Fuchsia | the short aliases `x86_64-fuchsia` and `aarch64-fuchsia` left rustc for `*-unknown-fuchsia` | rust-lang/rust#130657, merged 2024-09-23 and listed as a compatibility note | the pull request and tracking issue #130661, through a web search |
| macOS | the kernel is Darwin and `target_os` is `macos` | `uname -s` prints `Darwin` | recalled, not read |

Where nife differs. Each of these renames had users, so each kept the old name for a while: a
warning window, a frozen old crate or an alias. nife has none, and the tree rules out an alias of an
old nife spelling before the first release. So steps 3 to 5 rename in place. The one window with
two names is basalt's, between steps 1 and 6, because two repositories pin each other.
Redox's split between the package and the library name is the shape nife refused: `moho_abi` is
the code name and the package name alike.

An example from the plan: after step 3, `kernel/src/system_call.rs` writes
`moho_abi::SYSTEM_CALL_INVOKE` where it writes `abi::SYSTEM_CALL_INVOKE` today.
