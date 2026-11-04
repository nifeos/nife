# Milestone 118 appendix: the applicable cut, 2026-08-18

An appendix to [milestone 118 (CLAUDE.md has a budget)](../0118-constitution-budget.md), moved out
of that block on 2026-10-12 (UTC) under [§212 (a prose
budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The block is complete without
it; this is the second lane's verification, cut by cut, and the growth measurement behind it.

[The audit](audit.md) named eight rules with a mechanism. This lane checked each claim against the
tree, and four of the eight did not survive contact. That is the finding rather than a footnote.
The audit was assembled by reading `script/lint`'s banner list, and a banner names a check's
subject, not its coverage. What follows is only what was verified by reading the gate's body and
testing its claim against the files it scans.

This lane did not edit `AGENTS.md`, and could not. That file forbids a developer to edit it, so
the deliverable is a cut the integrator applies. Line numbers are against `AGENTS.md` at
`64c08ec9`, 955 lines; apply the cuts bottom-up so earlier ranges stay valid.

## The measurement

955 lines, 11,062 words, 66,724 bytes (`wc -lwc AGENTS.md`), against 738/8,306 when calef minted
this block on 2026-08-05 and 868/10,001 when it was re-measured on 2026-08-17. That is +217 lines
and +2,756 words since minting, +29% on lines.

The growth is not diffuse, and that is the useful half. Counts are of the file as it stood at each
commit that touched it (`git show <sha>:AGENTS.md | wc -lw`, following the 2026-08-14 rename from
`CLAUDE.md`):

| date | commit | lines | what it added |
|---|---|---|---|
| 2026-08-05 | (b0cf1245, 08-04) | 738 | the measurement calef acted on |
| 2026-08-13 | 7c65f10a | 748 | the ranking function is a customer running it |
| 2026-08-14 | 150628ac, 3b54f6fc | 768 | the `AGENTS.md` rename, then usernames as the standard |
| 2026-08-15 | 4f215acb, 5d00b8e9, 9d1d49fc, 6f71615f | 787 | milestone 120 (the rename), the two-directional leak check, §71 (a limitation is promoted when it stops being a fact), plural maintainers |
| 2026-08-16 | 1ff2a038, daf5e84a, 23aadabe | 822 | lane count against collision surface, draft-PR-first, the agent-voice line |
| 2026-08-16 | 1da0d68b | 868 | the elegance-and-performance tenet (+46) |
| 2026-08-17 | 77017f71 | 955 | milestone 94 (the untracked-work sweep)'s convention (+87) |

Two commits are 61% of the growth since 08-05. `1da0d68b` (+46) and `77017f71` (+87) added 133
of the 217 lines, and both are whole new tenet sections rather than accretion. The file does not
creep; it steps. That matters for the budget's design: a per-commit size gate would have fired
exactly twice in thirteen days, on the two commits whose authors most believed they were adding
something load-bearing.

One commit shrank it. `daf5e84a` took it 807 to 794 while adding the draft-pull-request rule,
by deleting what that rule replaced. That is the shape the budget is trying to make normal, and it
already happened once unprompted.

## Applied 2026-08-18 by the integrator: four cuts, 1,212 bytes

Landed. `AGENTS.md` went 955 lines / 11,062 words to 942 / 10,880. The three deletions removed
292, 377 and 543 bytes; cut 4 is a reword and saves nothing. Every line number in the proposal
below was still exact when it was applied, except rule 7's block, whose stated range included a
trailing blank line. Applying by text match rather than by index is the lesson, and it is why this
was applied by search rather than by `sed`.

The two rewordings the proposal said an integrator would have to write:

- *"The rule that matters more than the tidiness:"* became a standalone opener, "An unmerged branch
  is either abandoned...", since the paragraph it referred back to is gone.
- *"Three reasons, and the second is the one that matters"* became "Two reasons, and `script/lint`
  check 5 carries the third", which names where the deleted argument went rather than leaving a
  reader to wonder.

Cut 4 kept its prohibition and lost its overreach: *"Never squash across purposes"* stays a rule
because nothing gates it, and the squash-*merge* half now says the platform refuses it.

## Verified: four cuts, ~15 lines and ~1,150 bytes net

Small, and deliberately so. The honest total is about 1.7% of the file, against the audit's
estimate of "on the order of 90 lines, about a tenth". The gap between those two numbers is this
lane's actual result.

### Cut 1: delete lines 508-511, the stale citations paragraph (293 bytes)

> After any renumber, check citations by content, not by running the gate. `script/decisions
> --check` verifies that a cited `§N` resolves to *some* section, never that it resolves to the right
> one, so a well-formed wrong citation is invisible to it. This has already produced two of them.

Mechanism: `script/citations --check`, wired into `script/lint` at line 565 under the `==>
citations` banner. Verified by reading the check's body: it compares a citation's parenthetical
gloss against the record's own title or body, and confirms an attributed block quote still exists
in the file it names. `script/lint`'s comment above it (lines 555-563) names the exact blind spot
this paragraph describes and says it is closing it.

Safe because the prose is not merely duplicated, it is wrong. It instructs an agent to do by
hand what a gate now does, which teaches distrust of a working check. Nothing here is an argument
worth preserving: the reasoning (a well-formed wrong citation is invisible to a resolve-only gate)
is stated more fully in `script/lint`'s own comment and in `notes/citations.md`.

### Cut 2: delete lines 544-548, the branch-deletion rule (377 bytes)

> Delete a lane's branch when you merge it, and never use a branch as a filing cabinet. Forty-seven
> branches accumulated in about two days of lane work ... So it belongs in the merge, not in a
> periodic cleanup.

Mechanism: `delete_branch_on_merge` is `true` on this repository. Verified live:
`gh api repos/nifeos/nife --jq .delete_branch_on_merge` returns `true`. The platform deletes
the branch, so the rule is rung one and cannot be violated by forgetting.

The paragraph that follows must stay, and the integrator must reword its opening. Lines 549-553
carry a different rule with no mechanism at all: *an unmerged branch is either abandoned or it is
holding knowledge that is not on `main`*, with `fix/redoxfs-write-loop` as its worked example.
That is ungated and stays. Its first clause currently reads "The rule that matters more than the
tidiness:", which refers back to the deleted paragraph; it becomes a standalone opener. This is
the only cut in this set that requires the integrator to write a word rather than delete one.

### Cut 3: delete lines 686-692, rule 7's second reason (546 bytes)

> A `#[path]` module inside a `no_std` binary is unreachable by host tests and by Kani. ... `cseam`
> is the case that proves it ... A drift there shows up as a C component scribbling on the wrong page.

Mechanism: `script/lint` check 5 (lines 1174-1212), which counts consumers per `#[path]` include
target and fails at two. Verified by reading it. Its comment carries this reason verbatim in
substance, including the `cseam`/`c_seam.c` worked example and the drift-as-wrong-offset
consequence. It also carries something `AGENTS.md` lacks: that a single-consumer module is fine and
stays fine, with `net_transport` and `socket_test_client` named as today's instances.

Only this reason goes, not the rule and not the other two. The rule statement (676-678) stays,
because a reader needs to know the rule before they trip the gate. Reason 1 (682-684, "it removes a
category that nothing enforces") and reason 3 (693-694, "makes location self-enforcing for free")
stay, because neither appears in the gate's comment and deleting them would delete an argument.
Line 680's "Three reasons, and the second is the one that matters" becomes "Two reasons", which is
the second word the integrator writes.

The recorded caveat on this cut: the gate scans `user/src` only. `fs_server/` also declares
three `[[bin]]` targets, so a `#[path]` module shared between two of them would be invisible.
`fs_server/src` contains no `#[path]` today (verified by grep), so the gap is latent rather than
live, but it is real and it belongs in the gate's `BUGS`-equivalent rather than in nobody's head.

### Cut 4: reword line 878, the squash-merge prohibition (a reword, not a saving)

Mechanism: `allow_squash_merge` is now `false`. Verified live via `gh api`. This corrects the
audit, which recorded it as `true` and proposed turning it off; that landed in the meantime (#334,
the commit this lane branched from). Squash-merging is now unrepresentable, which is rung one.

So *"and never squash-merge a branch"* becomes an explanation rather than a prohibition. "Never
squash across purposes" stays a prohibition, because nothing gates it: a lane can still collapse two
purposes into one commit locally. That would destroy exactly the loader-ahead-of-migration structure
of milestone 96 (one init). The milestone 96
reasoning (879-883) stays either way.

This one saves almost nothing. It is in the cut because a prohibition against something the
platform already refuses reads as a live hazard, and a reader who believes the constitution is
guarding something the settings guard will not check the settings.

## Declined: five rules examined and not cut

This is the more informative number, and four of the five were on the audit's list of eight.

| rule | why not |
|---|---|
| rule 1, arch code stays under `arch/` (643-646) | The gate greps `asm!\(` and `core::arch::` under `kernel/src`. Rule 1 also claims system registers, and `kernel_main` in `kernel/src/main.rs` reads `aarch64_cpu::registers::CurrentEL` outside `arch/` today. Both are invisible to the gate. Cutting the prose would silently unenforce the clause that the check's own comment says the discipline already slipped on. Extend the gate to the register crates first, then cut. |
| `snake_case` for Rust things, hyphens for scripts (776-818) | The audit's claim is false. `script/lint`'s naming-conventions block runs seven checks and none is a case convention: no `-d` suffix, no "daemon", `*_proto` spelling, branch prefixes, milestone-branch-touches-roadmap, `#[path]` consumers, provenance presence. The only case gate is check 6, and it covers one of the six rows of that table (`notes/`, `design/` markdown filenames). Crates, programs, modules, `script/` entry points, repo-root markdown and directory names are ungated. |
| names are calef's call (700-775) | `script/names --check` verifies a name carries a provenance state, and `script/lint`'s own comment is explicit that it checks "presence of a STATE, never the state `ratified`", precisely so the gate is not a wall. 54 of 126 names are unratified. The gate does not enforce the rule; it records whether the rule was applied. |
| benchmarks are first-class; measure, do not argue (555-558) | The icount tripwire catches a performance regression. The rule is about honesty in reporting: state what a number means, name where it is not apples-to-apples, prefer a recorded tie to an overclaimed win. No gate reads a claim's honesty. This is judgment and stays prose. |
| `nifefs` caps names at 32 bytes (820-825) | The constant is real (in `crates/nifefs/src/lib.rs`, `pub const NAME_LEN: usize = 32`) and the compiler enforces the cap. But the paragraph's operative content is advice the compiler cannot give: *do not let it pick a name, do not spend a format change on bytes nothing needs*. A build failure tells you the name is too long; it does not tell you to pick a different name rather than widen the format. Could shrink to two lines; not a deletion. |

## What this lane did not get to

Named as remaining work rather than guessed at, because a proposed deletion that turns out to be
unenforced is the failure this milestone exists to prevent.

- "Every fence names its counterpart" was on the audit's list. There is a gate (`script/lint`
  line 268), but this lane did not find the corresponding *prose rule* in `AGENTS.md` to cut. Rule
  4 (664-666) is "assume weak memory ordering", which is judgment, not the fence-pairing
  bookkeeping the gate checks. Either the audit row names a rule that is not in the file, or it is
  in a section this lane did not read. Unresolved.
- The split (the core-plus-linked-documents piece) is untouched. So is the size gate and the
  violation ledger. Those are the three remaining pieces.
- The four budget rules are unchanged: the watchers, worktree pruning, squashing against the
  base SHA, and `needs-architect` enforcement all still have no mechanism.

## What this changes about the milestone's premise

The file is not carrying much deletable duplicate. Two lanes have now looked, and the verified
cut is 1.7%. The audit's own finding stands and is stronger than it reads: roughly 33 rules against
32 named `script/lint` checks plus six other gate scripts. The overlap between those two sets is
much smaller than the counts suggest, because the gates mostly check things the constitution never
claimed to rule on (unsafe contracts, dead-code suppression, conflict markers, dependency
direction, counted claims).

So the growth problem is real and the duplication problem is mostly not. The budget and the split
are where the value is. A future lane should not spend itself re-auditing for cuttable rules; this
lane and #309 have now covered that ground.
