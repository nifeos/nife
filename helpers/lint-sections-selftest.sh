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
fail() { echo "lint-sections-selftest: (jobs ${NIFE_LINT_JOBS:-default}) $1" >&2; exit 1; }

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
# lint-lane: writer
echo built >"$OUT/built"
if [ -n "${PLANT_C:-}" ]; then false; fi
echo "==> d, the cargo one"
cargo clippy
echo "==> e reads what c wrote"
# lint-waits-for: writer
[ -f "$OUT/built" ]
echo "e ran"
FIXTURE

# What `script/lint --no-cargo` ran before the helper: the kept sections as one script.
awk -v mode=emit -f helpers/lint-no-cargo.awk "$work/lint" >"$work/lint-no-cargo"

# Every combination at one job, which is the old fail-fast comparison; three at four jobs, none, all
# and a mixed one, since the verdict does not depend on the order and the polling makes a run slower.
for jobs in 1 4; do
NIFE_LINT_JOBS=$jobs
export NIFE_LINT_JOBS
combos="0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15"
[ "$jobs" = 1 ] || combos="0 6 15"
for combo in $combos; do
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

# Side by side, the part the combinations above cannot show. Five pool sections each hold half a second to a second.
# At most $jobs may be inside their second at once (counted by + and - lines, which a small append
# keeps whole), more than one must be inside it at once when jobs is above one, and the output must
# still come back in plan order with each section's lines under its own header, the slowest section
# first in the plan and the fastest last.
cat >"$work/lint-pool" <<'FIXTURE'
set -e
echo "==> p1"
echo "p1 body"
echo + >>"$OUT/conc"; sleep 0.6; echo - >>"$OUT/conc"
echo "==> p2"
echo "p2 body"
echo + >>"$OUT/conc"; sleep 0.3; echo - >>"$OUT/conc"
echo "==> p3"
echo "p3 body"
echo + >>"$OUT/conc"; sleep 0.3; echo - >>"$OUT/conc"
echo "==> p4"
echo "p4 body"
echo + >>"$OUT/conc"; sleep 0.3; echo - >>"$OUT/conc"
echo "==> p5"
echo "p5 body"
echo + >>"$OUT/conc"; sleep 0.3; echo - >>"$OUT/conc"
echo "==> p6"
echo "p6 body"
FIXTURE
jobs=3
NIFE_LINT_JOBS=$jobs
export NIFE_LINT_JOBS
rm -rf "$OUT"; mkdir "$OUT"
out="$(sh helpers/lint-sections.sh "$work/lint-pool" all 2>&1)" || fail "the pool fixture failed: $out"
order="$(printf '%s\n' "$out" | grep -E '^(==> p|p[0-9] body)' | tr '\n' ' ')"
[ "$order" = "==> p1 p1 body ==> p2 p2 body ==> p3 p3 body ==> p4 p4 body ==> p5 p5 body ==> p6 p6 body " ] \
    || fail "the output is out of plan order or mixed: $order"
peak="$(awk '$1 == "+" { n++; if (n > m) m = n } $1 == "-" { n-- } END { print m + 0 }' "$OUT/conc")"
[ "$peak" -le "$jobs" ] || fail "$peak sections ran at once with $jobs jobs"
if [ "$jobs" -gt 1 ] && [ "$peak" -lt 2 ]; then fail "nothing overlapped with $jobs jobs"; fi
printf '%s\n' "$out" | grep -q '^lint: all 6 sections passed$' || fail "the pool fixture did not pass whole: $out"
printf '%s\n' "$out" | grep -q '^lint-section-cpu: p1 [0-9][0-9]*\.[0-9]$' || fail "no CPU line for p1: $out"
printf '%s\n' "$out" | grep -q '^lint: [0-9]* s wall, [0-9]* s of sections added up' || fail "no wall-clock total: $out"

# Lanes and waits. Lane x holds x1 then x2 (x2 reads what x1 wrote); lane y holds y1, which waits for
# lane x and must see x2's file; z has no lane and runs in the pool. Run at every job count, and a
# lane's order must hold even when the pool is wide.
cat >"$work/lint-lanes" <<'FIXTURE'
set -e
echo "==> x1"
# lint-lane: x
sleep 0.3; echo 1 >"$OUT/x1"
echo "==> x2"
# lint-lane: x
[ -f "$OUT/x1" ]; echo 2 >"$OUT/x2"
echo "==> y1"
# lint-lane: y
# lint-waits-for: x
[ -f "$OUT/x2" ]
echo "==> z"
true
FIXTURE
jobs=2
NIFE_LINT_JOBS=$jobs
export NIFE_LINT_JOBS
rm -rf "$OUT"; mkdir "$OUT"
out="$(sh helpers/lint-sections.sh "$work/lint-lanes" all 2>&1)" || fail "the lane fixture failed at $jobs jobs: $out"
# A section that waits for a lane the mode drops does not wait forever (quick and no-cargo drop
# sections), and --quick runs exactly the marked ones.
cat >"$work/lint-quick" <<'FIXTURE'
set -e
echo "==> q1, marked"
# lint-quick
echo q1 ran
echo "==> q2, not marked"
echo q2 ran
echo "==> q3, marked and waits for q2's lane"
# lint-quick
# lint-waits-for: nosuch
echo q3 ran
FIXTURE
for jobs in 1 4; do
    NIFE_LINT_JOBS=$jobs
    export NIFE_LINT_JOBS
    out="$(sh helpers/lint-sections.sh "$work/lint-quick" quick 2>&1)" || fail "the quick fixture failed: $out"
    ran_q="$(printf '%s\n' "$out" | grep '^q[0-9] ran$' | tr '\n' ' ')"
    [ "$ran_q" = "q1 ran q3 ran " ] || fail "quick mode did not run exactly the marked sections: $out"
    printf '%s\n' "$out" | grep -q '^lint: all 2 sections passed$' || fail "quick mode ran another number of sections: $out"
done
echo "lint-sections-selftest: ok"
