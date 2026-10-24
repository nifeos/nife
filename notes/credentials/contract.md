# The credential contract: its wire, its fixtures and its proofs

An appendix to [notes/credentials.md](../credentials.md). It carries the design behind
`crates/credential_contract`, whose doc comments link here.

## The two endpoints

```text
                   ┌──────────────────────┐
  the provisioner ─┤ PROVISION endpoint   │  phase one: PUT an identity and its secret,
  (init, or a test)│  (write the store)   │             then SEAL. Then this endpoint is
                   └──────────┬───────────┘             deleted at BOTH ends and no
                              │                         capability to it exists anywhere.
                   ┌──────────▼───────────┐
                   │ credential service   │  the store lives here and only here
                   │  (holds the store)   │
                   └──────────┬───────────┘
                   ┌──────────▼───────────┐
       a client ───┤ VERIFY endpoint      │  phase two, forever: "is this secret right for this
                   │  (use the store)     │             identity?"
                   └──────────────────────┘
```

A verify holder can use a credential and cannot read one. No message returns one, no page carries
one, and the endpoint that could write one was destroyed before any client existed. It is a smaller
object, not a flag on a bigger one.

## An opcode is not an authority

`provision::PUT` and `verify::VERIFY` are both opcode 1 on purpose. The endpoint gives a number its
meaning, so a `PUT` sent to the verify endpoint is a verify, answered `MISMATCH`. There is no
privileged request to refuse. Separate opcode spaces would imply a distinction between forbidden and
unknown that this design does not have.

`MISMATCH` answers a wrong secret and an unknown identity alike, and `credentialer::Store` spends
the same work on both. So the verify endpoint is not an identity oracle. The reply's second word is
always `NO_DATA`, so it is not a decryption oracle either.

## Refusals that are distinct on purpose

`NO_ENTROPY` is its own code rather than a case of `MALFORMED`, because the two need different
responses. A malformed request is the provisioner's bug. `NO_ENTROPY` is the machine saying it has
no unpredictable bits. Serving the request anyway, with a salt the service made up, is the silent
degradation §44 (entropy is a capability, and the refusal is loud) forbids, applying §42 (a
filesystem declares what it offers and must be truthful)'s rule. It would also be invisible: every
login would keep working, and the whole store would be one rainbow table wide.

`place` refuses an empty identity on the client side, not only at the server. The empty name is the
one string that would collide with an unwritten slot in the service's fixed-size store.
`credentialer::Store` refuses it too, and two refusals for one hazard is deliberate.

## The fixtures

`fixture` holds the one login the gates share, in `filesystem_fixture`'s shape and for its reason.
Several programs must agree on an account down to the byte, and a second copy would drift into a
wrong answer that looks like a bug in the code under test. It has four readers.
`credentialer_test_client`'s provisioner role stores the key. The SMB adapter names the resource it
authenticates against. xtask's SMB prober computes a real proof over the password on the host. A
kernel test asserts that no key material is left in the frame the adapter and the service share.

The account is the one section 4.2.1 of [MS-NLMP] publishes, on purpose. Microsoft prints every
intermediate value for `Domain\User` with password `Password`, so a gate built on it asserts against
numbers somebody else published. `ntlm`'s own tests pin those numbers. It is a fixture, not a
deployment. A real share's account and password belong to somebody and must never be these
([notes/smb.md](../smb.md), BUGS).

`fixture::PEOPLE` (milestone 293 (a role that differs only in its credentials is an argument)) is
the three people every credential fixture authenticates, matching the three family members
design/roadmap/0056-secrets-and-entropy.md says the real deployment serves.
`credentialer_test_client`, `login_test_client` and `kernel::user::identity_provisioning_tests` all
read it. It holds pairs rather than two parallel arrays, because an identity and the secret that
opens it are one thing. Two arrays indexed in step are one careless insertion away from
authenticating `bob` with `carol`'s password and still passing. `carol` is the one with no home
subtree in `kernel::user::login_tests`: a real, authenticated identity that no one ran
`identity_provisioner` for.

## What the proofs cover, and the one that is blind alone

The host tests sample a few dozen of the 2^64 request words and 2^64 reply words. Both properties
are about what an adversary can send or receive, so the Kani harnesses check every word.

`no_request_word_makes_the_parse_read_outside_the_page` states the two length shifts as literals
(milestone 211 (a harness that states its property through the function under test cannot see that
function break)). Asserting the slice lengths only against `id_len` and `secret_len` would prove
that `read` used those accessors, and an `id_len` that read the wrong sixteen bits would satisfy
both sides.

`a_request_word_round_trips_every_field` is an encoder round-tripped through its own decoder, and on
its own it is blind (milestone 213 (a harness that re-implements the code instead of calling it
proves nothing about the code), measured). Swapping the two length fields in `req` and in
`id_len`/`secret_len` together leaves it verifying, while a client using the real wire format has
its identity read as its secret length. The sibling above goes red on exactly that patch. Restating
the shifts here as well would be a second copy of a claim one harness already pins, which DECISIONS
§46 (thin primitives or whole subsystems) refuses. The cost is that this harness's coverage is a
fact about the suite: deleting or weakening the sibling silently uncovers the wire format.
