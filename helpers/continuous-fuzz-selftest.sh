#!/bin/sh
#
# Selftest for the toolchain step of helpers/continuous-fuzz.sh, run by script/lint. A stub `rustup`
# in a throwaway $HOME/.cargo/bin (the supervisor puts that directory first on PATH) records its
# calls, so no download happens and no toolchain is needed. It proves three things: the supervisor
# installs the pinned toolchain exactly once before it looks at KVM or starts a slot; a failed
# install stops it with its own message; and a failed install writes no window row. Without the
# step, the failing-install case below reaches the KVM refusal instead and the check fails. The
# cases after it hold a window's verdict, a slot's seed file, and the disk bounds (the free-space
# floor and the build directory's clears) against stub `df` and `du`.

set -eu
here="$(cd "$(dirname "$0")" && pwd)"
sup="$here/continuous-fuzz.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fail() { echo "continuous-fuzz-selftest: $*" >&2; exit 1; }

mkdir -p "$work/home/.cargo/bin"
cat > "$work/home/.cargo/bin/rustup" <<'STUB'
#!/bin/sh
echo "$*" >> "$STUB_CALLS"
[ "${STUB_FAIL:-0}" = 1 ] || exit 0
echo "error: stub toolchain install failed" >&2
exit 1
STUB
chmod +x "$work/home/.cargo/bin/rustup"

# run FAIL: run the supervisor against a stub rustup and a KVM device that does not exist, so that
# a supervisor which got past the toolchain step stops at the KVM refusal and starts no guest.
run() {
    rm -f "$work/calls"
    : > "$work/calls"
    set +e
    out=$(HOME="$work/home" NIFE_FUZZ_HOME="$work/fuzz" NIFE_FUZZ_KVM_DEVICE="$work/no-such-device" \
        STUB_CALLS="$work/calls" STUB_FAIL="$1" NIFE_RUSTUP_ATTEMPTS=1 NIFE_RUSTUP_BACKOFF=0 \
        "$sup" 2>&1)
    rc=$?
    set -e
    calls=$(wc -l < "$work/calls" | tr -d ' ')
}

run 0
[ "$calls" -eq 1 ] || fail "install ok: $calls rustup calls, wanted exactly 1 ($out)"
grep -q '^show active-toolchain$' "$work/calls" || fail "install ok: wanted \`rustup show active-toolchain\`, got $(cat "$work/calls")"
[ "$rc" -ne 0 ] || fail "install ok: exit 0 with no KVM"
printf '%s' "$out" | grep -q 'KVM is not usable' || fail "install ok: did not go on to the KVM check ($out)"

run 1
[ "$rc" -ne 0 ] || fail "install fails: exit 0"
[ "$calls" -eq 1 ] || fail "install fails: $calls rustup calls, wanted 1"
printf '%s' "$out" | grep -q 'could not install the toolchain' || fail "install fails: no clear message ($out)"
printf '%s' "$out" | grep -q 'KVM is not usable' && fail "install fails: went on to the KVM check ($out)"
[ ! -e "$work/fuzz/state/windows.tsv" ] || fail "install fails: a window row was written"

# The class of a window. A wait that ran out its tick budget is held-budget-expired whether or not
# its message carries a replay line; a mismatch, a crash and a hang keep their own classes; a clean
# window stays ok. The marker is the one the guest prints (kernel/src/testing.rs).
marker=$(sed -n 's/^pub const BUDGET_EXPIRED_MARKER: &str = "\(.*\)";$/\1/p' "$here/../kernel/src/testing.rs")
[ "$marker" = "budget-expired:" ] || fail "the guest's budget marker is '$marker', the supervisor greps 'budget-expired:'"

verdict() { # LOG-TEXT RC FIRST
    printf '%s\n' "$1" > "$work/log"
    "$sup" --classify "$work/log" "$2" "$3"
}
want() { # NAME GOT WANT
    [ "$2" = "$3" ] || fail "$1: got '$2', wanted '$3'"
}

