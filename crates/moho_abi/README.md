# moho_abi

[![check](https://github.com/nifeos/moho_abi/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/moho_abi/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/moho_abi/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/moho_abi/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/moho_abi#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/moho_abi/blob/main/Cargo.toml)

The application binary interface (ABI) between a program and a capability kernel. It holds the
five system call numbers, the method number of every operation on every kind of capability, the error
codes, the rights bits and the shapes of the replies. The crate is `no_std` with no dependencies,
and carries no code beyond constants, a handful of `const fn` helpers and tests. A kernel and the
programs that run on it share this one definition, so a number cannot mean two things on the two
sides of the system call boundary.

## The contract

- The five system calls are `SYSTEM_CALL_EXIT` (0), `SYSTEM_CALL_YIELD` (1), `SYSTEM_CALL_INVOKE` (2),
  `SYSTEM_CALL_CAPABILITY_DELETE` (3) and `SYSTEM_CALL_EXIT_THREAD` (4). Everything else is
  `invoke(capability, method, a0, a1, a2)`, which acts on a capability the caller holds in its own
  table. Adding an operation is a new method, never a new system call.
- The trap moves six words in each direction. The crate root's table names the register that
  holds each word on aarch64, riscv64 and `x86_64`: the system call number, then the slot, the method
  and the arguments. Word 0 returns a result or a negative `Error`.
- Every method constant says what it takes, what it returns, the right it needs and its refusals.
  The table below lists the modules.
- `Error` is `#[repr(i64)]`, from `NoSuchSlot` (-1) to `ResetDidNotHappen` (-15). Each variant says
  when the kernel answers it. `NoSuchSlot` (nothing there) and `Gone` (it was there and is gone) are
  different facts on purpose. `NotSupervised` deliberately does not tell "gone" from "not yours".
- Rights are five bits (`READ`, `WRITE`, `GRANT`, `ENUMERATE`, `BIND`) that only ever narrow on
  delegation. `ALL` and `SPEND_ONLY` are the two useful masks.
- A capability table has `CAPABILITY_TABLE_SLOTS` (64) slots, and `fault::FAULT_RENDEZVOUS_SLOT`
  derives from that number, so changing it is an ABI change.
- `ABI_VERSION` (1) is one number for the whole surface, and `INTERFACE_ID` (1) names this
  interface in a program's declaration of what it links. A kernel refuses, at spawn, a program
  built against another version, and the ABI never dual-serves. A renumber of any public number
  bumps `ABI_VERSION`. A retired number stays in the crate as reserved and is never reused; the
  console's `WRITE` is one, and a test holds it. A change that alters or removes a public number
  changes `ABI_VERSION` in the same pull request, and a new number owes nothing.
- The crate's tests pin every number as a literal, and a test counts the public numbers to see
  that the pinned lists name each one. Changing a number fails the tests until its pinned line
  changes with it, and `pinned-at-version-1.txt` records every pinned line of this version, so a
  changed pinned line fails until `ABI_VERSION` moves and a new record joins it.

| module | what it holds |
|---|---|
| `rendezvous`, `reply` | send, receive, call, reap, survey and badge; the one-shot answer |
| `memory_region`, `usage`, `object_type` | retype, split, destroy and usage; which figure to ask for; what a page becomes |
| `page_frame`, `address_space`, `address_wait` | map, revoke and slice; map, list, unmap, wait and wake; the address wait flags |
| `thread_control_block`, `process`, `fault`, `survey` | threads and processes; the death message; the survey records |
| `notification`, `timer`, `interrupt`, `cycle_counter` | doorbells, deadlines, interrupts and what a cycle read counts |
| `virtual_io`, `uart`, `port_range`, `reboot`, `console` | device, console and machine methods (`console` holds one reserved number) |

## EXAMPLES

```rust
use moho_abi::{Error, SYSTEM_CALL_EXIT, SYSTEM_CALL_INVOKE, SYSTEM_CALL_YIELD, rendezvous};

assert_eq!([SYSTEM_CALL_EXIT, SYSTEM_CALL_YIELD, SYSTEM_CALL_INVOKE], [0, 1, 2]);
// Sending on a rendezvous is a method number, not a system call number.
assert_eq!(rendezvous::SEND, 0);
// Errors are negative, and "empty slot" is not "gone".
assert_eq!(Error::NoSuchSlot as i64, -1);
assert_ne!(Error::NoSuchSlot, Error::Gone);
```

## What is proven

Nothing is machine-checked: there are no Kani harnesses. The pinned-values tests carry the
compatibility claim.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

## BUGS

- `rendezvous::RECEIVE`: `x0` is the sender's own word, so a negative word equal to an `Error` code reads as that error.
- `rendezvous::RECEIVE_CAPABILITY`: a death read here loses the faulting `pc`.
- `rendezvous::RECEIVE_CAPABILITY`: a delegation passes the `NO_CAPABILITY` guard.
- `rendezvous::REAP`: `NotPermitted` has a second, unmeasured cause.
- `memory_region::USAGE`: a subtree record walks the whole region table under the region lock.
- `usage`: notification and timer objects have no record, so the object rows under-count.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
