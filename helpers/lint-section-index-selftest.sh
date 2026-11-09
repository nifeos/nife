#!/bin/sh
#
# helpers/lint-section-index-selftest.sh: prove helpers/lint-section-index.sh fails on each kind of
# change to the section list, and helpers/lint-sections.sh --section runs exactly one section.
#
#     helpers/lint-section-index-selftest.sh
#
# A fixture lint script has four sections. The gate must pass on the untouched fixture and fail
# after a rename, a drop, a reorder, a flipped `# lint-quick` flag, a changed lane, a changed
# `# lint-id:` and a changed `# lint-scope:`, and `write` must restore a pass. Then --section must run one section by slug and
# by exact name, with the preamble, and refuse an unknown name and an ambiguous one.
# No tree and no cargo; it runs in milliseconds.

set -eu
cd "$(dirname "$0")/.."
work="$(mktemp -d "${TMPDIR:-/tmp}/lint-section-index-selftest.XXXXXX")"
trap 'rm -rf "$work"' EXIT
fail() { echo "lint-section-index-selftest: $1" >&2; exit 1; }

cat >"$work/base" <<'FIXTURE'
set -e
shared=from-the-preamble
echo "==> alpha"
# lint-quick
# lint-id: first
# lint-scope: record
echo "alpha ran $shared"
echo "==> beta"
# lint-lane: writer
echo "beta ran"
echo "==> gamma"
echo "gamma ran"
echo "==> gamma again"
echo "gamma again ran"
FIXTURE
export LINT_SECTION_INDEX="$work/index.tsv"
sh helpers/lint-section-index.sh write "$work/base" >/dev/null
sh helpers/lint-section-index.sh check "$work/base" >/dev/null 2>&1 || fail "the untouched fixture fails its own snapshot"
[ "$(wc -l <"$LINT_SECTION_INDEX" | tr -d ' ')" = 4 ] || fail "the snapshot does not hold four rows"

# Each variant must fail the check, and the failure must name the changed row in a diff.
expect_fail() {
    name=$1
    file=$2
    if sh helpers/lint-section-index.sh check "$file" >"$work/out" 2>&1; then
        fail "$name: the gate passed"
    fi
    grep '^[-+]' "$work/out" | grep -qv '^\(--- \|+++ \)' || fail "$name: the failure shows no diff"
}
sed 's/^echo "==> beta"$/echo "==> beta renamed"/' "$work/base" >"$work/renamed"
expect_fail "a renamed section" "$work/renamed"
awk '/^echo "==> beta"$/ { skip = 1 } /^echo "==> gamma"$/ { skip = 0 } !skip' "$work/base" >"$work/dropped"
expect_fail "a dropped section" "$work/dropped"
awk '
    /^echo "==> / { n++ }
    { sec[n] = sec[n] $0 "\n" }
    END { printf "%s%s%s%s%s", sec[0], sec[2], sec[1], sec[3], sec[4] }' "$work/base" >"$work/reordered"
expect_fail "a reordered section" "$work/reordered"
sed 's/^# lint-quick$/# not-quick/' "$work/base" >"$work/unquick"
expect_fail "a flipped quick flag" "$work/unquick"
sed 's/^# lint-lane: writer$/# lint-lane: other/' "$work/base" >"$work/lane"
expect_fail "a changed lane" "$work/lane"
sed 's/^# lint-id: first$/# lint-id: second/' "$work/base" >"$work/slug"
expect_fail "a changed lint-id" "$work/slug"
sed 's/^# lint-scope: record$/# lint-scope: both/' "$work/base" >"$work/scope"
expect_fail "a changed lint-scope" "$work/scope"
sh helpers/lint-section-index.sh write "$work/reordered" >/dev/null
sh helpers/lint-section-index.sh check "$work/reordered" >/dev/null 2>&1 || fail "write did not bring the snapshot back in line"

# --section: one section, by slug or by name, with the preamble.
export NIFE_LINT_TIMES="$work/times.csv" NIFE_LINT_JOBS=1
out=$(sh helpers/lint-sections.sh "$work/base" --section first 2>&1) || fail "--section first failed"
printf '%s\n' "$out" | grep -q '^alpha ran from-the-preamble$' || fail "--section by slug did not run the section with the preamble"
printf '%s\n' "$out" | grep -q 'beta ran' && fail "--section by slug ran another section"
out=$(sh helpers/lint-sections.sh "$work/base" --section beta 2>&1) || fail "--section beta failed"
printf '%s\n' "$out" | grep -q '^beta ran$' || fail "--section by name did not run the section"
printf '%s\n' "$out" | grep -q 'alpha ran' && fail "--section by name ran another section"
out=$(sh helpers/lint-sections.sh "$work/base" --section gamma 2>&1) || fail "--section gamma failed (an exact name is not a prefix)"
printf '%s\n' "$out" | grep -q 'gamma again ran' && fail "--section gamma ran the section whose name only starts with it"
if sh helpers/lint-sections.sh "$work/base" --section nothing >/dev/null 2>&1; then fail "--section accepted an unknown section"; fi
if sh helpers/lint-sections.sh "$work/base" --section >/dev/null 2>&1; then fail "--section accepted no argument"; fi
sed 's/^echo "==> gamma again"$/echo "==> gamma"/' "$work/base" >"$work/twin"
if sh helpers/lint-sections.sh "$work/twin" --section gamma >/dev/null 2>&1; then fail "--section accepted a name two sections share"; fi
sed 's/^echo "beta ran"$/echo "beta ran"; false/' "$work/base" >"$work/failing"
if sh helpers/lint-sections.sh "$work/failing" --section beta >/dev/null 2>&1; then fail "--section hid a failing section"; fi
awk '/^echo "==> beta"$/ { print; print "# lint-id: first"; next } { print }' "$work/base" >"$work/dupslug"
if sh helpers/lint-sections.sh "$work/dupslug" --section first >/dev/null 2>&1; then fail "two sections with one lint-id were accepted"; fi
echo "lint-section-index-selftest: ok"
