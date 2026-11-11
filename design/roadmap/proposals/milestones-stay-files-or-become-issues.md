---
status: PROPOSED
raised: 2026-11-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Whether a milestone stays a file or becomes a GitHub issue

Raised by lane proposal/milestones-as-files-or-issues, at the architect's request. Name: provisional.

Ruled 2026-11-11 (UTC): shape A, milestones stay files, with no published index. The ruling also
answers question 7 of milestone 1018.

Reuse: GitHub's own issues, sub-issues and closing keywords, which shapes B and C would take whole.

## Recommendation

Keep each milestone a file in the record (shape A below). Code keeps citing a milestone by its
number. A code repository checks each `milestone N`, `§N` and `TODO(milestone N)` citation by file
existence at the record commit basalt pins: `design/roadmap/NNNN-*.md` for a milestone,
`design/decisions/NNNN-*` for a section. No index file exists anywhere. An index is a place where
merges conflict, and milestone files are already named by number, so the number resolves without
one. A generated index can be added if the existence check proves too slow; that is compatible.
`TODO(milestone N)` keeps its spelling.

The reason is not effort. If the three shapes cost the same, A would still win, for three
measured reasons. Every gate that reads a milestone today reads it at a commit, so anyone can repeat a run, and an
issue has no commit. The status a gate needs is 1.1% of the words in the
roadmap, and that small part is what most of the readers read. And nife's claim already is the
lighter object an issue would be (§90 (the claim is a draft pull request; the status flip is a
gate)).

One thing the hybrid (shape C) would buy that A does not: a code pull request that merges could
close its milestone's work item in the record repository directly. Under A the status flip is a
second pull request in the record repository. A pays that cheaply: the lane opens the record-side
draft at the claim and marks it ready once the code lands.

## Background

A milestone is one file, `design/roadmap/NNNN-<slug>.md`. It opens with frontmatter: a status, the
date someone raised it, and the five dependency fields of §207 (the roadmap is a graph, and the block
says so in fields a script can walk). Then it has prose: why the work exists, what else the author
weighed, the steps, what done means, the follow-on work and the recorded limitations.
`script/roadmap --check` reads all of it on every push and in the merge queue.

The record is leaving this repository for its own, `nifeos/record`, under milestone 1018 (the
design record moves to `nifeos.org`). Once it has left, code no
longer sits beside the files that its comments cite. That reopens a question the tree answered
once, in §90's "Why not GitHub Issues": should the work-tracking part of a milestone live in an
issue tracker instead? It matters now because the citation rules for the new layout are open, and only a fresh
repository can still match issue numbers to milestone numbers.

What constrains it. A milestone number is already the identity of a piece of work, cited about
22,000 times. Gates run in lane worktrees, often before a push, and in the merge queue. A status
change gets the same review as any record change. And on GitHub, issues and pull requests share one
number sequence per repository.

## What the measurements show

Every count reads this branch's merge base, `295a385d5`, through plain `git grep` and `ls`.

### What reads a milestone block, and which fields

| reader | fields and sections it reads |
|---|---|
| `script/roadmap` (lint section "roadmap status") | every frontmatter field: `status`, `raised`, `built`, `branch`, `promoted_from`, `superseded_by`, `refused_by` and the five dependency fields; the `## Index row`, `## Follow-on` and `## Revisit` sections; every path a block names, against the tree; the merge history, for the merged-branch check |
| `script/roadmap --ready-branch`, in `ready-status.yml` | the head branch's own block: its status must not read `IN-PROGRESS` once the pull request is ready |
| `helpers/milestone-branch-block.sh` (`script/lint-fleet` 4b) | whether a `milestone/N-*` branch changed `design/roadmap/N-*.md`; this is §90's status-flip gate |
| `helpers/roadmap_proposals.py` | `status`, `raised`, `promoted_from` of proposals |
| `helpers/coe_actions.py` | `status` and the correction line under a title |
| `helpers/consumer_census.py` | `status`, `milestone_dependencies`, `## Follow-on` |
| `script/metrics` | `status` and `built`, at many past commits |
| `script/catch-up` | `status`, `built` and the body, at a past commit |
| `script/journeys` | `status` and `built` |
| `script/citations` | each block's title, for glosses; `status` at a commit |
| `helpers/prose_ratchet.py` | the body, with the fields masked out |
| lint section "TODO markers cite a milestone" | that a block file exists for each cited number |
| `helpers/launch_pass.py` | nothing: it refuses an outsider prompt that names a block or carries frontmatter |

