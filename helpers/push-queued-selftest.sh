#!/bin/sh
#
# helpers/push-queued-selftest.sh: helpers/push-queued-check.sh against a fake `gh` on PATH.
# Three cases: queued (refuses), not queued (passes), gh failing (warns and passes). Plus the
# override, a missing gh, and a tag push. No network, no real gh.

here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
me="$(basename "$0")"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/push-queued-selftest.XXXXXX")" || exit 1
trap 'rm -rf "$tmp"' EXIT

# The fake prints $FAKE_GH_OUT, or fails when FAKE_GH_FAIL is set. It ignores its arguments,
# which is the point: the check's own handling of the answer is what is under test.
cat >"$tmp/gh" <<'FAKE'
#!/bin/sh
[ -z "$FAKE_GH_FAIL" ] || exit 1
printf '%s\n' "$FAKE_GH_OUT"
FAKE
chmod +x "$tmp/gh"

refs='refs/heads/lane/x 1111 refs/heads/lane/x 2222'
fail=0
# The hook runs this selftest under the pusher's own environment, so a real
# `NIFE_PUSH_QUEUED=1 git push` would otherwise turn every refusal case into a pass.
unset NIFE_PUSH_QUEUED FAKE_GH_FAIL FAKE_GH_OUT
# Each case runs in a subshell: a variable assignment before a function call can outlive the call in
# a POSIX shell, and a leaked NIFE_PUSH_QUEUED or FAKE_GH_FAIL would silently change the later cases.
expect() { # name expected-exit expected-stderr-fragment
	name="$1"; want="$2"; frag="$3"
	err="$(printf '%s\n' "$refs" | PATH="$tmp:$PATH" "$here/push-queued-check.sh" 2>&1 >/dev/null)"
	got=$?
	if [ "$got" != "$want" ] || { [ -n "$frag" ] && ! printf '%s' "$err" | grep -qF -- "$frag"; }; then
		echo "$me: $name: exit $got (want $want), stderr: $err" >&2
		return 1
	fi
}

(FAKE_GH_OUT='queued 42 PR_abc' expect "queued refuses and names the pull request" 1 '#42') || fail=1
(FAKE_GH_OUT='queued 42 PR_abc' expect "the refusal names the re-arming push" 1 'script/push --requeue') || fail=1
(FAKE_GH_OUT='queued 42 PR_abc' expect "the refusal names the override" 1 'NIFE_PUSH_QUEUED=1') || fail=1
(FAKE_GH_OUT='idle' expect "open, not queued passes" 0 '') || fail=1
(FAKE_GH_OUT='none' expect "no pull request passes" 0 '') || fail=1
(FAKE_GH_FAIL=1 expect "gh failing warns and passes" 0 'pushing lane/x anyway') || fail=1
(FAKE_GH_OUT='queued 42 PR_abc' NIFE_PUSH_QUEUED=1 expect "the override passes" 0 '') || fail=1

# No gh on PATH at all.
mkdir "$tmp/empty"
err="$(printf '%s\n' "$refs" | PATH="$tmp/empty" /bin/sh "$here/push-queued-check.sh" 2>&1 >/dev/null)"
got=$?
if [ "$got" != 0 ] || ! printf '%s' "$err" | grep -q 'not installed'; then
	echo "$me: missing gh: exit $got, stderr: $err" >&2
	fail=1
fi

# A tag push has no pull request; it must not even ask.
err="$(printf 'refs/tags/v1 1 refs/tags/v1 0\n' | FAKE_GH_OUT='queued 1 X' PATH="$tmp:$PATH" "$here/push-queued-check.sh" 2>&1)"
[ $? = 0 ] || { echo "$me: a tag push was refused: $err" >&2; fail=1; }

[ "$fail" = 0 ] && echo "$me: ok"
exit "$fail"
