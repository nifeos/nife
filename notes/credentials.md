# Credentials

Scope note, 2026-08-30. This service held two kinds of secret until that day: a password
verifier and an NTLM key. The NTLM half was removed with the SMB implementation that was its only
consumer (notes/smb.md), so what ships now is the password verifier alone: `provision::PUT`,
`provision::SEAL`, `verify::VERIFY`, and a reply that is one word carrying no data. Everything below
that describes `put_ntlm`, `ntlm_proof`, `PUT_NTLM`, `NTLM_PROOF` or a `SessionBaseKey` is history;
notes/ntlm.md carries that half's own record and the reason its removal is worth reading about.

An identity, a secret, and a way to check the second against the first without ever being able to
read it. Milestone 56's second half; the first half is [entropy](entropy.md), and this depends on
it for every salt it uses.

The contract is `crates/credential_contract`, the logic is `crates/credentialer`, the service is
`components/src/credentialer.rs`, and its clients are `fixtures/src/credentialer_test_client.rs`.

Milestone 65 (a secrets service) generalized this process in place ([NTLM](ntlm.md)). The
milestone's rule was that the credentialer becomes *an operation in* the secrets service, because a
second process holding secrets is what the design exists to avoid. Where this file says "the
credential service", read "the secrets service". The phases, the seal and the wipe are exactly as
described.

## The problem this exists to solve

Milestone 55 wants a Mac to authenticate against an SMB share. SMB requires an identity and a
secret; this system had neither, and had no cryptography at all. Adding "a password file" is the
obvious move and it is the one that gives away the property this kernel is for.

The tension is the interesting part of the milestone:

> **A secret is a bearer token. A capability is an unforgeable reference. Once a component can
> *read* a password hash, it holds it forever and can copy it anywhere; knowledge cannot be
> revoked. Every other authority in this system can be.**

So the answer is not to protect the file better. It is to hand out the operation, not the
secret. A client gets an endpoint that means *"you may ask whether a secret is right"*. It never
gets the secret, the salt, the tag, or the list of who exists.

This is the fifth appearance of one idea, and the roadmap gives it its name: **attenuation by
operation, not by object**.

| milestone | the object | the attenuated power |
|---|---|---|
| 51 | the wall clock | an NTP client may **propose** a time, not set it |
| 51 (§43) | the clock page | read is a read-only mapping, set is a writable one |
| 56 | the virtio-rng | **obtain** bytes, without reaching the device |
| 56 | the credential store | **use** a credential, without reading it |
| 56 | the credential store | **write** it, only during a phase that ends |
| 65 | an NTLM key | **compute a proof** with it, without reading it |

## The shape

```text
   the provisioner ──the provision endpoint──►┌────────────┐
   (once, at boot)      (PUT, then SEAL)      │ credential │──►the entropy service
                                              │  service   │   (salts; an endpoint
   a client ────────the verify endpoint──────►└────────────┘    that names no device)
                    (VERIFY, forever)            the store lives here and nowhere else
```

Two endpoints. Two phases. The second phase never ends and the first one never comes back.

1. **Provision.** The service blocks on the provision endpoint. Each `PUT` derives a record with a
   salt drawn from the entropy service. `SEAL` ends the phase.
2. **Delete.** The service `capability_delete`s its receive end. The provisioner drops its send end.
   Between them, nothing in the system can name the object any more.
3. Serve. The service blocks on the verify endpoint, forever. One opcode, `VERIFY`, and the answer
   is yes or no. (Between milestone 65 and 2026-08-30 there were two, and an NTLM yes carried a
   session key in the shared page.)

### Why two phases and not two operations

Because this kernel has one wait point. There is no wait-any primitive and no threads inside one
address space, so a process can block on exactly one endpoint. `components/src/clock.rs` records the
same constraint and answers it differently: the clock's wide authority (set) is a page write rather
than a message, so the service only ever serves the narrow one.

A credential store cannot copy that, because "write the store" is not a memory operation you can
hand out as a mapping. But it does not need to: **writing the store is not an operation at all, it
is a phase.** Provisioning happens once, before any client exists, and then it is over. Making that
a phase rather than a privileged opcode is what turns "the service refuses to let you write" into
"there is no object through which the request could travel".

