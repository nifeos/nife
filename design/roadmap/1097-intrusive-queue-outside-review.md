---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1097. `intrusive_queue`: what an outside maintainer would fix before its first release

Raised by a read-only outside review of `crates/intrusive_queue`, read the way a maintainer meeting
it on crates.io for the first time would read it. It changes nothing; it lists what the lanes that
move and release the crate change. The number is the recording lane's to assign.

The crate has no review package under `notes/crate-reviews/`; the accepted baseline is the ratified
block in `src/lib.rs` (the crate name and `Queue<T: Node>`), the `Unqueued` token as the ruling on
the drive-down-unsafe milestone recorded it, the Kani harness and its falsification patch, and the
release survey's row that says the crate moves. None of those is reopened here. Where a later rule
(the naming page, the units rule, the standard-library-names rule) bears on the crate, the item says
so; the template's name checks refuse nothing in it.

Reuse: the template `nifeos/crate-workflows` and its scaffold, Kani, Miri, cargo-llvm-cov,
cargo-mutants. No new tool.

## Goal

A stranger who finds `intrusive_queue` on crates.io can tell in one screen what it is for. The stranger can build, test, prove and falsify it from the README alone. The stranger can mint a token without stepping on an
aliasing rule the documentation never stated. The stranger can also rely on the auto traits the crate publishes
being the ones a `LinkedList` user expects. The repository's gates are green for the crate's own
reasons and red only for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1097-intrusive-queue-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1097-intrusive-queue-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the release (a public name, a published bound or
behavior, a layout, a license, a dependency); everything else is two-way and the maintainer
decides it.

### P0: blocks any publish

1. The manifest is a workspace member's, not a crate's.
   Finding: `Cargo.toml` has no `description`, `repository`, `documentation`, `rust-version`,
   `keywords` or `categories`; `license.workspace = true` has no workspace to inherit from once the
   crate moves; `publish = false` carries a comment that points at `crates/publish-allowlist.txt`,
   a file that will not be in the new repository. `cargo package --list` warns about the missing
   metadata today.
   Evidence: `crates/intrusive_queue/Cargo.toml` lines 1 to 9.
   Change: take the template sample's manifest shape. It has `rust-version = "1.85"` (verified to pass),
   `license = "MIT OR Apache-2.0"`, a one-sentence description, the repository URL and the template's
   `[lints.rust]` block (`missing_docs`, `unsafe_op_in_unsafe_fn`, `unexpected_cfgs` naming
   `cfg(kani)`). It also has a `publish = false` comment a stranger can act on until the release that
   publishes deletes the line.
   Door: two-way except publishing itself, which the publishing hold already governs. Effort: XS.

2. The crate's front page points into nife.
   Finding: the crate root says "See notes/intrusive-queues.md" twice, cites
   "notes/overflow-checks.md" and "DECISIONS §14 (the project's direction: a verified-Rust capability microkernel that runs real workloads)", and carries two `Name:` blocks, one of which
   names a person and a milestone round and asks a question the record never answers. The
   template's `standalone` check refuses all of it: three path lines, three provenance block lines,
   one milestone citation, two lines naming the architect, one section citation. A reader on
   docs.rs follows none of those pointers anywhere.
   Evidence: `src/lib.rs` lines 3, 11, 62, 64 to 66, 219 to 220, 251.
   Change: the crate's README carries what `notes/intrusive-queues.md` says about the crate (the
   three rules, why the token exists and what it cannot carry, what the proofs cover and what they leave out, the
   aliasing fine print rewritten per item 5). The crate root's doc keeps the contract and loses the
   pointers. The `Name:` blocks go, per the rule that crate repositories carry no provenance and
   keep the name gates. The overflow comment on `len` says what it means in the crate's own words.
   A plain `+` carries an overflow check in any profile that enables them, nife's release profile
   among them. A count of distinct live nodes of nonzero size cannot reach `usize::MAX`. So
   the wrapping form drops a check that can never fire from the path every push and pop takes.
   Door: two-way. Effort: S.