want "clean" "$(verdict 'system call fuzzer: 50 seeds, 900 ms (x)' 0 100)" "ok 50 150"
want "mismatch" "$(verdict 'system call fuzzer: seed 0x6f step 3: a slot differs (replay: NIFE_SYSTEM_CALL_FUZZ_SEEDS=111:1)' 1 100)" "held-finding 11 112"
want "teardown budget" "$(verdict "system call fuzzer: seed 0x6f step 3: $marker teardown should have woken actor 1 (replay: NIFE_SYSTEM_CALL_FUZZ_SEEDS=111:1)" 1 100)" "held-budget-expired 11 112"
want "confined budget" "$(verdict "confined fuzzer: seed 0x6f step 3: $marker fuzzer 0 never came back (replay: NIFE_CONFINED_FUZZ_SEEDS=111:1)" 1 100)" "held-budget-expired 11 112"
want "budget panic" "$(verdict "panicked at x: $marker actor 2 neither finished nor parked" 1 100)" "held-budget-expired 0 101"
want "crash" "$(verdict 'panicked at x: index out of bounds' 1 100)" "held-crash 0 101"
want "hang" "$(verdict 'booting' 143 100)" "held-hang 0 101"
# A guest a signal ended before its bound did not hang: sweep says so, and the window is
# infrastructure. The line is the supervisor's own (`early_stop`), so the two cannot drift.
early=$(sed -n 's/^early_stop="\(.*\)"$/\1/p' "$sup")
[ -n "$early" ] || fail "the supervisor defines no early_stop line"
want "stopped early" "$(verdict "booting
continuous-fuzz: $early after 41 s, short of its 300 s bound" 143 100)" "infrastructure 0 101"
want "killed early" "$(verdict "booting
continuous-fuzz: $early after 3 s, short of its 300 s bound" 137 100)" "infrastructure 0 101"
# A guest that ran out its bound hung, whatever its exit status: the runner hands the bound's
# TERM to QEMU, and QEMU then exits 0.
ran_out=$(sed -n 's/^bound_ran_out="\(.*\)"$/\1/p' "$sup")
[ -n "$ran_out" ] || fail "the supervisor defines no bound_ran_out line"
want "bound ran out" "$(verdict "booting
continuous-fuzz: $ran_out after 300 s" 0 100)" "held-hang 0 101"
want "infrastructure" "$(verdict 'booting' 125 100)" "infrastructure 0 101"

