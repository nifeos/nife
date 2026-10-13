---
status: NOT-STARTED
raised: 2026-10-10
promoted_from: an-image-carries-its-distributions-root-as-a-default-trust-line
milestone_dependencies: 666, 858
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 894. An image carries its distribution's TUF root as a default owner trust line

*(The title and slug are provisional.)*

calef ruled Q3 on #1884 on 2026-10-10 (UTC): "K4, a default owner trust line". The comment there is the record.

## What was ruled

The measured image carries basalt's TUF root as its SHA-256, installed as a default owner trust
line. The line takes the form §220 (signed builds: a vendor signs, a developer self-signs, and
trusting a key is scoped) gives a key, and the owner may remove or replace it. An image carries exactly one such
line, its own distribution's root. Q5 extends the model: any further repository the owner adds
is trusted by a line of the same form, with a §220 ceiling.

## The work

1. The line's form for a TUF root: §220's line names an Ed25519 key, and this one names a root
   document's digest. Recommend a second line kind beside `key`, so the table stays one table.
2. The build writes the line into the measured archive beside the catalog, from the root
   basalt's release tooling publishes. The boot installs it as the first generation of the
   owner's trust table (milestone 666 (a signed build installs up to its key's ceiling)).
3. The TUF client (milestone 858 (lab machines update themselves through packages, and only a new
   kernel reboots them)'s items 4 and 5) verifies `1.root.json` against the line before trusting
   anything the repository signs.
4. Removing or replacing the line is the owner's act at the console, as removing any trust line is.

Reuse: the trust table is milestone 666's, the verifier milestone 858's (rust-tuf with RustCrypto,
per its Fork 9), and the line is written by the archive build that already writes the catalog.
Nothing here is new code beyond the line's kind and its placement.

## What waits on others

- The decision text: §220 clause 1 and §195 (a reviewed recipe vouches for a package) clause 4
  are amended by the ruling, and the integrator writes that text.
- Key custody: who holds basalt's root keys, and how many. calef's, later.
- A root to carry: none exists until basalt publishes a TUF repository.

## Index row

The measured image carries basalt's TUF root as a default owner trust line, which the owner may
remove or replace, so a fresh install trusts its own distribution and nothing else.