3. No repository skeleton.
   Finding: no README, no `LICENSE-MIT` or `LICENSE-APACHE`, no `CHANGELOG.md`, no `_typos.toml`,
   no `docs/stranger-test.md`, no caller workflows, no `.github/accepted-words`. The release gate
   refuses while the stranger run reads `none`.
   Evidence: `crates/intrusive_queue/` holds `Cargo.toml`, `src/lib.rs` and one patch.
   Change: the scaffold, as every moved crate has taken it, with the README following
   `scaffold/README-crate.md`. The README has an `EXAMPLES` section (the run-queue example that is already a
   doctest) and a "What is proven" list of one line. It has "Building and testing" with the three bare-metal
   targets, the Kani install line on one line and the measured 16 seconds and 197 MB. It also has the one
   falsification command sequence with its real file name, and `BUGS`. `FIFO` and `TCB` go in
   `accepted-words` or expand at first use (item 12). Then a stranger run on record.
   Door: two-way. Effort: S.

### P1: before the first release

4. `Queue<T>` is `Send` for every `T`.
   Finding: `unsafe impl<T: Node> Send for Queue<T> {}` has no `T: Send` bound. The tree's audit
   accepted this while the crate was internal, on the premise that the kernel is the one user and
   the crate is not published; the move ends that premise. Every comparable container bounds it:
   `std::collections::LinkedList<T>` is `Send` only for `T: Send`, and `intrusive-collections`
   writes `unsafe impl<A: Adapter + Send> Send for SinglyLinkedList<A>` (read from its source).
   Soundness today rests on rule 3 forbidding any other access while a queue holds a node, which
   is a contract a published crate cannot see its callers keep. The bound is satisfiable now:
   `kernel::thread::Thread` is already `Send`, and both consumers type-check with the bound added.
   Evidence: `src/lib.rs` lines 160 to 163; `kernel/src/thread.rs` line 713; the audit's finding 9.
   Change: `unsafe impl<T: Node + Send> Send for Queue<T> {}`, and a doc sentence on `Queue` saying
   why. `Unqueued<T>` stays `!Send` by construction (`NonNull` is), which the doc can also say.
   Door: one-way. A bound added after release is a breaking change; one removed later is not.
   Effort: XS. Needs the architect: see decision 1.

5. The documentation teaches a minting shape that both aliasing models retire.
   Finding: `Unqueued::new`'s safety list and rule 3 ask only that no reference to the node be
   live across a queue operation. Every example in the crate mints the token from a reborrow:
   `NonNull::from(&mut a)` in the `Queue` doctest, `NonNull::from(&mut **n)` in the test helper,
   `NonNull::from(&mut nodes[i])` in the harness. Under Miri, with Stacked Borrows and with Tree
   Borrows, one sequence has undefined behavior. The caller mints a token from a reborrow. A later `&mut` to the same node
   comes from its owner and drops before the next queue operation. The undefined behavior occurs at that operation.
   The later reborrow retires the one the token came from, and the queue's write through the token
   fails the retag. The same sequence with the token copied from the owning raw pointer
   (`Box::into_raw`, or the raw pointer a table stores) passes both models. So the rule the crate
   needs is stronger than the one it states: the token must carry the owner's provenance, and the
   program may not use a reference derived before a queue operation after that operation. The kernel mints the retiring
   shape: `mint_token` takes `NonNull::from(&mut *t)` with `t` from `get_mut`, and `get_mut` runs
   again on queued threads as a matter of course. `notes/intrusive-queues.md` records the
   tension in its aliasing paragraph. It names `UnsafeCell` links as the eventual fix. This
   experiment suggests the problem is elsewhere: the provenance of the token is.
   Evidence: `src/lib.rs` lines 7 to 9, 74 to 81, 136, 293 to 298, 369;
   `kernel/src/sched/thread_table.rs` lines 253 to 258 and 100 to 104. The two Miri tests live in
   the review's scratch copy and appear in the appendix below.
   Change, in the crate: a fourth clause in `Unqueued::new`'s safety list ("`node` is the owner's
   pointer, or derived from it by raw-pointer operations; a pointer derived from a reference is
   retired by the next reborrow of that reference's parent"). The doctest and the test helper mint
   from `Box::into_raw` or `&raw mut`. A Miri-run test of the table-walk pattern in
   `tests/` so the template's weekly `miri` job guards it. Change, in the kernel, as a follow-on
   the move does not wait for: `mint_token` mints from the stored raw pointer
   (`self.table.get(entry).0`), not from `get_mut`. Considered and not recommended: a
   reference-free `Node` trait in the shape of `cordyceps::Linked` (unverified), which would stop
   the queue forming `&mut T` to a node. The experiment shows the token's provenance decides the
   outcome. A `&mut` held across an operation has undefined behavior under either trait shape.
   Door: the documentation and the tests are two-way; the trait shape, had it changed, would count as
   one-way, and the review recommends against it. Effort: M across the crate and the kernel.

