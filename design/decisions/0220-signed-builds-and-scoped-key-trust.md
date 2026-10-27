---
status: AMENDED
raised: 2026-09-26
decided: 2026-09-26
ratified_by: calef
---

# 220. Signed builds: a vendor signs, a developer self-signs, and trusting a key is scoped

*Amended 2026-10-10 (UTC) by calef, on #1884: an image carries its own distribution's root as a removable owner trust line. See §195's amendment.*

Raised 2026-09-26 (UTC) by calef: *"I do wonder if there is a case for software vendors signing
their builds in their manifest and local developers would just self sign. Does that make sense?"*

A proposal lane (`proposal/signed-builds`) wrote this section on the maintainer's instruction. Every
name in it is provisional. Nothing here is built; the build is the proposal
[`design/roadmap/0666-install-time-signature-verification.md`](../roadmap/0666-install-time-signature-verification.md).

## Ruled 2026-09-26 (UTC)

calef answered both questions "Yes", recorded by `maintainer/220-ruling`, which invented no part of
it.

1. Trusting a key is an owner-written line per key with a ceiling, a subset of `domain entropy
   network`: the grants the progenitor endows from a vouched manifest. An install asking more than
   the ceiling is refused, not narrowed. Signatures are checked at install and the digest is then
   pinned (S2), never at spawn. No key ships in any image.
2. The signature travels beside the binary, not inside it. Under §197's M2 the manifest is already
   inside the hashed bytes, so a signature over the digest covers it. An in-file signature needs a
   hashed range that excludes it: a second digest definition, and parsing unverified bytes before
   hashing. Authenticode's MS13-098 is the precedent.
3. Revocation, as the maintainer laid it out and calef accepted it:
   - Each installed entry records which key vouched it.
   - Removing a key's trust line refuses future installs and writes a new activation generation
     without that key's programs. calef ruled "Yes" that this drop is automatic. The dropped
     programs fall to unvouched, runnable only by a session holding the D2 capability of §219
     (how the shell names an installed program to the spawner), and rollback (§208) undoes a
     mistaken removal.
   - A deny list of digests, vendor-published or owner-local, refuses installs and drops installed
     copies in a new generation.
   - A vendor statement "key K revoked as of T" drops K's installs made after T, on machines that
     fetch it.
   - A machine that never fetches cannot learn. There is no good answer to that, and it is bounded
     by install-time checking and the key's ceiling.
   - Legitimate rotation: the old key signs a statement naming the new one, Android's
     proof-of-rotation shape.

