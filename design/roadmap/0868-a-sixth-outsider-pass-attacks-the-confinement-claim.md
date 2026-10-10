---
status: IN-PROGRESS
branch: milestone/868-a-sixth-outsider-pass-attacks-the-confinement-claim
raised: 2026-10-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 868. A sixth outsider pass attacks the confinement claim

*(Minted 2026-10-10 (UTC) by this lane as the maintainer's delegate, the way milestone 800 (a
non-Anthropic model attacks the confinement claim) was minted. The number 868 is provisional; other
lanes are minting nearby, so expect renumbering at merge. Title, slug and every name below are
drafts.)*

**Reuse:** the counting rule, the report format and the refusal log are milestone 800's, unchanged,
so the passes compare row for row. This pass is by GLM 5.3, the non-Anthropic model that ran pass 4
(milestone 800), so it serves criterion (c)'s non-Anthropic half: if the fifth pass (PR #1895,
Anthropic, unmerged when this was written) lands clean, this pass can be the second of the two
consecutive clean passes.

## Index row

The sixth outsider pass at risk 7's confinement claim, by GLM 5.3 (non-Anthropic). It attacks the
newest shipped surfaces first, the milestone 801 (packages over the internet) fetch path, then the
§255 (each socket is its own capability) socket-capability model as variants of pass 5's ground,
counts an attack only when it boots, and keeps the standing refusal log.

## Why

Risk 7 (the confinement claim is false) is AMBER. Its criterion (c) for green is two consecutive
independent attacks with no escape on a shipped path, at least one by a non-Anthropic model or a
human. A pass that leaves a refusal on a shipped path unexamined does not count (calef, "Add the
refusal log."). Pass 4 (milestone 800) booted the socket capture of milestone 649 (every client of
a network stack shares its socket numbers) and restarted the count at zero. The fifth pass (PR
#1895, Anthropic, unmerged when this was written) found no escape; once it lands, this pass is the
second of the two, and it supplies the non-Anthropic half.

## The attack

Informed, the posture milestone 800 set: the whole tree and its history are in hand, as any
attacker of a public repository has them. Variant analysis against each fixed escape, and new
ground where nothing has been found. The surfaces, newest first:

- The milestone 801 package fetch path, shipped 2026-10-10 in PRs #1884 and #1890:
  `package_index`'s admission of a hostile index (Q1's private and link-local refusal, the "moved
  to" field), the client's two index addresses, digest admission through
  `package_archive::installable_as`, the §252 (a resolver grant is one zone per client badge)
  resolver badge behind std's `ToSocketAddrs`, and the fetch pinned to one TLS root.
- The §255 socket-capability model, as variants of pass 5's booted row-34 ground.
- The 34 rows of the claims table, re-read, each attacked or refused with a written reason.

## Result

See `notes/confinement-outsider-pass-6.md` for the full pass, the per-row table and the refusal
log.

## Done means

- Every claim has an attack, or a written reason it was not attacked.
- Every escape is a failing test committed before any fix.
- The pass-6 note exists with its refusal log, and is indexed in `notes/README/` and its area page,
  which pass 5's draft forgot and lint caught.
- Risk 7's appendix cites the pass through the maintainer under §216 (fatal-risk facts are
  correctable, and verdicts are the architect's). Moving the color stays calef's.

## Follow-on

- **Milestone 198.** A human review, or a public bounty once a stranger can install nife, is the
  stronger form and waits on milestone 198 (a package manager, and the trivial install that makes
  a second customer possible).
- **Milestone 825.** The redoxfs name-window TOCTOU remains the open shipped-path refusal both
  passes declined to boot; its probe belongs to milestone 825 (a hostile client races the file
  server's name window), NOT-STARTED.