6. Seven public names lack ratification or a record, and the move publishes them.
   Finding: `Unqueued` is `provisional` and crosses into both consumers (`script/names
   --unratified`); its `new`, `as_ptr` and `as_non_null` are the lane's own; `Node`, `next`,
   `set_next`, `push_back`, `pop_front`, `is_empty` and `len` carry no block, and the ratified block
   covers the crate and `Queue`. The method names already follow the standard-library rule
   (`VecDeque` spells `push_back`, `pop_front`, `is_empty`, `len`; `NonNull` spells `as_ptr`).
   Evidence: `src/lib.rs` lines 11, 49 to 53, 64 to 66, 83, 90, 96.
   Change: one ruling on the list, then the blocks leave with item 2. The review proposes no rename.
   Door: one-way (published names). Effort: XS after the ruling. Needs the architect: see
   decision 2.

7. The compile-fail doctest accepts any compile error.
   Finding: the `push_back` doctest carries ```` ```compile_fail ```` and states `E0382` only in
   a comment, so a future edit that breaks the example some other way still passes. Rustdoc checks
   the code only when the attribute names it (`compile_fail,E0382`), and checks it on nightly only (unverified
   that stable still ignores it).
   Evidence: `src/lib.rs` line 192.
   Change: ```` ```compile_fail,E0382 ````.
   Door: two-way. Effort: XS.

8. The crate does not document dropping a non-empty `Queue`.
   Finding: `Queue` has no `Drop` and says nothing about dropping it while it holds nodes. The nodes stay linked to one another and their tokens are gone, so queueing any of them again needs
   a fresh `unsafe` mint. A later push of a node still carrying a stale `next` has undefined
   behavior only if the mint lies; it is otherwise a quiet loss. The consumers rely on moving
   queues by value (`core::mem::take` through `Default`, `*bucket = kept`), so a panicking `Drop`
   would be wrong. The documentation is the fix.
   Evidence: `src/lib.rs` lines 154 to 158, 245 to 249; `crates/inter_process_communication/src/futex.rs` lines 95 to 110.
   Change: a paragraph on `Queue`. Dropping a non-empty queue strands its nodes exactly as dropping
   a token does. `Default` exists so a caller can take a queue by value. `#[must_use]` on `Queue::new`
   adds nothing since `#[must_use]` sits on the token.
   Door: two-way. Effort: XS.

### P2: later

The P2 items are in [the appendix](1097-intrusive-queue-outside-review/later-items.md).

## Decisions for the architect

Two one-way doors. Each is independent of the other.

### Decision 1. Does `Queue<T>` stay `Send` for every `T`, or only for `T: Send`?

