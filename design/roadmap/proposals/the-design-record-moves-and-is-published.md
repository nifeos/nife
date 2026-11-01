---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: 201
machine_requirements: none
specific_machine: none
needs_person: yes
---
# The design record moves to `nifeos.org` and is published as a website

The lane `lane/the-design-record-moves-and-is-published` wrote this on 2026-11-01 (UTC), on that
day's rulings. Its measurements are in
[the appendix](the-design-record-moves-and-is-published/measurements.md).

## Background

The design record is the part of this repository that says why: the roadmap blocks, the decision
sections, the user journeys, the naming conventions, the tenets and most of `notes/`. Scripts read
it as ground truth. `script/roadmap` reads block status, `script/journeys` reads journeys against
blocks, and `script/citations` checks that every `milestone N` and `§N` resolves.

On 2026-11-01 (UTC) the architect ruled the end state. The nife repository breaks up into
single-purpose repositories and `nifeos/nife` keeps its name and remains for its history only. The
kernel moves to `nifeos/nife-kernel`, a provisional name. The same day set a direction for the
record: it will likely move to a new repository of its own, and the intent is to publish it as a
website.

Two older records already hold the question. Milestone 565 (a website for the project plan) and
milestone 684 (a documentation site, and documentation that ships in packages) each wait on a
direction. §201 (one roadmap until a citation has to cross, and the blocked side declares the
dependency) rules on citations across repositories. Its section 4 says a record points at code as
`nifeos/<repo>@<sha>`, code does not cite records, and the numbering namespace stays with nife.

On 2026-11-01 (UTC) the architect ruled question 1 below. The record moves to one repository named
`nifeos.org`, and the provisional name `nife-design` is withdrawn. That repository holds the whole
site published at nifeos.org: the record (roadmap, decisions, journeys, naming, notes), a page for
each crate, material about nife itself, and the planned tutorial on building a RISC-V operating
system, a class. The record and the site are one repository, not two. A crate's page pulls from that
crate's own repository at build time and is not copied in. If the record ever has to split from the
site, the path-filtered extraction measured in the appendix allows it, with history. The ruling
creates nothing: the repository is created when the move happens, and its creation needs the
architect's approval under the existing ruling. The ruling carries one condition, step 5 below.

Why now. If the record stays in `nifeos/nife`, it stays in a repository whose stated role is
history. Every other move then lands its record edits in a repository that is winding down.

## Recommendation

Move the record to the `nifeos.org` repository by a path-filtered extraction, so every commit that touched a
record file keeps its message, author and date. The record keeps its numbers, and the numbering
namespace moves with it. The same repository holds the rest of the site, so one build publishes
everything. Build the website from it in CI on every merge: a static
site with one stable address per record number, search, and a link check. Records that describe a
single crate or carry a tool's data stay with that code.

Most of this can start before the move. Steps 1 to 5 below land in this repository today, and each
makes the move smaller.

Reuse: the site generator is the only new code, and it reuses an existing one (mdBook, Zola,
Sphinx or Hugo, compared below) plus this tree's own parsers in `script/roadmap` and
`script/citations`. The extraction reuses `git filter-repo`.

## The measurements, in brief

Taken 2026-11-01 at `origin/main` `295782808`. The appendix has the commands and their limits.

- 138 tracked files under `script/`, `helpers/`, `xtask/`, `tools/`, `jig/` and `.github/` name a
  record path outside a comment. Of `script/lint`'s 128 sections, 56 name the record directly, 34
  more call a helper that reads it, and 38 do neither.
- Outside the record, 1,972 of 3,425 tracked files cite it: 7,774 `milestone N` and 3,525 `§N`
  occurrences, and 1,062 record paths. In Rust alone, 4,813 and 2,641.
- In the other direction, 812 of 952 top-level roadmap blocks name a code path, 7,845 times across
  963 roadmap files. Decision files name one 1,119 times.
- Of 11,661 non-merge commits, 4,134 touch only the record, 2,729 touch both, and 3,299 touch only
  code.
- The record holds 2,866 pull request references written as a bare `#N` and 36 links to
  `github.com/nifeos/nife/blob/`.

## How the record cites, once it leaves

§201 section 4 already covers most of this. Records cite records by number, as today. A record
cites code by repository, as `nifeos/<repo>@<sha>` for a claim about one commit. This proposal adds
one form for a path inside another repository, `nifeos/<repo>:<path>`, resolved at the commit
basalt's manifest pins. basalt already names the current commit of each component, so the record
gains no second list.

Code does not cite records, under §201 section 4. That rule needs no change. The 7,774 milestone
citations in code convert when each crate moves, under §315 (a crate's documentation leaves with
it), and not in this move.

The roadmap blocks already written are the hard part. A finished block's bullets name the files and
checks that did the work, as bare paths such as `crates/x/src/y.rs`. Rewriting 7,845 of them is the
sweep that `notes/citations.md` priced and refused for citations. So the recommendation is an
anchor. The move records the last `nifeos/nife` commit before it, and every bare path in the
record means that path at that commit. Text written after the move uses the qualified form. The
anchor is one line in `packages/moved.toml`, the registry §201 section 4 names.

