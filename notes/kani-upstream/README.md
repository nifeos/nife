# Kani upstream records

Name: unrecorded. Provisional, and not yet put to calef; the parent note is
[kani-upstream.md](../kani-upstream.md).

The texts for the upstream conversation (issue #2402 and the pull request), one file each, beside
the note that argues them. [model-checking/kani#4913](https://github.com/model-checking/kani/pull/4913)
is open and out of draft. feliperodri requested changes on 2026-10-05 (UTC); calef answered every
thread and asked for re-review. Upstream `main` was merged in on 2026-10-22 (merge `3b698421a`,
three conflicts from upstream #4941, which rewrote `--export-json` to the RFC 0015 shape), and
`c264c6929` makes that export report the verification target and updates RFC 0015's field table.
It waits on a maintainer approving the fork's CI run, then on re-review. The comment drafted in
[issue-2402-comment.md](issue-2402-comment.md) is not posted on #2402.

- [pull-request-4913-review.md](pull-request-4913-review.md): the rehearsal review of the opened
  pull request (2026-09-30, UTC).
- [pull-request-4913-before-ready.md](pull-request-4913-before-ready.md): the retest, patch, body
  edits and checklist from before the pull request left draft. The patch and body edits are in the
  pull request; the file is the record of the retest.
- [kani-lib-link-order.md](kani-lib-link-order.md) and [kani-lib-link-order.patch](kani-lib-link-order.patch):
  linking `kani_lib.c` as a precompiled object, measured on patagonia, in an arm64 Linux container
  and on a GitHub x86_64 Linux runner. It is part of #4913 (`1eaa2d9`, with the Apple Silicon
  `char` fix in `9b72fa016`), not a separate pull request. nife's carried patch has the same
  change (milestone 635 (riscv64 proofs check against the riscv64 model)).
- [pr-4913-followups.patch](pr-4913-followups.patch): three small review findings, in #4913 as
  `450cbc2`.

- `pull-request-4913-body` Name: ratified 2026-10-18 (calef, "Approve all"). Refused `pull-request`.
