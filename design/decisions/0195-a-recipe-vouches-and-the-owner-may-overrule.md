---
status: AMENDED
raised: 2026-09-19
decided: 2026-09-19
ratified_by: calef
---

# 195. A reviewed recipe vouches for a package, and the machine's owner may overrule it

*Amended 2026-10-10 (UTC) by calef, on #1884: basalt holds long-lived root keys (clause 4), and an image carries its root, amending §220 too. See the amendment at the end.*

calef, 2026-09-19 (21:51 UTC), after reading the prior art with the maintainer:
Homebrew's shape, with the owner-vouches escape hatch. *(Section number provisional until the
merge queue lands it.)*

What that means, and each clause is load-bearing:

1. A package's digest lives in a version-controlled recipe, changed by human review, and the
   machine trusts the recipes of the sources it was given. The boot image's own measurement
   table (milestone 104) becomes the first source rather than a special case, which is how T1 stops
   being an answer and becomes an entry.
2. Trust is scoped per source the owner opted into, never one global key. Every system read
   below does this (a Homebrew tap, a Debian keyring entry, a FreeBSD per-repository fingerprint
   directory, a pacman `SigLevel`, a Nix substituter), and it is what lets a distribution grow
   without mastering every package: anyone may stand up a source, and an owner may opt into it,
   which calef named as the point of the model.
3. The owner may vouch for a digest no source carries. This is the escape hatch every system
   read below keeps: apt's explicit confirmation, pacman's `SigLevel = Optional`, pkg's
   `SIGNATURE_TYPE=NONE`, Nix's `require-sigs = false` and `trusted-users`. Fuchsia is the only
   holdout, and it is a vendor-locked phone OS. It is also what lets a person run something they
   built themselves, which §135's amendment 1 already contemplates.
4. No long-lived signing key is held by anyone, for now. Homebrew held none for years (a
   SHA-256 in a reviewed formula), and now binds artifacts to the builder with GitHub Actions
   attestations instead. A third party verifies against its own builder identity, not ours. A
   signature over a source's catalog is not refused; it is simply not the thing being built
   first, and it can be added per source without changing clauses 1 to 3.

**What this does not decide**, and each is its own ruling: the package format (whether a package is
named by name-and-version or by content digest), the activation shape, the transport, and whether
`crickertech` runs a source at all. One consequence is worth stating because it couples two
rulings: hosting on GitHub (Releases or GHCR, which is what Homebrew does) forces HTTPS, so
choosing GitHub as the first host is close to choosing to carry a TLS stack, which
DECISIONS §196 rules.

Reversibility. Clauses 1 to 3 are code and reversible. The irreversible thing this deliberately
does **not** do is ship a public key in an image somebody else runs, which is why T2 is deferred
rather than refused.

## The prior art, read 2026-09-19 rather than recalled

| System | What is signed | How a package is accepted | The owner's escape hatch |
|---|---|---|---|
| Debian `apt` | the repository's `Release` file, not the `.deb` | signed `Release` to index checksums to package checksum | a prompt: "packages cannot be authenticated" |
| FreeBSD `pkg` | the repository catalog | `SIGNATURE_TYPE` `PUBKEY` or `FINGERPRINTS`, with trusted and revoked fingerprint directories | `SIGNATURE_TYPE=NONE` |
| Arch `pacman` | both packages and databases | packager keys in a keyring, master-key web of trust, default `Required TrustedOnly` | `SigLevel = Optional` or `Never` |
| Nix | store paths, by a cache's key | `trusted-public-keys`, **or** the path is content-addressed, which needs no signature | `require-sigs = false`, `trusted-users` |
| Fuchsia | base packages by hashes in the image; later packages by signature at load | content-addressed blobs plus signature verification at load | none, by design |
| Homebrew | historically nothing: a SHA-256 in a reviewed formula; now also GitHub Actions build attestations | checksum match against the recipe, plus optional attestation against the builder identity | attestation checking is opt-in |

Two findings from that table shaped the ruling. Almost nobody signs individual packages: the
signature, where there is one, is over the *index*, which is what nife's measurement table already
is. And the owner override is universal rather than a loophole, which answers milestone 104's
objection directly: the override changes what runs by the owner's own act, which is not the
"measurement that changes nothing" that §104 refused.

Sources, read 2026-09-19: `wiki.debian.org/SecureApt`; `man.freebsd.org` `pkg.conf(5)`;
`man.archlinux.org` `pacman.conf(5)`; `nix.dev` manual, configuration options;
`fuchsia.dev` verified execution; `docs.brew.sh/Bottles` and Homebrew's attestation module.

