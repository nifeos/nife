#!/bin/sh
# helpers/continuous-fuzz.sh: run the two x86_64 in-guest fuzz drivers for as long as the machine is
# up, a few guests at a time. The systemd unit helpers/systemd/nife-fuzz.service runs it.
#
#     helpers/continuous-fuzz.sh              run forever, 4 guests (NIFE_FUZZ_GUESTS)
#     helpers/continuous-fuzz.sh --once DRIVER SLOT   one sweep window, then print the verdict
#
# DRIVER is `kernel-thread` (milestone 752, a seeded syscall driver with a shadow model) or
# `confined` (milestone 779, fuzz the surface a confined process can reach). Guests alternate
# between them, so the default four guests are two of each.
#
# **What one window is.** Both drivers take their seed range at build time
# (`NIFE_SYSCALL_FUZZ_SEEDS` and `NIFE_CONFINED_FUZZ_SEEDS`, `<first>:<count>`) and stop between seeds
# after 60 s of guest time, so a window is one `cargo xtask test --arch x86_64 --test <driver>` that
# rebuilds the suite for the range, boots one guest under KVM, and sweeps. A seed replays the same
# calls every time, so a clean range is its own corpus: the record of which ranges ran clean is what
# the daily job publishes. See notes/fuzzing/continuous-host.md.
#
# **One checkout per guest.** `cargo xtask test` writes `target/nifefs.img` under the workspace root,
# and QEMU refuses one image to two writers, so each guest slot runs in its own git worktree of the
# clone this script lives in, each with its own `target/`. The worktrees are detached at the clone's
# HEAD when the slot starts; updating the clone and restarting the unit moves the whole fleet to a
# new commit. Seed ranges are per slot (a fixed stride apart), so two slots never sweep one seed.
#
# **A finding is held.** A window that prints a `replay:` line, panics, or runs out its bound is
# untriaged by definition. Its log moves to `$state/held/`, mode 700, and nothing here ever
# publishes or prints that directory's contents. The daily job reports only the count and the ages.
# A window that fails to build, or fails without any of those signs, is infrastructure: it backs off,
# and after five in a row the supervisor exits non-zero so systemd restarts it.
#
# State, all under `$NIFE_FUZZ_HOME` (default `$HOME/fuzz`):
#   work/<slot>/            the slot's worktree
#   state/next-<slot>       the next first seed for the slot
#   state/windows.tsv       one row per window: utc, driver, slot, first, ran, ms, status, commit
#   state/held/             held logs (private)
#   logs/<slot>/            the last logs of clean windows (the newest 20)
#
# Name: provisional, minted 2026-11-01 (UTC) by lane/continuous-fuzzing-host. calef has not ruled on it.

set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
home="${NIFE_FUZZ_HOME:-$HOME/fuzz}"
state="$home/state"
guests="${NIFE_FUZZ_GUESTS:-4}"
# One window asks for more seeds than 60 s of guest time can run, so the guest's own cap ends it.
ask=1000000
# The bound covers the rebuild, the boot and the 60 s sweep. A cold per-slot build is slower, so the
# first window of a slot gets the long bound.
bound="${NIFE_FUZZ_BOUND:-1800}"
# Slot n starts at base + n * stride. The weekly sweeps start near 2.0e11; this stays below them.
base=10000000000
stride=1000000000

# Put the pinned QEMU and rustup's cargo on PATH, as script/bootstrap's own callers do.
PATH="$HOME/.cargo/bin:$PATH"
. "$root/helpers/qemu-path.sh"

driver_test() {
    case "$1" in
        kernel-thread) echo "seeded_syscalls NIFE_SYSCALL_FUZZ_SEEDS" ;;
        confined) echo "a_confined_fuzzer NIFE_CONFINED_FUZZ_SEEDS" ;;
        *) echo "continuous-fuzz: unknown driver '$1'" >&2; return 1 ;;
    esac
}

slot_driver() {
    if [ $(($1 % 2)) -eq 0 ]; then echo kernel-thread; else echo confined; fi
}

