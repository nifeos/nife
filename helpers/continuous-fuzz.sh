#!/bin/sh
# helpers/continuous-fuzz.sh: run the two x86_64 in-guest fuzz drivers for as long as the machine is
# up, a few guests at a time. The systemd unit helpers/systemd/nife-fuzz.service runs it.
#
#     helpers/continuous-fuzz.sh              run forever, 4 guests (NIFE_FUZZ_GUESTS)
#     helpers/continuous-fuzz.sh --once DRIVER SLOT   one sweep window, then print the verdict
#     helpers/continuous-fuzz.sh --check-kvm  say whether a window could start, and why not
#     helpers/continuous-fuzz.sh --ci SHARD   one bounded CI window of shard SHARD (0 to 3), then exit
#
# DRIVER is `kernel-thread` (milestone 752 (a seeded syscall driver with a shadow model)) or
# `confined` (milestone 779 (fuzz the surface a confined process can reach)). Guests alternate
# between them, so the default four guests are two of each.
#
# **What one window is.** Both drivers take their seed range at build time
# (`NIFE_SYSCALL_FUZZ_SEEDS` and `NIFE_CONFINED_FUZZ_SEEDS`, `<first>:<count>`) and stop between seeds
# after 60 s of guest time. A window builds the x86_64 archive and the system test image once for the
# range, then starts one QEMU process through the PVH runner (`helpers/qemu-runner-x86_64.sh`) with
# `-accel kvm` and sweeps in that one guest. It does not call `cargo xtask test`: that clears
# `NIFE_ACCEL` and boots the OVMF legs as well, which are TCG, so a window sweeping that way ran two
# machines at about a tenth of the KVM rate (notes/fuzzing/rates-on-x86-64-under-kvm.md). A seed
# replays the same calls every time, so a clean range is its own corpus: the record of which ranges
# ran clean is what the daily job publishes. See notes/fuzzing/continuous-host.md.
#
# **KVM is checked, never assumed.** Before a window starts, `require_kvm` confirms that the device
# exists and that this user may open it, and asks QEMU to start a machine on it. If any of that
# fails the script says which, on stderr, and the window does not run. It never falls back to TCG: a
# silent fallback is how the first version of this supervisor spent its first night at a tenth of the
# rate.
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
# **One CI shard (`--ci SHARD`).** A pull request's merge-queue entry runs four shards, two per
# driver (even shards `kernel-thread`, odd shards `confined`), each one window on the checkout it
# stands in. The first seed derives from the commit and the shard alone (`ci_first_seed`), below the
# supervisor's `base` so the two never repeat a seed, which makes a shard deterministic: the fuzz
# host replays a shard from a commit and a shard number and needs nothing else (`--ci-range SHARD`
# prints the driver and first seed it would sweep). **The CI log is public, so the guest's output
# never reaches it.** Its builds print as usual; the guest's output goes to a mode-700 directory that
# an exit trap removes, and a finding prints only a count, the shard, the commit and the line
# `the fuzz host replays this shard`. A clean shard prints its seed range, which the host publishes
# anyway. Exit codes: 0 clean, 1 a finding, 3 infrastructure (no KVM, a failed build, a guest that
# ended with no sign of a finding). See notes/fuzzing/continuous-host.md.
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
# A CI shard asks for less than its 100,000-seed block, so two shards never overlap.
ci_ask=100000
# Two bounds, so a hung guest is not mistaken for a slow build. The build bound covers the archive and
# the test image; a cold per-slot build is slower, so the first window of a slot gets the long one.
# The run bound covers the boot and the 60 s sweep of one guest.
build_bound="${NIFE_FUZZ_BUILD_BOUND:-1800}"
bound="${NIFE_FUZZ_BOUND:-300}"
# The device the check reads; a variable only so that the refusal paths can be tried on a machine
# that has KVM.
kvm_device="${NIFE_FUZZ_KVM_DEVICE:-/dev/kvm}"
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

