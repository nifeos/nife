# Effort, calibrated from git history

*Name: provisional, minted 2026-10-13 (UTC) by lane/prose-cap-notes-c, after the parent's "Effort".*

Moved verbatim from [`../roadmap.md`](../roadmap.md)'s "Effort" under §212 (a prose budget) on
2026-10-13 (UTC).

Effort, calibrated from git history (2026-07-30), not guessed. The milestone files give effort in
lanes: one lane is one agent session end to end. Measured across the fourteen milestone branches
merged so far, a lane is 31 to 57 minutes of wall clock, 1 to 9 commits, 7 to 30 files, and 694 to
4,351 inserted lines. That is a much narrower band than the work's apparent ambition suggests.
Proving the DMA boundary, building a compositor, and confining a C component all cost about the same.
Milestones that took more than one lane took them as *phases that landed separately* (27 and 30 took
three each, 22, 29, 31 and 35 two each), not as one long push.

This replaces an S/M/L scale that was written before any of it was built. It was systematically
pessimistic where it can now be checked: 27, 29, 30 and 32 were each labeled "Effort L", and each
came in at roughly a lane per phase. Anything still labeled by feel rather than by history says so.
Re-derive with `git log --first-parent` over the merge commits rather than trusting these numbers as
they age.
