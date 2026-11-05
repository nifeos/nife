# Moving a crate out of nife: the runbook

*Name: provisional, minted 2026-11-05 (UTC) by milestone/1034-a-crate-move-runbook. The file name
says what the page holds: the steps of one move.*

This page takes one crate from `crates/<crate>` to a repository of its own, `nifeos/<crate>`, and
back into nife as a pinned dependency, then through its soak to publication. Every step names its
command, the result that says it worked, and who takes it. A lane is an agent working a
milestone; calef marks a step only the architect takes, because it creates something, holds a
credential, or publishes. [Journey 5](../design/journeys/05-crates-released-on-their-own.md) says
why crates leave and in what order, and
[milestone 857 (crates released on their own)](../design/roadmap/0857-crates-released-on-their-own.md)
holds the rulings. The pilot, `jh7110_entropy` of
[milestone 818 (proven JH7110 TRNG logic, released on its own)](../design/roadmap/0818-the-jh7110-entropy-crate-released-on-its-own.md),
took these steps by hand; `script/crate-move` now does the mechanical part.

The rules every step keeps:

- Bottom-up. A crate leaves only when nothing it depends on is still in this tree. A program
  package leaves as soon as its own crates have left.
- No record of names travels. The move commit deletes every `Name:` block. Git history in this
  tree, with the ratifying pull requests, is the only provenance record; the crate repository
  carries no table and no provenance file.
- The crate never mentions nife, in its source, README, docs, manifest or commit messages.
- The naming gates run in the crate repository, through the shared template
  `nifeos/crate-workflows` pinned at a release commit.
- The owned runners never run fork code. A crate repository runs on GitHub-hosted runners only,
  and a fork's pull request waits for a maintainer's approval before any workflow runs.
- A pin cites a landed commit. nife depends on `nifeos/<crate>` by `rev`, at the commit of a
  tag whose manifest carries the version, with that version stated beside it.

## The dry run

`script/crate-move --dry-run --into <empty dir> <crate>` builds the crate repository's first tree
locally and creates nothing. It copies the crate's tracked files and adds the template's scaffold,
with every caller pinned at the newest template release. It deletes the `Name:` blocks, rewrites
each falsification patch for `git apply -p1` and sets `repository` in the manifest. Then it builds
and tests the copy at its `rust-version` and runs the template's cheap checks from that release. It ends
with the nife-side follow-up: each consumer manifest's new line and every other file that names
the crate. The script's header lists what it does not check.

Run for `file_allocation_table` from `main` into a scratch directory, it staged 116 files. The copy
passed its host tests at `rust-version` 1.88 and built for the three bare-metal targets. It
reported these findings, each one a change to make before the move:

| finding | where it comes from | owner |
|---|---|---|
| README lacks `## EXAMPLES` and `## BUGS`; one sentence of 67 words; 17.5 bold spans per 1,000 words against a cap of 4 | the crate's README | a lane, in nife |
| four lines say "In nife"; the label bytes `NIFE` in `volume.rs` | the crate's text | a lane, in nife |
| 35 test and proof modules reported as orphans | the template's orphan check | the template (see BUGS) |
| the scaffold names nife and the architect | the template's scaffold | the template (see BUGS) |
| passive voice, history phrases, function length and both naming gates "not at template v7" | the template's release | the template (see BUGS) |

The falsification records pass `falsify.py --check` once the dry run has rewritten their paths and
moved each `Falsification:` line to the head of its harness's comment, where the template reads it;
all 31 patches apply with `-p1`.

## Before the move, in nife

1. The architect has ratified the names (lane). `script/names <crate>` prints `ratified` under the crate's
   line, and `script/crate-standalone <crate>` exits 0 with Kani: every harness proves on the copy
   and every falsification patch turns its harness red.
2. Nothing it depends on is still here (lane). The `manifest` step of
   `script/crate-standalone` passes: no path dependency leaves the crate.
