#!/bin/sh
#
# helpers/preflight-arm-check-selftest.sh: `script/preflight-queue --arm-check`, the lint helpers/arm.sh
# runs over the queue a pull request joins, against a local git fixture. No network and no gh.
# Provisional name, minted by lane maintainer/queue-entry-lint-guards (2026-11-11 UTC).
#
# The fixture is a bare origin whose `main` holds a file `words` of two lines, and a stub lint that
# fails the section "the word cap" once the `words*` files together pass three lines, printing the summary that
# script/lint prints. That is the shape of the week the proposal "lint fails in the queue after
# passing on the pull request" measured: two pull requests that each add a line pass alone, and the
# second fails on top of the first. A file `broken` fails a second section, "always red", so a
# stack that is red without the pull request can be built.

here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
root="$(dirname "$here")"
me="$(basename "$0")"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/preflight-arm-check-selftest.XXXXXX")" || exit 1
trap 'rm -rf "$tmp"' EXIT

fx="$tmp/fixture"
fxgit() { GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@example.invalid \
	GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@example.invalid git -C "$fx/work" "$@" >/dev/null 2>&1; }
# branch: a commit on main that writes a line to a file of its own, pushed; prints the commit.
branch() { # name file text
	fxgit checkout -q -B "$1" "$base" && printf '%s\n' "$3" >>"$fx/work/$2" && fxgit add -A &&
		fxgit commit -q -m "$1" && fxgit push -q origin "$1:refs/heads/$1" && git -C "$fx/work" rev-parse HEAD
}
mkdir -p "$fx" && git init -q --bare -b main "$fx/origin.git" >/dev/null 2>&1 &&
	git init -q -b main "$fx/work" >/dev/null 2>&1 && printf 'one\ntwo\n' >"$fx/work/words" &&
	fxgit add -A && fxgit commit -q -m base && fxgit remote add origin "$fx/origin.git" &&
	fxgit push -q origin main && base="$(git -C "$fx/work" rev-parse HEAD)" &&
	ahead="$(branch ahead words-ahead three)" && mine="$(branch mine words-mine four)" &&
	other="$(branch other notes 'a note')" && red="$(branch red broken x)" &&
	clash="$(branch clash words-ahead 'not three')" || { echo "$me: could not build the git fixture" >&2; exit 1; }

cat >"$tmp/lint" <<'LINT'
#!/bin/sh
failed=""
[ "$(cat words* | wc -l)" -le 3 ] || failed="$failed
  the word cap (exit 1)"
[ ! -e broken ] || failed="$failed
  always red (exit 1)"
[ -n "$failed" ] || { echo "lint: all 2 sections passed"; exit 0; }
[ "$(cat words* | wc -l)" -le 3 ] || printf '==> the word cap\nwords is %s lines, over the cap of 3\n' "$(cat words* | wc -l)"
echo "lint: some of 2 sections failed. Each one's own output is above its name's ==> line:$failed"
exit 1
LINT
chmod +x "$tmp/lint"

fail=0
# check <name> <want exit> <fragment> <pull request head> <entries ahead: number=sha ...>
check() {
	name="$1"; want="$2"; frag="$3"; head="$4"; shift 4
	printf '[' >"$tmp/entries.json"; sep=""
	for e in "$@"; do
		printf '%s{"number":%s,"headRefOid":"%s"}' "$sep" "${e%%=*}" "${e#*=}" >>"$tmp/entries.json"; sep=","
	done
	printf ']' >>"$tmp/entries.json"
	out="$(PREFLIGHT_REPO_DIR="$fx/work" PREFLIGHT_REMOTE="$fx/origin.git" PREFLIGHT_ENTRIES_JSON="$tmp/entries.json" \
		PREFLIGHT_HEAD="$head" PREFLIGHT_LINT="$tmp/lint" "$root/script/preflight-queue" --arm-check 42 2>&1)"
	got=$?
	if [ "$got" != "$want" ]; then
		echo "$me: $name: exit $got (want $want): $out" >&2; fail=1; return
	fi
	if [ -n "$frag" ] && ! printf '%s' "$out" | grep -qF -- "$frag"; then
		echo "$me: $name: output lacks '$frag': $out" >&2; fail=1
	fi
}

# The ruling's case: green alone, red on top of the entry ahead, and the section is named.
check "alone on main: green" 0 "green with #42" "$mine"
check "on top of the entry ahead: red" 1 "the word cap" "$mine" 7="$ahead"
check "the red names what the section said" 1 "over the cap of 3" "$mine" 7="$ahead"
# An unrelated entry ahead changes nothing.
check "an unrelated entry ahead: green" 0 "green" "$mine" 8="$other"
# A stack already red without the pull request is not its fault: could not tell.
check "a red stack is not blamed" 2 "red on the stack without #42 too" "$other" 9="$red"
# Red already, and the pull request adds a second red: blamed for that one only.
check "a red it adds on a red stack is blamed" 1 "the word cap" "$mine" 7="$ahead" 9="$red"
# An entry ahead that conflicts with the stack is left out, as the queue would leave it.
check "a conflicting entry ahead is left out" 0 "left out: #10 (conflict)" "$other" 7="$ahead" 10="$clash"
# The pull request itself does not merge onto the stack: could not tell.
check "a pull request that conflicts: could not tell" 2 "does not merge" "$clash" 7="$ahead"
# Already queued: only the entries ahead of it count, not those behind.
check "queued: entries behind it are not ahead" 0 "green" "$mine" 42="$mine" 7="$ahead"
out="$("$root/script/preflight-queue" --arm-check nope 2>&1)"
[ $? = 2 ] || { echo "$me: a non-number was not refused: $out" >&2; fail=1; }
# No worktree is left behind.
[ "$(git -C "$fx/work" worktree list | wc -l)" = 1 ] ||
	{ echo "$me: a stack worktree was left behind" >&2; fail=1; }

[ "$fail" = 0 ] && echo "$me: ok"
exit "$fail"
