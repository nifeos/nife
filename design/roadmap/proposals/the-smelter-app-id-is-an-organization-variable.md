---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The smelter App id is an organization variable, not a secret

*The title and slug are provisional.*

Raised by the lane `lane/crate-release-secrets-selected-repositories` on 2026-11-01 (UTC), recording
a follow-on approved the same day alongside the ruling that `NIFE_SMELTER_APP_ID` and
`NIFE_SMELTER_PRIVATE_KEY` use the visibility "selected repositories". An App id is not sensitive:
it appears in the App's public page and in token requests. Only the private key needs to be a
secret, so the id becomes an organization variable and the secret list shrinks to one.

The change is in `nifeos/crate-workflows`: its reusable release workflow reads the id from
`vars.NIFE_SMELTER_APP_ID` where it now reads `secrets.NIFE_SMELTER_APP_ID`, and a new release of
the template carries it. Each crate repository then moves to that release. The variable needs the
same "selected repositories" scoping as the key, for the same reason, and the extraction checklist
in milestone 857 (crates released on their own) loses one grant per new repository.

Not done in this lane; it is a milestone for a lane in `nifeos/crate-workflows`.

## BUGS

- Until it lands, every new crate repository needs both grants.