A bare `#N` needs the same treatment for a different reason. On GitHub and on most generators it
links to the repository that renders it. The site build rewrites `#N` to `nifeos/nife#N`; the
source text stays as written.

## Where the record's code paths already go stale

On 2026-10-31 (UTC) the lane for milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic,
released on its own) moved `jh7110_entropy` out of this tree, in commit `aabf387eb`. Milestone 159 (a real hardware entropy source: the JH7110's TRNG) is BUILT, and its
block still names `crates/jh7110_entropy` as the crate that holds the register layout. That
directory no longer exists. `script/roadmap --check` passed on 2026-11-01, because it reads paths
only in follow-on bullets.

Under this proposal the anchor makes that bullet true as written: the path existed at the anchor
commit. A new bullet would read `nifeos/jh7110_entropy:src/lib.rs`, and the check resolves it at
basalt's pin. On the website, the bare path links to the anchor commit and the qualified one to the
pin. Any page that says `milestone 159` links to that block's address.

## History: blame and the numbering survive

A path-filtered extraction rewrites history to keep only the chosen paths, in the manner of
`git filter-repo --path`. The 4,134 record-only commits arrive whole. The 2,729 mixed commits
arrive with the record half only, under the same message, author and date. `git blame` on a
decision then lands on the commit that wrote the line, with its reasoning.

Commit identities change in the new repository. A record citing a nife commit by hash (1,028 short
hashes in Markdown, measured loosely) still resolves, because `nifeos/nife` keeps its history. The
site links each one to `nifeos/nife`.

The numbering survives because it lives in file names and H1s, which the extraction carries
unchanged. `script/roadmap` and `script/decisions` mint from the directory listing, so they keep
working.

## What the website needs

- Static generation from the Markdown, with no hand-written index: the generator reads frontmatter
  and H1s, as `script/roadmap` does. The index this tree retired on 2026-09-21 stays retired as a
  file.
- One stable address per record number, such as `/roadmap/533/`, `/decisions/201/` and
  `/journeys/5/`. Titles are drafts, so the slug never forms part of the address.
- Each `milestone N` and `§N` in prose becomes a link to that address. `script/citations` already
  parses them.
- Search across 3.6 million words, built at generation time with no server.
- A link check: internal links fail the build, and external links run on a schedule outside the
  merge gate, since a remote outage should not block a merge.
- A CI build on every pull request and a deploy on every merge to main.

Generators considered, all recalled and not read: mdBook (Rust, client-side search, a
preprocessor hook), Zola (one binary, built-in search), Sphinx with MyST, and Hugo. A small
generator of our own over the existing parsers is the fifth option. The recommendation is mdBook
with a preprocessor that generates the summary and links citations, measured in step 3 before
anyone commits to it. Any of them is a new host-tool dependency, which needs the architect's
approval.

Hosting options, with no account created: GitHub Pages deployed from Actions in the record
repository; Cloudflare Pages; Netlify; Read the Docs, which the PEPs use for previews; Codeberg
Pages. The address choice is separate: the organization's `github.io` address or a domain, and
milestone 565 says this project has no domain registered.

## The checks, in a repository with no code to build

- Unchanged: block frontmatter and status (`script/roadmap --check`), decision files, journeys
  (`script/journeys` reads only the record), citations and their glosses, the prose ratchet,
  markdown links and the notes index, em-dashes.
- Changed: the path-claim checks in `script/roadmap`. Bare paths resolve at the anchor, and
  qualified paths at basalt's pins. Both need a fetch, so the record repository's CI gains a shallow
  fetch of each pinned commit's file list.
- Gone from the record repository: every clippy, build, QEMU and unsafe-listing section, and the
  rule that a `TODO` names a milestone, which binds code to the record.

The scripts move with the record. The tooling ruling of 2026-11-01 sends tooling to its own
repositories too, so this proposal does not pick their final home.

## Lanes across two repositories

The workspace rule applies as written: land in the repository that owns the code first, then
change the dependent side in a separate pull request that cites the landed commit.

- A record-only lane, such as a proposal or a decision, has one draft, in the record repository.
- A lane that builds code opens one draft in each. The code lands first, through its own queue.
  The record's pull request then flips the block's status and cites `nifeos/<repo>@<sha>`, which
  matches §90 (the claim is a draft pull request; the status flip is a gate).
- The board is `gh search prs --owner nifeos --draft`, which already spans repositories.
- The record repository needs `script/claim` and the lane skills. Where those live is part of
  question 4.

## Order of steps

Each step lands alone. Steps 1 to 5 can start now, with no move and no ruling beyond step 3's
dependency approval.

1. Tag each `script/lint` section as record or code, the way `lint-quick` tags sections. No
   behavior changes; the record repository's lint then becomes a selection.
2. Teach `script/roadmap --check` to resolve `nifeos/<repo>:<path>` at basalt's pin, and fix
   milestone 159's `jh7110_entropy` bullet as the first user.
3. Build the site in this repository's CI as an artifact, deployed nowhere. Measure build time,
   search index size and broken citations.
4. Sort each record into moves or stays in `packages/homes.toml`, which already gives every path a
   home and counts the notes left undecided.
5. Measure whether the record's checks can be limited to the record's own directories, so a change
   to the site's other paths does not run them. The ruling of 2026-11-01 (UTC) makes this a
   condition of the one-repository shape, and it is not yet verified. Three checks are
   repository-wide today. The `needs-architect` required check of §88 runs on every pull request and
   merge group (`.github/workflows/architect-hold.yml`) with no path condition. `script/lint` runs
   every section, and `ci.yml` never skips its `lint` job. The merge queue tests a group against the
   union of its queued changes, so one record change in a group runs every check for the others.
   The constraint found so far: `ci.yml` records that a workflow skipped by a path filter leaves its
   required checks Pending and blocks the merge, so the limit has to be a job-level condition that
   reports Success when skipped, as `ci.yml` does for code. Whether that holds for `architect-hold`,
   whether the `script/lint` sections tagged in step 1 can be selected by directory, and what a
   mixed queue group runs are the three things to measure. If the limit proves impossible, the
   ruling's condition fails and the shape returns to the architect.
6. After step 5's measurement and the architect's approval of creation: extract by path, then remove the
   paths from this repository in one pull request. That pull request writes `packages/moved.toml`
   with the anchor, and leaves a pointer in the README.
7. Turn on the new repository's CI: the record checks, limited to the record's directories, the site
   build and the link check.
8. After the hosting ruling: deploy.

## Prior art

- Python's PEPs, read 2026-11-01: a separate `python/peps` repository, published automatically to
  `peps.python.org`, with a render workflow in Actions and Read the Docs previews per pull request.
- Rust's RFCs, read 2026-11-01: `rust-lang/rfcs`, where an RFC's number is its pull request's
  number, rendered as a book at `rust-lang.github.io/rfcs`.
- FreeBSD, recalled: the documentation tree lives in its own repository apart from `src`, and Hugo
  builds the website and Handbook from it.
- Go, recalled: design documents in `golang/proposal`, citing code by change number, rendered only
  by GitHub.
- Kubernetes, recalled: KEPs in `kubernetes/enhancements`, numbered by issue.

Where nife differs: an integrator mints the number at merge. That is why the namespace can move as
a whole. A pull-request number, as Rust uses, would collide across repositories. And nife's records
feed gates, which the PEPs do not, so the checks move with the text.

## What this does not verify

- The lint section counts come from a text search of each section's body, and the reader counts
  from string matches outside comment lines. Both are approximations.
- Whether mdBook's search scales to 3.6 million words, and how long a build takes. Step 3 measures
  both.
- That GitHub Pages redirects a project site from `github.io` to a custom domain once configured.
  Recalled.
- That `git filter-repo` keeps the renames of files moved between `notes/` and `design/`. Not tried.
- How many notes concern a single crate. Not measured; step 4 produces the number.
- Where `script/claim` and the skills live once tooling leaves.

## Open questions for the architect

One decision each, recommendation first. Ask them one at a time.

1. Ruled 2026-11-01 (UTC): the repository is `nifeos.org`, one repository for the record and the
   whole site, as the Background states. Its condition is step 5's measurement.
2. The citation identity once the record leaves code. §201 keeps the namespace with nife, and nife
   now means a history-only repository. Recommended: (a) the number stays the identity and the
   namespace moves with the record repository. Bare code paths mean the anchor commit, and new ones
   use `nifeos/<repo>:<path>` at basalt's pin. Options: (b) a repository-qualified number; (c) the
   slug, option C of milestone 533 (whether the roadmap should become issues, and what a citation
   means after the repository split); (d) the website address. This amends §201 section 4.
3. Hosting and address. Publication cannot be undone once strangers quote a page, so the address
   matters more than the host. Recommended: (a) GitHub Pages at the organization's `github.io`
   address, with number-based paths so a later domain changes only the host. Options: (b) a domain
   from day one, which needs a registration; (c) Cloudflare Pages, Netlify or Read the Docs.
4. Which records stay with code. Recommended: (a) the record takes roadmap, decisions, journeys,
   naming, tenets, cross-cutting notes, `briefs/` and the lane skills. Each code repository keeps
   its crate documentation, its `BUGS` sections, notes about it alone, its lint baselines and
   workflow data, and its own `AGENTS.md`. Options: (b) everything under `design/` and `notes/`
   moves and the sort happens later; (c) only roadmap, decisions and journeys move.

5. Where the tutorial's source lives. The planned tutorial on building a RISC-V operating system
   publishes on the site. Its source could sit in the `nifeos.org` repository or in a repository of
   its own. Not asked yet; it needs background and a recommendation first.

Blocked until questions 2 to 4 are answered: steps 6 to 8. Nothing in flight waits on this.
