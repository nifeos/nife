# §123 appendix: option (c), a single-use bit

One of the options [§123 (boot-time re-derivation)](../0123-boot-time-rederivation-privilege.md)
considered. Moved out of that page on 2026-10-12 (UTC) under §212 (a prose budget). No ruled word
changed.

**(c) A capability minted with a kernel-enforced single-use bit (invoke once, then the kernel marks
the slot dead).** Considered and rejected as unnecessary machinery. This would be a new kernel
primitive on the capability surface, which AGENTS.md treats as one of the expensive, hard-to-undo
categories ("the system call surface... is a boundary rather than a habit"). It also solves a problem
`cap_delete` already solves for free: a capability the holder has deleted from its own cspace is
already "invoke once, never again," without a new bit, a new invariant to prove, or a new Kani
harness. Building a bespoke single-use mechanism when ordinary deletion gives the same guarantee is
the kind of "more abstraction, more machinery" this tree's elegance tenet warns against, not more
elegant for having a named feature.
