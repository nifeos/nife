#!/bin/sh
#
# helpers/lint-section-index.sh: keep script/lint's section list a reviewed interface.
#
#     helpers/lint-section-index.sh [check|write] [<lint script>]
#
# helpers/lint-no-cargo.awk's `index` mode prints one line per section, in file order: the
# `# lint-id:` slug (`-` when absent), keep (1 when `--no-cargo` runs it), lane, waits, quick, target
# dir, scope (`# lint-scope:`, `-` when absent) and name, tab separated. helpers/lint-section-index.tsv holds the committed copy.
#
# `check` (the default) fails with a diff when the live index differs. A renamed, dropped, added or
# reordered section, a changed lane, wait, target dir, scope or `# lint-quick` flag, or a section that
# gained or lost its cargo-ness (keep) all show. Why it exists: a section's name is what the
# section-times CSV (helpers/lint-sections.sh) and script/ci-log-baseline key on, and its quick,
# lane and wait markers decide what runs where and when, so none of them should move without a
# reviewer seeing the change in a diff of this file. `write` regenerates the copy after an
# intended change; commit the result with the change.
#
# Runs no cargo and needs only awk, so it belongs in `script/lint --quick` and `--no-cargo`.
# Selftest: helpers/lint-section-index-selftest.sh.

set -eu
cd "$(dirname "$0")/.."
cmd=${1:-check}
lint=${2:-script/lint}
snapshot=${LINT_SECTION_INDEX:-helpers/lint-section-index.tsv}
live="$(mktemp "${TMPDIR:-/tmp}/lint-section-index.XXXXXX")" || exit 1
trap 'rm -f "$live"' EXIT
awk -v mode=index -f helpers/lint-no-cargo.awk "$lint" >"$live"

case "$cmd" in
write)
    cp "$live" "$snapshot"
    echo "lint-section-index: wrote $(wc -l <"$live" | tr -d ' ') sections to $snapshot"
    ;;
check)
    if [ ! -f "$snapshot" ]; then
        echo "lint-section-index: $snapshot is missing; run helpers/lint-section-index.sh write" >&2
        exit 1
    fi
    if ! diff -u "$snapshot" "$live" >&2; then
        echo "" >&2
        echo "lint-section-index: script/lint's sections differ from $snapshot (- committed, + live)." >&2
        echo "  Columns: lint-id, keep, lane, waits, quick, target dir, scope, name." >&2
        echo "  If the change is intended, run helpers/lint-section-index.sh write and commit the file;" >&2
        echo "  a renamed section also changes its row in the section-times CSV and in script/ci-log-baseline's output." >&2
        exit 1
    fi
    echo "lint-section-index: $(wc -l <"$live" | tr -d ' ') sections match $snapshot"
    ;;
*)
    echo "usage: helpers/lint-section-index.sh [check|write] [<lint script>]" >&2
    exit 2
    ;;
esac
