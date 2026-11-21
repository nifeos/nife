# intrusive_queue

[![check](https://github.com/nifeos/intrusive_queue/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/intrusive_queue/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/intrusive_queue/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/intrusive_queue/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/intrusive_queue#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.85%2B-blue.svg)](https://github.com/nifeos/intrusive_queue/blob/main/Cargo.toml)

An intrusive first in, first out (FIFO) queue. The link lives inside the node, so a push and a pop
cannot allocate or fail, and a pop hands back the node itself. The queue is two pointers and a
count, and it owns none of its nodes. That makes a push legal where allocation is not, such as an
interrupt handler or a scheduler's run queue.

The crate is `no_std`, has no dependencies, and is dual licensed `MIT OR Apache-2.0`. Two Kani
harnesses prove its list logic, and each carries a falsification patch that shows what turns it
red.

## EXAMPLES

A run queue: threads leave in the order they arrived.

```rust
use core::ptr::NonNull;
use intrusive_queue::{Node, Queue, Unqueued};

struct Thread {
    id: u32,
    next: Option<NonNull<Thread>>,
}

// SAFETY: `next` and `set_next` are plain field storage and touch nothing else, which is the whole
// of the `Node` contract.
unsafe impl Node for Thread {
    fn next(&self) -> Option<NonNull<Self>> {
        self.next
    }
    fn set_next(&mut self, next: Option<NonNull<Self>>) {
        self.next = next;
    }
}

let a = NonNull::new(Box::into_raw(Box::new(Thread { id: 1, next: None }))).unwrap();
let b = NonNull::new(Box::into_raw(Box::new(Thread { id: 2, next: None }))).unwrap();
let mut ready: Queue<Thread> = Queue::new();
// SAFETY: live boxed nodes on no queue, minted from their owning pointers, freed below.
let (token_a, token_b) = unsafe { (Unqueued::new(a), Unqueued::new(b)) };
ready.push_back(token_a);
ready.push_back(token_b);

let first = ready.pop_front().unwrap();
ready.push_back(first);
let order: Vec<u32> = core::iter::from_fn(|| ready.pop_front())
    // SAFETY: each token names one of the two live boxed nodes.
    .map(|token| unsafe { (*token.as_ptr()).id })
    .collect();
assert_eq!(order, vec![2, 1]);
// SAFETY: both came from `Box::into_raw`, and the queue is empty, so nothing reaches them.
unsafe { drop((Box::from_raw(a.as_ptr()), Box::from_raw(b.as_ptr()))) };
```

## What is proven

Two Kani harnesses in `src/lib.rs` drive the real `Queue` against a model of an array and two
counters:

- Any interleaving of pushes and pops is first in, first out and lossless. Over six symbolic steps
  and three nodes, the queue hands nodes back in push order. It agrees with the model on its length
  and emptiness at every step, and Kani checks that no step follows a stale link.
- A popped node leaves with a clear link, whichever node it was and however many followed it.

The harnesses cover the list logic. They do not cover the caller's contract: no proof here shows
that a caller kept a node alive or minted its token from the owning pointer. Miri covers the
aliasing side through the tests, and the repository's weekly workflow runs it.

## The caller's contract

A node implements `Node`, an `unsafe` trait whose two methods read and store its link. One `unsafe`
call, `Unqueued::new`, mints a node into an `Unqueued` token. `Queue::push_back` consumes the
token and `Queue::pop_front` hands it back. The token is neither `Clone` nor `Copy`, so a second
push of the same node does not compile without a second `unsafe` mint.

The caller keeps three rules, and the documentation cites them by number:

1. A node is on at most one queue until popped. The token enforces this.
2. A node outlives its time on the queue. No type in the crate can check this, because the queue
   does not own its nodes. `Unqueued::new` states it once, where the caller mints the token.
3. The caller serializes all access to a queue and its links, and mints each token from the
   node's owning pointer, never from a reference.

Rule 3's second half is about pointer provenance. Mint the token from the pointer every later
access derives from: the one `Box::into_raw` returned, the pointer a table stores, or `&raw mut`
on a local the program does not name again. A token minted from a reborrow, such as
`NonNull::from(&mut *owner)`, dies at the owner's next `&mut` reborrow under Rust's aliasing
rules. The queue's next use of that token is then undefined behavior. Miri reports it under both
Stacked Borrows and Tree Borrows. The test
`a_token_from_the_owning_pointer_survives_later_borrows` runs the safe shape.

A dropped token strands its node, and so does a dropped queue that still holds nodes: queueing
one again needs a fresh mint. `Queue` has no `Drop`, so a caller can move a queue by value through
`Default` and `core::mem::take`.

`Queue<T>` is `Send` only when `T` is, as `LinkedList<T>` is. `Unqueued<T>` is never `Send`, since
`NonNull<T>` is not.

## Building and testing

```sh
cargo test
rustup target add x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo build --lib --target x86_64-unknown-none
cargo build --lib --target aarch64-unknown-none-softfloat
cargo build --lib --target riscv64imac-unknown-none-elf
```

The crate has no features, so a plain `cargo test` runs every unit test and doctest. Two doctests
are `compile_fail`: a double push (`E0382`) and a queue of `Rc` nodes sent to another thread
(`E0277`).

### Running the proofs

Install Kani first: run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. Setup downloads a model checker and
the Rust toolchain Kani pins, so it needs network access. Then run every harness:

```sh
cargo kani
```

The two harnesses take about 20 seconds and peak near 220 megabytes of memory, as
`/usr/bin/time -v cargo kani` measured on a shared 80-core x86_64 host. To run one harness, give
its full path, because `--exact` refuses a bare name:

```sh
cargo kani --harness verification::a_popped_node_leaves_with_a_clear_link --exact
```

A passing run prints lines that look alarming and mean nothing. They are the `use of an unstable
feature` warning, and many `aborting path on assume(false)` lines, which are paths Kani discards on
purpose. The answer is the last line, `Complete - 2 successfully verified harnesses, 0 failures, 2
total.`, and `cargo kani` exits 0.

### Running one falsification patch

Each file in `falsifications/` applies a small defect that turns its harness red. Apply one,
confirm the proof fails, then undo it with the same patch reversed:

```sh
patch -p1 --no-backup-if-mismatch < falsifications/verification.a_popped_node_leaves_with_a_clear_link.patch
cargo kani --harness verification::a_popped_node_leaves_with_a_clear_link --exact
patch -R -p1 --no-backup-if-mismatch < falsifications/verification.a_popped_node_leaves_with_a_clear_link.patch
```

The failing run prints `Failed Checks: assertion failed: unsafe { (*token.as_ptr()).next()
}.is_none()` and `VERIFICATION:- FAILED`. It ends with `Complete - 0 successfully verified
harnesses, 1 failures, 1 total.`, and `cargo kani` exits 1. Each patch's opening lines name the
check it expects to fail.

## BUGS

- The queue is singly linked and offers only `push_back` and `pop_front`. Taking one particular
  node out means popping every node and pushing the others back.
- Nothing in the crate checks rule 2. Freeing a node that a queue still holds causes undefined
  behavior at the queue's next touch of it.
- The proofs stop at six steps over three nodes.

## Minimum Rust version

Rust 1.85, the first release with edition 2024. To check it, install the toolchain and its
bare-metal targets, then run the tests and all three bare-metal builds with `+1.85`:

```sh
rustup toolchain install 1.85 --profile minimal
rustup target add --toolchain 1.85 x86_64-unknown-none aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf
cargo +1.85 test
cargo +1.85 build --lib --target x86_64-unknown-none
cargo +1.85 build --lib --target aarch64-unknown-none-softfloat
cargo +1.85 build --lib --target riscv64imac-unknown-none-elf
```

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
