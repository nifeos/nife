# helpers/qemu-path.sh: put this project's own QEMU on PATH, if it has one.
#
# There is no shebang to infer a dialect from, because this file is sourced rather than run, so
# ShellCheck is told which one to assume. The directive takes no trailing prose (SC1125).
# shellcheck shell=sh
#
# **Sourced, never executed.** It has no shebang and no `set -e` on purpose: it modifies the PATH of
# the shell that reads it, which an executed script cannot do. Every `script/` entry point that can
# end up running an emulator reads it, one line, immediately after the `cd` to the repository root:
#
#     . helpers/qemu-path.sh
#
# **Why it exists: a build nobody puts on PATH is a loop** (milestone 287 (`script/bootstrap`
# installs a working QEMU on Linux)). `script/ci-qemu` builds the pinned QEMU into
# `$HOME/.cache/nife-qemu` when the QEMU on PATH is not exactly the pin, since few distributions ship
# one with `riscv-iommu-pci` (see that script's header, and §20 (IOMMU-backed DMA isolation)).
# Without this file only `.github/workflows/ci.yml` would put that prefix on PATH, so a developer
# who built the right QEMU would find `/usr/bin`'s again on the next check: same failure, same
# message, same remedy, forever. The build was never the missing piece. PATH was.
#
# **The mechanism is inheritance, not a spawn site.** The emulator is invoked by bare name from 38
# places across 16 files, in three languages, including `exec qemu-system-aarch64` at the bottom of
# each `helpers/qemu-runner-*.sh`. Patching those would be 38 copies of one fact. Exporting PATH once
# at the top of the process tree reaches every one of them, and reaches the ones nobody has written
# yet. That is also why the helpers under `helpers/` do not source this: they are spawned by an entry
# point (or by `cargo xtask`, which the entry point spawned) and have already inherited it.
#
# **The contract on the caller is only "be somewhere inside the tree".** Every script in `script/`
# does `cd "$(dirname "$0")/.."` as its first act, but `script/crate-probes` keeps a `$ROOT` and never
# cds, so a rule spelled "be at the root" would have had exactly one exception on the day it was
# written. The pin is found by walking up instead, which costs four lines and removes the exception
# rather than documenting it. `script/lint`'s "the project's QEMU is on PATH" check enforces that the
# source line is present; `BUGS` in design/roadmap/0287-bootstrap-installs-a-working-qemu.md records
# what it cannot check.
#
# Name: provisional, minted by milestone 287's lane on 2026-09-13. Hyphenated because AGENTS.md's
# naming table hyphenates everything under `helpers/`, and a noun phrase because a namespace is a
# thing rather than an action: this is QEMU's path, not an instruction to path it. calef has not
# ruled on it.

# `QEMU_PREFIX` and the default are `script/ci-qemu`'s, spelled once there and read here; changing
# where the build lands is that script's decision and this file follows it.
nife_qemu_prefix="${QEMU_PREFIX:-$HOME/.cache/nife-qemu}"

if [ -x "$nife_qemu_prefix/bin/qemu-system-aarch64" ]; then
    # **The version gate, which is about demotion rather than tidiness.** Prepending unconditionally
    # would let a prefix left over from an older pin win over a system QEMU that is perfectly good,
    # which is a regression this file would have caused on any distribution that ships a new enough
    # emulator. `.qemu-version` stays the single source of truth (`script/qemu-check`,
    # `script/ci-qemu` and the CI cache key are the others); this reads that file, it does not carry
    # a fourth copy of the number.
    nife_qemu_dir="$PWD"
    while [ ! -r "$nife_qemu_dir/.qemu-version" ] && [ "$nife_qemu_dir" != / ] \
        && [ -n "$nife_qemu_dir" ]; do
        nife_qemu_dir="$(dirname "$nife_qemu_dir")"
    done
    nife_qemu_want=""
    [ -r "$nife_qemu_dir/.qemu-version" ] \
        && nife_qemu_want="$(tr -d ' \n' < "$nife_qemu_dir/.qemu-version")"
    nife_qemu_have="$("$nife_qemu_prefix/bin/qemu-system-aarch64" --version 2>/dev/null \
        | head -1 | awk '{print $4}')"
    if [ -n "$nife_qemu_want" ] && [ "$nife_qemu_have" = "$nife_qemu_want" ]; then
        # Idempotent: sourcing twice (an entry point that calls another) must not stack the prefix up
        # in PATH. The `:$PATH:` padding is what makes a first or last element match the same way a
        # middle one does.
        case ":$PATH:" in
        *":$nife_qemu_prefix/bin:"*) ;;
        *)
            PATH="$nife_qemu_prefix/bin:$PATH"
            export PATH
            ;;
        esac
    fi
    unset nife_qemu_want nife_qemu_have nife_qemu_dir
fi

unset nife_qemu_prefix