# Refuse, loudly and by name, unless a guest can start under KVM. Prints the reason on stderr.
require_kvm() {
    if [ ! -e "$kvm_device" ]; then
        echo "continuous-fuzz: KVM is not usable: $kvm_device does not exist (the kvm module is not loaded, or this machine has no hardware virtualization)" >&2
        return 1
    fi
    if [ ! -c "$kvm_device" ]; then
        echo "continuous-fuzz: KVM is not usable: $kvm_device is not a character device" >&2
        return 1
    fi
    if [ ! -r "$kvm_device" ] || [ ! -w "$kvm_device" ]; then
        echo "continuous-fuzz: KVM is not usable: $(id -un) cannot read and write $kvm_device ($(ls -l "$kvm_device")); add the user to the group that owns it" >&2
        return 1
    fi
    # The device can exist and open yet refuse to start a machine (nested virtualization switched
    # off, another hypervisor holding the extensions). Ask QEMU, which is the program that has to
    # succeed. `-S` keeps the guest stopped; the monitor's `quit` ends it.
    probe="$(printf 'quit\n' | "$root/helpers/qemu-bounded.sh" 30 \
        qemu-system-x86_64 -accel kvm -machine q35 -display none -nodefaults -S -monitor stdio 2>&1)" || {
        echo "continuous-fuzz: KVM is not usable: QEMU could not start a machine with -accel kvm: $(echo "$probe" | head -1)" >&2
        return 1
    }
}

