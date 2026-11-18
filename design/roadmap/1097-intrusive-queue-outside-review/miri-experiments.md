# Appendix: the two Miri experiments behind item 5

An appendix to [the milestone](../1097-intrusive-queue-outside-review.md).

The review ran both on a copy of the crate outside the tree, under `cargo +nightly miri test`, once with
the default Stacked Borrows and once with `-Zmiri-tree-borrows`. Both models agree on every case.

Passes under both: a token minted from the owning raw pointer survives a table walk.

```rust
let a: *mut N = Box::into_raw(Box::new(N { next: None, payload: 0 }));
let mut q = Queue::new();
q.push_back(unsafe { Unqueued::new(NonNull::new_unchecked(a)) });
unsafe { let r: &mut N = &mut *a; r.payload = 7; }   // the walk: a fresh `&mut`, dropped
let first = q.pop_front().unwrap();                 // fine
assert_eq!(unsafe { (*first.as_ptr()).payload }, 7);
```

Undefined behavior under both: the same walk with a token minted from a reborrow, the shape the
crate's examples and the kernel's `mint_token` use.

```rust
let a: *mut N = Box::into_raw(Box::new(N { next: None, payload: 0 }));
let mut q = Queue::new();
let ta = unsafe { let t: &mut N = &mut *a; Unqueued::new(NonNull::from(&mut *t)) };
q.push_back(ta);
unsafe { let r: &mut N = &mut *a; r.payload = 7; }   // retires the reborrow `ta` came from
let first = q.pop_front().unwrap();                 // UB: the retag through `ta` fails
```

The second shape with `Box<N>` owners and `NonNull::from(&mut *a)`, which is the crate's own
doctest and test helper, fails the same way as soon as any later `&mut a` exists. A `&mut` held
across a queue operation and used after it has undefined behavior in every variant, which is rule 3
as written.
