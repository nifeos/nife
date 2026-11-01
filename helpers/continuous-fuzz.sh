#!/bin/sh
# helpers/continuous-fuzz.sh: run the two x86_64 in-guest fuzz drivers for as long as the machine is
# up, a few guests at a time. The systemd unit helpers/systemd/nife-fuzz.service runs it.
#
#     helpers/continuous-fuzz.sh              run forever, 4 guests (NIFE_FUZZ_GUESTS)
#     helpers/continuous-fuzz.sh --once DRIVER SLOT   one sweep window, then print the verdict
#     helpers/continuous-fuzz.sh --check-kvm  say whether a window could start, and why not
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
# **The toolchain is installed first, once.** Before any slot starts, the supervisor runs
# `ensure_toolchain` (below), so a newer pin than the account has installed is fetched by one process
# and not raced by four. A failure there exits before any slot and writes no row to windows.tsv.
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

# Install (or confirm) the toolchain the repository pins, once, before any slot starts. With no
# argument `rustup toolchain install` reads rust-toolchain.toml, so the pin has one home. Four slots
# that each found the pin missing downloaded it at the same time and collided; the first windows
# were recorded as infrastructure failures three seconds after start. This runs serially in the
# supervisor, writes no window rows, and a failure stops the supervisor with its own message.
ensure_toolchain() {
    echo "continuous-fuzz: installing or confirming the pinned toolchain before any slot starts"
    (cd "$root" && "$root/helpers/rustup-retry.sh" toolchain install) >&2 || {
        echo "continuous-fuzz: refusing to start: could not install the toolchain that rust-toolchain.toml pins; no slot was started and no window was recorded" >&2
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

if [ "${1:-}" = --check-kvm ]; then
    require_kvm && echo "continuous-fuzz: KVM is usable"
    exit
fi

if [ "${1:-}" = --once ]; then
    ensure_toolchain || exit 1
    refuse_without_kvm || exit 1
    [ $# -eq 3 ] || { echo "usage: $0 --once DRIVER SLOT" >&2; exit 2; }
    mkdir -p "$state"
    window "$2" "$3"
    exit 0
fi

ensure_toolchain || exit 1
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