prepare_slot() {
    slot="$1"
    mkdir -p "$state/held" "$home/logs/$slot" "$home/work"
    chmod 700 "$state/held"
    if [ ! -d "$home/work/$slot" ]; then
        git -C "$root" worktree add --detach "$home/work/$slot" HEAD >/dev/null
    fi
    [ -f "$state/next-$slot" ] || echo $((base + slot * stride)) > "$state/next-$slot"
}

# One window. Prints the status word on stdout; everything else goes to the log.
window() {
    driver="$1"
    slot="$2"
    # shellcheck disable=SC2046  # two words, split on purpose
    set -- $(driver_test "$driver")
    filter="$1"
    var="$2"
    prepare_slot "$slot"
    first="$(cat "$state/next-$slot")"
    log="$home/logs/$slot/$(date -u +%Y%m%dT%H%M%SZ)-$first.log"
    commit="$(git -C "$home/work/$slot" rev-parse --short HEAD)"
    start="$(date +%s)"
    set +e
    (
        cd "$home/work/$slot"
        env "$var=$first:$ask" "$root/helpers/qemu-bounded.sh" "$bound" \
            cargo xtask test --arch x86_64 --test "$filter"
    ) > "$log" 2>&1
    rc=$?
    set -e
    ms=$((($(date +%s) - start) * 1000))

    ran=0
    status=ok
    next=$((first + 1))
    # The driver's own end line: `<n> seeds, <ms> ms (...)`.
    line="$(grep -E '(syscall|confined) fuzzer: [0-9]+ seeds, ' "$log" | tail -1 || true)"
    replay="$(sed -n 's/.*replay: NIFE_[A-Z_]*=\([0-9][0-9]*\):1.*/\1/p' "$log" | head -1)"
    if [ -n "$replay" ]; then
        status=held-finding
        next=$((replay + 1))
        ran=$((replay - first))
    elif [ "$rc" -eq 0 ] && [ -n "$line" ]; then
        ran="$(echo "$line" | sed 's/.*: \([0-9][0-9]*\) seeds, .*/\1/')"
        next=$((first + ran))
        [ "$ran" -gt 0 ] || next=$((first + 1))
    elif [ "$rc" -eq 143 ] || [ "$rc" -eq 137 ]; then
        status=held-hang
    elif grep -q -E 'panicked|PANIC' "$log"; then
        status=held-crash
    else
        status=infrastructure
    fi

    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$driver" "$slot" "$first" "$ran" "$ms" "$status" "$commit" \
        >> "$state/windows.tsv"
    echo "$next" > "$state/next-$slot"
    case "$status" in
        held-*) mv "$log" "$state/held/" ;;
        ok) ls -t "$home/logs/$slot"/*.log 2>/dev/null | tail -n +21 | xargs -r rm -f ;;
    esac
    echo "$status"
}

worker() {
    slot="$1"
    driver="$(slot_driver "$slot")"
    failures=0
    while :; do
        status="$(window "$driver" "$slot")"
        echo "continuous-fuzz: slot $slot $driver: $status"
        if [ "$status" = infrastructure ]; then
            failures=$((failures + 1))
            [ "$failures" -lt 5 ] || { echo "continuous-fuzz: slot $slot: 5 failed windows in a row" >&2; exit 1; }
            sleep 60
        else
            failures=0
        fi
    done
}

if [ "${1:-}" = --once ]; then
    [ $# -eq 3 ] || { echo "usage: $0 --once DRIVER SLOT" >&2; exit 2; }
    mkdir -p "$state"
    window "$2" "$3"
    exit 0
fi

mkdir -p "$state"
pids=""
trap 'kill $pids 2>/dev/null; wait' TERM INT
slot=0
while [ "$slot" -lt "$guests" ]; do
    worker "$slot" &
    pids="$pids $!"
    slot=$((slot + 1))
done
# One worker exiting means five failed windows in a row; end the unit so systemd restarts it.
# (`wait -n` is not POSIX; poll instead.)
alive=1
while [ "$alive" -eq 1 ]; do
    sleep 10
    for p in $pids; do
        kill -0 "$p" 2>/dev/null || alive=0
    done
done
kill $pids 2>/dev/null || true
exit 1
