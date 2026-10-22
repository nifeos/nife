#!/bin/sh
# helpers/memory-bounded-runner.sh: run one host test binary under a hard memory ceiling.
#
# **The name is PROVISIONAL** (milestone 277 (bound what one mutant may allocate)): names are an
# architect's, and a lane ships one saying
# so.
# It sits between the two families already in this directory. `helpers/qemu-runner-*.sh` are cargo
# runners; `helpers/qemu-bounded.sh` bounds a process that would otherwise never stop. This is both,
# which is what the compound is trying to say.
#
# WHY THIS EXISTS (milestone 277). cargo-mutants bounds a mutant on TIME and on nothing else:
# `--timeout`, `--build-timeout`, their two multipliers and `--minimum-test-timeout` are the
# complete list of limits in 27.1.0, checked against `cargo mutants --help` rather than assumed.
# The failure the weekly sweep actually dies of is on MEMORY. One mutant went 1.4 GB to 15.8 GB in
# twenty seconds and took the runner agent down with the machine, comfortably inside the 28-to-51
# second auto timeout that therefore never fired. A clock cannot catch an allocation. The BUGS
# header of .github/workflows/mutation.yml has the measurement and how it was nearly missed.
#
# WHY A CARGO RUNNER, rather than a limit around the whole sweep. cargo runs a test binary through
# `target.<triple>.runner` when one is set, so a runner is the one place in this pipeline that sees
# exactly one test binary: not rustc, not cargo, not the other `-j 2` job's binary. That is the
# granularity a PER-MUTANT bound wants, and a limit set once around `script/mutation` cannot have
# it. script/mutation points CARGO_TARGET_<HOST>_RUNNER here for the length of a mutation run and
# nothing else in the tree does, so an ordinary `cargo test` is untouched.
#
#   helpers/memory-bounded-runner.sh <test-binary> [args...]
#
# MUTATION_MEMORY_LIMIT_KB sets the ceiling in kibibytes, overriding the default. `0` disables the
# bound and runs the binary bare, which is how you reproduce the failure this exists to prevent.
#
# EXAMPLES
#   # What script/mutation does, for one binary.
#   helpers/memory-bounded-runner.sh target/debug/deps/glob-1a2b3c4d
#
#   # Watch the bound fire. 64 MiB is below what any real suite here needs, so this aborts with
#   # "memory allocation of N bytes failed" and exits non-zero, which is what a killed mutant is.
#   MUTATION_MEMORY_LIMIT_KB=65536 helpers/memory-bounded-runner.sh target/debug/deps/glob-1a2b3c4d
#
#   # Turn it on for a macOS run, which is not the default; see BUGS.
#   MUTATION_MEMORY_LIMIT_KB=4194304 script/mutation -p glob
#
# THE NUMBER, which is measured rather than picked. Across all 143 host test binaries in this tree
# (`cargo test --workspace` minus the bare-metal crates, at `--test-threads=4`, reading each child's
# own VmPeak), the largest peak is `serial_console` at **1,028 MiB** and the mean is 169 MiB. Four
# gibibytes is 4.0x the largest honest binary, which is the headroom half. The other half is the
# ceiling the MACHINE imposes: `-j 2` means two test binaries can be resident at once, so a runaway
# in each costs 2 x the ceiling, and 8 GiB is survivable on the 16 GiB boxes this runs on where
# 2 x 8 GiB would not be. Four is the largest value that keeps both true. Corroborated the other
# way round, by lowering it until it bites: the honest suite is unaffected at 4 GiB and at 1 GiB,
# and at 256 MiB `serial_console` fails, exactly where its measured peak says it should.
#
# BUGS: the ceiling is on ADDRESS SPACE (RLIMIT_AS, `ulimit -v`) rather than on resident memory, so
# it over-counts every reservation nothing ever touches. RUST_MIN_STACK is 16 MiB in
# .cargo/config.toml and `--test-threads=4` turns that into 64 MiB of untouched stack before one
# byte of heap, and glibc reserves malloc arenas the same way. That over-count is priced into the
# number above rather than argued away, and it is the reason the default is generous instead of
# snug: a snug ceiling would fail honest tests and the report would score them as caught mutants,
# which is a lie in the one column the report exists to produce.
#
# BUGS: **off by default on macOS, and that is a gap rather than a verdict.** XNU does enforce
# RLIMIT_AS, contrary to the folklore and contrary to what this lane first wrote down:
# `bsd/kern/kern_resource.c` hands the limit to `vm_map_set_size_limit()`, `vm_map_enter()` fails
# with `KERN_NO_SPACE` once `map->size` passes it, and `vm_map_inherit_limits()` carries it across
# exec. That has been true since at least xnu-8792 (macOS 13). What is NOT known is what number is
# safe there, because milestone 277's lane had only Linux to measure on, and macOS reserves address
# space far more freely than glibc does, so a ceiling that is generous here could reject an honest
# test binary there. Defaulting it on with an unmeasured number would fail every Mac sweep at its
# baseline; defaulting it off and saying so leaves the Mac exactly as exposed as it was before this
# existed, which is the honest half of the trade. Turning it on is one environment variable, and
# whoever does it first should record the measured number here.
#
# BUGS: this bounds the TEST binary, not the build. A mutant whose damage lands in rustc rather
# than in the suite is still unbounded. None has been observed (the measured runaway is a test-side
# allocation) and bounding rustc needs a different number anyway, because a linker legitimately
# wants a great deal of address space.

set -e

case "$(uname -s)" in
    Darwin) default_limit_kb=0 ;;
    *)      default_limit_kb=4194304 ;;   # 4 GiB
esac
limit_kb=${MUTATION_MEMORY_LIMIT_KB:-$default_limit_kb}

if [ "$limit_kb" = "0" ]; then
    exec "$@"
fi

# **A shell that cannot set the limit must not go on to run the binary.** The caller asked for a
# bound, and running without one would produce the exact failure this exists to prevent, minus the
# notice, which is the "reads as protection" failure the BUGS section above refuses.
#
# `ulimit -v` is outside POSIX (ShellCheck SC3045) and the suppression below is scoped to the one
# line, per DECISIONS §38. What makes it safe is the READ-BACK rather than the claim: both shells
# this ever runs under implement it (dash is /bin/sh on the Ubuntu runners, bash in POSIX mode on
# the dev Mac), and rather than trusting that, the next three lines ask the shell what the limit
# actually is and refuse to run if it is not the number we asked for. A shell without the builtin
# fails that comparison instead of silently running the binary unbounded.
# shellcheck disable=SC3045
ulimit -v "$limit_kb" 2>/dev/null || true
# shellcheck disable=SC3045
if [ "$(ulimit -v)" != "$limit_kb" ]; then
    echo "memory-bounded-runner: asked for a ${limit_kb} KiB address-space limit, got" \
         "'$(ulimit -v 2>/dev/null || echo unsupported)'; refusing to run unbounded" >&2
    exit 1
fi

exec "$@"