3. The text stands alone (lane). `script/crate-move --dry-run --into "$TMPDIR/<crate>-move"
   <crate>` exits 0. Fix each finding in nife, through an ordinary pull request, so the move commit
   stays mechanical. A finding against the template goes to the template.
4. The architect clears the crate (calef). `script/crate-review-packet <crate>` writes the
   packet; [the review packages](crate-reviews.md) hold the current ones. The clearance is also the
   approval to create the repository; no standing approval covers the order.

## Creating the repository

5. Create it (calef).

   ```sh
   gh repo create nifeos/<crate> --public --description "<one line from the manifest>"
   ```

   Expected: `gh repo view nifeos/<crate> --json isEmpty --jq .isEmpty` prints `true`.
6. Install the `nife-smelter` app on it (calef, as an organization owner, in the app's
   repository access settings). The release workflow's tag job authenticates as the app, and the
   tag ruleset accepts the app as its bypass actor only once the app sits on the repository.
7. Grant the three organization secrets (calef, with an `admin:org` token). Each has the
   visibility "selected repositories", so the list grows by one per crate:

   ```sh
   id=$(gh api repos/nifeos/<crate> --jq .id)
   for name in NIFE_SMELTER_APP_ID NIFE_SMELTER_PRIVATE_KEY SETTINGS_TOKEN; do
     gh api -X PUT "/orgs/nifeos/actions/secrets/$name/repositories/$id"
   done
   ```

   Expected: `gh api orgs/nifeos/actions/secrets/SETTINGS_TOKEN/repositories --jq '.repositories[].name'`
   lists the crate. The fine-grained token behind `SETTINGS_TOKEN` also needs the new repository in
   its repository access, which only its owner can widen. No agent handles any key value.

## The move

8. Stage from the merged tree (lane). From `main` at the commit the move will cite:

   ```sh
   script/crate-move --dry-run --into "$TMPDIR/<crate>-move" <crate>
   ```

   Expected: exit 0 and `<crate>: clean`. The staged tree has git initialized, every file added and
   nothing committed.
9. Commit and push (lane). The message names the template release and nothing of nife:

   ```sh
   cd "$TMPDIR/<crate>-move"
   git config core.hooksPath .githooks
   git commit -m "Adopt the shared crate workflows at crate-workflows <release>" \
     -m "Co-Authored-By: <model> <noreply@anthropic.com>"
   git remote add origin git@github.com:nifeos/<crate>.git
   git push -u origin main
   ```

   Expected: `git ls-remote origin refs/heads/main` prints the local `HEAD`, and the push starts the
   `check` and `proofs` runs on `main`. The repository starts with one root commit; nothing of this
   tree's history travels.
10. Apply the settings (lane, with repository administration). From a checkout of the template
    at the pinned release:

    ```sh
    script/apply-settings nifeos/<crate>
    script/check-settings nifeos/<crate>
    ```

    Expected: `apply-settings` reports nothing it could not set, and `check-settings` exits 0. That
    gives both rulesets, the `crates-io` environment with calef as required reviewer, the labels,
    the `crate-maintainers` team, and the fork approval policy for every outside contributor.
11. The first runs are green (lane). End the turn with
    `WAITING nifeos/<crate>:<run id>... on check and proofs`. Expected: `gh run list -R
    nifeos/<crate> --branch main --limit 4` shows `check` and `proofs` succeeded. A pull request in the
    crate repository fixes a red job, and the maintainer gates and merges it as in nife.

## The stranger test and the tag

12. Run the stranger test (a fresh-context agent the maintainer launches, never the move
    lane). It clones the repository and follows the README alone to a build, the tests and the
    proofs; the protocol is [the stranger test](stranger-test.md)'s. The lane records the run as a
    pull request in the crate repository that sets `Last run: <date>` in `docs/stranger-test.md`
    and adds the run's appendix beside it. Expected: `release / gate` stops refusing, since it
    refuses a record that reads `none`.
