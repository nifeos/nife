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

Each guest slot owns a git worktree and a `target/` directory, because `cargo xtask test` writes its
disk image under the workspace root and QEMU refuses one image to two writers. A window is one
`cargo xtask test --arch x86_64 --test <driver>` with the seed range set at build time. The driver
stops itself after 60 seconds of guest time, so each window costs a rebuild, a boot and one minute of
seeds. The BUGS list below records that limit.

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

The architect has not ruled where the output goes. The proposal
[`where-the-daily-fuzz-output-goes`](../../design/roadmap/proposals/where-the-daily-fuzz-output-goes.md)
sets out the two options. Until then the script writes local files and publishes nothing.

## Install, for a person with root

The lane has no root. A person copies the three files in `helpers/systemd/` to
`/etc/systemd/system/`, tells systemd to reread its unit files, then `enable --now nife-fuzz.service` and
`enable --now nife-fuzz-publish.timer`.

## BUGS

- Today a window runs under TCG, not KVM. `cargo xtask test --arch x86_64 --test <driver>` clears
  `NIFE_ACCEL` and boots the PVH leg and then the OVMF legs, so one window sweeps the same seed
  range on two machines at 18 and 34 seeds per second, about a tenth of the KVM rate the rates note
  measured. A smoke window took 226 seconds. The fix is a window that builds once and runs the PVH
  runner directly with `-accel kvm` from a copy of the image, as the rates note's measurement did.
  The 1, 4 and 8 guest measurements decide whether that is enough.
- The build fixes the seed range, so each 60-second window pays a rebuild. A runtime seed (for
  example a kernel command line word) would let one image sweep forever. That change belongs to the
  drivers' milestones.
- Coverage counters do not exist yet, so the report has no coverage figure. The heartbeat reads seeds
  run.
- The bound cannot tell a hung guest from a slow rebuild. It is 30 minutes, and a window that reaches
  it is held.
- Only x86_64 runs. aarch64 and riscv64 need emulation and a larger machine.
- Nobody has run the daily publish end to end against a real credential, because a person installs
  the credential.
