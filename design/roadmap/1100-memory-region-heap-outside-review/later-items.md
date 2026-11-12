# P2: later

An appendix to [the milestone](../1100-memory-region-heap-outside-review.md).

## P2: for later

10. `PAGE` carries no unit.
    Finding: `const PAGE: u64 = 4096` is a length; the units rule says a length carries its unit
    (`MAXIMUM_NAME_BYTES`, never `MAXIMUM_NAME_LEN`). `address_space_map` and `c_seam` spell it the
    same way, so this is a tree habit, not one crate's slip. `MINIMUM_GROW_PAGES` counts things and
    is right.
    Evidence: `src/lib.rs` line 31.
    Change: `PAGE_BYTES`, private, in this crate; the sibling crates in their own reviews.
    Door: two-way (private). Effort: XS.

11. A panic inside the allocator holds the lock.
    Finding: talc's `debug_assert!`s run in debug builds inside `alloc`, `dealloc` and `realloc`
    while `locked` is `true`. If the user-mode panic handler allocates (unverified: it was not read
    for this), a debug-build allocator panic spins forever in the handler instead of faulting
    legibly. There is no unwinding on these targets, so the lock is never released by a `Drop` on
    the way out either.
    Evidence: `src/lib.rs` lines 62 to 76 and 241 to 288.
    Change: read the handler. If it allocates, say so in the README and in the handler's doc, or
    give the heap a `try_lock` the handler can use to print without allocating.
    Door: two-way. Effort: XS to read, S to change.

12. Growth is monotonic and the README should say so.
    Finding: talc's `truncate` is never called, and the memory-region interface has no per-page
    return (`MAP`, `RETYPE`, `RETYPE_OBJECT`, `SPLIT`, `DESTROY`, `USAGE`), so a heap that peaked
    holds its peak until the region's owner destroys it. glibc trims. `dlmalloc` and `rlsf` release
    pages. A stranger from any of them will expect a high-water mark that falls.
    Evidence: `crates/abi/src/memory_region.rs`. `src/lib.rs` (no call to `truncate`).
    Change: one README sentence, and a BUGS line in the crate for the day the interface grows a
    return.
    Door: two-way. Effort: XS.

13. No accessor for the cap or the base.
    Finding: `committed_bytes` exists. `maximum_bytes` and `base_virtual_address` do not, so a test
    or an operator cannot ask what cap is in force after a rounding or a second `initialize`.
    Evidence: `src/lib.rs` lines 126 to 131.
    Change: two accessors, named with the architect in Decision A's table.
    Door: one-way (public names). Effort: XS.

14. The `unsafe impl Sync` and the guard are sound, and the argument deserves one written statement.
    Finding: `Guard::inner(&mut self)` hands out `&mut Inner` from an `UnsafeCell` under a
    compare-exchange spinlock with `Acquire` on take and `Release` on drop. The pairing is right,
    the `&mut self` receiver stops two live `&mut Inner` from one guard, and `grow` allocates
    nothing while holding the lock. The SAFETY comments say "all access goes through the lock" and
    stop there. Nothing states that `Inner` is otherwise `!Sync` because of talc's raw pointers, or
    that no `Source` or signal-like upcall may use the heap.
    Evidence: `src/lib.rs` lines 59 to 76.
    Change: one paragraph on the type: what the lock protects, why `Inner` needs it, and the two
    re-entrancy rules. No code change.
    Door: two-way. Effort: XS.
