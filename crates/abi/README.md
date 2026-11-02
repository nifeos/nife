# abi

The binary interface between a program and a capability kernel: the five syscall numbers, the
method number of every operation on every kind of capability, the error codes, the rights bits and
the shapes of the replies. `no_std`, no dependencies, and no code beyond constants, a handful of
`const fn` helpers and tests. It was written for the nife kernel and the programs that run on it,
which share this one definition so a number cannot mean two things on the two sides of the syscall
boundary.

## The contract

- **Five syscalls** (`SYS_EXIT` 0, `SYS_YIELD` 1, `SYS_INVOKE` 2, `SYS_CAPABILITY_DELETE` 3 and
  `SYS_EXIT_THREAD` 4). Everything else is `invoke(capability, method, a0, a1, a2)`, which acts on a
  capability the caller holds in its own table. The syscall number is in `x8` (or the architecture's
  equivalent), the slot in `x0`, the method in `x1` and the arguments in `x2` to `x4`. `x0` returns
  a result or a negative `Error`. Adding an operation is a new method, never a new syscall.
- **Methods**, one module per kind of capability, each constant documented with its arguments,
  result, the right it needs and its refusals: `rendezvous` (send, receive, call, reap, survey,
  badge), `reply`, `memory_region` (retype, split, destroy, usage), `page_frame` (map, revoke,
  slice), `address_space` (map, list, unmap, futex wait and wake), `thread_control_block`,
  `process`, `notification`, `timer`, `interrupt`, `virtio`, `uart`, `port_range`, `reboot` and
  `console`. `objtype` names the kinds a region can be retyped into.
- **Errors**: `Error` is `#[repr(i64)]`, from `NoSuchSlot` (-1) to `ResetDidNotHappen` (-15). Each
  variant says when it is answered. `NoSuchSlot` (nothing there) and `Gone` (it was there and is
  gone) are different facts on purpose. `NotSupervised` deliberately does not distinguish "gone"
  from "not yours".
- **Rights**: five bits (`READ`, `WRITE`, `GRANT`, `ENUMERATE`, `BIND`), only ever narrowed on
  delegation; `ALL` and `SPEND_ONLY` are the two useful masks.
- **Capability tables** have `CAPABILITY_TABLE_SLOTS` (64) slots, and `fault::FAULT_EP_SLOT` is
  derived from that number, so changing it is an ABI change.
- **Versions**: `ABI_VERSION` (1) is one number for the whole surface, and `INTERFACE_ID` (1) names
  this interface in a program's declaration of what it links. A program built against one version
  is refused by a kernel that serves another at spawn; the ABI never dual-serves. A renumber of any
  public number bumps `ABI_VERSION`.
- The numbers are pinned as literals in the crate's tests, so changing one is a deliberate act that
  fails the build until the test and the version change with it.

## Usage

```rust
use abi::{Error, SYS_EXIT, SYS_INVOKE, SYS_YIELD, rendezvous};

assert_eq!([SYS_EXIT, SYS_YIELD, SYS_INVOKE], [0, 1, 2]);
// Sending on a rendezvous is a method number, not a syscall number.
assert_eq!(rendezvous::SEND, 0);
// Errors are negative, and "empty slot" is not "gone".
assert_eq!(Error::NoSuchSlot as i64, -1);
assert_ne!(Error::NoSuchSlot, Error::Gone);
```

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

The minimum supported Rust version is 1.88. There are no Kani harnesses; the pinned-values tests
carry the compatibility claim.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
