#!/bin/sh
#
# helpers/lint-sections-selftest.sh: prove helpers/lint-sections.sh gives the verdict a fail-fast
# run gives, while naming every failure instead of the first.
#
#     helpers/lint-sections-selftest.sh
#
# A fixture lint script has four sections that fail on demand (a test, an `exit 3`, a plain
# command under `set -e`, and one that runs a stubbed cargo) and one that reads a file an earlier
# section wrote. For each of the 16 combinations of planted failures, in both `all` and `no-cargo`
# mode, five things must hold:
#   1. the helper exits non-zero exactly when the fixture run whole, as one `set -e` script, does;
#   2. its summary names exactly the planted sections that run in that mode;
#   3. the last section runs whatever failed before it;
#   4. every section that runs prints its lint-section-time line and appends one CSV row per
#      section to the file NIFE_LINT_TIMES names, the host column second naming the machine that
#      wrote the row, the section's exit last, and the section the mode drops prints neither;
#   5. a CSV file that cannot be written leaves the verdict and the output alone.
# No tree and no real cargo: it runs in milliseconds, and in `script/lint --no-cargo`.

set -eu
cd "$(dirname "$0")/.."
work="$(mktemp -d "${TMPDIR:-/tmp}/lint-sections-selftest.XXXXXX")"
trap 'rm -rf "$work"' EXIT
fail() { echo "lint-sections-selftest: $1" >&2; exit 1; }

mkdir "$work/bin"
printf '#!/bin/sh\n[ -z "${PLANT_D:-}" ]\n' >"$work/bin/cargo"
chmod +x "$work/bin/cargo"
PATH="$work/bin:$PATH"
OUT="$work/out"
export OUT
# The CSV rows land beside the fixture, through the override, so a selftest never writes the
# machine-local file a real lint run uses.
TIMES="$work/times.csv"
NIFE_LINT_TIMES="$TIMES"
export NIFE_LINT_TIMES

cat >"$work/lint" <<'FIXTURE'
set -e
shared=from-the-preamble
echo "==> a"
[ -z "${PLANT_A:-}" ]
echo "==> b reads the preamble"
[ "$shared" = from-the-preamble ]
[ -z "${PLANT_B:-}" ] || exit 3
echo "==> c writes a file"
echo built >"$OUT/built"
if [ -n "${PLANT_C:-}" ]; then false; fi
echo "==> d, the cargo one"
cargo clippy
echo "==> e reads what c wrote"
[ -f "$OUT/built" ]
echo "e ran"
FIXTURE

# What `script/lint --no-cargo` ran before the helper: the kept sections as one script.
awk -v mode=emit -f helpers/lint-no-cargo.awk "$work/lint" >"$work/lint-no-cargo"

