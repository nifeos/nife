# Milestone 579's table, the three roles

An appendix to
[`design/roadmap/0579-constitution-carry-vs-brief.md`](../0579-constitution-carry-vs-brief.md),
milestone 579 (which of the constitution must be carried, and which is a brief). It holds the
passage-by-passage verdicts for the three roles section of `AGENTS.md`, the largest; every other
section is in [classification.md](classification.md). It was moved out of the block on 2026-10-12
(UTC) under §212 (a prose budget); the block is complete without it. Line ranges are against
`AGENTS.md` as it stood when the proposal was filed on 2026-09-23 (925 lines).

A verdict of `brief` names the provisional brief it would join; `uncertain` says why the block's two
sub-questions did not both resolve cleanly.

### The three roles (304-623)

| Lines | Opens with | Verdict | Why |
|---|---|---|---|
| 304-309 | *Named 2026-08-04, after a night...* | carry | Sets up the role vocabulary the rest of the file uses. |
| 310-326 | **Maintainer.** role definition | carry | Authority and identity a session needs to know it holds; not looked up, it is who you are for the session. |
| 327-343 | **Developer.** role, poll-to-completion, continue-until-blocked | carry | Behavioral rules for every turn of every lane (do not end a turn waiting on your own gate); this is exactly the shape the user's own standing memory rule ("poll my own background work") exists to enforce, which argues it must stay ambient. |
| 344-351 | *Every pull request and comment an agent writes opens by saying so.* | **uncertain** | The mechanics (one line, fixed wording) look brief-shaped and would fit naturally inside `claim-a-lane.md` for PR bodies. But the rule also binds plain **comments**, which are not created through any single scripted task, so moving it risks it surviving only where the draft-PR brief is read and silently dropping off ad hoc comments. |
| 352-360 | **A lane's first act is a draft pull request** §90 (the claim is a draft pull request) | brief | `claim-a-lane.md`. A specific, recognized task (starting a milestone) with a fixed sequence: empty commit, push, draft PR. The proposing lane followed exactly this recipe from its own launch brief, typed out again rather than pointed to, which is itself evidence the recipe belongs in a file. |
| 361-363 | *A developer works in a lane*, the isolation definition | carry | Three-line vocabulary definition used throughout the rest of the file; too small and too load-bearing to extract. |
| 364-384 | **Steward.** role definition | carry | Authority and boundary of a role (does not brief developers, must never hold the main checkout mid-gate); the "must never" clause is a landmine (the race that took `nife-dev` out from under a lane), not a lookup. |
| 385-391 | **The top-up rule** | carry | Central behavioral rule for every maintainer turn ("launch the next thing before writing the report"); by construction this must already be a reflex, since the whole point is that it fires before anyone asks. |
| 392-395 | **A developer's final report ends by handing off** | carry | Short, applies at the end of every report; no separate task to consult it during. |
| 396-410 | **Identified work leaves the lane in a tracked form** + rung three | carry | Must fire the moment a lane notices untracked work, which happens unpredictably mid-task, not at a named task of its own. |
| 411-413 | **The merge checklist grows one line** | brief | `merge-and-cleanup.md`. One line added to a checklist a role already runs at a specific event (merging a lane); belongs beside the prune-worktree and `nife-dev`-relink passages below (lines 508-517, 587-597). |
| 414-418 | *Two things this deliberately does not do* | carry | Short scope clarification (does not gate, does not touch `BUGS`) read together with 396-410. |
| 419-426 | **Open decisions live in a file, not in a conversation.** | carry | Must fire the moment a decision surfaces in chat, which is unpredictable, not scheduled. |
| 427-445 | **And work waiting on calef carries its own label and its own ask** | **uncertain** | Recognizing *when* to hold something (outside standing authority: system call surface, new dependency, a `DECISIONS` section owed) is judgment that has to be ambient. Writing the `## What I need from you` comment well, once that recognition has fired, is closer to a known task with a checkable shape (three properties). The passage is genuinely both; splitting it cleanly did not resolve. |
| 446-479 | **Lane count is set against the collision surface** | carry | Judgment framework for deciding how many lanes to run and which ones can collide; consulted at the moment of briefing lanes, which happens constantly and is not itself a single named task with a brief anyone would think to open. |
| 480-492 | **And there is a second ceiling**, memory / `VERIFY_JOBS` | carry, flagged landmine | Same shape as the file's other landmines: the failure (an OOM kill) presents as an unrelated timing failure or cancellation, so the rule has to be known before the job is launched, not looked up after a confusing failure. |
| 493-504 | **And a third ceiling**, disk, obsolete, points to CI | carry | Short, mostly historical, and already models the pattern this proposal recommends: it points to `briefs/gate-in-ci.md` rather than duplicating it. Worth noting as a working precedent, not worth cutting further, since what remains is the landmine (disk destroys, not delays) rather than a procedure. |
| 505-507 | **The prover is the queue's long pole** | carry | Three lines, context for the ceilings above. |
| 508-517 | **Prune a lane's worktree the moment its pull request merges** | brief | `merge-and-cleanup.md`. Tied to one event (a pull request merging), performed by one role (maintainer or steward) that already commits to doing it (the steward role definition above, lines 364-384). The failure mode is severe (this file records hitting zero bytes free twice), which argues for precision in a brief rather than for keeping it as prose nobody re-reads carefully. The safety clause ("if a lane is blocked, commit and push before removing anything") must survive verbatim into the brief. |
| 518-522 | **Two watchers run unattended on patagonia via `launchd`** | brief | `session-start.md`. A specific action at the start of a session (confirm or start two named processes), with a fixed check-and-start recipe. |
| 523-531 | **And a maintainer session checks what they already found** | brief | `session-start.md`, and this one is mostly already built: `briefs/survey-the-queue.md` already reads `mergeStateStatus` and `statusCheckRollup` and reports `DIRTY`/`CONFLICTING`/held pull requests. The prose here should shrink to "run `briefs/survey-the-queue.md` every session and treat what it finds as a task," not move wholesale; most of these nine lines duplicate a brief that exists. |
| 532-539 | *They exist because on 2026-08-04...* | carry | Historical rationale for why the watchers exist at all; short, and pairs with the role definitions rather than a task. |
| 540-545 | **Do not try to route this by requesting a review.** | carry, flagged landmine | `gh pr edit --add-reviewer` silently succeeds and does nothing; exactly the "reads like a procedure, is really a landmine" shape the proposing lane's brief named git stash as the example of. No task boundary contains the risk; anyone at any moment might reach for this. |
| 546-549 | **Stop and bring it to calef only when it is genuinely his call** | carry | The single most load-bearing judgment criterion in the roles section; must already be in head. |
| 550-553 | **Keep the documentation current** | carry | Short, applies to every task that produces a deliverable. |
| 554-570 | **The standard to aim at is FreeBSD's**, four things | **uncertain** | Reads as a natural "how to document a milestone" brief, and doc-writing is a task an agent recognizes itself as doing. But documentation here is not confined to milestones: notes, `design/decisions/` sections, `BUGS` entries and this very proposal are all "documentation" too, so the trigger is broader than one nameable task, and moving it risks losing the standard on exactly the ad hoc writing it should govern. |
| 571-573 | *The point is not the format, which is theirs.* | carry | Three-line close to 554-570; travels with it. |
| 574-586 | **Anything global to the tree is assigned by the integrator at merge, never claimed by a lane.** | carry | A scar-driven rule (three `design/decisions/` collisions in one day); must already be known before a lane reaches for a shared resource, which is unpredictable. The proposing lane's own briefing repeated "do not edit `design/decisions/`" for exactly this reason. |
| 587-597 | **Some shared state is global to the *machine*, not the repo**, `nife-dev` | brief | `merge-and-cleanup.md`. The relink command and when to run it is the integrator's task-bound duty at merge time, same event as the prune-worktree and merge-checklist passages above (lines 508-517, 411-413). One clause is genuinely ambient and should stay behind as a short carried sentence even after the mechanics move: "every lane will take `nife-dev`; that is expected, do not try to stop it." |
| 598-603 | **An unmerged branch is either abandoned or it is holding knowledge...** | carry | "Nobody reads branches" has to be known before a lane decides to leave a finding on a branch, which is a decision made at arbitrary points, not a scheduled task. |
| 604-608 | **Benchmarks and cross-OS comparisons are first-class.** | carry | Short, honesty-in-reporting rule applied whenever any number is stated. |
| 609-614 | **Push back when he's wrong** | carry | Short ethos rule for every disagreement, which arises unpredictably. |
| 615-619 | **Correct yourself loudly.** | carry | Short ethos rule. |
| 620-623 | **Explain on request, however basic.** | carry | Short ethos rule. |

