# P2: later

An appendix to [the milestone](../1077-allocation-cache-outside-review.md).

## P2: later

10. The dev-dependency on `talc` leaves nife's patch behind.
   Finding: nife builds the crate's host test against `vendor/talc`, 5.1.1 plus an upstream fit-check
   fix whose pull request is still open; the crate repository will build crates.io's 5.1.1 without
   it. `vendor/README.md` records that Miri reports undefined behavior through this crate's host
   test on the unpatched crate. This review ran that test under Miri against unpatched 5.1.1 with
   default flags, strict provenance and three seeds and saw nothing, so the claim did not reproduce
   here; the template's weekly `miri` job is the gate that settles it.
   Change: nothing before the move. If the weekly job turns red on talc, the host test pins a talc
   that carries the fix, or swaps the backing heap for a simple arena the test owns.
   Door: two-way. Effort: S.

11. The crate has no harness and the `proofs` workflow may want one.
   Finding: the template's `proofs.yml` runs `kani` and `falsify` on every pull request. Whether a
   crate with zero harnesses passes both jobs is not verified here; one sibling in the review index
   has no harness and no repository yet.
   Change: item 8 removes the question. If item 8 waits, the move lane confirms the job's behavior
   on an empty harness set before the first pull request.
   Door: two-way. Effort: S.

12. `Cache` implements neither `Debug` nor a `Drop` that notices a leak.
   Finding: a public type with no `Debug` cannot sit in a user's `#[derive(Debug)]` struct. A `Cache`
   dropped with blocks in it leaks them silently; a `Drop` with `debug_assert!(self.cached_bytes()
   == 0)` costs nothing in release and catches the owner that forgot `drain`.
   Change: a `Debug` that prints the counts per class (not the pointers), and the `Drop` assertion,
   with a test. Adding either is compatible later; the `Drop` changes nothing a correct owner sees.
   Door: two-way. Effort: S.

13. A stranger with another heap has no way to tune the constants.
   Finding: depth 16, grid 16, ceiling 256 are the crate's, and its `BUGS` says a lane chose them
   without measuring. glibc, jemalloc and TCMalloc each expose depth and ceiling as tunables.
   Rust lets `Cache` take `const` parameters with defaults later without breaking a user who writes
   `Cache`, so this can wait for a second consumer.
   Change: none now; a `BUGS` line that says the parameters do not move and why.
   Door: two-way while users write the defaults. Effort: S.

14. nife's notes record the cross-owner free pattern; the crate does not.
   Finding: a block freed by a different owner than allocated it lands in the freeing owner's
   cache, bounded by the depth and spilled at the bound. `notes/std/allocation.md` records it under
   `BUGS`; the crate's `BUGS` does not, and the crate is the place a stranger reads.
   Change: one `BUGS` line in the crate, in owner-agnostic words.
   Door: two-way. Effort: S.

15. The abbreviation report flags one test name.
   Finding: `an_uncached_request_reaches_the_heap_as_asked_unless_empty` (`src/lib.rs:274`):
   `uncached` is not in the template's word lists. The step is report only.
   Change: `a_request_without_a_class_reaches_the_heap_as_asked_unless_empty`, or an
   `accepted-words` entry for `uncached` with its reason.
   Door: two-way. Effort: S.
