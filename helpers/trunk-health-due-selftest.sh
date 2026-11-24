#!/bin/sh
#
# Selftest: the DUE half of helpers/trunk-health.sh (milestone 495 (a cadence that says "due" reaches nobody))
# turns each way `script/stranger-test --due` can answer into the right
# row, and never into silence it cannot justify. Run by script/lint.
#
# The exit code is the authority and the sentence is the detail. A scratch copy of the tree holds
# stub scripts, so one real `--once` pass runs against canned answers, no network and no record:
#
#   - exit 1 with the DUE sentence: a DUE row in the three-column shape.
#   - exit 1 with no DUE sentence (a record that would not read): a `cannot ask` row.
#   - the script missing or not executable: the same `cannot ask` row, because the call fails.
#   - exit 1 with reworded text (`overdue:` for `DUE:`): `cannot ask`, never silence. This is the
#     guard against the scripts' wording drifting away from the sed this watcher reads them with.
#   - exit 0, even with DUE text on stdout: no row.

set -eu
me="trunk-health-due-selftest"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_CONFIG_PARAMETERS

tmp="$(mktemp -d "${TMPDIR:-/tmp}/nife-trunk-health-due-selftest.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
mkdir -p "$tmp/bin" "$tmp/tree/helpers" "$tmp/tree/script"
cp helpers/trunk-health.sh "$tmp/tree/helpers/trunk-health.sh"

# `gh` fails, which is fine: state() answers UNKNOWN and every other half is quiet or a different row.
printf '#!/bin/sh\nexit 1\n' >"$tmp/bin/gh"
printf '#!/bin/sh\ncase "$1" in remote) echo stub-origin;; ls-remote) printf "0000000000000000000000000000000000000000\\trefs/heads/main\\n";; *) exec /usr/bin/git "$@";; esac\n' >"$tmp/bin/git"
printf '#!/bin/sh\nexit 0\n' >"$tmp/tree/script/cadence-check"
chmod +x "$tmp/bin/gh" "$tmp/bin/git" "$tmp/tree/script/cadence-check"

# stub NAME EXIT [STDOUT...]: install script/NAME answering that way.
stub() {
	name="$1"; code="$2"; shift 2
	{ echo '#!/bin/sh'; for l in "$@"; do printf 'echo %s\n' "'$l'"; done; echo "exit $code"; } >"$tmp/tree/script/$name"
	chmod +x "$tmp/tree/script/$name"
}
pass() { PATH="$tmp/bin:$PATH" sh "$tmp/tree/helpers/trunk-health.sh" --once 2>/dev/null; }

row() { printf '%-32s %-24s %s\n' "$1" "$2" "$3"; }
want() { # want LABEL OUT ROW...
	label="$1"; out="$2"; shift 2
	for r in "$@"; do
		printf '%s\n' "$out" | grep -F -x -q -- "$r" || { echo "$me: $label: row absent: $r" >&2; printf '%s\n' "$out" >&2; exit 1; }
	done
}

stranger_unknown="$(row stranger-test "cannot ask" "DUE: unknown; script/stranger-test --check says what is wrong with the run record")"

# 1. exit 1 with the sentences: DUE rows, in the three-column shape.
stub stranger-test 1 'stranger-test: DUE. no run in 31 days' 'Last run: 4, 2026-09-14, somebody'
out="$(pass)"
want "due sentences" "$out" \
	"$(row stranger-test "last run 2026-09-14" "DUE: no run in 31 days")"

# 2. exit 1, no DUE sentence on stdout: cannot ask.
stub stranger-test 1
out="$(pass)"
want "no sentence" "$out" "$stranger_unknown"

# 3. missing, and present but not executable: the same cannot-ask rows.
rm "$tmp/tree/script/stranger-test"
out="$(pass)"
want "missing" "$out" "$stranger_unknown"
stub stranger-test 0
chmod -x "$tmp/tree/script/stranger-test"
out="$(pass)"
want "not executable" "$out" "$stranger_unknown"

# 4. wording drift on exit 1: cannot ask, never silence.
stub stranger-test 1 'stranger-test: overdue. no run in 31 days'
out="$(pass)"
want "wording drift" "$out" "$stranger_unknown"

# 5. exit 0 is authoritative even when stdout says DUE.
stub stranger-test 0 'stranger-test: DUE. no run in 31 days'
out="$(pass)"
if printf '%s\n' "$out" | grep -q "^stranger-test "; then
	echo "$me: exit 0 must produce no stranger-test row:" >&2; printf '%s\n' "$out" >&2; exit 1
fi

echo "$me: DUE rows, cannot-ask rows and exit-code authority all as expected"
