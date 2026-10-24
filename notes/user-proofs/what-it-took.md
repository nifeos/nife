# What it took to put `user/` within reach of Kani

*Name: provisional.*

An appendix to [`notes/user-proofs.md`](../user-proofs.md). It holds the three things that stopped
`cargo kani` on an EL0 program, the fix for each, and why the binaries are selected by derivation
rather than by a list.

It is shorter than milestone 193 (put `kernel/src` within reach of the prover)'s list. The kernel
had already paid for most of it.

| what stopped it | why | the fix |
|---|---|---|
| `unwinding panics are not supported without std` | the workspace deliberately does not set `panic = "abort"` (DECISIONS §7) | nothing: Kani passes `-C panic=abort` itself, so this is a `cargo check` problem and not a `cargo kani` one |
| `found duplicate lang item panic_impl` | Kani links `std`, which defines the handler `user_mode_runtime::panic_handler!()` expands to | `#[cfg(not(kani))]` on the macro invocation, in the one binary carrying a harness |
| `Failed to detect Kani functions ... seems to be using #[no_std]` | Kani refuses a `no_std` crate root that never mentions it, and 67 of the 68 programs never will | select the binaries instead: `--bin`, derived in `script/verify` from a grep of the tree |

There is no `--ignore-global-asm`, which is the difference from the kernel. It is DECISIONS §4
(kernel shape) rule 1 paying out again. There is no `global_asm!` anywhere under `user/`. The only
assembly a program has any business containing is the syscall itself, and that lives in
`user_mode_runtime`.

The `--bin` selection is the one piece of machinery worth arguing about. The argument is in
`script/verify`'s own comment. A hand-written list of binaries would be one name short the first
time somebody adds a harness to a 69th program. That failure is the invisible one this project has
now recorded twice (`multicast_dns_protocol`, `jh7110_entropy`). The suite goes green faster and
nothing says a harness stopped running. `script/lint` already catches a whole package missing from
the verify table. Only the derivation catches a binary missing from inside one.
