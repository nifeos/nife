#!/bin/sh
#
# Selftest for `script/decisions --renumber-from` (milestone 499 (a renumber that cannot cite the
# wrong section)), run by script/lint-fleet. One
# command rewrites every tracked file that cites a moved section, so the failure to guard is the
# one the mode exists to prevent: a citation that still resolves but names the wrong section. A
# small fixture tree holds each step to its promise: rename descending, bump the heading with its
# file, rewrite citations keyed on the filename, move a sigil only where a filename
# disambiguates, and list the bare ones for a person with a non-zero exit.
#
# The fixture reuses the tree's real section numbers, because the citation gates read this file:
# §2 (QEMU virt primary), §3 (crate ecosystem), §4 (kernel shape) and §5 (preemptive threads) name
# what those numbers are in this tree, once each, which is the ratchet's per-file gloss; the
# fixture's own sections are titled for the test and mean nothing here.
#
# Name: provisional, 2026-10-15 (UTC), after the other helpers/*-selftest.sh.

set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() { echo "decisions-renumber-selftest: $*" >&2; exit 1; }

mkdir -p "$work/script" "$work/design/decisions/0004-fourth" "$work/notes"
cp "$root/script/decisions" "$work/script/decisions"

# The index modes read ARCHITECTS.md on every run; the fixture's ratifier must be listed there.
cat > "$work/ARCHITECTS.md" <<'EOF'
| GitHub username | Role |
| --- | --- |
| `calef` | architect |
EOF

dec() { name=$1; shift; printf '%s\n' "$@" > "$work/design/decisions/$name"; }
dec 0001-first.md \
    '---' 'status: DECIDED' 'raised: 2026-10-15' 'decided: 2026-10-15' 'ratified_by: calef' '---' \
    '' '# 1. First' '' 'It defers to [§2](0002-second.md), its successor.'
dec 0002-second.md \
    '---' 'status: SUPERSEDED' 'raised: 2026-10-15' 'superseded_by: 3' '---' \
    '' '# 2. Second' '' 'Superseded by [§3](0003-third.md).'
dec 0003-third.md \
    '---' 'status: DECIDED' 'raised: 2026-10-15' 'decided: 2026-10-15' 'ratified_by: calef' '---' \
    '' '# 3. Third' '' 'It extends [§4](0004-fourth.md).' '§3 restates itself nowhere.'
dec 0004-fourth.md \
    '---' 'status: DECIDED' 'raised: 2026-10-15' 'decided: 2026-10-15' 'ratified_by: calef' '---' \
    '' '# 4. Fourth' ''
printf '%s\n' 'See [§4](../0004-fourth.md), its parent.' \
    > "$work/design/decisions/0004-fourth/appendix.md"
printf '%s\n' \
    'A link: [§2](../design/decisions/0002-second.md).' \
    'A bare §3 with no filename.' \
    'A path and a sigil: design/decisions/0003-third.md, §3.' \
    'Mixed: 0003-third.md but §2 means the old one.' > "$work/notes/citing.md"
printf '%s\n' '/* §4: design/decisions/0004-fourth.md */' > "$work/notes/code.c"

git -C "$work" init -q
git -C "$work" -c user.email=lane@example.com -c user.name=lane add -A
git -C "$work" -c user.email=lane@example.com -c user.name=lane commit -qm fixture
# The script copy cites real decisions in its own header comments, and the record check greps
# every tracked file. The tool is not part of this fixture's record: untracked, `git grep`
# skips it and the run is unchanged, since the dirty-tree refusal counts tracked files only.
git -C "$work" rm -q --cached script/decisions
git -C "$work" -c user.email=lane@example.com -c user.name=lane commit -qm "untrack the tool"

run() {
    set +e
    out=$(sh "$work/script/decisions" "$@" 2>&1)
    got=$?
    set -e
}

# Argument refusals, before anything is read.
run --renumber-from
[ "$got" -eq 2 ] || fail "missing number: exit $got, wanted 2 ($out)"
run --renumber-from two
[ "$got" -eq 2 ] || fail "non-numeric number: exit $got, wanted 2 ($out)"
run --renumber-from 2 --bogus
[ "$got" -eq 2 ] || fail "unknown flag: exit $got, wanted 2 ($out)"

# A dry run computes and writes nothing, and already reports the bare sigils it would leave.
run --renumber-from 2 --dry-run
[ "$got" -eq 1 ] || fail "dry run: exit $got, wanted 1 while bare sigils remain ($out)"
printf '%s' "$out" | grep -q "would rename 3 files" || fail "dry run: no rename plan ($out)"
printf '%s' "$out" | grep -q "A bare §3 with no filename" || fail "dry run: bare sigil unlisted ($out)"
[ -z "$(git -C "$work" status --porcelain --untracked-files=no)" ] \
    || fail "dry run wrote to the tree"

