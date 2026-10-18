# Documents, numbers, branches and the naming gates

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds where a
document goes, why `§N` is not milestone N, why a numbering gap passes, the branch convention, and
what `script/lint` checks. It exists to verify or challenge the main page, and a reader who only
needs to name, ratify or rename something should not have to open it. The directory `design/naming/`
and this file's stem are provisional names;
naming is an architect's.*

## Where a document goes

Four places, and the distinction is what the document is *for*, not what it is about.

| | holds | shape |
|---|---|---|
| `design/` | the option space, before a decision | "here are four answers and three are bad" |
| `design/decisions/` | the decision, and the argument that settled it | numbered `§N`, append-only |
| `design/tenets/` | why a rule in `AGENTS.md` holds | one file per theme, linked from the rule |
| `notes/` | what exists, and what building it taught us | a running glossary, indexed in `notes/README.md` |

`design/tenets/` was ratified by calef on 2026-09-24 (UTC), directory and filenames, when
`AGENTS.md` was split into rules plus appendices. The reason that survived scrutiny is that a tenet
gets cited on its own rather than only as `AGENTS.md`'s footnote. That makes it an argument about
how to work, and puts it beside the option space it reasons over.

`notes/` was refused: a note records what exists and what building it taught us, and a tenet records
neither. The first argument offered for `design/`, that thirteen rows would crowd `notes/README.md`,
was refused with it: `script/lint` reads `notes/*.md` without recursing, so a `notes/tenets/`
subdirectory would have cost that index nothing. The directory's own README carries the block, per
§75 (carry provenance in their own README).

`design/roadmap/` is the exception that proves the split. It lives in `design/` because a milestone
block is an argument for doing something, not a record of having done it. That stays true after the
milestone ships and the block gains a "Built" line.

A note is not optional. Every concept and every finding gets one, indexed in
[notes/README.md](../../notes/README.md). For a demonstration OS the documentation is part of the
deliverable rather than a courtesy to the author.

## `§N` is not milestone N, and they collide

DECISIONS section numbers and roadmap milestone numbers are separate schemes over the same small
integers, and both run into the hundreds, so almost every decision number is also a milestone
number:

| N | DECISIONS `§N` | roadmap milestone N |
|---|---|---|
| 24 | the two-tier Ctrl-C | a Virtualization.framework board |
| 28 | SMP placement | the line discipline |
| 31 | the foreign-language C seam | the capability shell |
| 39 | this naming rule | components, services, and the directory layout |

This has already produced a wrong citation in the tree. The block of milestone 50 (pipes and
redirection) cited "§31's FileSpec", which points at §31 (the foreign-language seam), the C seam,
and has nothing to do with file grants. The thing it meant was milestone 31 (a capability shell)
phase 2, granting against the §27 (the filesystem service) filesystem contract. Fixed in c0643bc.

The rules for writing either number are on the [main page](../naming.md).

`script/decisions --check` verifies that every cited `§N` resolves to *some* section. It cannot
verify that it resolves to the *right* one: a well-formed wrong citation is indistinguishable from a
correct one from the outside. `script/citations`, from milestone 444 (a citation says what it
cites), now asks a citation to carry a name, which closes that gap where it is used.

### Both schemes are sparse: a duplicate is the defect, a gap is not (milestone 443)

The rule came from milestone 443 (lanes wait on each other).

Two files claiming one number is fatal in both schemes. Every citation to that number is then
ambiguous, and no gate can tell which one a sentence meant. `script/decisions --check` and
`script/roadmap --check` each fail on it twice over, once for two index rows and once for two files.

A hole in the numbers is reported and passes. An unused section number misleads nobody: it appears
in no citation, every citation that does exist still resolves, and the sequence was never a promise
that it is dense. The roadmap works the same way.

The gate was buying a cosmetic property with the tree's most dangerous edit. A gap is closed by
renumbering, and
[§194 (sessions interleave rather than serialize)](../decisions/0194-sessions-interleave-rather-than-serialize.md)
records what renumbering costs. A citation rewritten by number can be silently wrong and still pass
every gate, because the section it now names exists. On 2026-09-19 one branch was renumbered four
times in two and a half hours, from §156-§189 to §160-§194. Another session was minting from the
same range, and a contiguous scheme made every collision displace the whole run. The range began at
§156 (what the package manager waits on); §160 (what a subshell copies) and §189 (which of two
definitions `caretaker` carries) sit inside it.

So when two sessions collide on a number, the later lander takes the next free ones and leaves the
hole. Only the colliding sections move, not the run behind them: four files rather than thirty-four.
The rule above it is unchanged and is `AGENTS.md`'s. A number is provisional until the merge queue
lands it, and anything global to the tree is the integrator's at merge.

