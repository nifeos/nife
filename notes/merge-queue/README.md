# The merge queue: the appendices to notes/merge-queue.md

[`notes/merge-queue.md`](../merge-queue.md) is the page to read first. The files here carry one
mechanism or one record each, at a length the main page's prose budget cannot hold:

- [prose-only.md](prose-only.md): a change that is only Markdown nobody reads skips the heavy CI
  jobs, and how the required checks stay satisfied.
- [lane-claim-check.md](lane-claim-check.md): the claim check's gap, false positives and clock.
- [what-the-merge-queue-took-over.md](what-the-merge-queue-took-over.md): what the queue took over, the drain's four earlier
  shapes, and the prevention half.
- [actions-migration.md](actions-migration.md): the move to Actions, the premise test, the
  `launchd` retirement, and what was lost.
- [at-risk-check.md](at-risk-check.md): the at-risk watch in full.
- [main-push-and-check-timeout.md](main-push-and-check-timeout.md): A′ and the 240-minute timeout.
- [what-the-queue-bought.md](what-the-queue-bought.md): what the queue bought, measured.
- [merge-methods-and-holds.md](merge-methods-and-holds.md): no squash at the repository, and
  `Blocked-by:`.
- [limitations.md](limitations.md): the BUGS list in full.

*Name: provisional. The directory follows the
`notes/<stem>/` appendix convention §212 (a prose budget) set, so its name is the main page's stem;
the file name `prose-only.md` matches the classifier it documents and changes if that is renamed.*

Each stem here is named for its content, and calef names files.