Background. A `Queue<T>` is two pointers and a count; it owns no node. Rust decides whether a type
may cross a thread boundary from its fields, and raw pointers never cross by themselves. So the
crate writes the answer by hand. Today it says every queue crosses, whatever its node type. The
tree's audit accepted that for an internal crate whose one consumer keeps every access behind a
lock. A published crate cannot see its callers. Suppose a caller's node type is `!Send` because it holds
something that must stay on one thread. A queue of those nodes crossing threads is exactly what
`!Send` exists to stop, and the crate today lets it through in safe code. Nothing in the kernel
depends on the looser form: `Thread` is `Send`, and both consumers type-check with the bound.

Recommendation: bound it, `T: Node + Send`. The stricter form is compatible with every current
caller and a later release can loosen it without breaking anyone; the looser form, once published,
needs a major version to tighten.

Prior art: `std::collections::LinkedList<T>` is `Send` only when `T: Send`;
`intrusive-collections` bounds `SinglyLinkedList<A>` on `A: Send` (read from its source);
`Vec<T>`, `Box<T>` and every std container do the same.

Concrete example: a node type holding an `Rc<Log>` beside its link, queued on one thread and the
queue handed to a worker thread, which pops and reads the log. Today it compiles. With the bound it
does not, which is the right answer.

### Decision 2. The seven public names the release publishes

Background. The ratified block covers the crate name and `Queue`. `Unqueued` is provisional, its
three methods are the lane's, and `Node`, `next`, `set_next`, `push_back`, `pop_front`, `is_empty`
and `len` carry no status. After the first release every one is a name other code spells, and a
rename is a major version.

Recommendation: ratify them as they stand. `push_back`, `pop_front`, `is_empty` and `len` are
`VecDeque`'s spellings, which the standard-library rule asks for. `as_ptr` is `NonNull`'s. `Node`
says what the implementing type is; the alternatives in the field name the glue rather than the
thing (`Adapter` in `intrusive-collections`, `Linked` in `cordyceps`). `Unqueued` says what the
token proves, and it reads correctly at every call site that passes or receives one. `as_non_null`
mirrors `Box::as_non_null` (unverified whether that method is stable yet).

Prior art: the rows of the prior-art table above; `heapless::spsc::Queue` spells `enqueue` and
`dequeue`, a spelling the standard-library rule weighs against.

Concrete example: `let first = ready.pop_front()?; inbox.push_back(first);` reads as the thread
leaving one queue for another, with the token named nowhere because the types carry it. A name
that said `dequeue` or `take_front` would read as well but would stop matching `VecDeque`.

## Done when

The checks are in [the appendix](1097-intrusive-queue-outside-review/done-when.md).

## BUGS

- The kernel-side change of item 5 (`mint_token` from the stored raw pointer) came from two
  Miri experiments on standalone nodes, not from a Miri run of the kernel, which has no Miri
  target. The crate's documentation change does not wait on it.
- Item 9 reopens nothing the architect ruled: the note that chose drain-and-rebuild is a note, and
  the timed-wait note already expects a `remove`. If the maintainer prefers to keep the surface at
  four methods, the five consumer loops stand and item 9 closes as declined.
- The `cordyceps` claims in the prior-art table are from memory; its documentation pages did not
  resolve during the review.
- This review ran no mutation census; the 19-mutant list is what `cargo mutants --list` prints. The
  template's `check / mutation-diff` and the weekly `mutation` run give the first figures after the
  move.
- `unsafe_scan` counts the `#[cfg(kani)]` blocks as shipped source, by its own design; the crate
  gets no fuzz opt-in either way, and that is the ruled outcome for a crate with `unsafe`.

## Appendix: the two Miri experiments behind item 5

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

## Index row

An outside review of `intrusive_queue` before its first release. Three P0 items are the
move's own skeleton (manifest, front page, scaffold). Five P1 items follow (a `Send` bound the move makes
necessary, a provenance clause the aliasing models demand and the kernel's mint violates, seven
names to ratify, a doctest that names its error, a drop rule). Five P2 items close it (a proved `remove`
behind five consumer loops, link hygiene in the harness, std traits, prose, test support).