refuse_without_kvm() {
    require_kvm || {
        echo "continuous-fuzz: refusing to run: a window under TCG sweeps about a tenth of the seeds per second, so this script never falls back to it" >&2
        return 1
    }
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

# Read a window's verdict from its log and exit status, and set `status`, `ran` and `next`. The
# supervisor and the CI shard (`--ci`) share it, so a finding means one thing in both places.
classify() {
    log="$1"
    rc="$2"
    first="$3"
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
    elif [ "$rc" -eq 125 ]; then
        status=infrastructure
    elif [ "$rc" -eq 143 ] || [ "$rc" -eq 137 ]; then
        status=held-hang
    elif grep -q -E 'panicked|PANIC' "$log"; then
        status=held-crash
    else
        status=infrastructure
    fi
}

# One window. Prints the status word on stdout; everything else goes to the log.
window() {
    driver="$1"
    slot="$2"
    # shellcheck disable=SC2046  # two words, split on purpose
    set -- $(driver_test "$driver")
    filter="$1"
    var="$2"
    refuse_without_kvm || { echo infrastructure; return 0; }
    prepare_slot "$slot"
    first="$(cat "$state/next-$slot")"
    log="$home/logs/$slot/$(date -u +%Y%m%dT%H%M%SZ)-$first.log"
    commit="$(git -C "$home/work/$slot" rev-parse --short HEAD)"
    start="$(date +%s)"
    set +e
    (
        cd "$home/work/$slot"
        # Build once. The archive does not depend on the range; the system test image does, because
        # the range and the filter are baked in at build time. 125 means the build failed or ran out
        # its bound, which is infrastructure whatever the log says.
        "$root/helpers/qemu-bounded.sh" "$build_bound" cargo xtask initial-ramdisk-x86 || exit 125
        NIFE_INITIAL_RAMDISK="$PWD/target/initial-ramdisk-x86_64.img"
        export NIFE_INITIAL_RAMDISK
        NIFE_TEST_FILTER="$filter"
        NIFE_TEST_FILTER_ACROSS_IMAGES=1
        export NIFE_TEST_FILTER NIFE_TEST_FILTER_ACROSS_IMAGES
        export "$var=$first:$ask"
        json="$home/work/$slot.build.json"
        "$root/helpers/qemu-bounded.sh" "$build_bound" cargo test -p system_tests --target x86_64-unknown-none \
            --no-run --message-format=json-render-diagnostics > "$json" || exit 125
        elf="$(sed -n '/"reason":"compiler-artifact"/{/"name":"system_tests"/s/.*"executable":"\([^"]*\)".*/\1/p;}' "$json" | tail -1)"
        [ -n "$elf" ] || { echo "continuous-fuzz: cargo named no system_tests executable" >&2; exit 125; }
        echo "continuous-fuzz: slot $slot: $driver, seeds from $first, accelerator kvm, image $elf"
        "$root/helpers/qemu-bounded.sh" "$bound" ./helpers/qemu-runner-x86_64.sh "$elf" -accel kvm
    ) > "$log" 2>&1
    rc=$?
    set -e
    ms=$((($(date +%s) - start) * 1000))

    classify "$log" "$rc" "$first"

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

# The first seed of CI shard $1 on the commit at HEAD: 100,000 seeds per shard (a 60 s window runs
# about 27,000 at the measured rates), the commit's first 20 bits choosing among 20,000 blocks of four
# shards. The largest value is below 8.2e9, under the supervisor's `base`.
ci_first_seed() {
    head20="$(git -C "$root" rev-parse HEAD | cut -c1-5)"
    echo $((100000000 + (0x$head20 % 20000) * 400000 + $1 * 100000))
}

ci_shard_valid() {
    case "$1" in
        0|1|2|3) ;;
        *) echo "usage: $0 --ci SHARD (0 to 3)" >&2; return 2 ;;
    esac
}

# One CI shard. See the header. Every line printed to standard output is safe to publish.
ci_shard() {
    shard="$1"
    driver="$(slot_driver "$shard")"
    # shellcheck disable=SC2046  # two words, split on purpose
    set -- $(driver_test "$driver")
    filter="$1"
    var="$2"
    refuse_without_kvm || { echo "continuous-fuzz: shard $shard: infrastructure failure, no KVM" ; exit 3; }
    first="$(ci_first_seed "$shard")"
    commit="$(git -C "$root" rev-parse HEAD)"
    private="$(mktemp -d)"
    chmod 700 "$private"
    trap 'rm -rf "$private"' EXIT
    cd "$root"
    # The builds print compiler output only; no guest has run yet.
    "$root/helpers/qemu-bounded.sh" "$build_bound" cargo xtask initial-ramdisk-x86 \
        || { echo "continuous-fuzz: shard $shard: infrastructure failure, the archive did not build"; exit 3; }
    NIFE_INITIAL_RAMDISK="$PWD/target/initial-ramdisk-x86_64.img"
    NIFE_TEST_FILTER="$filter"
    NIFE_TEST_FILTER_ACROSS_IMAGES=1
    export NIFE_INITIAL_RAMDISK NIFE_TEST_FILTER NIFE_TEST_FILTER_ACROSS_IMAGES
    export "$var=$first:$ci_ask"
    "$root/helpers/qemu-bounded.sh" "$build_bound" cargo test -p system_tests --target x86_64-unknown-none \
        --no-run --message-format=json-render-diagnostics > "$private/build.json" \
        || { echo "continuous-fuzz: shard $shard: infrastructure failure, the test image did not build"; exit 3; }
    elf="$(sed -n '/"reason":"compiler-artifact"/{/"name":"system_tests"/s/.*"executable":"\([^"]*\)".*/\1/p;}' "$private/build.json" | tail -1)"
    [ -n "$elf" ] || { echo "continuous-fuzz: shard $shard: infrastructure failure, no test executable"; exit 3; }
    # From here the guest speaks, and only the private file hears it.
    set +e
    "$root/helpers/qemu-bounded.sh" "$bound" ./helpers/qemu-runner-x86_64.sh "$elf" -accel kvm \
        > "$private/guest.log" 2>&1
    rc=$?
    set -e
    classify "$private/guest.log" "$rc" "$first"
    case "$status" in
        ok)
            echo "continuous-fuzz: shard $shard ($driver) clean at $commit: $ran seeds from $first"
            ;;
        held-*)
            echo "continuous-fuzz: 1 finding in shard $shard at commit $commit; the fuzz host replays this shard"
            exit 1
            ;;
        *)
            echo "continuous-fuzz: shard $shard: infrastructure failure, the guest ended without a verdict (its output is withheld)"
            exit 3
            ;;
    esac
}

if [ "${1:-}" = --check-kvm ]; then
    require_kvm && echo "continuous-fuzz: KVM is usable"
    exit
fi

if [ "${1:-}" = --ci-range ]; then
    [ $# -eq 2 ] || { echo "usage: $0 --ci-range SHARD" >&2; exit 2; }
    ci_shard_valid "$2" || exit 2
    echo "$(slot_driver "$2") $(ci_first_seed "$2")"
    exit 0
fi

if [ "${1:-}" = --ci ]; then
    [ $# -eq 2 ] || { echo "usage: $0 --ci SHARD" >&2; exit 2; }
    ci_shard_valid "$2" || exit 2
    ci_shard "$2"
    exit 0
fi

if [ "${1:-}" = --once ]; then
    refuse_without_kvm || exit 1
    [ $# -eq 3 ] || { echo "usage: $0 --once DRIVER SLOT" >&2; exit 2; }
    mkdir -p "$state"
    window "$2" "$3"
    exit 0
fi

refuse_without_kvm || exit 1
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
