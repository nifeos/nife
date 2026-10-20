---
status: BUILT
raised: 2026-10-10
built: 2026-10-19
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

BUILT, 2026-10-19 (UTC). The pass booted one escape on its first surface and stopped there. The
milestone 801 (packages over the internet) package client checked one resolution of a listed
location's host, then connected by name, which resolved again. A rebinding name server answered
the check public and the connect private, and the client reached the private peer Q1 forbids. The
failing test went in first. calef ruled on #1901 (21:04 UTC): "Yes, it is an escape on a shipped
path. It was literally just shipped, but counts." Criterion (c)'s count restarted at zero. Then
"Yes, launch the fix": `package_index::Location::check` hands back the one resolution it passed as a
`CheckedLocation`, and the client dials only those addresses and keeps the host name for TLS. The
test is green on aarch64, riscv64 and x86_64, and red on all three when connect-by-name is patched
back.

Row 35 of `notes/confinement-claims.md` holds the fetch path's claim: a listed location never
reaches a private or link-local address, and the address dialed is the one the check passed. Its
record is replayable: the patch makes the client connect by name again, and it booted red
on aarch64, riscv64 and x86_64 on 2026-10-19 (UTC). The weekly sweep replays it.

The table sweep and the variant work against the fixed escapes did not run here. With the count
already at zero, they could no longer make this pass a clean one. calef ruled on #2432 (2026-10-19
23:18 UTC): "New pass." They belong to the tenth outsider pass, a GLM 5.3 pass under
`briefs/confinement-outsider-pass.md` that can count toward criterion (c): milestone 936 (a tenth
outsider pass attacks the confinement claim), in
`design/roadmap/0936-a-tenth-outsider-pass-attacks-the-confinement-claim.md`. Its floor
includes row 35, so it covers this pass's variant ground. See
`notes/confinement-outsider-pass-6.md` for the escape, the fix and the refusal log.

## Done means

- Every claim has an attack, or a written reason it was not attacked.
- Every escape is a failing test committed before any fix.
- The pass-6 note exists with its refusal log, and is indexed in `notes/README/` and its area page,
  which pass 5's draft forgot and lint caught.
- Risk 7's appendix cites the pass through the maintainer under §216 (fatal-risk facts are
  correctable, and verdicts are the architect's). Moving the color stays calef's.

## Follow-on

- **Milestone 936.** The table sweep and the variant work go to the tenth outsider pass, by GLM 5.3,
  in `design/roadmap/0936-a-tenth-outsider-pass-attacks-the-confinement-claim.md`. It waits
  on calef's GLM token allowance.
- **Milestone 198.** A human review, or a public bounty once a stranger can install nife, is the
  stronger form and waits on milestone 198 (a package manager, and the trivial install that makes
  a second customer possible).
- **Milestone 825.** The redoxfs name-window TOCTOU remains the open shipped-path refusal both
  passes declined to boot; its probe belongs to milestone 825 (a hostile client races the file
  server's name window), NOT-STARTED.
