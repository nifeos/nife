# What Verus is

*Name: provisional, after the parent's own heading.*

An appendix to [`notes/verus.md`](../verus.md). It holds what the tool is and how it is released,
why deductive verification differs from Kani's bounded checking, what a human writes, its
sequentially consistent concurrency model, and which of this tree's Rust it would not parse.

A rustc driver against a forked compiler, not a separate tool. The OOPSLA paper, Section 7: *"We forked
the Rust compiler to introduce additional hooks and typechecking rules. We then implemented Verus as
a separate 'driver' that links against the Rust compiler."* Its `rust-toolchain.toml` asks for
`rustc-dev` and `llvm-tools`, which is the private-API build. The release ships a `verus` binary plus
that `rust_verify` driver, and it demands one exact toolchain:

```
verus: required rust toolchain 1.98.1-aarch64-apple-darwin not found
```

Stable 1.98.1, pinned to the patch version. This tree pins a nightly in `rust-toolchain.toml`
and uses nightly features (`custom_test_frameworks` among them, per AGENTS.md). Those are two
different compilers, and the verified crate has to build under the verifier's.

Releases are dated rather than numbered: the one this lane ran is `release/0.2026.09.20.aef82ed`,
published the day it was used, and the GitHub releases are marked pre-release. There is no 1.0,
and the project says so itself. `README.md`: *"Features may be broken and/or missing, and the
documentation is still incomplete."* The guide's feature table (last updated 2026-05-13): *"Note that
Verus is in active development. If a feature is unsupported, it might be genuinely hard, or it might
just be low priority."* And its `project-goals.md` lists among non-goals *"verify the verifier
itself"* and *"support all Rust features and libraries (instead, we will focus a subset that is easy
to verify)"*.

Compilation is a second step that the same tool can do: `cargo verus verify` checks without
producing a binary, `cargo verus build` *"Verifies all opted-in crates and compiles them to native
artifacts"*, and Verus-annotated code *"can also be built with a normal `cargo build` command"* once
the ghost code is erased.

## Deductive, not bounded, and that is the whole difference from Kani

Kani compiles a crate to a CBMC goto-binary and asks an SMT solver whether any input within a bound
reaches a failing assertion. Loops need unwind bounds. `notes/verification.md` and
`notes/user-proofs.md` are full of the consequences:

- a bound on a sum of 32 symbolic values that finished on neither CaDiCaL nor Z3;
- a symbolic index into a 3.5 KB struct that ran out of memory;
- the 64-bit division in the calendar costing more than the logic around it.

Verus asks a different question. The human writes `requires`, `ensures`, `invariant` and `decreases`,
and Verus discharges the verification conditions in Z3. There is no unwind bound: a loop is
handled by the invariant the human supplies, and termination by the `decreases` clause the human
supplies. The OOPSLA paper, Section 2: *"Given preconditions, postconditions, and loop invariants, Verus
uses standard weakest precondition reasoning [Dijkstra 1975] to generate a verification condition for
`fibo_impl`. It then sends this verification condition to the Z3 SMT solver."* A loop invariant is
checked once, inductively, and holds for every iteration count; a bound on iterations never appears.
SOSP '25 Section 2: *"Verus translates the Rust program into SMT expressions, which are then
passed to a solver (Z3 in case of Verus)."* And: *"To improve verification speed, Verus leverages the
idea that it is possible to use the type system, and specifically the linear type system of Rust, to
reason about aliasing and memory instead of SMT solving. This reduces the complexity of the SMT
queries by orders of magnitude."*

The trade is exactly the one that decides this whole question. Kani's harnesses cost almost
nothing to write and are bounded by what the solver can chew. Verus's proofs are unbounded and cost
the specification. The tree's Kani harnesses are typically twenty lines against code nobody
annotated; Atmosphere paid 20.1K lines of proof for 6K lines of kernel.

## The human's side of the contract

From SOSP '25 Sections 2 and 4, and confirmed by running it here:

- `spec fn` defines the abstract model; `proof fn` carries the argument; ghost and
  tracked variables exist at proof time and are erased. *"Specifications and proofs (ghost code)
  are erased by Verus using the Rust procedural macro during the compilation time, and thus do not
  incur any runtime overhead."*
- Verified code is a dialect. *"Executable code is written in a subset of Rust, while
  specifications and proofs are written in a functional extension of Rust."* Everything to be
  verified lives inside a `verus!{ ... }` macro.
- `int` and `nat` are not `i64` and `u64`, and the first thing this lane wrote failed on it:
  `expected 'nat', found 'int'` for `va + (count - 1) * 4096`, because subtraction leaves `nat`. That
  is representative of the annotation burden: the specification is written in mathematics and the
  code in machine words, and the human closes the distance.
- Raw pointers need permissions. `PPtr<T>` is the pointer and `PointsTo<T>` is a linear ghost
  token granting access. SOSP '25 Section 2: *"To use a raw pointer in Verus, the code must prove that it
  possesses the corresponding tracked permission to the pointer, and the permission is
  initialized."*
- Mutation through `&mut` is barely supported. SOSP '25 Section 5, TCB item 7: *"Verus currently has
  very limited support for `&mut`. This adds 300 lines of executable code with 1900 lines of
  specifications."* KISV '23 says the same from the other side: *"At the moment, Verus does not
  support returning mutable references from functions and support for mutable references in function
  arguments is limited to special cases."*

## Concurrency, and the one sentence that matters most to this tree

Verus has real concurrency machinery. It has `tracked` ghost state, and `LocalInvariant` and
`AtomicInvariant` containers opened around an operation. It has `atomic_ghost`, pairing a hardware
atomic with a ghost token, and the VerusSync / tokenized-state-machine method for proving an inductive invariant over shared
ghost state rather than enumerating interleavings. `vstd::invariant`'s own doc comment states the
price: an `AtomicInvariant` *"can be only opened for the duration of a single sequentially consistent
atomic operation"*, and the block *"cannot contain any exec-mode code with the exception of a single
atomic operation"*.

And there it is: sequentially consistent. The OOPSLA paper, Section 9, comparing itself to RustBelt:

> RustBelt can also handle atomics with relaxed memory ordering [Dang et al. 2020], which Verus does
> not support.

`vstd::atomic`'s wrappers hard-code `Ordering::SeqCst` on every load, store, swap and
`compare_exchange`; the subagent that read that file found no `Relaxed`, `Acquire`, `Release` or
`AcqRel` in it.

DECISIONS §4 (kernel shape: monolithic, deferred, with two cheap rules) rule 4, assume weak memory
ordering, is this project's fourth codebase rule, and its
stated reason is that being ARM-first is a gift because "we cannot develop hidden strong-ordering
assumptions the way an x86-first project would." A verifier whose concurrency model is sequential
consistency would hand back exactly the assumption this tree wrote a rule to avoid acquiring. That
does not make Verus useless for concurrency here, and milestone 80 (loom: the hand-rolled atomic
protocols, model-checked) already points a tool at interleavings here. It does mean a green Verus
proof over a `Relaxed` atomic in this kernel would not mean what a reader would take it, and is the
sharpest nife-specific caveat in the note after the hardware boundary.

## What of this tree's Rust would not parse

The guide's feature table puts eight things in "not supported":

- `async`/`await`, function pointer types and `transmute`;
- hardware intrinsics, and printing and I/O;
- `Debug` derives and user-defined `Drop`;
- and the standard `Mutex`/`RwLock` (vstd offers verified alternatives).

"Partially supported" includes
`as` casts, `const` generics, `for` loops, trait objects (`dyn`), `impl` types, iterators, raw
pointers, closures (*"no mutable captures"*), and multi-crate projects.

This lane did not test any of it against nife code. Several of those are load-bearing here: the
capability lifecycle uses `Drop`, the driver layer is generic over traits, and `as` casts are
everywhere in page-table arithmetic. "Hardware intrinsics: not supported" is the same boundary as
`asm!` arriving by another door.