A client is not refused permission. By the time a client holds anything, `PUT` is a word arriving at
a loop that implements one opcode, and the endpoint that gave the word meaning has been destroyed.

Compare Samba: `smbd` opens the password database directly, so compromising it leaks every hash,
crackable offline and reusable wherever the password was reused. Here a compromised client can
guess passwords while it runs and cannot exfiltrate one, and revoking its endpoint ends the access.

## Argon2id, and why we depend rather than write

DECISIONS §46 (thin primitives or whole subsystems) draws the line at exposure: write the thing
whose correctness you can argue from the spec, and depend on the thing whose correctness is won by
many people checking it over many years. A password KDF is the second kind, so this depends on
RustCrypto's `argon2` 0.5.3 (not a vendored copy, so `cargo-deny` and `cargo-audit` can see it).
Argon2id is RFC 9106's recommendation and OWASP's first choice.

- The RFC 9106 and reference-implementation vectors run through the service's own entry point, so a
  version bump that changes an answer fails first.
- A bug in the dependency: `Params::new` overflows on a huge `p_cost` in a debug build. `Cost::new`
  enforces Argon2's ranges itself, in an order that cannot overflow.
- The cost is m = 4 MiB, t = 3, p = 1, below OWASP's 19 MiB / t=2 because the system under test has
  128 MiB of RAM. It is roughly a third of the recommended work and still memory-hard. The parameters
  travel in the record, so raising them is a provisioning change. One verify costs 5.3 ms optimized
  on the development machine, which is why the KDF crates build at `opt-level = 2` even in debug.

The argument, the vectors, the bug and the measurements are in
[`credentials/argon2id.md`](credentials/argon2id.md).

## Three things a naive verifier gets wrong

1. The tag comparison is constant-time (`subtle::ConstantTimeEq`). A verifier that returned on
   the first differing byte lets an attacker recover the tag one byte at a time.
2. **The identity lookup is constant-time too, and does not stop at the match.** `Store::select`
   scans every slot, conditionally assigns with `ConditionallySelectable`, and has no early exit. A
   scan that short-circuited would leak *which* identities exist, which is the same oracle by a
   slower route.
3. A miss costs one full derivation. An identity nobody provisioned lands on a decoy record
   whose salt and tag come from the entropy service at start-up, and the KDF runs against it. Without
   that, "no such user" returns in microseconds and "wrong password" returns in milliseconds, and the
   store's membership is readable with a stopwatch.

A miss and a wrong password are also the same reply code (`MISMATCH`). Distinguishing them
would turn the verify endpoint into an identity oracle: ask about a thousand names, learn which
three exist.

### How the constant-time claim is checked

Mostly structurally, and deliberately so. `Store::select` is branch-free, and a unit test pins its
behavior at every slot position and on a miss: the decoy's salt, the decoy's tag, the store's cost,
`found = 0`. That is a deterministic test of the thing that matters.

There is also one timing test, and it is written to be robust rather than precise: 25 runs of
each kind, compare the medians, and fail outside a 0.4x–2.5x band. A machine under load inflates
both medians together, so the ratio is stable even when the numbers are not. The bug it exists to
catch is not subtle: an early exit on "no such identity" skips the whole KDF, which is a ratio near
zero. If it ever fails at 0.6, suspect the machine before the code, and re-run it quiet.

## The reply carries no data

Every reply is one word, and the second word is always zero. Not as a convention a future
opcode might relax: there is nothing about a credential store a caller is entitled to, so the reply
channel has no room for it. A service that answered a verify with the stored tag would be a
decryption oracle wearing a verifier's clothes, and the shape of the contract makes that a change
to the contract rather than a bug in a serve loop.

The reply codes are all small positives (1..=6), which is the trick `entropy_contract` established:
every failure the kernel can return from a `CALL` is one of its small negatives, which read as
enormous `u64`s. So `credential_contract::is_authenticated` can collapse "there is no credential
service", "the request was malformed", "the service died" and "wrong password" into one `false`, and
no caller has to remember which of six codes were the good ones. A caller that mistook a missing
capability for a successful authentication would be the single worst bug this contract could
permit, so it is the one made impossible by arithmetic rather than by care.