# Two files claiming one number is refused: the filename key cannot choose between them.
cp "$work/design/decisions/0002-second.md" "$work/design/decisions/0002-extra.md"
run --renumber-from 2
[ "$got" -eq 1 ] || fail "duplicate number: exit $got, wanted 1 ($out)"
printf '%s' "$out" | grep -q "claimed by two files" || fail "duplicate number: not refused ($out)"
rm "$work/design/decisions/0002-extra.md"

# Uncommitted tracked changes are refused: what the renumber did must be one reviewable diff.
printf '%s\n' 'uncommitted' >> "$work/notes/citing.md"
run --renumber-from 2
[ "$got" -eq 1 ] || fail "dirty tree: exit $got, wanted 1 ($out)"
printf '%s' "$out" | grep -q "uncommitted tracked changes" || fail "dirty tree: not refused ($out)"
git -C "$work" checkout -q -- notes/citing.md

# The real run. It exits 1 on purpose: two bare sigils await a person.
run --renumber-from 2
[ "$got" -eq 1 ] || fail "renumber: exit $got, wanted 1 ($out)"
d="$work/design/decisions"
for old in 0002-second.md 0003-third.md 0004-fourth.md; do
    [ ! -e "$d/$old" ] || fail "renumber: $old still exists"
done
[ -f "$d/0003-second.md" ] || fail "renumber: 0003-second.md missing ($out)"
[ -f "$d/0004-third.md" ] || fail "renumber: 0004-third.md missing ($out)"
[ -f "$d/0005-fourth.md" ] || fail "renumber: 0005-fourth.md missing ($out)"
[ -f "$d/0005-fourth/appendix.md" ] || fail "renumber: the appendix directory did not move"
[ ! -e "$d/0004-fourth" ] || fail "renumber: the old appendix directory remains"
grep -q '^# 3\. Second$' "$d/0003-second.md" || fail "step 2: 0003-second.md heading not bumped"
grep -q '^# 4\. Third$' "$d/0004-third.md" || fail "step 2: 0004-third.md heading not bumped"
grep -q '^# 5\. Fourth$' "$d/0005-fourth.md" || fail "step 2: 0005-fourth.md heading not bumped"
grep -q '^superseded_by: 4$' "$d/0003-second.md" || fail "the superseded_by field did not move"
grep -q 'Superseded by \[§4\](0004-third.md)\.' "$d/0003-second.md" \
    || fail "step 3/4: §3 with its filename did not move"
grep -q 'defers to \[§3\](0003-second.md)' "$d/0001-first.md" \
    || fail "step 3/4: §2 with its filename did not move"
grep -q 'extends \[§5\](0005-fourth.md)' "$d/0004-third.md" \
    || fail "step 3/4: §4 with its filename did not move"
grep -q 'See \[§5\](../0005-fourth.md)' "$d/0005-fourth/appendix.md" \
    || fail "step 3/4: the appendix citation did not move"
grep -q 'A bare §3 with no filename\.' "$work/notes/citing.md" \
    || fail "step 4: a bare sigil was rewritten"
grep -q '0004-third\.md, §4\.' "$work/notes/citing.md" \
    || fail "step 4: a sigil beside its filename did not move"
grep -q '§5: design/decisions/0005-fourth\.md' "$work/notes/code.c" \
    || fail "step 3/4: the code comment did not move"
# The list shows where the files now stand, not where they were.
printf '%s' "$out" | grep -q "design/decisions/0004-third.md:[0-9]*: §3 restates itself nowhere." \
    || fail "step 5: the decision's own bare self-mention is not listed ($out)"
printf '%s' "$out" | grep -q "notes/citing.md:[0-9]*: A bare §3 with no filename\." \
    || fail "step 5: the note's bare sigil is not listed ($out)"
# A line that names one moved file must not carry a different moved section's bare sigil along
# with it: the filename vouches for its own number only.
grep -q 'Mixed: 0004-third.md but §2 means the old one\.' "$work/notes/citing.md" \
    || fail "step 4: a bare sigil of a different moved section was rewritten on a line naming another file"
printf '%s' "$out" | grep -q "notes/citing.md:[0-9]*: Mixed: 0003-third.md but §2" \
    || fail "step 5: the mixed line is not listed ($out)"
[ "$(printf '%s\n' "$out" | grep -c '^  .*:[0-9]*: ')" -eq 3 ] \
    || fail "step 5: expected exactly three listed lines ($out)"

# The person's half: decide each listed line, then the existing gate holds the result.
perl -pi -e 's/^§3 restates itself nowhere\.$/§4 restates itself nowhere./' "$d/0004-third.md"
perl -pi -e 's/^A bare §3 with no filename\.$/A bare §4 with no filename./' "$work/notes/citing.md"
perl -pi -e 's/but §2 means/but §3 means/' "$work/notes/citing.md"
run --check
[ "$got" -eq 0 ] || fail "check after the shift: exit $got, wanted 0 ($out)"

echo "decisions-renumber-selftest: ok"
