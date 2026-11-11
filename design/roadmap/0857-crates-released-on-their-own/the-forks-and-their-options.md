# The forks and their options

<!-- writing-standards: exception. Marked 2026-11-11 (UTC) by lane maintainer/queue-entry-lint-guards. Reason: this text moved verbatim out of milestone 857's block, where the passive baseline held it, to keep that block under its word cap; rewording a record while moving it would change what it says. -->

An appendix of [milestone 857 (crates released on their
own)](../0857-crates-released-on-their-own.md): the eight forks as they were raised, with the options
weighed for each. The rulings in force are on that page, under The rulings.

## Fork 1. Where a released crate lives (ruled: one repository per crate)

| option | shape | prior art | cost |
|---|---|---|---|
| R-a. Publish from this repository | Each crate keeps its path; `cargo publish -p` from a workflow here | `wasmtime` publishes its `crates/` directory (38 entries) from one repository | A stranger's issue lands in an OS tracker. No move, so §235 holds |
| R-b. One repository for every released crate | `nifeos/<name>`, one workspace | google/gpt-disk-rs holds three crates | A move before 610's column exists |
| R-c. One repository per crate | `nifeos/<crate>` each | rust-osdev (`x86_64`, `uefi-rs`), rust-embedded-community | Nine repositories, each with its own CI, and two pull requests whenever a driver and its crate move together |
| R-d. One repository per family | Two today: device logic (seven crates) and formats (FAT, PE) | RustCrypto: `hashes` holds 23 crates | A move before 610's column exists; the family lines are a guess until it does |

Recommendation as raised, replaced by the ruling: R-a now, R-d once 610 measures co-change.

Prior art beyond the table. seL4 keeps its proofs in a repository apart from the kernel (`l4v`,
recalled rather than re-read), and its Rust crates are consumed by git, not from crates.io. The
rust-embedded working group gives each crate its own repository and owns them through GitHub teams.
Fuchsia vendors third-party crates in its own tree and publishes few of its own (recalled). None of
them publishes proofs beside the crate, which is the part this tree would add.

## Fork 2. How nife consumes a crate that has left (ruled: C-a, `[patch]` only in development)

| option | prior art | cost |
|---|---|---|
| C-a. crates.io version, pinned in `Cargo.lock` | RustCrypto, rust-osdev | `cargo-deny` and advisories work against it, which is the reason §46's 2026-07-31 amendment prefers registry crates |
| C-b. git dependency at a commit | seL4's `rust-sel4`, whose README says its crates are not on crates.io | A crates.io crate cannot depend on a git crate, so nobody can publish on top of it |
| C-c. path, as today | this tree | Only while the crate is in this repository |

Recommendation as raised, and ruled with a `[patch]` rule added: C-c while a crate is here, C-a after. C-b would block exactly the third parties §151
names as the goal. §305 (git dependencies on `nifeos/*` only) takes C-b for the soak before a
first publish.

## Fork 3. Versions, and what a release carries (ruled as recommended)

Recommendation as raised, and ruled:

- Each crate carries its own semver version and starts at 0.1. Nothing moves in lockstep.
- Each crate states a minimum Rust version.
- A release ships its Kani harnesses (behind `#[cfg(kani)]`, as now) and its `falsifications/`
  patches. A proof a stranger cannot rerun is a claim, and the patches show the proofs can fail.
- `publish = false` becomes the workspace default, and a lint check allows publishing only for
  crates on a list calef ratifies: rung 2 of the ladder in place of rung zero.

## Fork 4. How proofs run where the crate lives (ruled as recommended)

- P-a. In this repository, `script/verify` and the falsification workflow as today.
- P-b. In a released repository, `model-checking/kani-github-action`, which Kani's maintainers
  publish, plus a job that applies each falsification patch and expects Kani to fail.
- P-c. Proofs stay here and the released crate carries none.

Recommendation as raised, and ruled: P-a while a crate is here, P-b after a move. P-c is refused: it publishes the code without
the evidence, and the evidence is milestone 39's reason to publish at all.

## Fork 5. The crates.io names (ruled: names that stand on their own)

Options only, because a crates.io name cannot be taken back: a crate can be yanked but its name
stays.

- N-a. This tree's names, prefixed only where taken.
- N-b. One prefix for all, as `sel4-*`, `cranelift-*` and `embassy-*` do. It says who stands behind
  the proofs. It also says "nife" to a reader who wanted a GPT parser.
- N-c. A short name chosen per crate, as RustCrypto does (`sha2`, `aes-gcm`). Readable, and every
  one is a separate naming decision.

## Fork 6. Who publishes (ruled: W-a)

- W-a. crates.io trusted publishing from a `nifeos` workflow (RFC 3691; the
  `rust-lang/crates-io-auth-action` action exists since 2025-06-26). Short-lived tokens, no secret
  stored, and the owner is a `nifeos` GitHub team.
- W-b. calef's own token, by hand.

Recommendation as raised, and ruled: W-a. It is the same reasoning §220 (signed builds) applied to keys: no
long-lived secret where a workflow can mint one.

## Fork 7. Each crate's disposition (ruled: one crate at a time)

The note's table classes every A and B crate as contribute upstream, publish ours, or keep internal,
with the evidence for each. The ask is to approve the classes, not to send anything. Every upstream
approach and every first publication then comes to calef one at a time, with its text, as the
upstream-hardening ruling of 2026-10-06 requires. A security finding in another project goes to
its maintainers privately first.

Ruled 2026-11-01 (UTC): internal is at most a short-lived state, not a standing class, and
[the survey note](../../../notes/releasable-crates-2026-10-07.md) reclassifies every row.

Recommendation as raised, replaced by one review per crate: approve the table whole. The survey
note keeps each row's reasoning.

## Fork 8. Who writes an offer to another project (ruled: A-a without calef's hand, bug reports everywhere)

The survey found contribution policies, not licenses, to be the binding constraint, and its note
lists each project's.

- A-a. Approach only projects whose policy admits agent-written code with disclosure. calef writes
  the message where a project asks for a person's words, and the commit carries no co-author
  trailer where that is asked.
- A-b. calef writes and submits every offer himself, and an agent only prepares the patch.
- A-c. Offer findings only: an issue describing a bug and the harness that found it, never code.

Recommendation as raised, replaced by the ruling: A-a, with A-c for projects that refuse AI work. A refusal is the owner's call,
and a bug report is still welcome under every policy read. Each approach is a fact that leaves the
machine, so the policy is reversible and each use of it is not.
