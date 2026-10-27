---
status: NOT-STARTED
raised: 2026-08-17
milestone_dependencies: none
decision_dependencies: 165
machine_requirements: none
specific_machine: none
needs_person: no
---
# 131. Configure a service instead of compiling it, and type its secret at a console

Two deliverables, re-aimed by §165 (where a stored secret comes from) on 2026-10-27 (UTC):

1. A configuration document for one real service whose settings are compiled-in constants
   today. The service is chosen when a lane takes the block; it names the one it picked and why.
2. Console provisioning of a secret before `provision::SEAL`. A person types the secret at a
   console on an interactive boot, and the credential service stores it ahead of the seal. An
   unattended boot is out of scope; a separate device and measured-boot sealing wait for a
   deployment that needs one (§165). Image-build provisioning stays a test path.

The SMB deliverables (per-resource credential endpoints for the SMB adapter, and the boot that stops
admitting guests to the share) are overtaken by the removal of the SMB implementation on 2026-08-30
(notes/smb.md). Per-resource credential endpoints are also refused for the credential service
itself by §172 (whether a verify endpoint names the identity it asks about).

## What "configured" means today

Nothing is configurable. The only provisioner of a secret is a test program carrying a published
fixture, so nothing in the tree can tell a running system a secret. Changing a service's settings
means editing Rust and rebuilding.

## The precedent to copy

The retired mDNS responder's configuration document is the shape (notes/mdns.md; the code is at
commit `0652c981`). A document described what the machine advertised, a host-tested crate parsed it
with line-numbered errors, and what the machine advertised lived in a document a person edits rather
than in constants. `crates/timetable` is a live copy of the same shape.

## BUGS

- A configuration document is a new parser reading attacker-adjacent bytes if it is ever read from
  anywhere but the image. The retired `multicast_dns_config` was host-tested and fuzzed; this
  document is held to the same standard on day one rather than after.
- The secret is typed once per boot and the store takes no second write (`credentialer::Store::put`
  offers no replacement, and `SEAL` ends provisioning). A mistyped secret means a restart of the
  credential service.

## Index row

every service setting is a compile-time constant, and nothing can tell a running system a secret
except a test program carrying a published fixture. Re-aimed 2026-10-27 by §165: a configuration
document for one real service, and console provisioning of a secret before `provision::SEAL`. The
SMB parts of the block are overtaken by the 2026-08-30 removal of the SMB implementation.
