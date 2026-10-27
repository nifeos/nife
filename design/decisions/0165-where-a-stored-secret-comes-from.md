---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-27
ratified_by: calef
---

# 165. Where a stored secret comes from on a boot that is not a test

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote
down)'s lane. That lane found milestone 131 (a share is configured, not compiled, and its secret
arrives from somewhere) gated on `DECISION` with no decision anywhere a reader can open. The block
has carried the fork with four options and no recommendation since it was filed, and says outright
that *"the choice is calef's, it is a fact that leaves the machine."*

## Ruled 2026-10-27 (UTC)

1. Question 1: an unattended boot is not in scope yet. On a real boot a secret is typed at a console
   (option A), before the store is sealed with `provision::SEAL`. A separate device (B) and
   measured-boot sealing (D) are decided when a deployment needs an unattended boot. Provisioning at
   image build (C) stays a test path only and a known defect, not a candidate. §220 (signed builds
   and scoped key trust), whose signing keys rest in the same place, inherits this answer.
2. Question 2: milestone 131 is re-aimed, not retired. It now delivers (a) a configuration document
   for one real service whose settings are compiled-in constants today, and (b) console
   provisioning of a secret before `provision::SEAL`. The SMB parts are struck as overtaken by the
   2026-08-30 removal of the SMB implementation.

The sections below are the analysis that preceded the ruling.

## Two questions, because the block's subject was removed under it

1. Where a secret physically comes from on a real boot. This is the fork, and it survived the
   removal of the thing that motivated it.
2. Whether milestone 131 is re-aimed or retired, which the block says is calef's and which no
   record answers.

## Why question 2 exists

The subject was removed on 2026-08-30. Milestone 131 was about configuring an SMB share and
provisioning its secret, and calef decided that day to remove the SMB implementation
(`notes/smb.md`). Two of its four deliverables went with it: per-resource credential endpoints for
the SMB adapter, and the boot that stops admitting guests to the share.

**Two did not, and they are not about SMB.** The configuration document generalizes to any service
that is a set of compile-time constants today. And the provisioning path is a gap in the credential
service whoever its clients are: nothing in this tree can tell a running system a secret, and the
only provisioner is a test program carrying a published fixture.

So the block is half a milestone with no name for its state. Its own status line says as much:
*"The status word is unchanged because the vocabulary has no word for a block whose subject is gone,
and minting one is calef's."*

## Question 1: the options, with what each costs

Milestone 56 built the store and the entropy. What is missing is the path into it on a boot
that is not a test.

| | where the secret comes from | cost |
|---|---|---|
| **A** | **Typed at a console.** | Honest, needs no new authority, and matches how a person expects to set a password. Requires an interactive boot to reach the credentialer, and cannot serve a machine that boots unattended. |
| **B** | **Read from a separate device** (a USB key, a second partition). | Works unattended. Moves the question rather than answering it: the secret is now at rest somewhere this system does not control, and "who may read that device" is a capability question of its own. |
| **C** | **Provisioned at image build.** | Simplest, and what the test path does today. The secret is in the image, so **the image becomes the secret** and every copy of it is a copy of the password. |
| **D** | **Bound by measured boot**, sealed to the measurement milestone 22 already takes. | Strongest and largest. Needs a sealing story this tree does not have, and fails closed in ways that are hard to recover from at 2am. |

The record offered no recommendation, because a secret's resting place is the irreversible category
by AGENTS.md's own definition: it cannot be un-decided by deleting code.

A and C are not alternatives on the same axis. C is what the tree does today and is a known defect
rather than a candidate; A is the smallest thing that is not that. Whether B or D is needed is a
question about whether an unattended boot is in scope, which is a different question and was
answered first.

## What this tree already does in the analogous case

A secret is already an endpoint rather than a value. [§41](0041-endpoint-as-broker.md) (the
endpoint is the broker) settled that the thing handed to a program is the right to ask, not the
material. [§111](0111-inert-config-is-a-validated-page.md) (inert configuration is a read-only page)
then split what Unix puts in one environment map into three, and routed secrets to §41's broker
explicitly rather than letting them ride on a page. So the shape of what a program holds is
settled; this decision is only about how the material reaches the broker in the first place.

The credential store's refusal to be updated is also considered rather than accidental.
`credentialer::Store::put` offers no replacement because *"put twice, second wins"* has a race in
it, and `credential_protocol::provision::SEAL` says that after it the store cannot be changed short
of restarting the service. Any provisioning path has to arrive before the seal, or argue with it.

## How reversible it is

Question 1 is the least reversible thing in this file, and it is worth separating the two halves.
Choosing wrong is recoverable by choosing again. Having *run* the wrong one is not: a secret that was
in an image is compromised in every copy of that image, forever, and no later decision retrieves it.

Question 2 is cheap. Re-aiming a roadmap block or retiring it is a text edit, and AGENTS.md's
*move fast on what can be undone* says to decide it quickly rather than deliberate it.

## What was blocked

Milestone 131, both halves. Both are now unblocked by the ruling above.

The parser caution stands: a configuration document read from anywhere but the image is a new
parser reading attacker-adjacent bytes, and should be host-tested and fuzzed on day one rather than
after.
