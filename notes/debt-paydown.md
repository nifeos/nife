# Which pull requests pay down debt

The `debt-paydown` label marks a pull request whose purpose is to repay something the tree already
owes, rather than to give it something new. It exists so the share of work spent repaying can be
counted (calef, 2026-10-10 UTC: "label every lane that's debt pay down, technical or otherwise. I
include all work to make the tree comply with our standards").

Name: ratified 2026-10-10 (calef, ruling Q1 on #1894), as both the label and the `script/claim`
flag. Minted the same day by lane `maintainer/debt-paydown-label`. A label name is a name, so it was
his to ratify (naming authority (`nifeos/nife:notes/skills/naming-authority/SKILL.md`)). The companion flag
`--new-work` is still provisional.

## The definition

Label a pull request when its title and opening paragraph give one of these as its main purpose.

1. Repair. Something already merged does not behave as its record says, and this makes it. That
   covers a bug or security fix in existing code and a flaky, racy or load-sensitive test. It also
   covers a leak, a gate that fires wrongly or never fires, a regression brought back inside its
   band, and lost work restored.
2. Cleanup with no new capability. A refactor, removing dead code, unused dependencies or a
   workaround, moving code to where a codebase rule says it lives (`arch/`, a crate, out of the
   kernel crate), lock contention or false sharing removed, divergence from a vendored upstream
   reduced.
3. Record repair. A record is wrong or is missing what it owes. That covers a correction of error
   (a COE), a fact correction, and a stale status, date, path or claim brought current. It also
   covers an owed record or falsification backfilled, a missing citation or provenance added, and
   knowledge landed from a branch that was holding it.
4. Compliance. The tree is brought up to a standard it has already written down. That covers
   §212 (a prose budget), §213 (writing standards), and the bold, heading, spelling and
   comment-block sweeps. It covers a ratified rename applied, survivor triage, coverage and lint
   ratchets, and an audit with its findings closed. It covers an architectural parity gap closed
   (rule 5). And it covers a gate whose purpose is enforcing a rule decided before the pull request
   that adds it.

Do not label:

- A new capability, a milestone's first build, a new driver, program or protocol.
- A proposal, a provisional milestone block, or a decision recorded in the normal course. The pull
  request that does the work gets the label; the one that plans it does not.
- Benchmarks, measurements and experiments, unless they measure a named defect so it can be fixed
  (a flake, a regression).
- Routine upkeep on a schedule: the weekly metrics, the nightly toolchain bump, Dependabot.
- A new rule, or a gate added in the same pull request as the rule it enforces.

When a pull request mixes the two, judge by what its title leads with. A fix found while building a
feature, and landed inside the feature's pull request, is not labeled; the same fix in its own pull
request is.

## Calls that set precedent

From the passes of 2026-10-10 (UTC), so the next labeler can match them:

| pull request | call | why |
|---|---|---|
| #1630, #1686 | labeled | lock contention and false sharing in existing paths: cleanup |
| #1534 | labeled | an existing claim proved on the two ISAs that lacked it: parity |
| #1409 | labeled | an unmeasured stack risk given headroom and a gate; repair of a recorded risk |
| #1447, #1486 | labeled | measurements, but of a named flake and a named drift |
| #1868 | labeled | ratchet for §266 (a Rust source file stays under 2,000 lines), decided first in #1867 |
| #1735, #1791 | not labeled | each records a new rule and ratchets it in the same pull request |
| #1589 | not labeled | a new check for a rule not written before it, and it found nothing |
| #1444, #1458 | not labeled | existing CI checks copied into pre-push: throughput, not compliance |
| #1299, #1473 | not labeled | hardware bring-up and a performance gain, not a repair of a record |
| #1744 | not labeled | a new protocol that closes a confinement claim; new capability first |
| #1309 and other rulings | not labeled | recorded promptly; #1411 and #1324 say "owed" and are |
| #802, #860, #1254 | labeled | a ruled rename carried out across the tree: compliance with the naming rule |
| #812, #862, #1720 | not labeled | a ratification that renames nothing changes no code or record that was wrong |
| #1246, #760 | not labeled | each writes a naming rule; #1254 and #1255, which apply it, are labeled |
| #168, #380 | labeled | QEMU-only assumptions that real boards disproved: latent bugs, repaired |
| #313, #989 | labeled | stranger-test runs: an audit of principle 3, with its findings fixed |
| #592, #821, #876 | labeled | a program or feature removed with nothing replacing it |
| #70, #123 | not labeled | a new analysis tool (Miri, loom) first, though each found real bugs |
| #1186 | not labeled | condensing a document before §212 existed; #1200 and later trims are labeled |
| #1553, #962, #1260 | not labeled | backfilling a metrics series adds a chart's history, not an owed record |
| #494 | not labeled | a capability's first build on a second ISA; a stated parity gap (#1487) is labeled |

## Applying it

Every lane says at the claim: `script/claim` refuses to run without exactly one of
`--debt-paydown` or `--new-work` (calef's ruling R1 on #1894, 2026-10-10 UTC). The first opens the
draft with the label on. The second adds no label but writes the choice into the draft's body. A
brief says which one to pass. A pull request opened some other way, or one whose purpose changed
mid-lane, is labeled or unlabeled by hand:

    gh pr edit <N> --add-label debt-paydown

Counting it:

    gh pr list --state merged --label debt-paydown --search "merged:>=2026-09-26" --limit 1000

## The backfill

Every merged pull request through 2026-10-10 (UTC) was judged against this definition that day, in
three windows, by one lane. calef ruled on #1894 that the backfill should cover all of history.

| merged | judged | labeled |
|---|---|---|
| before 2026-09-10 | 766 | 231 |
| 2026-09-10 to 2026-09-25 | 498 | 222 |
| 2026-09-26 to 2026-10-10 | 560 | 188 |
| all merged | 1,824 | 641 |

None of the 4 open pull requests qualified on the day.

## BUGS

- The classification is made at the claim, before the work is done. A lane that set out as new work
  and became a repair keeps the wrong answer unless somebody changes the label.
- Judged from titles and opening paragraphs. A pull request whose title undersells its purpose is
  missed.
- Adding any label to a pull request, open or merged, runs `architect-hold.yml` once, since it
  listens for `labeled`. The run is one API call; the backfill cost 641 of them.
