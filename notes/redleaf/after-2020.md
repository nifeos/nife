# What happened to RedLeaf after 2020

*Name: provisional. After the parent's "What happened after 2020".*

An appendix to [`notes/redleaf.md`](../redleaf.md). It holds the state of the public repository,
the authors' own critique a year later (PLOS '21), and the group's next kernel, Atmosphere, which
took the other bet.

The repository is public, unarchived, and stopped. `github.com/mars-research/redleaf`, read
2026-09-20 through the GitHub API: last push 2022-05-09, and that commit is `add paper link`; the
last substantive commit is 2022-01-09. 144 stars, 35 open issues, `archived: false`, and the API
reports no license, which for a reuse question means "all rights reserved" rather than
"permissive". The pinned toolchain is `nightly-2021-12-15`, so a build today needs that exact nightly.
Measured on the clone: 48,749 lines of `.rs` across the tree (kernel 10,364; domains 19,818; interface
2,499; lib 14,621; tools 1,370), which excludes the `redIDL` compiler, a git submodule this lane
did not fetch.

The authors published their own critique a year later. *Isolation in Rust: What is Missing?*
(PLOS '21) reads as a list of the places RedLeaf had to work around the language, and it is more
useful to this tree than the OSDI paper in one respect: it says what the design cost. On the IDL:
*"lacking support to cleanly express the necessary invariants for exchangeable types in the Rust type
system, RedLeaf relies on a separate, complex interface definition language (IDL) that enforces these
isolation invariants outside of Rust."* On unwinding: *"RedLeaf implements costly continuations to
unwind execution across domain boundaries."* On the type identifiers their shared-heap destructors
depend on: *"this implementation of RTTI does not guarantee collision freedom, an attacker can
generate a type with a colliding identifier to trigger an unsafe deallocation of object."* Their asks
of the language are concrete: trait bounds on function pointers of any arity, type information in
procedural macros, a collision-free unique type identifier, and a `no_std` extendable unwind library.
As of this reading none of the four has landed, which is the honest reason RedLeaf's mechanism
has not been picked up elsewhere.

And the group's next kernel took the other bet. Atmosphere is *"a full-featured microkernel
developed in Rust and verified with Verus"*, *"conceptually similar to the line of classical L4
microkernels, i.e., without the capability interface"* (KISV '23). It uses hardware isolation, a
conventional syscall interface, and a proof effort rather than a compiler assumption: SOSP '25 reports
*"6K lines of executable code"* against *"20.1K lines of proof code"*, a 3.32:1 proof-to-code
ratio, *"less than 2.5 person-years"*, and full verification *"in under 20 seconds"* on an i9-13900hx.
Their own Table 1 puts that beside seL4's 20:1 and CertiKOS's 14.9:1.

That is the single most consequential fact in this note, and it is about neither paper's benchmarks.
The team that built the strongest language-isolated OS in the literature spent the following five
years building a hardware-isolated, machine-verified Rust microkernel instead. Nothing in either
Atmosphere paper says RedLeaf was abandoned or why, and this note does not claim it: the two lines
share authors and could be complementary. What can be said is what the artifacts show, which is that
the Verus-verified L4-shaped kernel is the one still being published and the RedLeaf tree has not
moved since January 2022.