What no longer has a gate: a decision file deleted outright, with its index row deleted in the same
commit and no citation to it anywhere in the tree. That now leaves nothing behind to notice. Every
partial shape still fails: a file with no row, a row with no file, a row pointing at a missing file,
and a `§N` in the tree that resolves to nothing.

## Branches

A prefix says what kind of work a branch is, one spelling per idea (`feature/`, not `feat/`):

`milestone/` (a roadmap milestone), `maintainer/` (the maintainer's own work), `roadmap/`, `fix/`
(a bug with a name), `bench/` (measurement work), `audit/` (reading rather than writing),
`integration/` (joining lanes), `finalize/` (landing them). Plus `main`, and the tooling's own
`worktree-agent-*`, which no person types. The prefix is unbounded by design.

This is a convention and not a gate (2026-08-18). Not an enforced allowlist, because nothing
consumes the taxonomy except the check itself. A grep across `script/`, `helpers/`, `.github/workflows/` and `xtask/` for any other
reader of a branch prefix returns only false positives. Only `milestone/N-` is read by anything, and
`script/lint`'s milestone-branch-touches-its-block check is what reads it.

Every observed failure of the allowlist was a false rejection of legitimate work, four times:

- `roadmap/`, the repository's *second* commonest prefix, refused while about thirty-five merges
  using it were already on `main`.
- `gh-readonly-queue/*`, which failed every group build's clippy job, so the queue evicted and
  rebuilt forever.
- `dependabot/*`.
- `claude/*`: a harness names its own branches and a lane cannot rename them from inside.

Each was fixed by widening the list after something broke. That is the signature §61 (a lint is
adopted on evidence) used to drop three lints and milestone 78 (the load-sensitive assertions) used
on three assertions: a check that only ever rejects valid work is measuring the wrong thing.

What survives as a gate is the one prefix that carries a mechanism. It survives as a branch name
rather than a label because the check that reads it runs locally and offline, from `symbolic-ref`,
with no network and no GitHub context. A label cannot be read without an authenticated round trip.
It would not exist yet anyway, because a lane runs `script/lint` before it runs `gh pr create`. A
branch name is also fixed when the lane is cut, where a label is editable at any time. For a check
whose job is "you claimed 126, so move 126's block", the claim has to be the immutable half.

## File names in an appendix directory

Settled in the tier 1 review of 2026-10-18 (UTC), each a precedent and not a new gate.

- An appendix is named for the heading it continues (rule 1 in
  [convention-rulings.md](convention-rulings.md)), even when the heading is a sentence, as in
  `lift-is-only-worth-what-its-caller-does` and `functions-that-answer-yes-or-no`. When the slug is
  wrong, change the heading. There is no exception.
- A dated record is named by its subject and date (calef, 2026-10-18 UTC: "Yes"). For a bench run the
  subject is the machine: `radon-2026-09-03`, `qemu-2026-09`. Otherwise it is what was examined:
  `census-`, `inflow-`, `baseline-`, `guard-page-faults-`, `kernel-stack-overflow-2026-08-15`. Use the
  month when the record spans days. A word such as "measurements" claims a role, so leave it out.
- The machine stays in the stem when the directory does not name it: `network-boot-xenon` under
  `x86-uefi-boot/`, while `visionfive2/network-boot` needs none.
- A file name says what the file is in the sense its directory gives it. `survey-design` fits under
  `process-view/`; `survey-method` reads as a methodology.
- The "stem is provisional" note comes out of a document when its stem is ratified.
- An appendix of a `## BUGS` section is `bugs.md`. American spelling applies to stems (`-colors`).
- When one section has several appendices, each file is named for its own title and not the shared
  heading (calef, 2026-10-18 UTC: "Yes").
- An appendix that continues no single section (reference tables, logs, datasets, indexes) is named
  for its own title, cut down to the noun that says what it is (calef, 2026-10-18 UTC: "Yes").

## Script names

calef, 2026-10-18 (UTC): "Yes". A command that does one action takes a verb: `claim`, `bench`, `fmt`,
`record-ruling`, `vendor-verify`, `repeat-under-load`, `toolchain-bump`. A standing tool or role
takes a noun: `board-boot-watcher`, `merge-queue-rebaser`, `project-metrics`,
`outsider-pass-launcher`. A gate ends in `-check`.

## What is checked, and what cannot be

`script/lint`'s `naming conventions` block enforces seven things, and the first five are cheap greps
because lint runs constantly. Take the count from the script.

1. No name ending in `-d`, over `components/src/*.rs` and `fixtures/src/*.rs`, both `Cargo.toml`s'
   `[[bin]]` names, and `crates/*`. Four characters or more, so a three-letter name ending in `d` is
   read as an abbreviation rather than a daemon. Words that genuinely end in `d` go in `naming_allow` with a reason, the same
   shape as a per-item `#[allow]`. `uuid` (RFC 9562's own term) is the one there today. An exemption
   that exists only because a name is an unreadable acronym is one the expansion deletes, as
   §154 (the acronym test)'s rename of `asid` did.
2. The word "daemon" appears nowhere, outside `design/decisions/` and `design/`. Those are where the
   argument about the word lives, and therefore they have to be able to name it.
3. Contract crates spelled `*_protocol`.
4. The current branch is not a near-miss of a milestone claim, such as `milestone-126-pgrep`, which
   would skip check 4b in silence. There is no prefix allowlist (see Branches). The shape rule
   lives in `helpers/branch-name-check.sh`, shared by `script/lint-fleet` and `script/claim`.
5. No `#[path]` module is shared by two or more binaries (CLAUDE.md rule 7). This is the newest and
   the one with teeth: it counts consumers per include target and fires at two. A module with a
   single consumer is an ordinary submodule and is fine. The rule is about *agreement between
   binaries*, not file layout. The allow-list is empty, which is the intended steady state.

   `virtio` was its one entry for about an hour. It could not be a crate while it reached back into
   whichever binary included it for `check`, and the resolution was to delete `check` rather than
   pass it in. Rust already has the per-binary "how this program dies" hook, `#[panic_handler]`.
   Both binaries already had one executing the same instruction by two different routes. An entry
   here needs a reason of that calibre.
6. `notes/` and `design/` filenames are lowercase and hyphenated, because they are URL slugs the
   moment any of this is published. `README.md` is the only exception, and it is GitHub behaviour
   rather than style.
7. Every crate, program and `script/` entry point carries a `Name:` block (milestone 115 (the names
   that were ratified), and the provenance appendix). Presence only: it cannot check that the reason
   is still true. It checks that the block names one of the four states (`ratified`, `recorded`, `provisional`,
   `unrecorded`, as `helpers/name_provenance.py`'s `STATUSES` lists them), that `ratified` carries a
   date and that `recorded` carries a citation. It never checks that the state is `ratified`, so a
   name waiting on an architect does not fail anybody's build. `script/names --unratified` is how
   that queue gets worked.

   And exactly one such block per file, in the spelling the parse reads (milestone 283 (one
   provenance block per file)). That was a convention 205 files happened to follow until two did
   not. The two ways of breaking it compound into silence. The parse stops at the first `Name:`
   line, so a stale proposal block above a ratified one is what gets read. The parse matches
   `^<prefix> ?Name:`, so a header wearing markdown (`//! **Name: ratified ...**`) is invisible even
   when it is the only one. Both at once is how two of calef's ratifications sat on the worklist for
   a week. The gate reported `provisional`, which is a legitimate answer nothing disputes.

   So the check asks the question a person asks by looking at the file: does anything here read as
   a provenance header without being the one that was read? A comment line counts if its content,
   after the marker and any leading markdown, begins `Name:`. That holds at `///` and `//` as well
   as the surface's own prefix. The failure names the file, the line, and which of four things is
   wrong with it: markup, indentation, a different comment marker, or a second block nothing reads
   past.

   Two things are deliberately not headers. `` `Name:` `` in backticks is a *mention* of the
   convention, which the scripts implementing it write constantly. A line carrying an angle-bracket
   placeholder (`Name: ratified <YYYY-MM-DD>`) is showing the *form*, which is how `script/names`'
   own header documents the three spellings.

   The parse was not widened to admit bold, deliberately. 205 files use the plain form, so admitting
   a second spelling would make both legal, which is the opposite of the fix.

Everything else here is prose because it needs judgement and no checker can supply it. In
particular a checker cannot catch the jargon half of §39: `linedisc` would have passed every
rule above. It ends in `c`, contains no daemon, is not a proto crate, and had a perfectly good
branch. What caught it was a person reading the name and not knowing what it meant, and that
remains the test.

Two limits. The checks read the filesystem for names and use `git grep` for the word, so an
untracked file with "daemon" in it is invisible until it is added. (The conflict-marker check has
the same blind spot.) And check 1 sees the *names* of things rather than the things, so a component
whose name is fine and whose behaviour is a daemon is not its problem.

## Why this file is the rule

§155 (the naming conventions move out of the constitution, and the note becomes the rule) made this
the authority on 2026-09-18, in `design/` because a file that is normative is not a note. It covers
what §39 (a component is named for what it is) does not: crates, scripts, where a document goes, and
the two numbering schemes. `AGENTS.md` keeps only the authority itself, the part a lane must act on
without opening anything. Where the two disagree, this file is the rule for conventions and
`AGENTS.md` is the bug. Each rule is stated once, here, so a changed rule is edited in one place and
nothing has to compare two copies.

What makes that safe is the provisional name. A lane that has never read this file invents a name
that is expected to change. So the conventions are needed at ratification and at rename, which are
the unhurried moments, and not at invention.
