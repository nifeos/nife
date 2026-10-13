---
status: PARTIAL
raised: 2026-10-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 871. A sixth outsider pass attacks the confinement claim

*(Title, slug and every name below are provisional.)*

**Reuse:** the counting rule, the report format and the refusal log are milestone 800 (a
non-Anthropic model attacks the confinement claim)'s, unchanged, so the passes compare row for row.
This pass is by GLM 5.3, the non-Anthropic model that ran pass 4 (milestone 800), so it serves
criterion (c)'s non-Anthropic half. If the fifth pass, milestone 867 (a fifth outsider pass attacks
the confinement claim), is clean, this pass can be the second of the two consecutive clean passes.

## Index row

The sixth outsider pass at risk 7's confinement claim, by GLM 5.3 (non-Anthropic). It attacks the
newest shipped surfaces first, the milestone 801 (packages over the internet) fetch path, then the
§255 (each socket is its own capability) socket-capability model as variants of pass 5's ground.
It counts an attack only when it boots, and keeps the standing refusal log.

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

- The milestone 801 package fetch path, shipped 2026-10-10 in PRs #1884 and #1890. Its parts:
  `package_index`'s admission of a hostile index (Q1's private and link-local refusal, the "moved
  to" field) and the client's two index addresses. Then digest admission through
  `package_archive::installable_as`, the §252 (a resolver grant is one zone per client badge)
  resolver badge behind std's `ToSocketAddrs`, and the fetch pinned to one TLS root.
- The §255 socket-capability model, as variants of pass 5's booted row-34 ground.
- The 34 rows of the claims table, re-read, each attacked or refused with a written reason.

## Result

PARTIAL, 2026-10-10 (UTC). The pass booted one escape on its first surface and stopped there. The
milestone 801 (packages over the internet) package client checked one resolution of a listed
location's host, then connected by name, which resolved again. A rebinding name server answered
the check public and the connect private, and the client reached the private peer Q1 forbids. The
failing test went in first. calef ruled on #1901 (21:04 UTC): "Yes, it is an escape on a shipped
path. It was literally just shipped, but counts." Criterion (c)'s count restarted at zero. Then
"Yes, launch the fix": `package_index::Location::check` hands back the one resolution it passed as a
`CheckedLocation`, and the client dials only those addresses and keeps the host name for TLS. The
test is green on aarch64, riscv64 and x86_64, and red on all three when connect-by-name is patched
back.

Not done: the 34-row sweep and the variant work against the fixed escapes. With the count already
at zero, they could no longer make this pass a clean one. See
`notes/confinement-outsider-pass-6.md` for the escape, the fix and the refusal log.

## Done means

- Every claim has an attack, or a written reason it was not attacked.
- Every escape is a failing test committed before any fix.
- The pass-6 note exists with its refusal log, and is indexed in `notes/README/` and its area page,
  which pass 5's draft forgot and lint caught.
- Risk 7's appendix cites the pass through the maintainer under §216 (fatal-risk facts are
  correctable, and verdicts are the architect's). Moving the color stays calef's.

## Follow-on

- **Outstanding.** The rest of this pass: the 34-row sweep and the variant work, unrun. A seventh
  pass starts criterion (c)'s count afresh and would cover the same rows, so this remainder is
  likely covered by it rather than run on its own; that is the maintainer's call when it is
  minted.
- **Milestone 855.** A claims-table row for the package fetch: "A listed location never reaches a
  private or link-local address" has no row in `notes/confinement-claims.md`. Criterion (b) wants
  every row replayable, and this test's falsification can only be `attested` until milestone 855
  (the TLS graph enters the gated build) puts the exerciser in a gate, so the row waits on 855.
- **Milestone 198.** A human review, or a public bounty once a stranger can install nife, is the
  stronger form and waits on milestone 198 (a package manager, and the trivial install that makes
  a second customer possible).
- **Milestone 825.** The redoxfs name-window TOCTOU remains the open shipped-path refusal both
  passes declined to boot; its probe belongs to milestone 825 (a hostile client races the file
  server's name window), NOT-STARTED.