## The shared page, and what is left in it

Bulk rides in a page (DECISIONS §10) because an identity and a passphrase do not fit in two
registers. Two frames, never one: the provisioner writes plaintext secrets into its page, and a
client sharing that frame would read them.

The service zeroes the request area after reading every request, on every path, including the
malformed ones. So after an answer, the frame the client and the service share holds neither the
presented secret nor anything else. The wipe uses `write_volatile` and a compiler fence, because a
compiler that can prove nobody reads those bytes again is entitled to delete a plain store, and
"the optimizer removed the wipe" is the classic way a zeroing loop turns into a comment.

## EXAMPLES

### Provision three identities and seal the store

From a provisioner holding the provision endpoint in slot 0 and its page mapped:

```rust
use credential_contract as proto;

let page = /* the mapped provision page, 4096 bytes */;
for (identity, secret) in [
    (&b"alice"[..], &b"correct horse battery staple"[..]),
    (b"bob", b"a different secret entirely"),
    (b"carol", b"and a third one"),
] {
    let w0 = proto::place(page, identity, secret, proto::provision::PUT).unwrap();
    let (r0, _) = call(SERVICE, w0, 0);
    assert_eq!(r0, proto::OK);
}

// After this the provision endpoint is dead at both ends.
let w0 = proto::place(page, b"seal", b"seal", proto::provision::SEAL).unwrap();
let (r0, _) = call(SERVICE, w0, 0);
assert_eq!(r0, proto::OK);
```

### Authenticate a client

From anything holding the verify endpoint in slot 0:

```rust
let w0 = proto::place(page, b"alice", presented, proto::verify::VERIFY).unwrap();
let (r0, _) = call(SERVICE, w0, 0);
if proto::is_authenticated(r0) {
    // let them in
}
```

`is_authenticated` is the whole client-side API. There is no way to ask "does this identity exist",
"what is the salt", or "how many identities are there", because there is no message that would
answer.

### Check a secret without a service (host, in a test)

```rust
use credentialer::{Block, Cost, Store, Verdict};

let cost = Cost::new(256, 2, 1).unwrap();          // cheap, for a test
let mut scratch = vec![Block::default(); cost.blocks()];
let mut store = Store::<3>::new(cost, decoy_salt, decoy_tag);
store.put(b"alice", b"correct horse", salt, &mut scratch).unwrap();

assert_eq!(store.verify(b"alice", b"correct horse", &mut scratch).unwrap(), Verdict::Match);
assert_eq!(store.verify(b"alice", b"wrong",         &mut scratch).unwrap(), Verdict::Mismatch);
assert_eq!(store.verify(b"nobody", b"correct horse", &mut scratch).unwrap(), Verdict::Mismatch);
```

There is no `store.get()`, no `store.record_for()`, no iterator. The absence
is the API expressing what the process boundary enforces.

## What is proven, and where

Host tests (`cargo test -p credentialer -p credential_contract`) pin the known-answer vectors,
right and wrong secrets, every single-byte corruption of a record, a hostile cost, and the decoy
lookup at every slot. Three Kani harnesses over `credential_contract` prove that no request word
makes the parse read outside the page and that nothing but `MATCH` authenticates, over all 2^64
words. Guest tests on aarch64 and riscv64 show provisioning sealing, a client with one endpoint
getting right answers, the same endowment unable to write the store, the service surviving it, and
the shared frame left all zero. Each test and what it pins is listed in
[`credentials/what-is-proven.md`](credentials/what-is-proven.md).

## BUGS

- **Revocation is per holder, not per secret.** Destroying a client's endpoint ends its access,
  which is the thing this design buys and a stored hash could never offer. Revoking one *secret* is
  a different question, and this service cannot answer it. The store is sealed, so there is no
  object through which a record could be removed, which is the same property that makes the seal
  worth having. Rotating one secret means
  restarting the service and reprovisioning, which restarts every other secret with it. A
  deployment needing finer granularity runs more than one service.
