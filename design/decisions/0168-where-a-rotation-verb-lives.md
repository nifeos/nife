---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-28
ratified_by: calef
---

# 168. Where a proof-gated credential rotation verb lives

Ruled 2026-10-28 (UTC): B, the rotation verb gets its own dedicated endpoint; verify stays read-only;
the three endpoints are provision (capability-held), verify (read-only), rotate (identity-proven,
current secret and new secret together). Unblocks milestone 178.

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote
down)'s lane. That lane found milestone 178 (a credential a user can change, without reopening the
whole store) gated on `DECISION` with no decision anywhere a reader can open. The fork is the
block's own and calef minted the block for it on 2026-08-26, while deciding how a real interactive
boot's demo credential is provisioned. *(Section number provisional until the merge queue lands
it.)*

## What is being decided

Where a rotation verb lives on the credential protocol, given that it is a wire change two
programs must agree on: whichever client calls it, and `components/src/credentialer.rs`.

## Why this is a design question and not a missing feature

The gap is deliberate and the code says so. `credentialer::Store::put` refuses replacement in
its own words:

> Replacing an existing identity is not offered: this runs once, before the seal, and "put twice,
> second wins" is a rule with a bug in it (which of two concurrent provisioners won?) that a store
> with no update path simply does not have. A duplicate identity is refused.

And `credential_protocol::provision::SEAL`'s documentation is blunter: after it, the store cannot be
changed by anything short of restarting the service. Both are what let the service promise a client
that a stored secret is exactly what an operator vouched for at boot.

**So this is not "add an update path."** It is a second trust model beside the first:

- Provisioning is capability held. Whoever holds `WRITE` on the provision endpoint may write any
  record, because the system trusts the operator who wired that endpoint at boot.
- Rotation must be identity proven. Holding a capability proves nothing; presenting the current
  secret proves everything. It is the shape login already uses.

And only the second model avoids the race the store refuses. A change gated on proving the *old*
secret can only be initiated by whoever already is that identity, one at a time, by construction. So
the trust model is not a preference here; it is what makes the verb safe to add at all.

## What this tree already does in the analogous case

The two-endpoint split is the existing shape (`notes/credentials.md`): provision and verify,
deliberately minimal. `login_protocol::CONNECT` mints an endpoint per connection, which is the
tree's standard answer to "a client needs its own thing"; and
[§41](0041-endpoint-as-broker.md) (the endpoint is the broker) is the reason a secret is reached
through an endpoint rather than handed over as a value.

Prior art: Unix `passwd(1)`, for the shape (prove who you are, then replace your own line) and
not the mechanism. `/etc/shadow`'s file permissions have no capability analogue here, and nothing in
this tree has a uid to key on ([§117](0117-subtree-name-is-identity.md), a principal's subtree is
named by its identity string).

## The options

| | where the verb lives | cost |
|---|---|---|
| **A** | **A new `ROTATE` op on the verify endpoint**, carrying the current secret and the new one in one request, verified and replaced atomically. | Reuses the trust boundary a client already crosses to log in, and needs no new capability wired to anyone beyond what a logged-in session already holds. It widens the verify endpoint from read-only to mutating, so a capability that meant "may check a password" now means "may change one after checking it". |
| **B** | **A dedicated rotation endpoint**, minted and handed out its own way. | Keeps verify read-only, which is the cleaner statement of what each capability means. A third capability shape for a service whose two-endpoint design is deliberately minimal, and a new wiring question at boot. |
| **C** | **Reuse provisioning's shape**, scoped to one record: reopen one identity's slot for a bounded window. | Closest to what `Store::put` already refuses, and for the same reason in a harder-to-see form. Named for completeness rather than as a candidate. |

No recommendation forced between A and B. They are close enough in cost that the choice is about
what a capability should mean rather than about work, which is the case AGENTS.md sends to calef as
options. C is not recommended and the reason is stated: it reintroduces the race the store was built
without.

The question that decides A against B, if one is wanted: does "may verify" imply "may change
after verifying"? If yes, A; if the two are different authorities that should be separately
grantable, B. Nothing else in the comparison moves much.

## How reversible it is

A wire change to `credential_protocol` is the irreversible category: two programs agree on it,
and an opcode cannot be un-shipped. The boot wiring around it is reversible and cheap.

## What is blocked until this is answered

Milestone 178, which completes [§120](0120-boot-entropy-stopgap-declined.md)'s amendment: the boot
now generates a fresh password each time and prints it before the prompt, and nothing lets a person
replace it with one they chose.

**Not blocked, and deliberately separate**: an operator resetting *another* identity's password,
which is the provisioning path and is already capability-shaped for that case. Keeping the two
conceptually apart is worth doing even though both eventually touch the same store.

Unaffected either way: nothing in the credential store survives a reboot, so a rotated password
is exactly as volatile as a provisioned one until that separate gap closes.
