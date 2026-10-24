# What the reconnaissance found

*Name: provisional. After the parent's "What the reconnaissance found". calef names files.*

An appendix of [reviewing the work of one model](../model-attribution-review.md). The parent keeps
one paragraph per finding, and the plan built on them. This file keeps the seven findings whole,
with their tables, in the order they were run. Everything here was measured on `origin/main` at
`0b993b27`.

### 1. Sonnet's work is one week and one session, and the session is still open

Every one of the 180 Sonnet-trailered commits falls between 2026-08-22 and 2026-08-27. No other
model has a single commit after 2026-08-21.

| Model | Commits | Span | Distinct `Claude-Session` ids |
|---|---|---|---|
| Claude Opus 5 (1M context) | 623 | 07-29 .. 08-20 | 4 |
| Claude Fable 5 | 338 | 07-27 .. 08-16 | 2 |
| Claude Opus 4.8 | 233 | 07-22 .. 08-13 | 3 |
| Claude Sonnet 5 | 180 | 08-22 .. 08-27 | **1** |

178 of the 180 carry a `Claude-Session` trailer and all 178 name the same session,
`01XbKDohBwwjgJtdWD9qmoa4`. That is the session this lane is running inside. A session is not a
model, since the model can be switched mid-session, but the direction of the inference still holds:
Sonnet's entire footprint in this tree came out of one session over six days.

This is the finding that reshapes the plan. The comparison anyone would naturally draw, Sonnet
against the other models, is a comparison of one session against another, over a different week, on a
different tree, at a different milestone range, with different lanes and different briefs. Model is
perfectly confounded with date, and date carries the merge queue's arrival, milestone numbers in the
150s to 180s, an x86_64 port in flight, and a tree-wide rename campaign. Nothing in the commit record
can separate those.

It also means the concern itself has a plausible alternative explanation that costs nothing to state:
the work calef has been reading most recently is Sonnet's, because all of it is recent.

### 2. Attribution is clean per pull request, and that recovers a third of the blind spot

648 of 2054 non-merge commits carry no model trailer, so coverage is 68%. The gap is not random.
Mapping every non-merge commit onto the pull request that merged it:

- 95 pull requests contain at least one Sonnet-trailered commit.
- Zero of those 95 contain a commit trailered to any other model. A pull request is a lane, and a
  lane ran one model.
- Those 95 pull requests hold 242 commits: 180 trailered, 62 untrailered, 0 other-model.

So the 62 untrailered commits inside Sonnet pull requests are Sonnet's in everything but the trailer,
and Sonnet's real footprint is at least 242 commits, 34% more than the trailer count. Attribute
by pull request, not by commit. Across the whole tree the same rule holds well: of 533 pull requests
carrying any trailer at all, 338 are single-model and only 5 mix models.

### 3. The rest of the blind spot cannot be recovered from the commits

No untrailered commit carries any other Claude marker. Not a `Claude-Session` line, not a "Generated
with Claude Code" footer, zero out of 648. Against 1022 of the 1406 trailered commits that do carry a
session id. The markers are all-or-nothing per commit, so there is no second source to fall back on.

That matters most in the window under question. From 2026-08-22 onward there are exactly two
populations:

| Population | Pull requests | Commits |
|---|---|---|
| Sonnet-trailered lanes | 95 | 242 (180 trailered + 62 not) |
| Lanes with no trailer of any kind | 80 | 140 |

There is no other model in the window at all. The only same-window comparison available is
against 80 pull requests whose author is unknown, and they could be Sonnet lanes whose trailer never
fired.

### 4. Where the work went, and it is not mostly code