- Nothing survives a reboot. The store is memory only, provisioned at boot. Secrets at rest is
  the open question and it is the same chicken-and-egg as milestone 51's NTS problem: encrypted
  under what key, held where? `credentialer::Record` has a versioned encoding with a round-trip test
  precisely so that question has a starting point, but nothing in the tree writes one to a disk.
- ~~This cannot serve NTLMv2.~~ Closed by milestone 65, and since removed with SMB on 2026-08-30
  (see the scope note at the top). The gap turned out to be
  shaped differently than this entry predicted. The prediction was a second operation of the form
  "here is a challenge, give me the response"; what shipped folds the comparison in, because the
  thing needing it is an SMB *server* and a server that gets the expected proof can compare it
  itself. A record carried an `NTOWFv2` beside its Argon2id tag, `put_ntlm` derived both from one
  password, and MD4 and MD5 were in the tree on purpose. Record version 3 dropped that half. See
  [NTLM](ntlm.md) for what crossed
  the boundary, what never did, and the cost of storing a password-equivalent key at all.
- The store holds six secrets, three logins and three shares, and that is a compiled-in
  constant rather than a policy anything reads. It is sized to the requirement
  (design/roadmap/0056-secrets-and-entropy.md's three family members, each of whom also has a Time
  Machine share), which is what makes "the seventh is refused" a thing the tests show rather than a
  branch nothing reaches. A real deployment with a fourth person edits a constant and rebuilds.
- One verify page means one client at a time. The page is per service, not per channel, so two
  clients sharing the endpoint would share the frame each writes its presented secret into. Nothing
  detects that. `filesystem_contract`'s answer (one page per channel) is the shape to copy when a second client
  exists; today the intended client is the single SMB adapter.
- No rate limit, no lockout, no attempt counter. A client holding the verify endpoint can guess
  as fast as it can `CALL`. Each guess costs the service one Argon2id derivation, which is the only
  thing slowing an online attack down and is also a way to make the service unresponsive to
  everyone else. A store with three identities and human-chosen passwords is not safe against an
  unlimited online guesser at any KDF cost.
- The cost parameters are below OWASP's, for the reason given above. On real hardware they
  should be raised, and nothing currently checks that they were.
- No rehash on verify. When the cost parameters move, existing records keep their old ones. The
  encoding carries per-record parameters so this is implementable; it is not implemented. A
  consequence: if two identities were ever provisioned at *different* costs, the verify time would
  distinguish them. Nothing today can produce that, because a store has one cost and `put` uses it.
- The identity is an opaque byte string and nothing more. No uid, no group, no home directory,
  no session, no login. milestone 49 (users, login, and attribution) is a different milestone and
  this one deliberately does not start it. What is built here is the credential *primitive*; who
  gets to ask, and what an answer of "yes" then permits, is §49's question. The roadmap's answer,
  which this does not contradict: the adapter authenticates the client because the protocol demands
  it, then uses the directory capability it already holds. **Identity never becomes ambient
  authority.**
- The provisioner's plaintext exists somewhere. Provisioning takes a secret in the clear, so
  whatever hands the provisioner its passwords is holding them in memory. Today that is a test
  program with the strings compiled in, which is fine for a test and is not a deployment. Where a
  real deployment's passwords come from is unanswered and is part of the secrets-at-rest question.
- The decoy is one record. Every miss derives against the same salt. An attacker who can
  time verifies precisely enough to distinguish *two* misses from a miss and a hit learns nothing.
  But an attacker who can observe the service's memory access pattern is outside this threat model
  entirely, and so is one who shares a core with it. There is no defense here against a local
  side-channel adversary.
- **The kernel-side test driver is not a capability holder.** The strongest form of "no capability
  to the provision endpoint exists" is demonstrated by the userspace clients' endowments (they hold
  only the verify endpoint), not by the test harness, which names endpoints by id and reaches around
  the capability system by construction. The claim is true of the system; the test proves it of
  userspace.
