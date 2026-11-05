#!/bin/sh
# helpers/verify-stub-selftest.sh: a stubbed `script/verify` run never touches the machine's Kani pool.
#
# script/verify waits for Kani units before it runs a harness (helpers/heavy_slot.py, "The Kani
# pool"). A stub `cargo` proves nothing and needs no memory, so a lint section that drives
# script/verify over one must turn the pool off (NIFE_KANI_POOL=off), or it idles at 0 percent CPU
# for as long as another lane's real proof holds the pool. This file is that section and its
# regression test at once:
#
#   1. extra arguments: `script/verify --only capability --harness X` hands `--harness X` to
#      `cargo kani` (once dropped by an option-parsing `set --`, found 2026-10-05).
#   2. the pool is held in full by a fake holder, taken with the tool's own mechanism in a scratch
#      NIFE_HEAVY_SLOT_DIR, and the stubbed run still finishes under a deadline. Fails if the
#      stubbed run waits for Kani units. A stubbed run with the pool left on is also run under the
#      same deadline and must NOT finish, so the holder is proved to hold (a pass of stage 2 means
#      something).
#
# Both stages use the scratch slot directory, so neither waits on, nor takes a unit of, a real job.
# Run by script/lint.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/nife-verify-stub-selftest.XXXXXX")"
holder=""
cleanup() {
	[ -z "$holder" ] || kill "$holder" 2>/dev/null || true
	rm -rf "$tmp"
}
trap cleanup EXIT HUP INT TERM
fail() { echo "verify-stub-selftest: $*" >&2; exit 1; }

mkdir -p "$tmp/bin" "$tmp/slots"
cat >"$tmp/bin/cargo" <<'STUB'
#!/bin/sh
# Stub: `cargo kani --version` succeeds; any other call is recorded.
[ "$1" = kani ] && [ "$2" = "--version" ] && { echo "cargo-kani $(cat .kani-version)"; exit 0; }
printf '%s\n' "$*" >>"$VERIFY_STUB_LOG"
STUB
chmod +x "$tmp/bin/cargo"

unset LINT_NO_CARGO GITHUB_ACTIONS NIFE_KANI_POOL NIFE_KANI_UNITS
NIFE_HEAVY_SLOT_DIR="$tmp/slots"
NIFE_HEAVY_SLOTS=16
export NIFE_HEAVY_SLOT_DIR NIFE_HEAVY_SLOTS PATH="$tmp/bin:$PATH"

# A two-unit pool, held in full by a holder that takes both units and sleeps.
NIFE_KANI_UNITS=2
export NIFE_KANI_UNITS
python3 helpers/heavy_slot.py --pool kani --units 2 verify-stub-selftest-holder -- sleep 600 &
holder=$!
i=0
until python3 helpers/heavy_slot.py --status 2>/dev/null | grep -q verify-stub-selftest-holder; do
	i=$((i + 1))
	[ "$i" -le 100 ] || fail "the fake holder never took the pool"
	sleep 0.1
done

# bounded <seconds> <command...>: exit 124 when the command is still running at the deadline.
bounded() {
	secs="$1"
	shift
	"$@" &
	pid=$!
	n=0
	while kill -0 "$pid" 2>/dev/null; do
		n=$((n + 1))
		if [ "$n" -gt $((secs * 10)) ]; then
			kill "$pid" 2>/dev/null || true
			wait "$pid" 2>/dev/null || true
			return 124
		fi
		sleep 0.1
	done
	wait "$pid"
}

stubbed_verify() {
	VERIFY_STUB_LOG="$tmp/log" script/verify --only capability --harness lint_probe_harness >/dev/null 2>&1
}

# The control: with the pool left on, the same run waits behind the holder. If this finished, the
# holder holds nothing and the stage below proves nothing.
: >"$tmp/log"
rc=0
bounded 3 stubbed_verify || rc=$?
[ "$rc" -eq 124 ] || fail "control: a stubbed run with the pool on finished (exit $rc) though the pool is held in full"

# 1 and 2. The pool off, as the lint section runs it: finishes in seconds and hands --harness on.
: >"$tmp/log"
rc=0
NIFE_KANI_POOL=off bounded 60 stubbed_verify || rc=$?
[ "$rc" -ne 124 ] || fail "a stubbed run with the pool off waited for the held Kani pool"
if ! grep -q '^kani -p capability .*--harness lint_probe_harness' "$tmp/log" 2>/dev/null; then
	echo "verify-stub-selftest: script/verify --only capability --harness X did not hand --harness X to cargo kani:" >&2
	sed 's/^/  /' "$tmp/log" >&2 2>/dev/null || echo "  (stub cargo was never called)" >&2
	exit 1
fi
echo "script/verify: extra arguments reach cargo kani, and a stubbed run takes no Kani unit"
