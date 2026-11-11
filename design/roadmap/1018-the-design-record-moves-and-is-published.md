---
status: PARTIAL
promoted_from: the-design-record-moves-and-is-published
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: 201
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1018. The design record moves to `nifeos/record` and is published as a website

The lane `lane/the-design-record-moves-and-is-published` wrote this on 2026-11-01 (UTC), on that
day's rulings. Its measurements are in
[the appendix](1018-the-design-record-moves-and-is-published/measurements.md).

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
`nifeos/<repo>@<sha>`, code does not cite records, and the numbering namespace stayed with nife until the ruling of 2026-11-01 (UTC) below.

On 2026-11-10 (UTC) the architect ruled question 1 below. The record lives in a repository of its
own, `nifeos/record`. The website is a separate repository, `nifeos/nifeos.org`, a consumer of the
record. At build time it pulls the record, a page for each crate from that crate's own repository,
and the planned tutorial on building a RISC-V operating system, a class. None of them is copied
into the website's repository. The same day ruled the name, `nifeos/record`, over `nifeos/project`
and `nifeos/project-management`. Question 8's ruling of 2026-11-11 (UTC) creates each at its step
below.

The architect ruled question 4 below on 2026-11-01 (UTC), and its contents on 2026-11-10 (UTC).
`nifeos/record` holds `design/`, `briefs/`, the lane skills, and `notes/` except the tool data that
stays with code: `notes/project-metrics/`, `notes/mutation-testing/`,
`notes/kernel-dependency-unsafe/sites.tsv` and `notes/scripts.md`. It also holds the record's own
checks and tooling: `script/roadmap`, `script/decisions`, `script/journeys`, the core of
`script/citations`, and the inputs the site build reads from the record. Each code repository
keeps its crate documentation, its `BUGS` sections, the notes about that crate alone, its lint
baselines and workflow data, and its own `AGENTS.md`. When a crate leaves this repository, its
notes move with it under §315 (a crate's documentation leaves with it): the sort that section
orders puts a note about the crate alone into the crate's README, rustdoc or `docs/` folder.

Why now. If the record stays in `nifeos/nife`, it stays in a repository whose stated role is
history. Every other move then lands its record edits in a repository that is winding down.

## Recommendation

Move the record to `nifeos/record` by a path-filtered extraction, so every commit that touched a
record file keeps its message, author and date. The record keeps its numbers, and the numbering
namespace moves with it. `nifeos/record` runs the record's checks on every pull request.
`nifeos/nifeos.org` builds the website: it pulls the record, the crate pages and the tutorial at
build time and publishes a static site with one stable address per record number, search, and a
link check. What describes a single crate or carries a tool's data stays with that code, as the
Background states.

Most of this can start before the move. Steps 1 to 5 below land in this repository today, and each
makes the move smaller.

Reuse: Eleventy builds the site and Pagefind indexes it. A small markdown-it plugin in `site/lib/`
links citations with its own pattern, since `script/citations` is Python and Eleventy runs in Node.
The extraction reuses `git filter-repo`.

## How the record cites, once it leaves

§201 section 4 already covers most of this. Records cite records by number, as today. A record
cites code by repository, as `nifeos/<repo>@<sha>` for a claim about one commit. This proposal adds
one form for a path inside another repository, `nifeos/<repo>:<path>`, resolved at the commit
basalt's manifest pins. basalt already names the current commit of each component, so the record
gains no second list.

Code does not cite records by number, under §201 section 4. That rule needs no change. Code that
names a record file by path, 1,062 times today, writes `nifeos/record:<path>`, such as
`nifeos/record:design/naming.md`. The 7,774 milestone citations in code convert when each crate
moves, under §315 (a crate's documentation leaves with it), and not in this move. The same section sends a note about one crate alone with that crate.

A code repository checks each `milestone N`, `§N` and `TODO(milestone N)` citation by whether
`design/roadmap/NNNN-*.md` or `design/decisions/NNNN-*` exists at the record commit basalt pins.

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

Under this proposal the anchor makes that bullet true as written, since the path existed at the
anchor commit. A new bullet would read `nifeos/jh7110_entropy:src/lib.rs`, resolved at basalt's pin.

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
  `/journeys/5/`. Titles are drafts, so no slug forms part of an address.
- Each `milestone N` and `§N` in prose becomes a link to that address. `script/citations` already
  parses them.
- Search across 3.6 million words, built at generation time with no server.
- A link check: internal links fail the build, and external links run on a schedule outside the
  merge gate, since a remote outage should not block a merge.
- A build in `nifeos/nifeos.org` that pulls the record's main, and a deploy when either moves.

The generator is Eleventy, with Pagefind for search, approved on 2026-11-04 (UTC) as CI-only
dependencies. The terms: a committed lockfile, installation with `npm ci --ignore-scripts`, exact
versions at least two weeks old when pinned, and bumps through the ordinary pull request path. The architect knows Eleventy and would maintain it. Hugo and Zola, measured on the same
record, also worked; Jekyll lost on build speed, and mdBook fits poorly with addresses made of a
number alone.

Hosting, ruled 2026-11-01 (UTC): GitHub Pages, deployed from Actions in `nifeos/nifeos.org`, with nifeos.org as the custom domain. The domain dates from 2026-10-03, sits with its registrar's name servers, expires 2027-10-03 and has no web address set. The domain is ours, so the host
stays reversible: a later move changes DNS only. Page addresses derive from the record's kind and
number alone, and no address names the host. Enabling Pages and setting the custom domain each
wait for the architect's approval, and changing DNS is a person's step (step 8).

The host's known limits: no server-side redirects, which number-based addresses avoid needing; no
built-in pull-request preview; and a soft bandwidth limit of 100 GB a month.

## The checks, in a repository with no code to build

- Unchanged: block frontmatter and status (`script/roadmap --check`), decision files, journeys
  (`script/journeys` reads only the record), citations and their glosses, the prose ratchet,
  markdown links and the notes index, em-dashes.
- Changed: the path-claim checks in `script/roadmap`. Bare paths resolve at the anchor, and
  qualified paths at basalt's pins. Both need a fetch, so the record repository's CI gains a shallow
  fetch of each pinned commit's file list.
- Gone from the record repository: every clippy, build, QEMU and unsafe-listing section, and the
  rule that a `TODO` names a milestone, which binds code to the record.

The record's own checks and tooling live in `nifeos/record`, under the ruling on question 4.
[The appendix](1018-the-design-record-moves-and-is-published/measurements.md#where-each-script-goes)
sorts each script and helper, provisionally. The record repository takes `script/roadmap`, `script/decisions`, `script/journeys`
and the core of `script/citations`, the website takes `script/site` and `site/`, and the fleet and
template tooling (`script/claim`, `script/record-ruling`) follows the tooling ruling of 2026-11-01.
A counted claim in the record whose derivation walks code fetches that code at basalt's pins,
provisionally.

## Lanes across two repositories

The workspace rule applies as written: land in the repository that owns the code first, then
change the dependent side in a separate pull request that cites the landed commit.

- A record-only lane, such as a proposal or a decision, has one draft, in the record repository.
- A lane that builds code opens one draft in each. The code lands first, through its own queue.
  The record's pull request then flips the block's status and cites `nifeos/<repo>@<sha>`, which
  matches §90 (the claim is a draft pull request; the status flip is a gate). The two checks that
  read the block on the lane's own branch, `helpers/milestone-branch-block.sh` (`script/lint-fleet`
  4b) and `.github/workflows/ready-status.yml`, move that flip to the record's pull request,
  provisionally.
- The board is `gh search prs --owner nifeos --draft`, which already spans repositories.
- The record repository needs `script/claim` and the lane skills. The ruling on question 4 puts the
  lane skills in the record repository. Where `script/claim` lives stays with the tooling ruling.

## Order of steps

Each step lands alone. Steps 1 to 5 land in this repository.

1. Built: each `script/lint` section carries `# lint-scope: record`, `code` or `both`, as
   `lint-quick` tags sections. No run selects by it. Of 140 sections, 8 read only the record, 35
   read the record and code, and 97 read no record document.
2. Teach `script/roadmap --check` to resolve `nifeos/<repo>:<path>` at basalt's pin, and fix
   milestone 159's `jh7110_entropy` bullet as the first user.
3. Built: `script/site` builds the site from `design/`, `notes/` and `briefs/`, and
   `.github/workflows/record-site.yml` runs it on each pull request that touches them and uploads
   the site as an artifact, deployed nowhere. Its step summary reports build time, index size and
   broken citations and links, and it never blocks a merge. The builder's home is
   `nifeos/nifeos.org`, where it pulls the record at build time.
4. Sort each record into moves or stays in `packages/homes.toml`, which already gives every path a
   home and counts the notes left undecided. The sort follows the ruling on question 4: whole-project
   material moves, and crate documentation, `BUGS` sections, lint baselines, workflow data and
   `AGENTS.md` stay. For each note it decides between cross-cutting and about one crate alone, and
   it produces the number of the second kind. It also lists every link from a record into a path that
   stays, since the site's link check fails the build on such a link.
5. Measured: the record's checks can run by directory, which sets what `nifeos/record`'s CI runs.
   The hold check of §88 (`needs-architect` as a required check) reads labels and no file, in 4 to
   5 seconds, and applies to every change. Of `script/lint`'s 140 sections a record-only change
   needs the 43 tagged `record` or `both`: 80 of 454 seconds on a measured run. The merge queue
   classifies each entry against the entry ahead, so a record entry queued behind a code entry
   skips the heavy jobs while the code entry runs them.
6. Once steps 2 and 4 have landed, create `nifeos/record`, public. Then extract by path into it,
   and remove the paths from this repository in one pull request. That pull request writes
   `packages/moved.toml` with the anchor, and leaves a pointer in the README.
7. Turn on `nifeos/record`'s CI: the record checks, limited to the record's directories, with the
   record halves of the `both` sections and the tooling that runs them.
8. Once step 7 runs the record's CI in `nifeos/record`, create `nifeos/nifeos.org`. Build there,
   pulling the record, the crate pages and the tutorial, and deploy from Actions to GitHub Pages.
   Three tasks belong to a person, since no lane holds the registrar's credentials. They are setting the DNS records that
   point nifeos.org at Pages, setting auto-renew on the registration or a longer term, and
   confirming the organization's settings allow a custom domain on Pages. A lane prepares the
   workflow and the exact record values and stops there.

## Prior art

- Python's PEPs, read 2026-11-01: a separate `python/peps` repository, published automatically to
  `peps.python.org`, apart from the `python.org` website's own repository, with a render workflow
  in Actions and Read the Docs previews per pull request.
- Rust's RFCs, read 2026-11-01: `rust-lang/rfcs`, where an RFC's number is its pull request's
  number, rendered as a book at `rust-lang.github.io/rfcs`. The Reference lives in
  `rust-lang/reference` and the website in `rust-lang/www.rust-lang.org`, recalled.
- Redox and seL4, recalled: each keeps its RFCs in a repository apart from its website's.
- FreeBSD, Go and Kubernetes, recalled: each keeps its documentation or design records apart from
  the source (FreeBSD's `doc` tree apart from `src`, `golang/proposal`, `kubernetes/enhancements`).

Where nife differs: the website pulls the record, crate pages and tutorial from three kinds of
repository, where each project above renders one source. An integrator mints the number at merge. That is why the namespace can move as
a whole. A pull-request number, as Rust uses, would collide across repositories. And nife's records
feed gates, which the PEPs do not, so the checks move with the text.

The appendix lists [what this does not
verify](1018-the-design-record-moves-and-is-published/measurements.md#what-the-proposal-does-not-verify).

## Open questions for the architect

One decision each, recommendation first. Ask them one at a time.

1. Ruled 2026-11-10 (UTC): the record lives in its own repository, `nifeos/record`, and
   `nifeos/nifeos.org` is the website, which pulls the record, the crate pages and the tutorial at
   build time, as the Background states. The name `nifeos/record` is ruled the same day.
2. Ruled 2026-11-01 (UTC), as recommended: the number stays the identity and its namespace moves
   with the record, under the citation forms above. §201 section 4 says so, and its revisit
   condition stays armed.
3. Ruled 2026-11-01 (UTC): GitHub Pages, deployed from `nifeos/nifeos.org`, as the website
   section states; step 8 names the person-only tasks.
4. Ruled 2026-11-01 (UTC), as recommended, with the contents of `nifeos/record` ruled 2026-11-10
   (UTC): the division the Background states, with a crate's notes moving with the crate under
   §315 (a crate's documentation leaves with it).

5. Provisional, recorded by the maintainer on 2026-11-04 (UTC), and the architect may reverse it:
   the planned tutorial's source lives in a repository of its own, which the site pulls at build
   time as it pulls a crate's page. The tutorial has code to build and test, and a stranger can
   fork it.

6. Provisional, recorded by the maintainer, and the architect may reverse each. The appendix's
   sort of the record's own tooling into `nifeos/record`. The rework that moves the status flip in
   `helpers/milestone-branch-block.sh` and `ready-status.yml` to the record's pull request. Counted
   claims that fetch code at basalt's pins.

7. Ruled 2026-11-11 (UTC): milestones stay files, with no published index, since every new number
   would edit an index and merges would conflict there. Code checks its citations by file
   existence, as the section on citations states. A generated index may follow if that proves too
   slow.

8. Ruled 2026-11-11 (UTC): `nifeos/record` and `nifeos/nifeos.org` are created, each at its step.
   Steps 6 and 8 state when. Setting DNS, auto-renew and the organization's custom-domain setting
   stay a person's steps.


## Follow-on

- **Outstanding.** Steps 2, 4 and 6 to 8 above. Checked 2026-11-09 against `script/roadmap`,
  which resolves no `nifeos/<repo>:<path>`, and `packages/homes.toml`, which sorts no note.
- **Outstanding.** Two comments in `ci.yml` and the header of `empty-diff.yml` call
  `merge_group.base_sha` the base branch's tip; the appendix's runs log the entry ahead. The skip
  rule is safe either way, so only the comments change.

- **Outstanding.** §201 (one roadmap until a citation has to cross), journey 5 and milestone 857
  (crates released on their own) still name `nifeos.org` as the record's repository. Each changes
  to `nifeos/record` in its own pull request, and §201's needs the architect's approval.

## Index row

The design record moves to its own repository and becomes a website with a stable address per number.