Deferred, not ruled: expiring trust statements (TUF's shape), until the verifier has a clock it can
trust. Not ruled either, and kept as a recommendation: build no signer on nife until a build made on
nife must run somewhere else.

For the architect, two things this record found and did not settle:

- The ruling as relayed said the dropped programs are runnable by a D2 holder "under §170's option
  A". §170's A is an argv and §170 is `PROPOSED`, so the record above cites §219's D2 alone. Correct
  it if another section was meant.
- The signed statement's bytes and the algorithm were never asked. The proposal builds the 54-byte
  statement and Ed25519 provisionally; both become irreversible when anyone outside signs one.

## The premise, checked

- Vouching is by digest today. The progenitor looks the executable's SHA-256 up in the live
  generation (`activation_set::lookup_digest`), and a miss runs only under D2's capability.
  Corrected at the ruling: since milestone 198 (a package manager, and the trivial install), 2026-09-26, the progenitor is also the installer
  (`package install`, notes/packages.md), so "the installer" below is a progenitor request today.
- §195 (a reviewed recipe vouches for a package) clause 4 says no long-lived signing key is held by
  anyone, for now. It deferred a publisher's signature (its T2) rather than refusing it. Anything
  below that adds a key amends clause 4.
- The manifest question is settled. calef ruled M2 of §197 (a package is one archive file) on
  2026-09-26: the manifest is a `PT_NOTE` in the executable (#1319). So the executable's digest
  already covers its manifest. M1, a sibling member, is refused.
- What a vouch confers is five booleans. `spawn_service` reads `clock`, `domain`, `config`,
  `entropy` and `network` from a vouched manifest (`wants_clock` through `wants_network`).
  Everything else a child holds comes from the caller's line. §219 already gives an unvouched child
  the clock and configuration pages. So a key's scope, today, is a subset of three things: the
  process domain, entropy and the network.
- "Vendor the crypto" is stale, and this section's brief repeated it. The 2026-07-31 amendment to
  §46 (thin primitives or whole subsystems) made crypto "an ordinary dependency", pinned in
  `Cargo.lock` and not vendored, so advisories reach it. `CLAUDE.md` rule 6 still says vendor.
- §170 (how a foreign program is told what to do) has no "confirm at the prompt" step for unvouched
  programs. Its only prompt output is a typed refusal. The nearest rule is in `notes/installing.md`:
  a confirmation is asked by the service that grants, never by the program asking for the grant.
- Per-session scope has a mechanism gap. Endpoints carry no sender identity (§26 (the fault
  endpoint), part 5, declined badges). So each distinct scope a session may hold is a distinct
  endpoint.

## What is signed

A signature covers a fixed statement, never a byte range of the file:

```
"nife-signed-build-v1\0" || algorithm (1 byte) || SHA-256 of the executable (32 bytes)
```

Under M2 the executable's digest covers the manifest note, so one statement binds the code and the
grants it asks for. It is the digest the activation set stores and the progenitor already computes.
Signing a digest rather than a file is TUF's and Sigstore's shape.

The statement is a format third parties sign into, so its exact fields are options below, not a
recommendation.

## Where the signature lives, priced against the `PT_NOTE` shape

| | Beside: a separate file | Inside: a second `PT_NOTE` the hash excludes |
|---|---|---|
| Where | a member `rg.sig` in the §197 archive; `a.out.sig` next to an unpackaged build | a fixed-size `.note.nife.signature`, reserved by the linker and filled in place by a signing tool |
| Digest | unchanged: SHA-256 of the file | new: SHA-256 with that note's descriptor zeroed. Two digests exist, and the activation set must say which it stores |
| Parsing before hashing | none | the note walk runs over unverified bytes to decide what not to hash |
| `crates/elf` | untouched | the walk #1319 prototyped (113 lines, 3 Kani harnesses) becomes load-bearing for integrity, plus new hole arithmetic to prove |
| Toolchain | a host tool writes a file | the linker must reserve the note. #1319 measured that `llvm-objcopy --add-section` makes a section with no program header, which a program-header reader cannot see. `user_mode_runtime/link.ld` gains a reserved note, and a foreign build links a placeholder object |
| Separable | yes: bytes without their signature are unvouched, which fails safe | no: one artifact |
| Under §219 D, verified at spawn | the shell sends 97 more bytes on `spawnproto` | travels with the frames for free |
| Under §219 D, verified at install | no wire change: the installer reads both files | no wire change |
| Prior art | IMA's `security.ima` attribute; Sigstore bundles; APK v4's `.idsig`; TUF and Fuchsia sign an index | Apple's `LC_CODE_SIGNATURE`; Authenticode's certificate table; APK v2 and v3's signing block |

Inside is a known hazard. Authenticode excludes its certificate table from the hash, and MS13-098
(CVE-2013-3900) let attackers add content in that unhashed padding. The strict check is still
opt-in, because too many installers broke. Zeroing a fixed-size descriptor rather than skipping a
range closes that hole: the length and every other byte stay covered. It does not remove the second
digest or the parse before the hash.

Placement cannot be taken back once a vendor signs into it. calef ruled Beside.

## How trust in a key is represented

The owner trusts a key by writing it into a table beside the activation set, versioned the same way
(§208 (installing a package is granting it)). One line per key:

```
key ed25519 <public key, 64 hex digits> <ceiling> <label>
```

`<ceiling>` is a subset of `domain entropy network`. The install is refused if the manifest asks for
more than the ceiling, and the refusal names the missing grant. Narrowing silently would give a
program that fails later with no reason shown. TUF's delegations add a second axis, path patterns,
which here would be package-name prefixes. Recommended: leave that out until a second vendor exists.

No key ships in the image. That is the line §195 drew: its T2 becomes irreversible "the day a public
key ships in an image somebody else runs". A key in the owner's table is removed by the owner.

## Options

| | What vouches | Where the check runs | Rebuild friction | Revocation | New dependency |
|---|---|---|---|---|---|
| S0. Digest only, D2 as ruled (today) | a digest in the owner's table; D2 for a session's unvouched bytes | progenitor, SHA-256 | a re-vouch per rebuild, or run under D2 without the network | remove the digest; roll back a generation | none |
| S1. Verify at spawn, key trust as a D2 variant | a trusted key's signature, up to its ceiling | progenitor, Ed25519 on every unlisted digest | none | remove the key: all it signed stops | Ed25519 in the progenitor |
| S2. Verify at install, then pin the digest | a trusted key's signature, once; the digest is then recorded as today | installer; the progenitor keeps SHA-256 and a lookup | one install command per rebuild, no human judgment | removing the key stops new installs; a deny line stops a pinned digest | Ed25519 in the installer |
| S3. S2 plus a transparency log | as S2, plus an inclusion proof from a public log | installer, plus the log's key | as S2, plus a round trip to the log | detection, not prevention: a stolen key's use is public | a log client, and somebody runs a log |
| S4. A source signs its catalog (TUF shape) | §195's reviewed recipe, signed per source | installer | none for a vendor's users; a developer's loose build is untouched | expiry and version metadata give freshness | a TUF metadata parser |

S4 is §195's deferred T2, which signs the index, not each package. A per-build signature serves
bytes with no catalog, which under §219 D is how a developer's build reaches another machine.

## Recommendations on the reversible parts

1. On nife itself, a developer does not sign. S0 with D2 covers edit, compile and run. If an
   unvouched build needs the network, the change is a ceiling on D2's capability, one endpoint per
   ceiling. That widens §219's ruling, it is calef's to widen, and it needs no cryptography.
2. Ruled: S2, not S1. The process that decides what runs keeps SHA-256 and a lookup. §26 wanted a
   signature "in addition to the measured root (so the hash stays the floor if key handling
   fails)", and S2 is that sentence built.
3. Ruled: the per-key ceiling above.
4. Per-session scope, the maintainer's framing, holds with one change. What a session holds cannot
   be "trust key K", because that needs sender identity the kernel lacks. It is an endpoint to an
   installer instance configured with the keys and ceilings that session may use. `login` hands it
   out per session, as §219 records for D2. The activation set is still one per machine, so what
   user A installs, user B can run with B's own grants. That is §208's shape and an existing
   limitation, not a new one.
5. S3 and S4 wait for a second vendor.

## Self-signing, and what it proves

A self-signed build proves authorship, not behavior. It confers what the owner mapped the key to,
and nothing by default.

On a host such as patagonia, the private key sits where the developer's SSH key sits (§78 (signed
commits) notes the same key for commit signing). The owner of the board adds the public half once,
with a ceiling. After that every rebuild installs with no re-vouch. That is the friction calef asked
about, removed where it bites: building on one machine and running on another.

