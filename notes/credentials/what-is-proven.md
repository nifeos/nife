# Credentials: what is proven, and where

*Name: provisional, after the parent's own heading.*

An appendix to [`notes/credentials.md`](../credentials.md). It lists every host test, proof and
guest test behind the credential service, and what each pins.

Host tests (`cargo test -p credentialer -p credential_contract`, milliseconds, no emulator):

- RFC 9106's and the reference implementation's known-answer vectors, through the same entry
  point the service uses.
- The right secret matches; the wrong one does not; an unprovisioned identity does not; and one
  person's password does not open another person's account.
- Every single-byte corruption of an encoded record is rejected or decodes to a different
  record. Exhaustive over positions and four bit patterns.
- A hostile cost is refused before it reaches the library (the overflow in
  [argon2id.md](argon2id.md)).
- The lookup lands on the decoy for a miss and on the record for a hit, at every slot position.
- A miss and a hit take comparable time.

Proofs (`script/verify`, three Kani harnesses over `credential_contract`, 30 checks, 0.2 s). Both
properties are about what an adversary can send or receive, and an adversary is not limited to the
values a test author thought of:

- No request word makes the server's parse read outside the page. For every one of the 2^64
  first words a client can send, `read` either refuses it or returns two slices inside the page with
  exactly the lengths the word claimed. This is what lets the serve loop have no arithmetic in it
  that could go wrong.
- Nothing but `MATCH` authenticates, for every one of the 2^64 words a caller can receive. The
  host test sweeps a few dozen values around the boundary; this sweeps all of them.
- A request word round-trips its opcode and both lengths, over every combination `place` can build.

Guest tests (`system_tests/src/user/credential_tests.rs`, on aarch64 and riscv64, same assertions):

- Provisioning fills the store and the seal closes it. Three identities go in, and the fourth is
  refused with `FULL` rather than silently replacing somebody. The seal is accepted, and the
  service's readiness message arrives *after* the seal, which is the evidence that the provisioning loop was
  left rather than merely that a `SEAL` was answered.
- A userspace client with one endpoint and no store gets the right answer to four questions,
  over a real Argon2id verification with a salt drawn from a real virtio-rng.
- The identical endowment cannot write the store. The attacker holds exactly what the honest
  client holds and tries `PUT`, `SEAL`, an undefined opcode, and lengths outside the contract. `PUT`
  answers `MISMATCH` and the rest `MALFORMED` (see "Two opcode spaces" below), and the credential
  it tried to install does not work.
- The service survives all of it and answers correctly afterwards, because a credential service
  a malformed request can kill is a login outage anybody can cause.
- The kernel reads the shared frame through the direct map, which no userspace program can do,
  and finds neither the presented secret nor any nonzero byte.

## The guest fixture

`credential_tests::provisioned` wires the entropy service, the credential service and the
provisioner once per boot, and every guest test that needs the sealed store calls it. The seal is
irreversible, so there is no way to ask for a second wiring. The service sends its readiness word
between the seal and its serve loop, and a `SEND` on this kernel blocks until somebody receives.
So the fixture drains that word before it returns. A test that let a client `CALL` first would find
a service that had not reached its serve loop, and the boot would deadlock with a sender and a
caller both waiting on nobody. The entropy service does not show this, because its wiring receives
at once.

## Two opcode spaces

The provision endpoint and the verify endpoint number their opcodes independently, because the
endpoint gives a number its meaning. `provision::PUT` and `verify::VERIFY` are both opcode 1. So
an attacker's `PUT` on the verify endpoint is a verify of an identity nobody provisioned, and the
honest answer is `MISMATCH`. There is no privileged request to refuse. `provision::SEAL` is opcode
2, which the verify endpoint does not serve, so it answers `MALFORMED`. If the verify space grew an
opcode 2, the same word would become that request instead. Nothing would break, because a word
arriving at a serve loop never carries authority.
