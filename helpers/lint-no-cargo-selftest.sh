#!/bin/sh
#
# helpers/lint-no-cargo-selftest.sh: prove helpers/lint-no-cargo.awk partitions script/lint's
# sections the way `script/lint --no-cargo` relies on, against fixtures with no tree and no cargo.
#
#     helpers/lint-no-cargo-selftest.sh
#
# Each case is a property the pre-push hook would lose silently if it broke:
#   1. a section that invokes cargo is dropped, in shell and in a Python argv;
#   1b. a section that starts cargo through the clippy recorder, or says it needs the cargo
#       sections' saved output, is dropped too (milestone 980 (script/lint stops paying per-item costs), part 6);
#   2. prose that only mentions cargo is kept (a false drop shrinks the hook without a signal);
#   3. a section that opts in with the marker and the guard is kept;
#   4. a marker on a section with no cargo, and a marker with no guard, fail;
#   5. cargo before the first header fails, because the preamble cannot be dropped;
#   6. the real script/lint partitions, and `--list`, `emit` and `split` agree on the sections kept.

set -e
cd "$(dirname "$0")/.."
awk_file=helpers/lint-no-cargo.awk
dir="$(mktemp -d "${TMPDIR:-/tmp}/lint-no-cargo.XXXXXX")"
trap 'rm -rf "$dir"' EXIT

fail() { echo "lint-no-cargo-selftest: $1" >&2; exit 1; }
run() { awk -v mode="$1" -f "$awk_file" "$2"; }

cat >"$dir/ok.sh" <<'FIXTURE'
set -e
echo "==> shell cargo"
cargo clippy --workspace
echo "==> python cargo"
python3 - <<'PY'
import subprocess
subprocess.run(["cargo", "metadata"])
PY
echo "==> recorded clippy"
python3 helpers/clippy_record.py host cargo clippy --workspace
echo "==> reads what the cargo sections saved"
# needs-cargo-output: this one runs no cargo but judges what the sections above saved
python3 helpers/lint_ratchet.py --check target/lint-clippy
echo "==> prose only"
# cargo clippy is only named in this comment
echo "the word cargo appears here"
echo "==> opted in"
# no-cargo-ok: only the tail needs cargo
if [ -n "$LINT_NO_CARGO" ]; then exit 0; fi
cargo metadata
FIXTURE
got="$(run list "$dir/ok.sh" | tr -s ' ' | tr '\n' ';')"
want="skip ==> shell cargo;skip ==> python cargo;skip ==> recorded clippy;skip ==> reads what the cargo sections saved;run ==> prose only;run ==> opted in;"
[ "$got" = "$want" ] || fail "partition was '$got', wanted '$want'"
run emit "$dir/ok.sh" | grep -q 'cargo clippy --workspace' && fail "emit kept a cargo section"
run emit "$dir/ok.sh" | grep -q 'the word cargo appears' || fail "emit dropped a prose-only section"

printf 'echo "==> plain"\n# no-cargo-ok: nothing needs it\necho LINT_NO_CARGO\n' >"$dir/stale.sh"
run list "$dir/stale.sh" >/dev/null 2>&1 && fail "a marker on a section with no cargo passed"

printf 'echo "==> unguarded"\n# no-cargo-ok: forgot the guard\ncargo metadata\n' >"$dir/unguarded.sh"
run list "$dir/unguarded.sh" >/dev/null 2>&1 && fail "a marker with no guard passed"

printf 'cargo fmt\necho "==> later"\n' >"$dir/preamble.sh"
run list "$dir/preamble.sh" >/dev/null 2>&1 && fail "cargo in the preamble passed"

real="$(run list script/lint)" || fail "script/lint itself does not partition"
kept="$(printf '%s\n' "$real" | grep -c '^run')"
emitted="$(run emit script/lint | grep -c '^echo "==> ')"
[ "$kept" = "$emitted" ] || fail "--list keeps $kept sections and emit keeps $emitted"
[ "$kept" -gt 0 ] || fail "script/lint has no section that runs without cargo"
mkdir "$dir/split"
awk -v mode=split -v dir="$dir/split" -f "$awk_file" script/lint || fail "script/lint does not split"
split_kept="$(awk -F '\t' '$2 == 1' "$dir/split/index" | wc -l | tr -d ' ')"
[ "$split_kept" = "$kept" ] || fail "split keeps $split_kept sections and --list keeps $kept"
echo "lint-no-cargo-selftest: ok"
