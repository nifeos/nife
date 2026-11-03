#!/bin/sh
#
# helpers/replay-line-check-selftest.sh: prove helpers/replay-line-check.sh refuses each form of a
# reproducer and passes clean text, including the prose that names the forms. The planted values are
# invented and replay nothing.
#
#     helpers/replay-line-check-selftest.sh

set -e
cd "$(dirname "$0")/.."
check="$(pwd)/helpers/replay-line-check.sh"
dir="$(mktemp -d "${TMPDIR:-/tmp}/replay-line-check-selftest.XXXXXX")"
trap 'rm -rf "$dir"' EXIT
fail() { echo "replay-line-check-selftest: $1" >&2; exit 1; }

# expect <pass|refuse> <label> <text>
expect() {
    printf '%s\n' "$3" | "$check" >"$dir/out" 2>"$dir/err" && got=pass || got=refuse
    [ "$got" = "$1" ] || fail "$2: wanted $1, got $got"
    # A refusal names a line number and never repeats the text.
    if [ "$1" = refuse ]; then
        grep -q 'line [0-9]' "$dir/err" || fail "$2: the refusal does not name a line"
        ! grep -q 'planted' "$dir/err" || fail "$2: the refusal repeated the text"
    fi
}

# Rule 1, a replay line.
expect refuse "plain replay line" "fix the thing

replay: NIFE_SYSCALL_FUZZ_SEEDS=planted:1"
expect refuse "replay line, upper case" "REPLAY: planted"
expect refuse "replay line, quoted" "> replay: planted"
expect refuse "replay line, in a list" "- replay: planted"
expect refuse "replay line, in backticks" '`replay: planted`'
expect refuse "replay line, indented" "    replay: planted"
# Rule 2, a seed assignment.
expect refuse "assignment" "run it with NIFE_CONFINED_FUZZ_SEEDS=planted"
expect refuse "export" "export NIFE_SYSCALL_FUZZ_SEEDS=planted:1"
expect refuse "single seed name" "NIFE_X_SEED=planted"
expect refuse "spaced assignment" "NIFE_SYSCALL_FUZZ_SEEDS = planted"
expect refuse "assignment on a later line" "title

body line
more NIFE_SYSCALL_FUZZ_SEEDS=planted at the end"
# Clean text, including prose that names the forms.
expect pass "plain message" "the allocator now rejects a zero-length extent

Co-Authored-By: Someone <noreply@example.invalid>"
expect pass "family named with a star" "the gate refuses a NIFE_*_SEEDS= assignment in a body"
expect pass "variable named without an equals sign" "NIFE_SYSCALL_FUZZ_SEEDS is read by the driver"
expect pass "replay mid-sentence" "the host will replay: it reruns the shard"
expect pass "replayed word" "replayed: the shard was rerun"
expect pass "other NIFE variables" "set NIFE_FUZZ_HOME=/srv/fuzz"
expect pass "empty text" ""

# --range: a commit that carries a replay line is named by short id, a clean range passes.
git init -q "$dir/repo"
git -C "$dir/repo" -c user.name=t -c user.email=t@example.invalid commit -q --allow-empty -m "base"
base="$(git -C "$dir/repo" rev-parse HEAD)"
git -C "$dir/repo" -c user.name=t -c user.email=t@example.invalid commit -q --allow-empty -m "clean one"
(cd "$dir/repo" && "$check" --range "$base..HEAD") >/dev/null 2>&1 || fail "range: a clean range was refused"
git -C "$dir/repo" -c user.name=t -c user.email=t@example.invalid commit -q --allow-empty -m "fix

replay: planted"
bad="$(git -C "$dir/repo" rev-parse --short HEAD)"
(cd "$dir/repo" && "$check" --range "$base..HEAD") >/dev/null 2>"$dir/err" && fail "range: a planted replay line passed"
grep -q "$bad" "$dir/err" || fail "range: the refusal does not name the commit"

echo "replay-line-check-selftest: ok"