## The proposal as calef ruled on it

## What is being decided

Today's chain is exact and closed:

1. The kernel compiles in the digests of the boot program and of `program_measurements`
   (`kernel/src/trust.rs`, `TRUST_ROOT`, generated by `kernel/build.rs`).
2. The progenitor loads every other program only if that table vouches for it: "One rule: the
   progenitor runs nothing it cannot vouch for" (milestone 104, `crates/system_initializer`).

So a kernel and its archive are one sealed set, and that seal has already refused real boots
twice: radon on 2026-08-15 (boot 12) and xenon on 2026-09-17, both `MEASURED BOOT REFUSED` over a
kernel and an archive from different builds (`xtask/src/main.rs`, `uefi_image`'s comment;
`notes/visionfive2.md`, "The microSD payload"). Any package installed after the image was built is
unvouched by construction. DECISIONS §26 (milestone 22 phase B) already named the condition under
which this changes: the signature variant "becomes worth paying for when init is delivered
independently of the kernel". A package is that condition, one level down.

## Options

| | What vouches | New code in a trusted place | Key custody | Prior art (read) |
|---|---|---|---|---|
| **T1. The image, always** | Installing means composing a new image on the host; the kernel is rebuilt against the new table and the machine reboots | None | None | Fuchsia's base packages: the verified system image lists the hash of every base package, so base packages need no signature check at runtime (paraphrased, read: `fuchsia.dev/.../concepts/security/verified_execution`) |
| **T2. A publisher's signature, checked in userspace** | A package carries a signature over its digest; a public key compiled into the measured image vouches for it; the check runs in the progenitor or a package service, **not the kernel** | An Ed25519 verifier in a measured userspace program. §46's 2026-07-31 amendment makes crypto "an ordinary dependency", so this is a RustCrypto crate taken under §46, calef's call | Who holds the private key, how it rotates, what revokes it (§26 lists all three as the reason the kernel variant was declined) | Nix: `trusted-public-keys`, `require-sigs` on by default, Ed25519 over the store path and NAR hash (read: `nix.dev/manual/nix/latest/command-ref/conf-file`). Fuchsia universe packages: "by signature and content hash, every time the package is loaded", TUF keys inside what verified boot covers (read: `verified_execution`, above). Genode: GPG per publisher, archives "not extracted before their signature is checked" (read: `genode.org/documentation/genode-foundations/23.05/development/Package_management.html`) |
| **T3. The owner of the machine** | The person installing records the package's digest in a table on their own storage, holding a capability to that table; the progenitor accepts either the image's table or the owner's | A second table and a rule for who may write it | None, but **whoever can write the owner table can make anything runnable** | Sculpt: "by merely knowing a URL but no public key, Sculpt won't be able to verify the integrity", and untrusted software can still be run safely "as long as one does not explicitly grant the untrusted components access to sensitive parts of the system" (read: `genode.org/documentation/articles/sculpt-25-10`) |

**T2 and T3 are not exclusive**, and Fuchsia runs a T1 floor under a T2 layer. §26's own sequence
for its kernel variant was the same: "signature verification *in addition to* the measured root (so
the hash stays the floor if key handling fails)".

## What the tree already does in the analogous case

T1, everywhere, and with a written reason: "today they are built by one command in one tree in one
sequence, so the hash is strictly better" (§26). What moves the question is that a signature
verifier in userspace is cheaper than the one §26 declined. §26's first objection was that
Ed25519 "enters the TCB"; here it would enter a measured userspace program whose compromise is
bounded by its own grant, which is the microkernel version of the same trade and weakens that
objection without removing it: the verifier still decides what runs. §26's second objection, key
custody, is untouched by moving the code.

## The question under T3 that makes it a real option rather than a loophole

Milestone 104 rejected "recording a mismatch and loading anyway" because "a measurement that changes
nothing about what runs is theater". T3 is not that: it changes what runs, by the owner's act. The
argument for it is §135's and Genode's: in a capability system an unvouched program holds only
what it is granted, so the question a signature answers ("did this publisher build it") is not the
one confinement depends on. The argument against is that milestone 104's chain exists to stop a
program being substituted behind the owner's back, and a writable table is a substitution target. The
nearest precedent for an owner-held, durable authority record is the credential store
(`credentialer`, milestone 49); whether its shape fits a table of runnable digests was not checked.

## Costs

