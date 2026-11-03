# The continuous fuzzing host: what runs, what it keeps, what it publishes

*Name: provisional.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md). A dedicated x86_64 machine with KVM runs the two
in-guest drivers all the time. The maintainers keep its specification off-tree. This page covers what
the repository supplies to it.

## What runs

`helpers/continuous-fuzz.sh` runs four guests at once, two of each driver: the kernel-thread driver
(milestone 752 (a seeded syscall driver with a shadow model)) and the confined user-mode driver
(milestone 779 (fuzz the surface a confined process can reach)). The false-red measurements in
[`rates-on-x86-64-under-kvm.md`](rates-on-x86-64-under-kvm.md) support four. Raise
`NIFE_FUZZ_GUESTS` only after a measurement at the new size comes back clean.

Each guest slot owns a git worktree and a `target/` directory, so two windows never share a build
directory. A window builds the x86_64 archive and the system test image once, baking the seed
range in at build time, then starts one QEMU process through the PVH runner with `-accel kvm` and sweeps
in that guest. It does not call `cargo xtask test`, which clears `NIFE_ACCEL` and also boots the
OVMF legs under TCG. The driver stops itself after 60 seconds of guest time, so each window costs a
rebuild, a boot and one minute of seeds. The BUGS list below records that limit.

Before each window the supervisor checks that KVM is usable: the device exists, the user may open it,
and QEMU can start a machine on it. If not, it names the problem on standard error and does not run
the window. It never falls back to TCG, which sweeps about a tenth of the seeds per second.
`helpers/continuous-fuzz.sh --check-kvm` runs the same check by hand. Two bounds apply:
`NIFE_FUZZ_BUILD_BOUND` (default 1800 seconds) covers the build, and `NIFE_FUZZ_BOUND` (default 300)
covers the guest, so a window that reaches the second is a hung guest, and the supervisor holds it.

Before it starts any slot, the supervisor installs, or confirms, the toolchain that
`rust-toolchain.toml` pins, once and serially, through `helpers/rustup-retry.sh show active-toolchain`
with the same `PATH` it uses for everything else. On the host's first start the repository pinned a
newer nightly than the `lane` account had, all four slots downloaded it at once and collided, and the
first windows failed as infrastructure within seconds. If the install fails, the supervisor stops
with a message that names it, starts no slot and writes no row to `windows.tsv`.
`helpers/continuous-fuzz-selftest.sh`, run by `script/lint`, holds this against a stub `rustup`.
Decided by the maintainer 2026-11-01 (UTC), provisional; reversing costs one pull request.

`helpers/systemd/nife-fuzz.service` starts the supervisor at boot and restarts it on failure. The
supervisor exits non-zero after five windows in a row fail without a finding. A broken build reaches
systemd that way instead of looping silently.

## Following main

The host updates itself from main with no person pulling, within four limits that an architect ruled
on 2026-11-01 (UTC) in
[`fuzz-findings-without-a-person`](../../design/roadmap/1015-fuzz-findings-without-a-person.md).
At most every 5 minutes (`NIFE_FUZZ_FOLLOW_EVERY`, in seconds; 0 turns it off) the supervisor runs
`helpers/fuzz-follow-main.py look`. It fetches main alone from `https://github.com/nifeos/nife.git`,
with every other protocol refused and no credential helper. It refuses a commit that does not contain
the clone's commit. It reads the names of main's required checks from the branch's rules and the
commit's check runs from the public API, with no credential. It moves only when each required check
has a successful run by GitHub Actions on that commit and none is unfinished or failed. An error
or a rate limit keeps the current commit, and the journal says why once.

When `look` names a commit, each slot stops after its current window. Then `apply` moves the clone
and the supervisor re-executes itself from it, which reruns the toolchain step and moves every slot
worktree. A change under `helpers/systemd/` is never applied. `apply` prints one journal line naming
the changed unit files, and a person with root installs them as below. The mint helper lives outside
the clone, so an update never replaces it.

