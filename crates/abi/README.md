# abi

The application binary interface (ABI) between a program and a capability kernel. It holds the
five syscall numbers, the method number of every operation on every kind of capability, the error
codes, the rights bits and the shapes of the replies. The crate is `no_std` with no dependencies,
and carries no code beyond constants, a handful of `const fn` helpers and tests. A kernel and the
programs that run on it share this one definition, so a number cannot mean two things on the two
sides of the syscall boundary.

## The contract

- The five syscalls are `SYSCALL_EXIT` (0), `SYSCALL_YIELD` (1), `SYSCALL_INVOKE` (2),
  `SYSCALL_CAPABILITY_DELETE` (3) and `SYSCALL_EXIT_THREAD` (4). Everything else is
  `invoke(capability, method, a0, a1, a2)`, which acts on a capability the caller holds in its own
  table. Adding an operation is a new method, never a new syscall.
- The trap moves six words in each direction. The crate root's table names the register that
  holds each word on aarch64, riscv64 and `x86_64`: the syscall number, then the slot, the method
  and the arguments. Word 0 returns a result or a negative `Error`.
- Every method constant says what it takes, what it returns, the right it needs and its refusals.
  The modules are listed below.
- `Error` is `#[repr(i64)]`, from `NoSuchSlot` (-1) to `ResetDidNotHappen` (-15). Each variant says
  when it is answered. `NoSuchSlot` (nothing there) and `Gone` (it was there and is gone) are
  different facts on purpose. `NotSupervised` deliberately does not tell "gone" from "not yours".
- Rights are five bits (`READ`, `WRITE`, `GRANT`, `ENUMERATE`, `BIND`) that only ever narrow on
  delegation. `ALL` and `SPEND_ONLY` are the two useful masks.
- A capability table has `CAPABILITY_TABLE_SLOTS` (64) slots, and `fault::FAULT_RENDEZVOUS_SLOT`
  derives from that number, so changing it is an ABI change.
- `ABI_VERSION` (1) is one number for the whole surface, and `INTERFACE_ID` (1) names this
  interface in a program's declaration of what it links. A kernel refuses, at spawn, a program
  built against another version, and the ABI never dual-serves. A renumber of any public number
  bumps `ABI_VERSION`.
- The numbers are pinned as literals in the crate's tests, and a test counts every public number
  to see that each one is pinned. Changing a number fails the build until the test and the version
  change with it.

| module | what it holds |
|---|---|
| `rendezvous`, `reply` | send, receive, call, reap, survey and badge; the one-shot answer |
| `memory_region`, `usage`, `object_type` | retype, split, destroy and usage; which figure to ask for; what a page becomes |
| `page_frame`, `address_space`, `futex` | map, revoke and slice; map, list, unmap, wait and wake; the futex flags |
| `thread_control_block`, `process`, `fault`, `survey` | threads and processes; the death message; the survey records |
| `notification`, `timer`, `interrupt`, `cycle_counter` | doorbells, deadlines, interrupts and what a cycle read counts |
| `virtio`, `uart`, `port_range`, `reboot`, `console` | device, console and machine methods (`console` is historical) |

## EXAMPLES

```rust
use abi::{Error, SYSCALL_EXIT, SYSCALL_INVOKE, SYSCALL_YIELD, rendezvous};

assert_eq!([SYSCALL_EXIT, SYSCALL_YIELD, SYSCALL_INVOKE], [0, 1, 2]);
// Sending on a rendezvous is a method number, not a syscall number.
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
