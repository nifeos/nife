---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 346. The first RedoxFS pin bump, on the engine that holds backups

Premise re-checked 2026-09-19 and it holds. `vendor/README.md` still pins RedoxFS at 0.9.1, upstream git sha
`473b4baeb041ebe14504f30693393b1cae52558c`. It still lists the five divergences in the same order
with divergence 3 kept rather than reverted. Nobody has performed a bump; the "Bumping a pin"
procedure has still never been executed. How far upstream has moved since the 37 non-merge commits
`script/vendor-watch` reported cannot be re-measured without network, so whoever takes this re-runs
the watch rather than quoting that number.

Everything this needs exists: the pinned sha, the five recorded divergences,
`script/vendor-verify --write-patch`, the suite, and milestone 37's crash injector. One clause is
calef's if it comes up, and it is named below rather than blocking the start.

**In brief.** `script/vendor-watch`'s first run found upstream RedoxFS 37 non-merge commits ahead
of the pinned sha, including `fix: do not hardcode # of sectors per block`, with 0.9.1 still the
newest published version. Bumping the pin means raising it, re-applying the five divergences,
regenerating the patch with `script/vendor-verify --write-patch`, and re-running the suite plus
milestone 37's crash injector. Nobody has ever performed this job in this tree, which is the second
reason to do it: the procedure in `vendor/README.md`'s "Bumping a pin" section has never been
executed.

## Why this matters

This is the case milestone 203 was built to catch, and it was live on the day the watch was built. A
correctness fix is sitting on upstream master and will not appear in a published version on any
schedule anyone here controls. The pinned engine is the one holding a filesystem, and the whole
argument for vendoring rather than writing it (§46: vendor where correctness is won by exposure) is
that upstream's exposure accrues to us. It only accrues if somebody moves the pin.

There is a second cost that grows on its own. The five divergences are re-applied by hand at every
bump, and three of them are re-applied forever. The further the pin drifts behind, the more upstream
restructuring those three have to survive at once, and `vendor/README.md` is already honest that if
upstream restructures the code they touch they may not be re-applicable at all. Thirty-seven commits
is the cheapest this job will ever be.

## The clause that is an architect's, if it fires

Divergence 3 is kept rather than reverted only because reverting it is an architect's call. A bump
is where the five get re-litigated. If a divergence will not re-apply, the honest outcome is a
decision (rewrite it, stay on the pin and record why, or fork permanently) rather than a forced
patch, and that is where a lane stops and writes it up.

## Index row

`script/vendor-watch`'s first run found upstream RedoxFS 37 non-merge commits ahead of the pinned
sha, including `fix: do not hardcode # of sectors per block`, with 0.9.1 still the newest published
version. A correctness fix sits on upstream master that no published version will carry on any
schedule anyone here controls, in the engine that holds backups. DECISIONS §46 (thin primitives or whole subsystems; we write)'s argument for
vendoring is that upstream's exposure accrues to us, and it accrues only if somebody moves the pin.
The cost grows on its own: the five divergences are re-applied by hand at every bump, three of them
forever, and `vendor/README.md` admits they may stop being re-applicable. Nobody has performed the
job in this tree. If a divergence will not re-apply, the outcome is a decision and not a forced
patch, and that is where a lane stops.
