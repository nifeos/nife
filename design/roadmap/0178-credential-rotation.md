---
status: NOT-STARTED
raised: 2026-08-26
milestone_dependencies: none
decision_dependencies: 168
machine_requirements: none
specific_machine: none
needs_person: no
---
# 178. A credential a user can change, without reopening the whole store

A real interactive boot generates a fresh demo password each boot and prints it before the prompt.
That is the amendment of §120 (a QEMU-only virtio-rng stopgap for the interactive boot) and the
login-boot-wiring piece of milestone 49 (users, login, and attribution). The first thing calef
expects a person to do with a new system is set their own password. Nothing in this tree lets them.

Where a proof-gated rotation verb lives is a `credential_protocol` wire change two
programs must agree on, and the options below are close enough in cost that the choice is an architect's
rather than a lane's. It is
§168 (where a proof-gated credential rotation
verb lives), written up 2026-09-19 by milestone 435's lane, which found this gate naming no decision
while "The design question this needs answered" below was addressed to one person.

## What this is, in brief

Unix `passwd(1)`, one level down: a logged-in identity presents its current secret and a new one,
and the credential service replaces its own stored record, in place, without touching anyone
else's. It does not exist today, and it is not an oversight.

## Why the gap is deliberate, and why that makes this a real design question rather than a bug fix

`credentialer::Store::put`'s own doc comment names the reason directly: **"replacing an existing identity
is not offered: this runs once, before the seal, and 'put twice, second wins' is a rule with a bug
in it (which of two concurrent provisioners won?) that a store with no update path simply does not
have."** `credential_protocol::provision::SEAL`'s own doc is blunter still: after it, "the store
cannot be changed by anything short of restarting the service." Both are considered, not
accidental: the sealed store is what lets `credentialer.rs` promise a client that a stored secret
is exactly what an operator vouched for at boot, with nothing able to move it later.

So this is not "add an update path to `Store::put`." Provisioning's trust model is capability
held: whoever holds `WRITE` on the provision endpoint may write any record, because the whole
system trusts the operator who wired that endpoint at boot. A self-service password change needs a
different trust model entirely: identity proven, the same shape login itself already uses,
where holding a capability proves nothing and presenting the current secret proves everything. Only
that second model actually avoids `Store::put`'s own named race (concurrent writers), because a
change gated on proving the *old* secret can only ever be initiated by whoever already is that
identity, one at a time, by construction.

## The design question this needs answered, and why it is an architect's

Where the rotation verb lives, and it is a wire change to `credential_protocol` two programs (whichever
client calls it, and `credentialer.rs`) must agree on, which the *move fast on what can be undone*
tenet puts in the irreversible column. Options, not decided here:

- **A new op on the verify endpoint** (`credential_protocol::verify`), the endpoint a real login flow
  already reaches: extend it with a `ROTATE` opcode carrying the current secret and the new one in
  one request, verified and replaced atomically. Reuses the trust boundary a client already crosses
  to log in at all, and needs no new capability wired to anyone beyond what login-boot-wiring
  (milestone 49's own remaining piece) already grants a logged-in session.
- **A dedicated rotation endpoint**, held separately from both provision and verify, minted and
  handed out its own way. More machinery, and a third capability shape to reason about for a
  service whose two-endpoint design (`notes/credentials.md`) is otherwise deliberately minimal.
- Reuse provisioning's own shape, scoped to one record: reopen a single identity's slot for a
  bounded window rather than the whole store. Closest to what `Store::put`'s comment already refuses
  and for the same reason (a second, harder-to-see version of "which of two concurrent writers
  won"); likely the wrong shape, named for completeness rather than as a real candidate.

No recommendation forced here: this is exactly the shape of fork this project's own convention asks
be brought to calef with the options priced rather than guessed at, and the first two above are
close enough in cost that the choice should be his rather than a lane's.

## What this unblocks

Completes the boot-generated-password decision (§120's amendment, this milestone's own trigger):
today a person can only ever read whatever the boot printed; this is what lets them replace it with
something they chose and can remember. Also the natural home for a operator resetting *another*
identity's password (the provisioning-endpoint path, already capability-shaped for that case) versus
a user changing their own (this milestone's actual gap): worth keeping the two paths conceptually
separate even though both eventually touch the same store.

## Prior art

Unix `passwd(1)`: the shape (prove who you are, then replace your own line), not the mechanism
(`/etc/shadow`'s file permissions have no capability analog here, and nothing here has a uid to key
on in the first place, `DECISIONS §117`).

## What it does not decide

Where the credential store's contents live across a reboot (`credentialer.rs`'s own BUGS: "nothing
survives a reboot") is untouched by this milestone either way; a rotated password is exactly as
volatile as a provisioned one until that separate gap closes.

## BUGS

Not started; nothing built yet to carry its own BUGS section. This file's own "design question"
above is the gate.

## Index row

Surfaced deciding how a real boot's demo credential is provisioned (§120's amendment): the boot
now generates and prints a fresh password each time, and nothing lets a person replace it with one
of their own choosing. The sealed credential store's no-update-path is deliberate (`Store::put`'s
own doc names the race it refuses), so this needs a real design fork on where a proof-gated
rotation verb lives, not a bug fix. Gate: DECISION.
