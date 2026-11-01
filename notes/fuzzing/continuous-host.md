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

## What it keeps

Everything lives under one state directory. `windows.tsv` holds one row per window: the driver, the
slot, the first seed, the seeds run, the time, the status and the commit. A seed replays the same
calls every time, so a clean range is its own corpus.

The supervisor holds every window whose status is not `ok`. That covers a `replay:` line (a
shadow-model mismatch or an escape), a panic, and a window that ran out its bound. Its log moves to a
private directory and stays there. The next window for that slot starts one seed past the failing one.

## What it publishes

`helpers/fuzz-daily-publish.sh` runs from a daily timer and writes counter totals and a table of
clean seed ranges. It reports held findings by count and age only. It never opens a held log, and it
leaves the held window's own range out of the table, so nothing published reproduces a held finding.
A finding stays private until its fix merges, and starts as a private security advisory. A finding
sorted as public at triage is the exception.

With `--publish` it creates one GitHub release per day on nifeos/nife, tagged `fuzz-YYYY-MM-DD` (the
UTC date), with `report.md` and `clean-ranges.tsv` as its assets: `gh release create fuzz-YYYY-MM-DD
report.md clean-ranges.tsv`. The newest release's date is the heartbeat. A maintainer can delete an
old release without touching history, and no clone carries it. An architect ruled this on 2026-11-01
(UTC) in
[`where-the-daily-fuzz-output-goes`](../../design/roadmap/proposals/where-the-daily-fuzz-output-goes.md),
which also records the rejected branch of artifacts.

The publish path refuses unless the host's environment holds `NIFE_FUZZ_PUBLISH=1`, and the timer
stays disabled until a person installs the credential and sets that variable. `--dry-run` prints the tag
and the asset list and runs nothing else.

The credential needs Contents: write on a fine-grained token (from memory of GitHub's
permission model, not verified). That scope also permits a push, so the guarantee that the host never
pushes a branch rests on the script, which only calls `gh release create`.

## Install, for a person with root

The lane has no root. A person copies the three files in `helpers/systemd/` to
`/etc/systemd/system/`, tells systemd to reread its unit files, then `enable --now nife-fuzz.service` and
`enable --now nife-fuzz-publish.timer`.

That installs phase 1: the publish unit writes the day's report and clean ranges to local files and
publishes nothing. Phase 2 publishes the release, and takes three more steps as root:

1. Write a credential for the publisher (`GH_TOKEN`, the variable `gh` reads) to a file that only root
   can read, mode 0600 and owned by root.
2. Run `systemctl edit nife-fuzz-publish.service` and add a drop-in with `EnvironmentFile=` naming that
   file, `Environment=NIFE_FUZZ_PUBLISH=1`, and an `ExecStart=` that first clears the shipped one
   (`ExecStart=` on its own line) and then runs `helpers/fuzz-daily-publish.sh --publish`. The unit as
   shipped has no `EnvironmentFile=`, sets no `NIFE_FUZZ_PUBLISH` and passes no flag.
3. Tell systemd to reread its unit files, as for the install above. Without `NIFE_FUZZ_PUBLISH=1` the script refuses to publish, and
   without the credential `gh` fails; either way the local files are still written.

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