On nife, a private key follows the rule of milestone 65 (a secrets service), kept in §79
(password-equivalent material): hold the key, expose the operation, never the key. A signing service
holds the key and a session holds a `SIGN` endpoint to it, the shape §41 (the endpoint is the
broker) gives every secret. But then the signature proves only "a session holding that endpoint
asked". D2's capability already says that. Where the key rests across a reboot is §165's answer (where a
stored secret comes from): typed at a console before the store is sealed, with an unattended boot
out of scope until a deployment needs it, and a signing key inherits it whole. Recommended: build
no signer on nife until a build made on nife must run somewhere else.

## Revocation

Ruled above. Every system read either checks online or has none. Apple revokes through OCSP.
Android only rotates, and a thief can sign a rotation too. IMA keeps a `.blacklist` keyring and
FreeBSD `pkg` a directory of revoked fingerprints.

## The crypto dependency

The candidate is Ed25519 through `ed25519-dalek` 2.2.0 with default features off. The tree already
carries it: `cryptography_provider` (milestone 442 (a crypto provider `rustls` can use)) verifies
Ed25519 for TLS, and stays outside the main workspace until its primitive set is ratified. Its note
records the crate building for aarch64 and riscv64, and for x86_64 with `sha2`'s `force-soft`
feature. §196 (nife carries TLS) counted 16 crates. The main `Cargo.lock` holds none of them;
`argon2` is its only cryptography. TUF lists Ed25519 first among its schemes. Android's and IMA's
pages, as read, do not list it.

