---
status: DECIDED
raised: 2026-11-12
decided: 2026-11-12
ratified_by: calef
---

# 322. The QEMU bootstrap is the one QEMU check

*(The section number and title are provisional until the merge lands them.)*

## Background

The suite boots three emulators, `qemu-system-aarch64`, `qemu-system-riscv64` and
`qemu-system-x86_64`, and `.qemu-version` pins the release they must be. Two scripts provision a
machine. `script/bootstrap` installs the Rust toolchain, python3, gh, ShellCheck, Ruby, a cross
clang and the lint tools. `script/ci-qemu` builds the pinned QEMU from source into
`$HOME/.cache/nife-qemu`, which `helpers/qemu-path.sh` puts first on PATH.

Before this ruling, bootstrap also handled QEMU. It installed a packaged QEMU (Homebrew's, or apt's
five `bootstrap-qemu` packages) when none was on PATH, and then ran `script/qemu-check`. That check
failed on a missing device and only warned on a version other than the pin. On a Linux failure
bootstrap ran `script/ci-qemu` and checked again. CI never used the packaged path: every job runs
`script/ci-qemu` first, so bootstrap found the pinned build and skipped its install. The packaged
QEMU therefore served nobody in CI and, on a cold Linux box, was installed only to be shadowed by
the build. Two scripts held the question "is this the right QEMU", with two different answers to a
version mismatch.

## The decision

Ruled 2026-11-12 (UTC):

- `script/bootstrap` neither checks for QEMU nor installs it.
- `script/ci-qemu` is the one place that makes sure the right QEMU is present, on every machine and
  not only in CI. When the QEMU on PATH, or the one in its cache, is not exactly `.qemu-version`, it
  builds the pinned release and installs it into its prefix. When the QEMU on PATH is still wrong
  afterwards, it fails and says what is wrong.
- The `bootstrap-qemu` apt group goes, since nothing reads it.

What follows in the tree:

- A version other than the pin is now a failure, not a warning. `script/qemu-check` is the verdict
  ci-qemu renders before and after its build; it installs nothing, and bootstrap does not run it.
- `script/setup`, `script/update` and `script/ci-build`'s no-argument path run `script/ci-qemu`
  after `script/bootstrap`, so a newcomer still gets the pinned emulator from one command, as
  [§181 (bootstrap provisions and judges)](0181-bootstrap-provisions-and-judges.md) required of
  setup and update.
- macOS builds the pin from source too, with Homebrew supplying the build dependencies, because
  Homebrew installs its current QEMU and not the pin.

## How other systems do it

- rustup reads `rust-toolchain.toml` and installs exactly the pinned toolchain on first use, rather
  than accepting whatever `rustc` the system has. This tree already does that for Rust, and the
  ruling gives QEMU the same rule.
- Yocto and Buildroot build the QEMU their images run under from their own pinned recipe
  (`qemu-system-native`, `host-qemu`) instead of using the host's, for the same reason: the build
  must not depend on what a distribution happens to ship.
- Many projects instead accept any packaged QEMU new enough to boot, and check only a minimum
  version.

nife builds its pinned QEMU on purpose: an icount benchmark and a confinement test mean what they
mean only on one emulator, so a developer and CI run the same release.

## A concrete example

An Ubuntu 24.04 machine has apt's QEMU 8.2.2, which lacks `riscv-iommu-pci`. `script/setup` runs
bootstrap, which leaves QEMU alone, and then `script/ci-qemu`. `script/qemu-check` reports the
8.2.2 binaries and the missing device. ci-qemu installs its build dependencies, builds the pinned
release into `$HOME/.cache/nife-qemu`, sources `helpers/qemu-path.sh` and checks again. The pinned
build is now first on PATH, so the check passes, and every later `script/` entry point runs that
build. A Mac whose Homebrew QEMU is one patch release past the pin takes the same route.