This runs main's code on the host unattended, as the fuzz account, within about one window of a
merge. It widens nothing that account may do. `helpers/fuzz-follow-main-selftest.sh`, run by
`script/lint`, holds each limit against a scratch upstream and stub API answers. The maintainer chose
three details, provisionally; reversing any costs one pull request. The required checks come from
main's rules, not from a list in the tree; a run counts only when GitHub Actions made it; and a check
whose runs were all skipped is not green.

## What it keeps

Everything lives under one state directory. `windows.tsv` holds one row per window: the driver, the
slot, the first seed, the seeds run, the time, the status and the commit. A seed replays the same
calls every time, so a clean range is its own corpus.

The supervisor holds every window whose status is not `ok`. That covers a `replay:` line (a
shadow-model mismatch or an escape), a panic, and a window that ran out its bound. Its log moves to a
private directory and stays there. The next window for that slot starts one seed past the failing one.

## What it publishes

`helpers/fuzz-daily-publish.sh` runs from a daily timer and writes counter totals and a table of
clean seed ranges. It reports held findings by count and age only. It never opens a held log. After a
held replay the slot's next window starts one seed past the held seed, so the table leaves out every
range of a slot from that slot's first held window onward. A finding starts as a private security advisory and stays
private until its fix is on main and a rerun of its input passes against that build, or until 90
days after the report. A finding sorted as public at triage is the exception.

## Who reads a held finding