- T1: none new, and it is what already runs. Its cost is that install needs the host build and
  a reboot, and a stranger cannot install anything without the toolchain. Checked: composing
  prebuilt packages into an image still recompiles the kernel, because `TRUST_ROOT` is
  `include!`d at build time from `target/init-measure-<arch>.txt`. A composition step with no
  compiler would need the kernel to carry the table digest in a slot a host tool can patch after
  linking, which is a change to how the trust root is built and is part of this ruling if T1 is
  meant to serve people without a toolchain.
- T2: a dependency (no Ed25519 crate is in either `Cargo.lock` today; `argon2` is the only
  crypto crate present), a key, and a revocation story.
- T3: a table, its authority, and an amendment to milestone 104's one rule.

## Reversibility, and who has acted on it

T1 is reversible and is the status quo. T2 is irreversible the day a public key ships in an image
somebody else runs: that system trusts the key until it is reimaged. T3 is reversible in code and
not in what an owner has already run under it.

## The §92 test

T1 is the cheapest and would still be chosen for images we build ourselves if all three cost the
same, because it has no key to lose. It is not an answer for installing onto a running system at all,
so for that case the choice is between T2 and T3 on grounds of who should be able to make code
runnable, which is not an effort question.

## Who runs the first repository, which T2 makes a question and §135 already half-answered

§135 says a package manager "moves the obligation to whoever runs the repository" and that if nife
does not distribute a GPL binary it carries no obligation for it. The GPL FAQ, read on 2026-09-19,
is plain about the other half: "If you make object code available on a network server, you have to
provide the Corresponding Source on a network server as well" (`gnu.org/licenses/gpl-faq.html`,
entry `AnonFTPAndSendSources`). Upstream `git` and `nano` publish no nife binaries, so somebody
builds them: the user, from a recipe on their own host (no conveyance, §135's amendment 1), or a
repository operator, who then carries the source obligation. Under T2 the key and the repository
are usually the same party. So choosing T2 is also choosing whether `crickertech` runs a binary
repository that carries GPL programs, which §135 did not rule on. This is not legal advice; it is the
premise §135 rests on, checked.

## If calef says no to T2 and T3

Packages remain build inputs to images, which is the first slice. Runtime install, and therefore
§135 (running GPL software is aggregation)'s channel for `git` and `nano` reaching a machine by the user's own act, waits. *(DECISIONS §157 (a trivial install is a web page, a USB drive, and packages over the internet) replaces that first slice; see milestone 198 (a package manager)'s "Rescoped 2026-09-19". Under §157 a "no" here also stops rung 3.)*

## Amendment, 2026-10-10 (UTC): basalt holds a root key, and its images carry it

calef, on pull request #1884, for milestone 801 (packages over the internet), ruling Q3: *"K4, a
default owner trust line."* It amends this section's clause 4 and clause 1 of §220 (signed builds:
a vendor signs, a developer self-signs), and is recorded here for both.

1. Clause 4 here, "No long-lived signing key is held by anyone, for now", is replaced: basalt holds
   long-lived TUF root keys, and its index is signed under them. That is the signature over a
   source's catalog clause 4 left room for, added per source as it said, so clauses 1 to 3 are
   unchanged.
2. §220 clause 1's "No key ships in any image" is narrowed: an image carries exactly one key, its
   own distribution's TUF root (for basalt, basalt's root), identified by its SHA-256. It is
   installed as a default owner trust line in §220's form, and the owner may remove, narrow or
   replace it like any other line. No other key ships. Its ceiling is all three of §220's grants,
   `domain entropy network` (calef, 2026-10-10, on #1890: *"All three"*), so basalt can ship
   networked programs such as `jig`.
3. A further repository the owner adds is trusted the same way: its root is an owner trust line
   with a ceiling (#1884, Q5).

The irreversibility named under "Reversibility" above is therefore taken, and bounded: the owner
holds the line and may remove it without reimaging, and nothing outside the owner's table trusts
the key. A machine whose owner never removes it trusts basalt's root until that root rotates under
TUF's rule (root N+1 signed by N's threshold).

Not ruled here, and owed. Who holds the keys, how many there are, and the threshold are open, and
no root key exists yet. The line's format for a TUF root, a digest rather than an Ed25519 public
key, is provisional in the milestone that builds the default line. Also open is how a ceiling covers
the grants added since §220 named its three: the machine and job views of §225 (`free` sees the
machine and your share) and the reboot object of §251 (restarting the machine is a kernel object
the progenitor hands out).