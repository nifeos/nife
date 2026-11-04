# A prose-only change skips the heavy jobs

A pull request or merge group whose every changed file is Markdown that no code reads skips every
job in `ci.yml` except `lint`, and every job in `verify.yml` after its gate. The required checks
still report success, because a job skipped by its own `if:` posts Success. Lint still runs, and
with it the roadmap, citation, decision and prose-ratchet checks.

Added 2026-10-05 on lane/docs-only-ci, which calef approved that day. The classifier is
`helpers/prose_only.py` (name provisional) and its header is the full rule.

## Why

On 2026-10-05 twelve bold-backlog pull requests, each removing `**` from Markdown, took GitHub's
hosted runner pool. Required jobs on #1708 and #1712 were cancelled with "The job was not acquired
by Runner of type hosted even after multiple attempts", and real code waited behind them. Each
heavy job already had a step-level "documentation only" regex, so it ran nothing, but each still
took a runner to learn that: about twenty jobs per pull request across the two workflows.

That regex was also wrong. It called everything under `notes/` and `design/` documentation, and
`notes/pipes.md`, `notes/stack.md` and five other notes are installed in the disk image by xtask's
`DOC_BUNDLES`, where swish-check runs `apropos` over them.

## The rule

A changed file is prose when it ends in `.md`, lies under `notes/`, `design/`, `briefs/` or the
repository root, and no code names it as a path. The change is prose-only when it is non-empty and
every file is prose. Anything else runs everything.

**There is no list of consumers.** A list someone has to extend when they add an `include_str!`
would rot; the classifier reads the tree under test on every run instead, so a reference added in
the same diff is seen. Two shapes count, outside comments:

- a `.md` path used as a path (`include_str!`, a `DOC_BUNDLES` entry, `cat "$root/notes/x.md"`)
  makes that file an input. The same path inside a sentence-shaped string, a panic message for
  instance, is a mention and does not;
- a quoted prose directory or a glob under one (`"notes"`, `"design/roadmap/*.md"`) is a walk. If
  the walker is Rust, the prose-only run tests its package in the `lint` job: today that is
  `crates/documentation`'s corpus test and xtask.

The compiler's dep-info would be exact for `include_str!`, but it exists only after a build, and
the gate decides before any build takes a runner. It also cannot see a runtime read, which is most
of what matters here.

## How the required checks stay satisfied

GitHub's "Troubleshooting required status checks" page, read 2026-10-05: a job skipped by a
conditional reports Success, while a workflow skipped by a path filter leaves its checks Pending
and blocks the merge. So the skip is `needs.gate.outputs.prose != 'true'` on each job, never
`paths-ignore:`. #567 (2026-08-28) is this tree's own record that skipped jobs satisfied the ruleset.

In a merge group the diff is the group's head against `merge_group.base_sha`, which spans every
queued pull request, so one code change anywhere in the group runs everything. A push to `main` is
classified against `HEAD~1` when A′ did not already skip it.

Everything fails toward running: a classifier error leaves `prose` empty, and `!= 'true'` admits
that.

## EXAMPLES

The gate's log names the rule for each file. From #1700's head (37 notes), which ran in full:

```console
$ python3 helpers/prose_only.py classify "$(git merge-base HEAD origin/main)" | grep -v '^SKIP'
RUN   notes/stack.md: named by code (script/stack-depth-check:540, xtask/src/manual.rs:34)
RUN   notes/x86-port.md: named by code (system_tests/src/user/compositor_tests.rs:923, ...)
prose-only: no (37 changed files)
```

`DOC_BUNDLES` in `xtask/src/manual.rs` lists it: that note ships in the image, so the full run was right.

```console
$ python3 helpers/prose_only.py --selftest | tail -1
prose_only selftest: 18 cases
```

## BUGS

- None of the eleven bold-backlog pull requests open that day would have skipped. Ten also
  changed `script/citations`, which is code, and #1700 touched `notes/stack.md`, which ships in the
  image. The skip helps a change that is only prose nobody reads, and that sweep was not one.
- A directory walked by a script is logged, not acted on. Lint walks `design/` on every pull
  request anyway. A script run only by a skipped job that walks a prose directory would slip
  through; none did on 2026-10-05, and nothing checks that later.
- A path the text cannot see is invisible: one built from an environment variable, one inside a
  sentence-shaped string that a shell then splits, or the repository root named as `""` (xtask's
  `apropos` does that, and no gate runs it).
- It is conservative about mentions it cannot place. The middle line of a long Rust string
  counts as a reference, so a note named only in a multi-line panic message still runs everything.
- The checkout in each heavy job keeps `fetch-depth: 0`, which only the removed scope steps were
  known to need. Dropping it is a cheap follow-up that nobody has measured.
