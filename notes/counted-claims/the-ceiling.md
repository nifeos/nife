# The ceiling, which is the floor read upside down

*Name: provisional. Named after the parent's own heading.*

Milestone 134 (the register of measures) added the third relation
[`notes/counted-claims.md`](../counted-claims.md) lists, on 2026-08-18. This is why it exists, how its headroom is chosen, the one ceiling that became a
ratchet and was retired, and the auto-fix that was refused.

The floor suits a quantity where more is better and the bad event is a deletion. Some quantities
are the other way up, and unsafe is this tree's clearest one: nobody wants more of it, every
addition should carry a reason, and the bad event is an upward drift that no single commit looks
responsible for. calef asked it as one question, *"is that something we should be monitoring and
driving in a particular direction over time?"*, and a direction is exactly what the other two
relations cannot express. Equality says "this is the number"; a floor says "at least"; only a
ceiling says "and it should not grow".

A ceiling is written above the tree, and the headroom is measured rather than guessed. In the
fourteen days to 2026-08-18, 38 non-merge commits changed the number of `unsafe {` outside
`kernel/src/arch/`. A ceiling written at the tree's exact value would have fired on all 38. It would
have sent every one of them to edit the same line of the same note, which is the merge hotspot this
convention already manufactured once with `kani-harnesses`. The exception is a population small and
consequential enough that every addition deserves the stop: `unsafe-thread-safety-claims` is written
at the tree's exact value, and it moved twice in three weeks rather than 38 times in two.

And the quantity often has to change to make a ceiling honest. The obvious ceiling on unsafe is
a count of blocks, and it is wrong for a tree that is still being built. Outside `arch/` the count
went 171 to 747 in a month, while the density fell from 228 to 93 per 10,000 lines. A count
ceiling would have fired on almost every lane, which is the signature of a check that gets deleted.
A density ceiling holds a trend that is already going the right way, and stays silent for a lane
adding a driver at the tree's own rate. Choosing the relation is half the work and choosing the
quantity is the other half.

## A third ceiling, `agents-md-lines`

Milestone 118 (CLAUDE.md has a budget) added it on 2026-08-22, written at the tree's exact value for
the same reason `unsafe-thread-safety-claims` is: every line added to `AGENTS.md` deserves the stop. Milestone 118 measured that file's growth as stepwise rather than diffuse (two
commits were 61% of thirteen days of growth, each a whole new tenet section) and wanted exactly what
"the point is it converts 'should I add this rule?' into 'what does this replace?'" describes: not
headroom to absorb ordinary drift, but a stop on every single addition, forcing a deliberate,
recorded raise. The marker cannot live in `AGENTS.md` itself, because a developer lane may not edit that file
(AGENTS.md's own naming section). So it lives in `design/roadmap/0118-constitution-budget.md`,
which is where the budget question was raised. That is exactly the kind of indirection the floor's
BUGS entry already names ("a reader who wants the number has to go to" the other file). The
gate still runs against the real file; only the claim about it moved.

And since 2026-09-24 that ceiling is a ratchet, the one ceiling that fails in both directions.
Splitting `AGENTS.md` (#1189) took it from 935 lines to 584, and an ordinary ceiling is silent below
itself, so the old number would have stopped nothing. A name in `script/lint`'s `RATCHET_CEILINGS`
also fails when the tree falls below the claim, and the only fix is to lower the claim. It stays in
the marker rather than a separate baseline file, because the marker already is the baseline.
Retired 2026-10-06 (UTC) by calef's "drop 118's limit and rely on §212's cap", per §212 (a prose budget);
`RATCHET_CEILINGS` is now empty.

## Deliberately not built: an auto-fix

 A `--fix` that rewrote marked numbers was considered and
refused. This gate's failure message offers two responses, and they are not equally likely to be
right: *fix the number*, or *fix the derivation if the tree is right and the gate is asking the
wrong question*. An auto-fix biases every disagreement toward the first, and the second is where the
real bugs are; the `multicast_dns_protocol` shard hole was found exactly that way. With two exact sites left,
hand-editing is cheap and thinking is the point.
