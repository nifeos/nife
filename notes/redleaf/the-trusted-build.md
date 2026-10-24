# The trusted compilation environment, and the safe-Rust rule the public tree does not enforce

*Name: provisional. For the trusted compilation environment it describes.*

An appendix to [`notes/redleaf.md`](../redleaf.md), under "Bet one: what each system has to
trust". It holds what RedLeaf's safety across separately compiled domains depends on, and the audit
of the 2026-09-20 clone that found the safe-Rust rule unenforced.

Two admissions sit beside RedLeaf's statement of what it trusts. On unsafe code: *"At the moment, we do not address vulnerabilities in
unsafe Rust extensions, but again speculate that eventually all unsafe code will be verified for
functional correctness."* On hardware: *"We trust devices to be non-malicious. This requirement can
be relaxed in the future by using IOMMUs to protect physical memory. Finally, we do not protect
against side-channel attacks."*

And the trusted compilation environment is the part that is easiest to underrate. RedLeaf's
safety across separately compiled domains depends on it: *"RedLeaf relies on a trusted compilation
environment. This environment allows the microkernel to check that domains are compiled against the
same versions of IDL interface definitions, and with the same compiler version, and flags. When a
domain is compiled, the trusted environment signs the fingerprint that captures all IDL files, and a
string of compiler flags. ... Additionally, we enforce that domains are restricted to only safe Rust,
and link against a white-listed set of Rust libraries."* By their own PLOS '21 assessment, this is
where the design is weakest: *"development of a trusted build environment is incomplete and error
prone"*, and their own prescription is "Research: Support typed assembly language for Rust" and
"Ecosystem: Support trusted build environments."

## The public tree does not enforce the safe-Rust rule

Checked rather than assumed, on the
2026-09-20 clone: of 23 domain crate roots outside `domains/lib/`, seven carry an active
`#![forbid(unsafe_code)]`, four have it present but commented out, and twelve never had it.
The four commented-out ones include both flagship drivers (`domains/sys/driver/ixgbe/src/lib.rs`,
`domains/sys/driver/nvme/src/lib.rs`), the shadow block driver
(`domains/usr/shadow/bdev/src/lib.rs`), and the Rv6 network domain. Six domain files outside the
trusted `domains/lib/` tree contain real `unsafe` blocks, `ixgbe` among them. Nothing in the
`Makefile`s forbids it, and the `tools/signer` binary is 31 lines that append an Ed25519 signature to
an ELF, which is the *version* half of the fingerprint and not the *safety* half. This is a research
prototype and the gap is unsurprising. It is recorded because the paper's central invariant and the
artifact's enforcement of it are not the same thing. A comparison that took the paper's word for it
would be comparing against a system that does not exist.