Measured 2026-09-26 on patagonia, on the host and not on a target:

- One verification of a short message: 25.3 µs (20,000 runs, `fast` feature).
- A `no_std` verifier for `aarch64-unknown-none-softfloat` is 24,340 bytes of text (opt-level `s`,
  LTO, `panic = abort`).
- Bernstein's page gives 273,364 cycles per verification on a 2011 amd64 part. Target cycles are
  unmeasured.

Beside leaves the Kani-proven path untouched, and under S2 the progenitor gains no code. Inside
touches it: the note walk in `crates/elf` would decide what the hash skips.

Taking `ed25519-dalek` into the shipping graph is a separate §46 decision.

## The seven questions

1. What else was considered: the options table and the recommendations give each refusal its
   reason.
2. The analogous case in this tree: §26 measured the boot program by digest and declined a
   signature, naming key custody. §195 keeps digests and defers signatures. Milestone 65
   put a key behind an endpoint. The activation set is versioned and owner-written. S2 reuses all
   four.
3. Prior art outside the tree: read, and cited [below](#prior-art-read-2026-09-26).
4. The premise: mostly true. Three corrections above: crypto is depended on and not vendored,
   §170 has no confirmation step, and per-session key trust costs an endpoint per scope.
5. Cost: the figures above; target cycles and the installer's size are unmeasured.
6. Reversibility: nobody has acted; no key or signature exists. The split is in the next section.
7. At equal cost: yes for S2 over S1. S1's one advantage is no install step, and the reason for S2 is that no curve arithmetic sits in the process that
   builds every child. Yes for no signer on nife: the reason is fewer moving parts, not effort. For
   Beside against Inside this section gave options only; calef ruled Beside.

## What cannot be undone, and what can

Irreversible once anyone outside acts on it, so options only:

- The signed statement. Options: the 54-byte statement above; a signature over the raw file, which
  is simpler and has no algorithm byte; or a statement that also lists a manifest digest, which only
  M1 needed.
- The placement. Ruled Beside.
- The algorithm and key encoding. Options: Ed25519 (RFC 8032, a 32-byte key, a 64-byte signature);
  ECDSA P-256, which the provider also carries and whose crate says its curve arithmetic "has never
  been independently audited"; or a post-quantum scheme, which nothing in the tree carries.
- The published claim, restating the rule of milestone 104 (the measurement continues past init)
  again: "the progenitor grants nothing of its own to what it cannot vouch for, and a vouch may come
  from a key the owner trusts."
- Any public key that ships in an image. None is proposed.

Everything else recommended above is reversible code.

## What this blocks and unblocks

Nothing is blocked; S2 composes with D2 as ruled. The path to a first customer does not move: a
first customer installs by digest vouching.

## Prior art, read 2026-09-26

In [`prior-art.md`](0220-signed-builds-and-scoped-key-trust/prior-art.md).
