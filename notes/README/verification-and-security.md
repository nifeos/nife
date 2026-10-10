# Notes index: Verification and security

Part of [the notes index](../README.md), which says how to add a line.

- [Machine-checked proofs (Kani)](../verification.md): how the Kani proofs work, and what they prove.
- [Proving things about `kernel/src`](../kernel-proofs.md): proving kernel code, and the stub boundary. Name provisional.
- [Proving things about `user/`](../user-proofs.md): proving the EL0 programs, and what it found. Name provisional.
- [Verus, and whether it reaches the code Kani stops at](../verus.md). Name provisional.
- [Upstreaming the riscv64 target to Kani](../kani-upstream.md): the branch, and the pull request text. Name provisional.
- [Did the proofs catch the bugs?](../proof-retrospective.md). Name provisional.
- [Does a standing proof notice a regression?](../kani-reach-2026-10-04.md): every mutant in a harness's reach, proved; which proofs can fail. Name provisional.
- [Have the Kani proofs ever failed in CI?](../kani-catches-2026-10-06.md): 9,794 verify runs, no harness failure on a pull request. Name provisional.
- [Falsification records](../falsification.md): recording that each proof harness can fail.
- [Falsification coverage](../falsification-coverage.md): which kinds of test carry a replayed falsification, and what gates on it.
- [Fuzzing the parse surface](../fuzzing.md): coverage-guided fuzzing of the parsers that read outside bytes.
- [Fuzzing the services' request handlers](../fuzzing-the-services.md): session targets over the file server, the system log and the compositor, each checking a confinement rule.
- [Overflow checks](../overflow-checks.md): which builds panic on integer overflow and which wrap, what checking the shipped build found and costs, and the options (provisional name).
- [Dynamic undefined-behavior checking (Miri)](../undefined-behavior.md): Miri over the host crates, and what "clean" means.
- [Interleavings, model-checked (loom)](../interleaving.md): loom over the hand-rolled concurrency protocols, and its finds.
- [Mutation testing](../mutation-testing.md): the cargo-mutants triage rule, the current census, and per-crate triage in 17 appendices.
- [Untested error paths](../untested-error-paths.md): how many error paths no test executes, by crate and kind, and the twenty that release memory or authority.
- [The mutation census record](../mutation-census.md): per-crate mutation scores for every census, comparable. Names provisional.
- [Where an unsafe obligation is written, and where it is only implied](../unsafe-obligations.md).
- [What nife claims a confined component cannot do](../confinement-claims.md).
- [A second outsider pass over the confinement claims](../confinement-outsider-pass-2.md): each claim attacked, and where each attack landed.
- [A third outsider pass over the confinement claims](../confinement-outsider-pass-3.md): every claim attacked again, counted only when booted on three ISAs, and the one escape it found.
- [A fourth outsider pass over the confinement claims](../confinement-outsider-pass-4.md): the informed non-Anthropic attack. A booted re-discovery of the socket capture in milestone 649 (every client of a network stack shares its socket numbers), claim 26's own test made falsifiable, and a refusal log.
- [A sixth outsider pass over the confinement claims](../confinement-outsider-pass-6.md): the informed non-Anthropic attack on the newest shipped surface, the milestone 801 (packages over the internet) fetch path. A booted escape: the listed-location address check and the connection resolve the name twice and a rebinding DNS answers them apart, so the client reaches a private address Q1 forbids. In progress.
- [Verdict briefs for fatal risks 6 and 7](../fatal-risks-6-and-7-verdict-briefs.md): evidence and a recommended color each, for the architect to rule on.
- [A security audit](../security.md): the first adversarial review of the whole kernel.
- [Auditing the shared pages](../shared-page-audit.md): the second security audit, reading for double fetches.
- [Auditing untrusted counterparty input](../untrusted-input-audit.md): network and device input read as hostile.
- [Code scanning](../code-scanning.md): which languages CodeQL scans, why Rust is off, and every alert's disposition. Name provisional.
- [What each system makes you trust, measured](../trusted-base.md).
- [The incremental path to a safer kernel, and why nife is not on it](../incremental-path.md). Name provisional.
- [RedLeaf, and the opposite bet about where isolation comes from](../redleaf.md).