| Bucket, % of that model's touched-file rows | Opus 5 (1M) | Fable 5 | Opus 4.8 | Sonnet 5 | (none) |
|---|---|---|---|---|---|
| `kernel/` | 11.0 | 11.5 | 28.1 | **25.7** | 19.4 |
| `kernel/src/arch/` | 2.7 | 2.2 | 8.8 | **6.5** | 5.3 |
| `crates/` | 14.4 | 17.2 | 12.7 | 13.5 | 17.6 |
| `user/` | 14.6 | 9.1 | 8.1 | **15.6** | 8.9 |
| `design/` | 16.7 | 22.9 | 8.4 | 14.9 | 14.9 |
| `notes/` | 17.9 | 14.6 | 15.1 | 11.4 | 15.9 |
| `script/`, `xtask/`, CI | 9.0 | 7.7 | 9.1 | **3.7** | 7.0 |

Sonnet sits second only to Opus 4.8 in kernel share, which is the highest-risk destination in the
tree, and lowest of all five in tooling. Its commits touch 7.3 files each against 4.1 to 5.2 for
everyone else. 80 of its 180 commits touch no `.rs` or `.s` file at all.

Sizes, for pricing anything below: 46,223 diff lines total, of which 38,459 are in commits that touch
code and 31,414 are in the 72 commits that touch `kernel/`. 22,407 of the lines it added are live in
`HEAD` as `.rs` or `.s`.

### 5. It sat squarely on the irreversible list, and that is the review worth doing

*Move fast on what can be undone* names the expensive categories: anything two programs agree on,
names, dependencies, the syscall surface, and facts that left the machine. The Sonnet window is
unusually concentrated in exactly those.

| Category | Sonnet commits | Detail |
|---|---|---|
| `crates/abi` (the syscall surface) | 7 | all of them renames, 649 diff lines across this row and the next two |
| `kernel/src/syscall.rs` | 6 | |
| `crates/*_proto` (wire crates) | 23 | 14 files across 10 protocol crates |
| Cargo manifests | 14 | 17 manifests, plus 12 commits touching a `Cargo.lock` |
| Roadmap and decision records | 90 | 47 files |
| `AGENTS.md` | 3 | |
| Renames | 52 | including `untyped.rs` to `memory_region.rs`, `Tcb` to `ThreadControlBlock`, `CSpace` to `CapabilityTable` |
| New crates | 7 | `jh7110_entropy_source`, `login_contract`, `schedule_store`, `counter_frequency_contract`, `uptime`, `user_mode_runtime` additions, `watch` |
| New user programs | 7 | `login`, `printenv`, `uptime`, `watch`, `session_reviver`, `jh7110_entropy_source`, `login_test_client` |

Milestone 158 (build the eleven kernel object and identifier renames) is most of that, and it landed
in this window. So the week's largest single act was a tree-wide rename of the capability vocabulary,
which is the most expensive thing on the reversibility list and the one that lands in a reader's head
rather than in a file.

The naming half of this needs no new review, and that is worth saying because it is the part
that looks most alarming. `script/names` already tracks it. calef already ratified the big ones in
the window (`memory_regions`, `page_frames` and `login_contract` all carry 2026-08-23). The rest sit
on the existing worklist as `provisional` or `unrecorded` (`uptime`, `watch`, `printenv`,
`session_reviver`, `schedule_store`, `counter_frequency_contract`, `jh7110_trng`, `login`, `login_test_client`).
That is the mechanism working exactly as designed. `script/names --unratified` is the queue; it does
not want a study.

*Two of the names in the three rows above have since moved: the crate and program `jh7110_trng`
became `jh7110_entropy_source` when calef ratified it on 2026-09-13, and `jh7110_entropy` when he
replaced that later the same day (performed 2026-09-14). They are spelled here as they were in the
window this reconnaissance measured, which is what makes the counts checkable against the commits
they came from.*

### 6. Three defect metrics, and they rank the models three different ways

This is the part that most wanted to become a finding and must not.

**Survival.** Share of a model's added lines still live in `HEAD`:

| | Opus 5 (1M) | Fable 5 | Opus 4.8 | Sonnet 5 | (none) |
|---|---|---|---|---|---|
| lines added | 147,042 | 72,991 | 30,536 | 36,886 | 131,111 |
| survival | 70% | 77% | 63% | **88%** | 77% |