`script/roadmap` alone reads `machine_requirements` and `specific_machine`, to sort the
1,027 blocks by gate (480 classified, 273 ready to start). Nothing reads a block's steps or effort
as data; those are prose. Seven of the workflows and scripts that read a block read it at a commit
other than the working tree's, through `git show` or `git log`.

### What a block holds

Over the whole roadmap, 1,027 blocks and 1,268,629 words: frontmatter is 1.1%, `## Index row`
6.0%, `## Follow-on` 7.7%, `## BUGS` 6.2%, `## Done when` 0.7%, and everything else 78.1%.

A finer sample read 43 blocks, every 24th by number from milestone 1 (boot to Rust on QEMU `virt`)
to milestone 1050 (`designware_mobile_storage`: the outside review's findings), 51,405 words. A
script classed each section by its heading, and the opening prose by paragraph, into three kinds. Work tracking is the frontmatter, steps, dependencies, cost, done-when,
follow-on and the index row. Design text is background, the problem, options, prior art, rulings
and rationale. An outcome record is what a lane built, what it measured and the recorded
limitations.

| kind | share of words | median per block | middle half of blocks |
|---|---|---|---|
| work tracking | 29.7% | 27% | 22% to 39% |
| design text | 39.8% | 44% | 19% to 61% |
| outcome record | 30.5% | 23% | 0% to 43% |

Classing by heading overstates tracking, since a step carries its reasons in the same sentence.
The tracking that is data, what an issue's fields could hold, is the frontmatter at 1.1%. The
outcome record stays reviewed text under every shape, since readers check claims against it.

### What the claim, the queue and the holds depend on

- §90's claim is a draft pull request, made by `script/claim`; nothing in the claim reads or
  writes a block. Its close is the gate in `helpers/milestone-branch-block.sh` and
  `ready-status.yml`, which read the block on the lane's own branch.
- The merge queue and `merge-drain.sh` read pull requests, labels and check runs. They read no
  block.
- `needs-architect` and its labeler (`helpers/architect-label-rules.py`, §88 (`needs-architect` as
  a required check)) read the diff of `design/decisions/` and `CLAUDE.md`. They read no block.
  `script/architect-queue` already reads open issues as well as pull requests.
- Lane briefs cite blocks by path, and `briefs/gate-a-lane.md` runs `script/roadmap --check`.

### Issues on GitHub

- Rate limits. A personal token gets 5,000 REST requests an hour; an Actions `GITHUB_TOKEN` gets
  1,000 an hour per repository; content creation stops at 80 requests a minute and 500 an hour.
  `ci.yml` ran 336 times in the last day and a half here (87 queue runs). Listing every milestone
  issue is about 11 requests. Learning a past status means one timeline request per issue, about
  1,068 per run, over a repository's hourly `GITHUB_TOKEN` budget.
- Offline. Every reader above works offline today. With issues, `script/lint` in a lane worktree
  needs the network and a token, and the merge queue reads issue state as it is at the moment of
  the run, not at the commit under test. A rerun of the same commit can disagree with the first.
- Audit. GitHub keeps at most 100 edits per issue or comment, and anyone with write access can
  delete a comment revision's content; the page does not say whether the same holds for an issue body. A label change or a body edit passes no lint, no review and
  no `needs-architect` hold: §88's hold is a check on pull requests.
- Cross-repository references. `nifeos/record#N` links from any repository, and a closing keyword
  in a pull request, `Fixes nifeos/record#N`, closes an issue in another repository when the pull
  request merges into the default branch.
- Transfer. An issue moves only within one owner; its comments come along, labels and milestones
  only where the target repository has them, and the old address redirects. The page does not say
  what happens to the number; from memory, not read, it takes the target repository's next number.
- Numbering. Issues and pull requests share one sequence. In `nifeos/nife` the next number is
  3,432, of which 59 are issues. Milestone numbers run to 1,068 with 41 unused. Only a fresh
  repository whose first 1,068 numbers are all issues, created in order before any pull request,
  could make issue `#N` equal milestone `N`. After that, a new milestone would take the next shared
  number, and nothing could restore the pairing.
- Sub-issues: up to 100 under one parent, eight levels deep, across repositories.

### What migration would touch

- The record: 1,027 blocks, 76 block directories holding 272 appendix files (239,086 words), and
  2 proposals. The largest block is 22,269 bytes, under an issue body's limit.
- Code-side citations, outside `design/`, `notes/`, `briefs/`, `site/` and the agent files: 7,785
  `milestone N` and 3,284 `§N` occurrences, in 1,518 files, naming 526 distinct milestones, plus 668
  `design/roadmap/NNNN` paths. The largest holders are `crates/` (1,687 milestone citations),
  `bench/`, `script/`, `kernel/` and `xtask/`.
- `TODO(milestone N)` markers in code: none. The TODO question is a rule with no instances; the
  11,069 citations are the real exposure.
- Status flips across repositories. Of the 566 pull requests merged into main in the last two
  weeks, 149 changed a block and code together, 82 changed a block alone, and 335 changed no block.
  After the record leaves, those 149 are the lanes whose status flip lands in a second repository.

## The three shapes

### A. All files

What changes. Nothing in the block, and nothing new is published. Each code repository's lint
checks `milestone N`, `§N` and `TODO(milestone N)` by file existence at the record commit basalt
pins: a file matching `design/roadmap/NNNN-*.md` for a milestone, one matching
`design/decisions/NNNN-*` for a section. A lane that builds code opens one draft in each repository at the claim, as milestone
1018 already lays out; the record draft carries the status flip and turns ready once the code
merges.

Cost: S to M, a guess. One file-existence check at a pin in the shared lint, and the
reworked flip gate, which is already provisional work under 1018.

What we lose. The cross-repository flip stays a second pull request through a second queue.
Status still lags code by that queue's time.

The code side. `TODO(milestone N)` and the existing citations stay as written; none is rewritten.
They convert as each crate moves, under §315 (a crate's documentation leaves with it).

### B. All issues

What changes. Each block becomes an issue in `nifeos/record`: the body holds the text, labels hold
the status and the person and machine fields, and the issue's dependency links hold
`milestone_dependencies`. Appendices become linked files or comments.

Cost: XL, a guess. About 1,300 issues to create at 500 an hour, the ordered backfill with 41
placeholders, and every reader in the table above rewritten against the API. `script/metrics` and
`script/catch-up` would rebuild past states from timelines. The site build would read the API.

What we lose. Review of design text: an edit passes no lint, no review and no hold. `git blame`
over 1.27 million words, and 5,702 commits of block history; GitHub keeps 100 edits. Offline gates
and gates that read a commit. The prose ratchet and the citation glosses, which run on files.
§90's own argument: a second place where truth lives.

The code side. `TODO(nifeos/record#N)`, with `N` equal to the milestone only for the backfill.
The 7,785 citations stay readable if the backfill aligns, and the 668 paths are rewritten to issue
addresses.

### C. Hybrid

What changes. The file keeps the design text and the outcome record, reviewed as now. An issue
holds the work item: status as a label, the five dependency fields, the steps as sub-issues, the
effort and the done-when checklist. The file names its issue and the issue names its file. A code
pull request closes its step's sub-issue with `Fixes nifeos/record#N`.

Cost: L, a guess. 1,027 issues created and each block split into two halves by a script that
knows which sections go where, which the sample shows headings cannot always say. The status
readers (`script/roadmap`, `script/metrics`, `script/journeys`, `script/catch-up`,
`script/citations`, `consumer_census.py`, `coe_actions.py`, `ready-status.yml`, `lint-fleet` 4b)
move to the API. `script/roadmap`'s graph and gate classification move with them.

What we lose. Status leaves review and the commit: the 1.1% that nine readers depend on is the
part that moves. Gates that read a status at a past commit lose it. A milestone has two numbers
unless the backfill aligns them, which is §76 (what catches a milestone status that is wrong in
both places?)'s failure in a new place. A closing keyword knows two states; the roadmap has eight
(`BUILT`, `PARTIAL`, `REFUSED`, `SUPERSEDED` among them).

What we gain. A code pull request that merges marks its work item done in the same moment, with
no second queue, and the step list becomes clickable.

The code side. `TODO(nifeos/record#N)` against the issue, or `TODO(milestone N)` against the file
file existence as in A. The 11,069 citations stay as written in either case.

## Prior art

- Rust. An RFC is a markdown file merged into `rust-lang/rfcs`, and "Every accepted RFC has an
  associated issue tracking its implementation in the Rust repository." Accepted RFCs "should not be
  substantially changed." Code cites the tracking issue: a feature gate's declaration carries
  `Some($tracking_issue_number)`. Tracking issue rust-lang/rust#63063 holds a steps checklist and
  unresolved questions, and links its RFC, rust-lang/rfcs#2515.
- Kubernetes. A KEP is a file in `kubernetes/enhancements`, "prefixed with their associated
  tracking issue number." Its `kep.yaml` holds `status`, `stage` and per-stage milestones, and the
  tracking issue repeats the release targets and asks the owner to "keep this description up to
  date". Both hold the status, by design. This lane did not read whether a check runs over
  `kep.yaml`.
- Python. PEPs are "maintained as text files in a versioned repository, their revision history is
  the historical record," with the status a header in the file. PEP 1 names no issue for tracking
  implementation.
- Fuchsia. An RFC is a file added by a code review, numbered at acceptance; an approved RFC that
  needs work links an issue in its header. Code TODOs cite the tracker as
  `TODO(https://fxbug.dev/42074368)`. When the tracker changed, the bug numbers changed, and
  commits titled "[fxbug.dev] Migrate bug numbers" rewrote them across the source, one of them in
  384 files.
- seL4. An RFC is a pull request on `seL4/rfcs`; its stage is the pull request's state and labels,
  and an approved one merges as a file. The steering committee "may create an issue for tracking
  implementation."

Every one keeps the design text as a reviewed file. Rust and Kubernetes add an issue for the work,
and Kubernetes then holds the status in both. Where nife differs: its gates read the status and the
dependency graph as data on every lint, and those projects' process documents describe none that
do. nife's claim is already a draft pull request, which fills the claiming role an issue plays for
them. And Fuchsia shows what code that cites a tracker's numbers pays when the tracker moves.

## One milestone under each shape

Milestone 1062 (the device control planes leave the kernel) has seven steps, one pull request
each, and reads `NOT-STARTED`. Step 7's design merged as #3419; steps 4 and 6 are draft pull
requests #3427 and #3426, titled "1062 step 4" and "1062 step 6".

- Under A it is as it is. Each step lane opens a draft in the code repository and one in the record
  repository. The code merges; the record draft then moves the block to `PARTIAL` with
  `nifeos/<repo>@<sha>`. A comment in moved code reads `TODO(milestone 1062): ...`, checked by the
  existence of `design/roadmap/1062-*.md` at the pin.
- Under B, `nifeos/record#1062` holds the whole block. Labels say `not-started` and
  `needs-person`; a link says blocked by `#811`; decision dependencies are prose links. Editing step
  2's design is an issue edit, unreviewed.
- Under C, the file keeps the background, the step 7 design and `## Done when`. The issue holds the
  status label, the dependencies and seven sub-issues; #3427's body says
  `Fixes nifeos/record#<step 4>`, and merging it ticks that step. `PARTIAL` is still a label someone
  sets by hand.

## What I need from you

The record is leaving for its own repository, and its milestone files are what every roadmap gate
reads. The question is whether the work-tracking part of a milestone (its status, dependencies and
steps) should move into GitHub issues while the record moves.

Ruled: A, keep milestones as files, and answer the open code-side question by file existence at
basalt's record pin, with no index, keeping `TODO(milestone N)`.

- A. Files, with no index. It also answers milestone 1018's question on code citations.
- B. All issues.
- C. Files for design text, an issue per milestone for the work item.

If the answer is B or C, the order matters: someone must create the issues in number order in
`nifeos/record` before its first pull request, or issue numbers and milestone numbers never match.
That is the one irreversible step here.

What waits on the answer: milestone 1018's question on what code citations check against, and
the shape of its step creating `nifeos/record`.

## Sources

- https://github.com/rust-lang/rfcs (README)
- https://rustc-dev-guide.rust-lang.org/feature-gates.html
- https://github.com/rust-lang/rust/issues/63063
- https://github.com/kubernetes/enhancements/tree/master/keps
- https://github.com/kubernetes/enhancements/tree/master/keps/sig-architecture/0000-kep-process
- https://github.com/kubernetes/enhancements/issues/3157
- https://peps.python.org/pep-0001/
- https://fuchsia.dev/fuchsia-src/contribute/governance/rfcs/rfc_process
- https://fuchsia.googlesource.com/fuchsia/+/main/docs/development/languages/c-cpp/cpp-style.md
- https://fuchsia.googlesource.com/fuchsia/+/dca592988a82dc6f6fc2327973420f6ce597aee9
- https://github.com/seL4/rfcs
- https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api
- https://docs.github.com/en/communities/moderating-comments-and-conversations/tracking-changes-in-a-comment
- https://docs.github.com/en/issues/tracking-your-work-with-issues/administering-issues/transferring-an-issue-to-another-repository
- https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue
- https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/adding-sub-issues
- https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/autolinked-references-and-urls
