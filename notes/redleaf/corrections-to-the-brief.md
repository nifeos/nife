# Corrections to the sketch the RedLeaf lane was briefed with

*Name: provisional. After the parent's "Corrections to the sketch this lane was briefed with".*

An appendix to [`notes/redleaf.md`](../redleaf.md). It records, claim by claim, how the
maintainer's from-memory sketch of RedLeaf and the 2017 paper held up against the text.

The maintainer sketched this comparison from memory and flagged it as a sketch. Three of its five
claims survived checking, one is wrong, and one is a third of the real answer. Recording the misses
is the point of having checked.

Wrong: "the Rust operating system from the University of Utah." The RedLeaf paper is UC
Irvine, with one author at VMware Research; every one of the six academic authors is listed
"University of California, Irvine" on the title page. The Utah association is real but belongs to a
*later* system: Burtsev's group moved, and the Atmosphere papers (KISV '23, SOSP '25) are
"University of Utah." The confusion is easy to acquire, because the most-linked copy of the RedLeaf
PDF is hosted at `users.cs.utah.edu/~aburtsev/doc/redleaf-osdi20.pdf`.

Incomplete: "termination with heap ownership tracking." Ownership tracking is one of five
named principles, and on its own it does not deliver termination. The paper lists them as heap
isolation, exchangeable types, ownership tracking, interface validation, and cross-domain call
proxying. Ownership tracking only covers the *shared* heap; what makes termination safe is the
private heap invariant, and the paper is explicit that the private heap is freed without running
anything: *"the microkernel walks the registry of all untyped memory regions allocated by the
allocator assigned to the domain and deallocates them without calling any destructors. Such untyped,
coarse-grained deallocation is safe as we ensure the heap isolation invariant."*

Right: single address space, ring 0. Stronger than the sketch, in fact. *"As RedLeaf does not
rely on hardware isolation primitives, all domains and the microkernel run in ring 0."*

Right: domains must be Rust, and zero-copy `RRef`. *"Domains, however, are restricted to safe
Rust (i.e., microkernel and trusted libraries are the only parts of RedLeaf that are allowed to use
unsafe Rust extensions)."* `RRef<T>` is the shared-heap pointer: *"RRef<T> consists of two parts: a
small metadata and the value itself. The RRef<T> metadata contains an identifier of the domain
currently owning the reference, borrow counter, and type information for the value."*

On the 2017 paper the sketch was accurate except in one word, and the word matters. The thesis
quote, the garbage-collection objection, the four library abstractions, the six kernel pieces, the
`TakeCell` rationale and all three stated limits check out verbatim against the text. The size figure
was given as *"around 1000 lines out of over 6000"*; the paper says under 1000, and says it
inside a sentence whose subject is a TCB that also *"includes the Rust core library"*. Both details
cut the same way, which is that the headline ratio is smaller and the actual trusted base is larger
than the short form suggests. Detail is in [`the-2017-ancestor.md`](the-2017-ancestor.md).

Right, with a caveat the paper does not make: x86_64 only. The paper never states an
architecture claim. It does not have to: every experiment is on Intel, the interrupt entry discusses
*"the x86-interrupt function ABI"*, and the repository contains exactly one target
(`domains/x86_64-unknown-redleaf.json`, `kernel/x86_64-unknown-none.json`) and no other. So the
sketch is right about the artifact and is reading something into the paper that is not written there.