No lane opens, copies, prints or moves anything under the held directory. A plain service on the
host, not an agent, files each held finding as a private security advisory on nifeos/nife, holding
the finding class and the reproducer, and a fix lane reads that advisory and nothing else. Crash
inputs, seeds and reproducers stay out of every public text until the fix is on main. An architect
ruled this on 2026-11-01 (UTC) in
[`fuzz-findings-without-a-person`](../../design/roadmap/1015-fuzz-findings-without-a-person.md),
which also lists the milestones that build it. The filer is `fuzz-filer` in
[nifeos/fuzz-host](https://github.com/nifeos/fuzz-host); it runs as the fuzz account and files from a
one-hour token that a root timer mints. It is built and not yet installed, so until a person installs
it, nothing files an advisory.

With `--publish` it creates one GitHub release per day on nifeos/nife, tagged `fuzz-YYYY-MM-DD` (the
UTC date), with `report.md` and `clean-ranges.tsv` as its assets: `gh release create fuzz-YYYY-MM-DD
report.md clean-ranges.tsv`. The newest release's date is the heartbeat. A maintainer can delete an
old release without touching history, and no clone carries it. An architect ruled this on 2026-11-01
(UTC) in
[`where-the-daily-fuzz-output-goes`](../../design/roadmap/1026-where-the-daily-fuzz-output-goes.md),
which also records the rejected branch of artifacts.

The publish path refuses unless the host's environment holds `NIFE_FUZZ_PUBLISH=1`, and the timer
stays disabled until a person installs the credential and sets that variable. `--dry-run` prints the tag
and the asset list and runs nothing else.

The credential is a one-hour installation token of the `nife-fuzz` GitHub App, which a mint helper
on the host makes from the App's private key, readable by root alone. The helper is `fuzz-token` in
[nifeos/fuzz-host](https://github.com/nifeos/fuzz-host), and it mints one token per service, each
narrower than the App. Nobody has placed the key yet; [`finishing-the-setup.md`](finishing-the-setup.md) lists what remains and in what order. The App holds Contents write and Repository
security advisories write on nifeos/nife alone (ruled 2026-11-01 UTC).
[`the-fuzz-app.md`](the-fuzz-app.md) is the runbook for the person who creates it, and it states
what the key can do. Main's ruleset requires a pull request and has no bypass actor, so the token
cannot push to main; the script calls only `gh release create`.

## When the loop breaks

`.github/workflows/fuzz-heartbeat.yml` runs daily at 17:00 UTC on a GitHub-hosted runner and runs
`helpers/fuzz-heartbeat.py`. It reads the releases of the last two UTC days and opens one issue
with the label `fuzz-loop-broken` when no release exists, when the report shows no `ok` window, or when
`infrastructure` windows are half or more of all windows. The issue names the check and the day, and
no second one opens while one is open. It repairs nothing. A lane fixes anything in the tree; a repair
of the host is a written spec the architect hands to the homelab agent. While no `fuzz-*` release has
ever existed the workflow passes quietly, and it switches on by itself at the first release. The
maintainer decides the bound, the label and the two-day window, provisionally; reversing costs
one pull request. See [the proposal](../../design/roadmap/1023-the-fuzz-loop-dead-mans-switch.md).

## Install, for a person with root

The lane has no root. A person copies the three files in `helpers/systemd/` to
`/etc/systemd/system/`, tells systemd to reread its unit files, then `enable --now nife-fuzz.service` and
`enable --now nife-fuzz-publish.timer`.

That installs phase 1: the publish unit writes the day's report and clean ranges to local files and
publishes nothing. Phase 2 publishes the release. It waits on the key and the installed mint helper
(the host clone already carries the clean-range fix), and step P7 of [`finishing-the-setup.md`](finishing-the-setup.md)
gives its drop-in. The unit as shipped has no `EnvironmentFile=`, sets no `NIFE_FUZZ_PUBLISH` and passes no flag. Without
`NIFE_FUZZ_PUBLISH=1` the script refuses to publish. It also refuses, by name and before any `gh`
call, a missing `GH_TOKEN`, a missing or unreadable `GH_TOKEN_EXPIRES_AT`, and a token that has expired
or expires within five minutes. Either way the local files are still written.

The supervisor moves every slot worktree to the clone's commit when it starts, so a restart after
a person moves the clone runs every slot at that commit. The clone reaches the commit that carries
the follower once, by hand, in step P1 of that page; from then on it follows main by itself.

To check the wiring before the first release, run `helpers/fuzz-daily-publish.sh --dry-run` as the
unit's user. It prints the tag and the assets and publishes nothing.

## BUGS

- The windows have been timed one guest at a time, at about 457 seeds per second for the
  kernel-thread driver and 205 for the confined driver. Nobody has measured four windows at once
  through this path, so the rates note's four-guest figures stand in for them.
- The build fixes the seed range, so each 60-second window pays a rebuild. A runtime seed (for
  example a kernel command line word) would let one image sweep forever. That change belongs to the
  drivers' milestones.
- Coverage counters do not exist yet, so the report has no coverage figure. The heartbeat reads seeds
  run.
- Only x86_64 runs. aarch64 and riscv64 need emulation and a larger machine.
- Nobody has run the daily publish end to end against a real credential, because a person installs
  the credential.
- `windows.tsv` sits outside the held directory and any process of the fuzz account can read it. A
  held window's row carries its first seed and the seeds it ran before the replay, and their sum is
  the held seed. Only clean rows leave the host, so nothing has leaked; the filer's milestone should
  move held rows' seeds under the held directory.
- Each move to a newer main idles every slot until the slowest current window ends, and the first
  window after it rebuilds. Main lands many commits a day, so this costs a share of the rate that
  nobody has measured.
- An unfinished run of any required check name keeps the current commit, even the skipped duplicate
  that a push to main starts after the merge queue's run. The next look, 5 minutes later, moves.
- A person who sets `url.<base>.insteadOf` for GitHub in the fuzz account's git configuration to a
  non-https address stops the follower: the fetch refuses it, and the journal says so.
- A slot that ever held a finding publishes no more clean ranges, even after the finding turns
  public. The ranges return only once
  [a held finding turns public after its fix reruns clean](../../design/roadmap/1010-a-held-finding-turns-public-after-its-fix-reruns-clean.md)
  defines the signal that a finding has turned public
  ([the proposal](../../design/roadmap/1016-the-clean-range-table-does-not-locate-a-held-seed.md)).