13. Tag the version (calef). The tag ruleset refuses a hand-made `v*` tag, so the release
    workflow makes it:

    ```sh
    gh workflow run release.yml -R nifeos/<crate> -f version=0.1.0 -f dry-run=false
    ```

    The gate and the tag job run, and the publish job waits in `crates-io`. calef cancels the run
    there (`gh run cancel <run id> -R nifeos/<crate>`), so nothing publishes. Expected:
    `gh api repos/nifeos/<crate>/git/ref/tags/v0.1.0` resolves, and the soak holds its first
    semver baseline.

## The nife side

14. Replace the path dependency (lane, one pull request in nife). Take the commit of the tag:

    ```sh
    rev=$(gh api repos/nifeos/<crate>/commits/v0.1.0 --jq .sha)
    ```

    In each consumer manifest the dry run listed, the line becomes

    ```toml
    <crate> = { version = "0.1.0", git = "https://github.com/nifeos/<crate>", rev = "<rev>" }
    ```

    keeping any other key the old line carried. Remove `crates/<crate>` from the workspace members
    and delete the directory with `git rm -r crates/<crate>`. The same pull request reads every
    other file the dry run listed. For the pilot and the FAT crate those are:

    - the `script/verify` table and the Kani reach list in `.github/workflows/kani-reach.yml`;
    - `helpers/trust_boundary_crates.tsv` and `packages/*.package.toml`;
    - the mutation baseline, the `deny.toml` wrappers and `_typos.toml`;
    - fuzz targets and their seeds, which move to the crate repository's `fuzz/`
      ([the proposal](../design/roadmap/proposals/a-moved-crate-is-fuzzed-in-its-own-repository.md));
    - citations into `crates/<crate>/src`, which repoint to the repository at the tag, by symbol.

    The crate's milestone block states the soak's state. Expected: `cargo tree --locked -i <crate>`
    prints the git source at `rev`, `git grep -n 'crates/<crate>' -- '*.toml'` prints nothing,
    `script/lint` exits 0, and the merge queue lands it.
15. Bump basalt's pin (lane, a separate pull request in `nifeos/basalt`). It cites the landed
    commit as `nifeos/nife@<sha>` and names the nife pull request it waits for.

A later bump of the crate's pin is its own nife pull request, citing `nifeos/<crate>@<sha>` of a
tagged commit.

## The soak's five end conditions

The soak ends on evidence; condition 5 also sets the shortest soak. Each condition below has its check; all five hold before
a publication can come to calef. Condition 5 does not read `weekly / semver`, which reports a break
in its run's summary and succeeds whatever it finds; it reads the labeler's finding on each merged
pull request and a direct run. [Milestone 1034](../design/roadmap/1034-a-crate-move-runbook.md)
holds the grounds for four weeks.

| condition | check | who |
|---|---|---|
| 1. nife builds and passes CI against the pinned commit, with no path dependency left | `git grep -n 'path = .*crates/<crate>' -- '*.toml'` is empty, and `gh run list -R nifeos/nife --branch main --workflow ci.yml --limit 1` shows success at or after the step 14 merge | lane |
| 2. the template's weekly run is green | `gh run list -R nifeos/<crate> --workflow weekly.yml --limit 1 --json conclusion` prints `success`; `gh workflow run weekly.yml -R nifeos/<crate>` starts one early | lane |
| 3. a release dry run is green | `gh workflow run release.yml -R nifeos/<crate> -f version=<manifest version> -f dry-run=true`, then that run's `gate` succeeds and it creates no tag | lane |
| 4. the stranger test builds and proves the crate from its README alone | `grep '^Last run:' docs/stranger-test.md` in the crate repository prints a date, with that run's appendix beside it | step 12 |
| 5. the API goes four weeks with no change `cargo-semver-checks` calls breaking | 28 days have passed since the tag nife pins, with at least four scheduled `weekly` runs green; `gh search prs --repo nifeos/<crate> --merged-at '>=<tag date>' '"major break"'` prints nothing; and `cargo semver-checks --baseline-rev v<pinned version>` on the crate's `main` reports no major change. A break restarts the period at the tag that carries it. | lane |

