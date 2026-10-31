---
status: PROPOSED
raised: 2026-10-30
milestone_dependencies: none
decision_dependencies: 250
machine_requirements: none
specific_machine: none
needs_person: yes
---
# The index serves its second address

Raised 2026-10-30 (UTC) by the session that landed milestone 956 step 1. §250 clause 6 ruled the index at up to two addresses on different registrable domains. The first, `basalt.nifeos.org`, serves. The second, `basalt.crickertech.com`, does not: basalt's `backup` job exits 1 because `BACKUP_DEPLOY_KEY` is unset.

Reuse: basalt's `backup` job already downloads `publish`'s artifact, writes a `CNAME`, and force-pushes one orphan commit to `nifeos/basalt-backup`'s `pages` branch. This proposal writes no code. The steps live in basalt's README under "What calef must do for the second address".

## Why a second address

§250 clause 6 chose a second domain on a different registrar, `crickertech.com` at GoDaddy against `nifeos.org` at Name.com. One registrar account's compromise or lapsed payment cannot take both, and the client tries the second address when the first stops answering. The second address covers no shared failure: GitHub Pages, the `nifeos` organization, the one publish workflow and NS1's name servers all still serve both.

## What the tree does today

`publish` serves `basalt.nifeos.org` from the Pages site of `nifeos/basalt`. `backup` copies the same artifact to `nifeos/basalt-backup` and serves it at `basalt.crickertech.com`. On `main` it stops at its first check, `BACKUP_DEPLOY_KEY is not set`, so the second address serves nothing. The primary index stays current; that failure never held it back.

## What it takes

Six steps, all in basalt's README:

1. Generate a deploy key, add its public half to `nifeos/basalt-backup` with write access, and store the private half as `BACKUP_DEPLOY_KEY` on `nifeos/basalt`.
2. Run `packages` on `main`; `backup` creates the `pages` branch.
3. Verify `crickertech.com` for the organization before the DNS record exists.
4. Add `basalt CNAME nifeos.github.io.` in the crickertech.com zone.
5. Set `nifeos/basalt-backup`'s Pages to the `pages` branch at the root, custom domain `basalt.crickertech.com`, then enforce HTTPS.
6. Check that `https://basalt.crickertech.com/rolling/metadata/stand-in-index` answers 200 and the chain ends at `ISRG Root X1`.

## Why it needs a person

Every step is an organization-owner act: a deploy key and a secret, a domain verification, a DNS record, a Pages setting. A lane cannot do any of them, and none of them needs a machine. Nothing may record the key; `gh secret set` reads it from a file that the same command removes.

## What it does not do

It does not touch the gate, the argv contract or any package. The cross-repository gate fix is its own proposal. It does not add a third address, and it does not remove the shared failures §250 records.

## Cost and reversibility

An hour of acts, plus DNS and certificate propagation. Every step reverses: delete the deploy key and secret, the CNAME and the site, and the second address is gone. The cost of leaving it down is a failover with one address, which is the failure §250 clause 6 exists to bound.