for combo in 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
    unset PLANT_A PLANT_B PLANT_C PLANT_D
    planted=""
    [ $((combo & 1)) -eq 0 ] || { PLANT_A=1; export PLANT_A; planted="$planted a"; }
    [ $((combo & 2)) -eq 0 ] || { PLANT_B=1; export PLANT_B; planted="$planted b"; }
    [ $((combo & 4)) -eq 0 ] || { PLANT_C=1; export PLANT_C; planted="$planted c"; }
    [ $((combo & 8)) -eq 0 ] || { PLANT_D=1; export PLANT_D; planted="$planted d"; }
    for mode in all no-cargo; do
        if [ "$mode" = all ]; then whole="$work/lint"; want="$planted"; else
            whole="$work/lint-no-cargo"; want="$(printf '%s' "$planted" | sed 's/ d//')"; fi

        rm -rf "$OUT"; mkdir "$OUT"
        set +e
        LINT_NO_CARGO=1 sh "$whole" >/dev/null 2>&1
        fast=$?
        rm -rf "$OUT"; mkdir "$OUT"
        rows_before=0
        if [ -f "$TIMES" ]; then rows_before=$(wc -l <"$TIMES" | tr -d ' '); fi
        out="$(sh helpers/lint-sections.sh "$work/lint" "$mode" 2>&1)"
        keep=$?
        rows_after=0
        if [ -f "$TIMES" ]; then rows_after=$(wc -l <"$TIMES" | tr -d ' '); fi
        set -e

        [ $((fast != 0)) -eq $((keep != 0)) ] \
            || fail "planted [$planted] $mode: fail-fast exit $fast, keep-going exit $keep"
        named="$(printf '%s\n' "$out" | sed -n 's/^  \([a-e]\)[ ,].*(exit [0-9]*)$/\1/p' | tr '\n' ' ' | sed 's/^/ /; s/ $//')"
        [ "$named" = "$want" ] || fail "planted [$planted] $mode: summary named [$named], wanted [$want]"
        printf '%s\n' "$out" | grep -q '^e ran$' || fail "planted [$planted] $mode: the last section never ran"

        # One lint-section-time line per section that ran, never one for the section the mode
        # drops, and one CSV row per section that ran, the section's exit last.
        for entry in "a|a" "b|b reads the preamble" "c|c writes a file" "d|d, the cargo one" \
            "e|e reads what c wrote"; do
            key=${entry%%|*}
            label=${entry#*|}
            if [ "$mode" = no-cargo ] && [ "$key" = d ]; then
                if printf '%s\n' "$out" | grep -q "^lint-section-time: $label "; then
                    fail "planted [$planted] $mode: the dropped section reports a time"
                fi
            else
                printf '%s\n' "$out" | grep -q "^lint-section-time: $label [0-9][0-9]*\$" \
                    || fail "planted [$planted] $mode: no lint-section-time line for $key"
            fi
        done
        want_rows=5
        [ "$mode" = all ] || want_rows=4
        [ $((rows_after - rows_before)) -eq "$want_rows" ] \
            || fail "planted [$planted] $mode: $((rows_after - rows_before)) CSV rows, wanted $want_rows"
        failed_rows=$(tail -n "$want_rows" "$TIMES" | grep -c ',[1-9][0-9]*$' || true)
        [ "$failed_rows" -eq "$(printf '%s' "$want" | wc -w | tr -d ' ')" ] \
            || fail "planted [$planted] $mode: $failed_rows failed CSV rows, wanted [$want]"
        # The host column: each row the run wrote names the machine that wrote it, `hostname`'s
        # own output, so rows combined from several machines later stay attributable to their
        # machine.
        bad_host=$(tail -n "$want_rows" "$TIMES" \
            | awk -F, -v want="$(hostname)" '$2 != want' | wc -l | tr -d ' ')
        [ "$bad_host" -eq 0 ] \
            || fail "planted [$planted] $mode: $bad_host CSV rows do not name this machine's host"
        # The name column is quoted CSV-style because a name can carry a comma, and section d's
        # now does: its row must quote the name whole, with exactly the seconds and the exit after
        # it, and a planted d failure must show its exit as the row's last field.
        if [ "$mode" = all ]; then
            tail -n "$want_rows" "$TIMES" \
                | grep -q ',"d, the cargo one",[0-9][0-9]*,[0-9][0-9]*$' \
                || fail "planted [$planted] $mode: section d's row does not quote its name whole"
            if [ -n "${PLANT_D:-}" ]; then
                tail -n "$want_rows" "$TIMES" \
                    | grep -q ',"d, the cargo one",[0-9][0-9]*,[1-9][0-9]*$' \
                    || fail "planted [$planted] $mode: section d's planted exit is not its row's last field"
            fi
        fi
    done
done

# The CSV rows are a measurement, never a gate: point the file at a parent that is a plain file,
# so its directory cannot be created, and hold the verdict and the output to what they were.
unset PLANT_A PLANT_B PLANT_C PLANT_D
: >"$work/blocked"
rm -rf "$OUT"; mkdir "$OUT"
out="$(NIFE_LINT_TIMES="$work/blocked/times.csv" sh helpers/lint-sections.sh "$work/lint" all 2>&1)"
blocked=$?
[ "$blocked" -eq 0 ] || fail "an unwritable CSV file changed the verdict (exit $blocked)"
printf '%s\n' "$out" | grep -q '^lint: all 5 sections passed$' \
    || fail "an unwritable CSV file changed the output: $out"
if printf '%s\n' "$out" | grep -qiE 'mkdir|permission|denied|no such'; then
    fail "an unwritable CSV file was not skipped silently"
fi
echo "lint-sections-selftest: ok"