## Publication and consumption by version

Each publication waits for its soak and its turn in the ruled order, and a contract crate never
publishes.

16. Allow the publication (calef approves; a lane writes the line). The crate joins
    `crates/publish-allowlist.txt` in nife, and a pull request in the crate repository removes
    `publish = false` and `allow-unpublished: true`.
17. The first-publish token (calef). crates.io binds a trusted publisher only to a crate that
    exists, so 0.1.0 publishes with `CRATES_IO_FIRST_PUBLISH_TOKEN`, a `publish-new` token limited
    to the crate's name with a short expiry, stored on the repository's `crates-io` environment.
18. Publish (calef). `gh workflow run release.yml -R nifeos/<crate> -f version=<version> -f
    dry-run=false`, then approve the `crates-io` deployment. Expected: `cargo search <crate>
    --limit 1` shows the version.
19. Bind the owners and the trusted publisher, and delete the token (calef). `cargo owner --add
    github:nifeos:crate-maintainers <crate>`; on crates.io, a trusted publisher for
    `nifeos/<crate>`, workflow `release.yml`, environment `crates-io`; then delete the secret. Every
    later release publishes through trusted publishing alone.
20. nife consumes the release (lane). Each consumer line becomes `<crate> = "<version>"`.
    Expected: `cargo tree --locked -i <crate>` resolves from crates.io and `Cargo.lock` records the
    checksum. The milestone's exit holds.

## What calef does, in one list

- Clear the crate's review, which approves creating its repository (step 4).
- Create `nifeos/<crate>` (step 5).
- Install the `nife-smelter` app on it (step 6).
- Grant `NIFE_SMELTER_APP_ID`, `NIFE_SMELTER_PRIVATE_KEY` and `SETTINGS_TOKEN` to it, and widen the
  `SETTINGS_TOKEN` token's repository access (step 7).
- Dispatch the release that makes each tag, and cancel its publish (step 13).
- Approve each publication onto the allowlist (step 16).
- Create the first-publish token on the `crates-io` environment (step 17).
- Approve the `crates-io` deployment that publishes (step 18).
- Add `github:nifeos:crate-maintainers` as the crates.io owner, bind the trusted publisher and
  delete the token (step 19).

## BUGS

- The template's newest release lacks the naming gates. `spelled_out.py`, `unit_symbols.py`,
  the passive-voice, history-phrase and function-length checks are on the template's `main` but in
  no `v*` release; `v7` is the newest. A move pinned at a release runs none of them, and the
  pilot pins a commit between releases. A template release, adopted by the canary, closes it.
- The template's orphan check misreads modules declared in `foo.rs`. A `mod tests;` in
  `src/foo.rs` resolves to `src/foo/tests.rs`, and the check looks for `src/tests.rs`. It reports
  35 false orphans in `file_allocation_table`, so `check / standalone` would fail its first run.
- The template's scaffold mentions nife. The commit hook, `rustfmt.toml`, `deny.toml` and
  `docs/stranger-test.md` name it, and the pull request template names the architect. Every crate
  repository inherits those lines.
- No lint fails a `[patch]` that reaches nife's `main`. Milestone 857's fork 2 rules one.
- The dry run does not stage fuzz targets. They move to the crate repository's `fuzz/`, with
  `fuzz: true` in its weekly caller, and
  [the proposal](../design/roadmap/proposals/a-moved-crate-is-fuzzed-in-its-own-repository.md) builds
  the staging; until then the move lane copies them by hand.
- The mutation baseline does not travel. nife's row for the crate stays in
  `.cargo/mutants-baseline.txt`, and the crate repository's `weekly / mutation` fails on any
  survivor until `.github/mutants-exempts` accounts for it; the pilot needed one pull request for it.
  The dry run does not run cargo-mutants.
- Condition 3 is unmeasured on the pilot. Its release runs were the two that made its tags, and
  none ran with `dry-run=true`, so whether a dry run of an already tagged version passes the gate is
  unknown.