Sonnet looks best by a wide margin. The number is worthless: survival rises monotonically as code
gets younger, and Sonnet's is the youngest by three weeks.

**File-level fix-up.** For each code commit that went through a pull request and has a full 72 hours
of follow-up available, did a later commit *in a different pull request* touch one of the same files
with a fix-shaped subject:

| Opus 5 (1M) | Fable 5 | Opus 4.8 | Sonnet 5 | (none) |
|---|---|---|---|---|
| 36% (124) | 24% (140) | 0% (4) | **57% (51)** | 31% (232) |

Sonnet looks worst by a wide margin. This number is also close to worthless, in the opposite
direction. It is file-level, so any commit landing near another in the same file counts, and a
tree-wide rename campaign guarantees a wave of follow-up commits repairing stale references in files
the renamer touched. Read the actual pairs and most of them are coincidence: a clippy lint fix
following an unrelated builtin, a docs pass on stale symbol names following an x86_64 interrupt
change.

**Line-level, blamed back.** The measurement worth running. For each fix-shaped commit, take the
lines it deleted, blame them at its parent, attribute them to whoever wrote them. Normalized per
1,000 code lines added, split by subsystem, with the follow-up window capped at 6 days for every
model so exposure is equal:

| per 1k lines, fixes within 6 days | Opus 5 (1M) | Fable 5 | Opus 4.8 | Sonnet 5 | (none) |
|---|---|---|---|---|---|
| `kernel/` | 2.1 | 12.1 | 16.2 | **11.5** | 3.9 |
| everything else | 1.2 | 1.4 | 11.9 | **8.9** | 2.4 |

Sonnet lands mid-pack in the kernel, indistinguishable from Fable and better than Opus 4.8. It is
higher than everyone but Opus 4.8 outside the kernel. The rename campaign was checked as a confound
and cleared: only 9 of the 251 lines blamed to Sonnet were written by a rename commit.

Three metrics, three orderings, and the whole tree has 77 fix-shaped code commits to build all of
them on. The counts behind each cell are in the low hundreds of lines, the follow-up window for
Sonnet's most recent commits is a day rather than six, and no confidence interval drawn around these
would exclude "the models are the same". There are zero commits in this tree whose subject begins
with `Revert`, so the sharpest available signal does not exist at all.

The honest reading is that **the cheap evidence does not discriminate**, and that anyone who wants it
to will find a metric that says what they came for. That is the finding, and it is why the tiers
below exist rather than a verdict.

### 7. The sample is uneven, and the good end of it is very good

`1259dc07` is Sonnet's, dated 2026-08-27. The icount tripwire caught an 11.4% `spawn_el0`
regression. The commit bisects it by swapping `kernel/` alone between three of its own branch's
commits, with the rest of the tree held constant. It attributes 29,302 of the 35,512 ticks to a
specific reclamation sweep by stubbing that sweep out, and halves it. It states the remaining 6.4%
as the price of a correctness fix rather than hiding it, and re-records the baselines with the
reason. And it reports a further optimization that was built, measured and recovered 3,820 ticks,
then was dropped because it destabilized block layout. It also names a movement on an unrelated bench row that nobody asked about.

That is better work than most of this tree, by any model. Commit-message length says the same thing
about the middle of the distribution: Sonnet's median body is 1,038 characters against Opus 5's 1,457
and Fable's 673, and 8% of Sonnet's are under 200 characters against Opus 5's 1%.

If the sample is uneven rather than uniformly weak, then the thing to review is conditions and
not a model. Which lanes were rushed, which briefs were thin, which subsystems were unfamiliar, and
how many lanes were running at once against the hot files? A review that reports "the model is worse"
when the truth is "four lanes were racing in `system_tests/src/user/tests.rs` on Tuesday" has found nothing
actionable, because nobody can change the model but everybody can change the brief.
