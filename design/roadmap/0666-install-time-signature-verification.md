---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: install-time-signature-verification
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 666. A signed build installs up to its key's ceiling, and removing the key drops what it vouched

*Title and slug are drafts.*
Filed with calef's rulings on
§220 (signed builds: a vendor signs, a developer self-signs, and trusting a key is scoped), in
[`design/decisions/0220-signed-builds-and-scoped-key-trust.md`](../decisions/0220-signed-builds-and-scoped-key-trust.md). This is the build those rulings describe. Every
name below is provisional.

§220 is decided. The dependency addition still reaches an architect at merge, as
every dependency does (see below).

It is not on fatal risk 8's path (nobody needs it). §220 records that a first customer installs by
digest vouching, which milestone 198 (a package manager) built on 2026-09-26. This serves a second
vendor, or a developer whose build crosses machines, and it waits behind anything that gets a real
workload running.

## What gets built

1. Trust lines. A table beside the activation set, versioned the same way (§208 (installing a
   package is granting it)), one line per key: `key ed25519 <public key hex> <ceiling> <label>`,
   where the ceiling is a subset of `domain entropy network`. Owner-written. No key in any image,
   and a test that proves the image carries none.
2. Install-time verification. `package install` of a build with a beside signature (`rg.sig` in the
   archive of §197 (a package is one archive file), `a.out.sig` next to a loose build) checks the
   Ed25519 signature over §220's 54-byte statement. It then pins the executable's digest as today.
   Spawn is unchanged: SHA-256 and a lookup.
3. Refusal, not narrowing. A manifest asking for more than the key's ceiling is refused, and the
   refusal names the missing grant.
4. The vouching key per entry. Each activation entry records which key vouched it, or that a digest
   did. That is a new column in `activation_set`'s entry line.
5. Removing a trust line writes a new generation without that key's programs. They fall to
   unvouched, runnable only under the D2 capability of §219 (how the shell names an installed
   program to the spawner), and rollback restores them.
6. A deny list of digests, owner-local or vendor-published. A denied digest is refused at install,
   and its installed copies are dropped in a new generation.
7. Revocation statements. A vendor statement "key K revoked as of T" drops K's installs made after T
   on a machine that fetches it. Who may sign one was not ruled; the lane proposes it.
8. Rotation. A statement signed by the old key names the new one (Android's proof-of-rotation
   shape). What happens to the old key's ceiling was not ruled; the lane proposes it.

Proven on all three architectures by the same suite (§19 (architectural parity is a tenet)), with a
host-tested crate for the statement, the table and the ceiling check.

## Where the check runs: a fork this proposal leaves to the lane

§220 recommended S2 so that no curve arithmetic sits in the process that builds every child. Since
milestone 198 the progenitor is the installer, so the check lands there unless it moves out.

- In the progenitor, on the install request only. One process, no new endpoint, and the ruling
  holds: nothing is verified at spawn. It still puts Ed25519 in the process §220 wanted kept small.
- A verifier service the progenitor asks at install. It holds the trust table and returns a verdict
  and a ceiling. The progenitor keeps SHA-256 alone, at the cost of a new component and an endpoint.

Both are reversible code. Measure the progenitor's text growth before choosing.

## The dependency

Ed25519 through `ed25519-dalek`, default features off. The 2026-07-31 amendment to §46 (thin
primitives or whole subsystems) made crypto an ordinary dependency, pinned in `Cargo.lock` and not
vendored. §220 records that taking it into the shipping graph is its own §46 decision, so the pull
request that adds it goes to `needs-architect`. `cryptography_provider` already builds it for all
three architectures, outside the main workspace. Which major line to take (2.2.0 there, 3.0.0 on
`docs.rs`) is unreconciled.

## Not in this build

- A signer on nife. §220 recommends none until a build made on nife must run elsewhere.
- Expiring trust statements, deferred by calef until the verifier has a clock it can trust.
- A transparency log (S3) or a signed catalog (S4), which wait for a second vendor.
- A machine that never fetches cannot learn of a revocation. §220 says so plainly; the ceiling and
  the install-time check are the bound.

## What an architect still owes before anyone outside signs

The statement's bytes and the algorithm were never asked, so this builds both provisionally. Both
become irreversible the first time someone outside the tree signs one. Ask before publishing a
format, not before building.

## Index row

DECISIONS §220 rules that a signed build installs up to its key's ceiling and that removing the key drops what it vouched. Proposed: the build that implements it, for a second vendor or a developer whose build crosses machines; it is off fatal risk 8's path.