# Each driver's held log names its own seed variable and no other. The supervisor sets one variable
# per window (`driver_test`), the driver reads it with `option_env!` and prints it on its replay
# line, and the filer refuses a log whose replay line names another driver's variable (partsfoundry/nife
# issue 3921). For each driver in `driver_test`, find the source file that defines its test, and
# check that the file reads that variable, prints a replay line with it, and names no other
# `NIFE_*_FUZZ_SEEDS` variable in either place.
sources="$here/../system_tests/src"
drivers=$(sed -n 's/^ *\([a-z-]*\)) echo "[a-z_]* NIFE_[A-Z_]*_FUZZ_SEEDS" ;;$/\1/p' "$sup")
[ "$(echo "$drivers" | wc -l | tr -d ' ')" -eq 2 ] || fail "driver_test names '$drivers', wanted two drivers"
for driver in $drivers; do
    pair=$(sed -n "s/^ *$driver) echo \"\([a-z_]* NIFE_[A-Z_]*_FUZZ_SEEDS\)\" ;;\$/\1/p" "$sup")
    filter=${pair% *}
    var=${pair#* }
    file=$(grep -rl -E "^fn ${filter}[a-z_]*\(\)" "$sources" || true)
    [ "$(echo "$file" | grep -c .)" -eq 1 ] || fail "$driver: wanted one file defining a test that starts '$filter', found '$file'"
    grep -q "option_env!(\"$var\")" "$file" || fail "$driver: $file does not read $var"
    grep -q "(replay: $var={}:1)" "$file" || fail "$driver: $file prints no replay line naming $var"
    other=$(grep -o -E 'option_env!\("NIFE_[A-Z_]*_FUZZ_SEEDS"\)|replay: NIFE_[A-Z_]*_FUZZ_SEEDS=' "$file" \
        | grep -v -F -e "option_env!(\"$var\")" -e "replay: $var=" || true)
    [ -z "$other" ] || fail "$driver: $file names another variable: $other"
    # The supervisor's own reading of the replay line takes the seed from this driver's form.
    want "$driver replay line" "$(verdict "x (replay: $var=111:1)" 1 100)" "held-finding 11 112"
done

# A slot's seed file. One window runs through `--once` against stubs: `/dev/null` stands in for
# KVM (a character device anyone may open), a stub QEMU passes the KVM probe, and a stub cargo fails
# the build, so the window is infrastructure within a second and no guest starts. The slot's
# worktree is a throwaway repository made here, so `prepare_slot` adds no worktree to this clone.
cat > "$work/home/.cargo/bin/qemu-system-x86_64" <<'STUB'
#!/bin/sh
exit 0
STUB
cat > "$work/home/.cargo/bin/cargo" <<'STUB'
#!/bin/sh
exit 1
STUB
chmod +x "$work/home/.cargo/bin/qemu-system-x86_64" "$work/home/.cargo/bin/cargo"
seeds="$work/seeds"
mkdir -p "$seeds/work/0"
git -C "$seeds/work/0" init -q
git -C "$seeds/work/0" -c user.name=selftest -c user.email=selftest@invalid commit -q --allow-empty -m selftest
once() { # SEED-FILE-TEXT
    rm -rf "$seeds/state" "$seeds/logs"
    mkdir -p "$seeds/state"
    printf '%s' "$1" > "$seeds/state/next-0"
    set +e
    out=$(HOME="$work/home" NIFE_FUZZ_HOME="$seeds" NIFE_FUZZ_KVM_DEVICE=/dev/null \
        STUB_CALLS=/dev/null NIFE_RUSTUP_ATTEMPTS=1 NIFE_RUSTUP_BACKOFF=0 \
        "$sup" --once kernel-thread 0 2>&1)
    rc=$?
    set -e
}

# A good seed: the window runs, writes one row with that first seed, and moves the slot one on.
once 12345
[ "$rc" -eq 0 ] || fail "good seed: exit $rc ($out)"
want "good seed row" "$(cut -f3,4,7 "$seeds/state/windows.tsv")" "0	12345	infrastructure"
want "good seed next" "$(cat "$seeds/state/next-0")" "12346"

# An empty seed file, as every slot's was after the write below failed on the host: the slot stops
# with a message, writes no row, and leaves the file as it found it. Before the fix the window read
# the empty file as seed 0, wrote a row with an empty first seed and set the file to 1.
for bad in '' 'x12' '12
13'; do
    once "$bad"
    [ "$rc" -ne 0 ] || fail "seed '$bad': exit 0 ($out)"
    printf '%s' "$out" | grep -q 'refusing to start a window' || fail "seed '$bad': no refusal ($out)"
    [ ! -e "$seeds/state/windows.tsv" ] || fail "seed '$bad': a window row was written: $(cat "$seeds/state/windows.tsv")"
    want "seed '$bad' file" "$(cat "$seeds/state/next-0")" "$bad"
done

# A write that fails after the shell has opened the file, as the host's did with an I/O error:
# `ulimit -f 0` makes every write fail, and `--write-seed` is the step a window ends with. An
# `echo > file` there leaves the file empty; the seed must stay as it was.
printf '12345\n' > "$seeds/state/next-0"
set +e
out=$(trap '' XFSZ; ulimit -f 0; NIFE_FUZZ_HOME="$seeds" "$sup" --write-seed 0 12346 2>&1)
rc=$?
set -e
[ "$rc" -ne 0 ] || fail "failed write: exit 0 ($out)"
printf '%s' "$out" | grep -q 'could not record its next first seed' || fail "failed write: no message ($out)"
want "failed write file" "$(cat "$seeds/state/next-0")" "12345"
[ ! -e "$seeds/state/next-0.tmp" ] || fail "failed write: the temporary file was left behind"
NIFE_FUZZ_HOME="$seeds" "$sup" --write-seed 0 12346 || fail "write: exit non-zero"
want "write file" "$(cat "$seeds/state/next-0")" "12346"

# The disk bounds. A stub `df` reports the free space in $STUB_FREE_KIB and a stub `du` the size of a
# build directory in $STUB_SIZE_KIB, both in 1024-byte blocks, so no test fills a disk.
disk="$work/disk"
mkdir -p "$disk/home/.cargo/bin" "$disk/fuzz/work/0"
cp "$work/home/.cargo/bin/rustup" "$work/home/.cargo/bin/qemu-system-x86_64" "$disk/home/.cargo/bin/"
cat > "$disk/home/.cargo/bin/df" <<'STUB'
#!/bin/sh
echo "Filesystem 1024-blocks Used Available Capacity Mounted on"
echo "/dev/stub 100000000 1 $(cat "$STUB_FREE_KIB") 1% /"
STUB
cat > "$disk/home/.cargo/bin/du" <<'STUB'
#!/bin/sh
printf '%s\t%s\n' "$(cat "$STUB_SIZE_KIB")" "$2"
STUB
# A stub `python3` stands in for both looks and records that one ran.
cat > "$disk/home/.cargo/bin/python3" <<'STUB'
#!/bin/sh
echo "$*" >> "$STUB_LOOKS"
echo "stub look"
exit 1
STUB
chmod +x "$disk/home/.cargo/bin/df" "$disk/home/.cargo/bin/du" "$disk/home/.cargo/bin/python3"
kib() { echo $(($1 * 1000000000 / 1024)); }  # GIGABYTES, as the supervisor counts them
# With `disk_exec=exec` (in a subshell started with `&`), the supervisor replaces that subshell, so
# `$!` is the supervisor's own pid and `kill` reaches it rather than orphaning it.
disk_exec=""
disk_env() {
    $disk_exec env HOME="$disk/home" NIFE_FUZZ_HOME="$disk/fuzz" STUB_CALLS=/dev/null STUB_LOOKS="$disk/looks" \
        STUB_FREE_KIB="$disk/free" STUB_SIZE_KIB="$disk/size" NIFE_RUSTUP_ATTEMPTS=1 NIFE_RUSTUP_BACKOFF=0 "$@"
}

# The floor: 20 GB by default, and the variable moves it.
kib 20 > "$disk/free"
disk_env "$sup" --has-room >/dev/null || fail "floor: 20 GB free counts as below the default floor"
echo $(($(kib 20) - 1)) > "$disk/free"
disk_env "$sup" --has-room >/dev/null && fail "floor: just under 20 GB free counts as room"
disk_env NIFE_FUZZ_FREE_FLOOR_GIGABYTES=19 "$sup" --has-room >/dev/null || fail "floor: the variable does not lower it"
disk_env NIFE_FUZZ_FREE_FLOOR_GIGABYTES=20GB "$sup" --has-room >/dev/null 2>&1 && fail "floor: a floor that is not a number was taken"

# Below the floor a window refuses to start and writes no row, before it looks at the seed file.
once_low=$(HOME="$work/home" NIFE_FUZZ_HOME="$seeds" NIFE_FUZZ_KVM_DEVICE=/dev/null STUB_CALLS=/dev/null \
    NIFE_RUSTUP_ATTEMPTS=1 NIFE_RUSTUP_BACKOFF=0 NIFE_FUZZ_FREE_FLOOR_GIGABYTES=1000000000 \
    "$sup" --once kernel-thread 0 2>&1) && fail "floor: a window started below it ($once_low)"
printf '%s' "$once_low" | grep -q 'below the 1000000000 GB floor' || fail "floor: no refusal ($once_low)"
[ ! -e "$seeds/state/windows.tsv" ] || fail "floor: a window row was written"

# The supervisor below the floor, with no slots so no guest can start: it writes one line however
# many times it checks, runs neither look, and writes one more line when the space comes back.
echo 0 > "$disk/free"
: > "$disk/looks"
(disk_exec='exec'; disk_env NIFE_FUZZ_GUESTS=0 NIFE_FUZZ_KVM_DEVICE=/dev/null NIFE_FUZZ_FOLLOW_EVERY=1 \
    NIFE_FUZZ_REPLAY_EVERY=1 "$sup" > "$disk/journal" 2>&1) &
supervisor=$!
sleep 3
[ ! -s "$disk/looks" ] || { kill "$supervisor"; fail "floor: a look ran below it: $(cat "$disk/looks")"; }
kib 100 > "$disk/free"
sleep 2
kill "$supervisor" 2>/dev/null || true
wait "$supervisor" 2>/dev/null || true
want "floor: lines while low" "$(grep -c 'below the 20 GB floor' "$disk/journal")" 1
want "floor: lines once back" "$(grep -c 'back above the 20 GB floor' "$disk/journal")" 1
[ -s "$disk/looks" ] || fail "floor: no look ran once the space came back ($(cat "$disk/journal"))"

# The build directory. A slot's first bound records its pin and keeps nothing it has not built
# against that pin; the same pin keeps `target/`; a newer pin clears it; a size past the cap clears it.
slot0="$disk/fuzz/work/0"
pin() { printf '[toolchain]\nchannel = "%s"\n' "$1" > "$slot0/rust-toolchain.toml"; }
bound() { disk_env "$sup" --bound-target 0 2>&1; }
echo 1 > "$disk/size"
pin nightly-2026-01-01
mkdir -p "$slot0/target" && : > "$slot0/target/artifact"
bound >/dev/null
[ ! -e "$slot0/target" ] || fail "toolchain: a target/ of an unknown toolchain was kept"
want "toolchain mark" "$(cat "$disk/fuzz/state/toolchain-0")" nightly-2026-01-01
mkdir -p "$slot0/target" && : > "$slot0/target/artifact"
said=$(bound)
[ -e "$slot0/target/artifact" ] || fail "toolchain: the same pin cleared target/ ($said)"
[ -z "$said" ] || fail "toolchain: the same pin said '$said'"
pin nightly-2026-01-02
said=$(bound)
[ ! -e "$slot0/target" ] || fail "toolchain: a newer pin kept target/"
printf '%s' "$said" | grep -q 'slot 0: cleared target/ before its next build: its toolchain is now nightly-2026-01-02' \
    || fail "toolchain: no line for the clear ($said)"
want "toolchain mark after a move" "$(cat "$disk/fuzz/state/toolchain-0")" nightly-2026-01-02
mkdir -p "$slot0/target" && : > "$slot0/target/artifact"
kib 10 > "$disk/size"
bound >/dev/null
[ -e "$slot0/target/artifact" ] || fail "cap: a target/ at the 10 GB cap was cleared"
echo $(($(kib 10) + 1)) > "$disk/size"
said=$(bound)
[ ! -e "$slot0/target" ] || fail "cap: a target/ past the 10 GB cap was kept"
printf '%s' "$said" | grep -q 'past the 10 GB cap' || fail "cap: no line for the clear ($said)"
mkdir -p "$slot0/target" && : > "$slot0/target/artifact"
disk_env NIFE_FUZZ_TARGET_CAP_GIGABYTES=11 "$sup" --bound-target 0 >/dev/null 2>&1
[ -e "$slot0/target/artifact" ] || fail "cap: the variable does not raise it"

echo "continuous-fuzz-selftest: ok"
