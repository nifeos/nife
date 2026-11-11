#!/bin/sh
#
# helpers/push-requeue-selftest.sh: `script/push --requeue` and helpers/arm.sh against a stub `gh` and
# a stub `git` on PATH. No network, no real repository (lane/push-requeue, 2026-11-04 UTC).
#
# The stub `gh` keeps one pull request's state in files: queued, auto-merge, labels, draft. Its
# `dequeuePullRequest` clears `queued`; its `pr merge` sets `queued` (a CLEAN pull request enqueues
# directly, so `autoMergeRequest` stays null) unless STUB_MERGE_NOOP is set, which makes the re-arm a
# no-op and is the falsification: with it set, the re-arm cases must fail. Every call is logged to
# $tmp/log so the order (dequeue, push, merge) can be asserted. The head is always `h1`. Its `lint`
# check runs come from $tmp/lint (default: one passed run) and the last queue removal from
# $tmp/removal ("reason group-commit built-head"), and `runlog` is what `gh run view --log-failed` prints for
# the group's one failed run, as helpers/arm.sh's `--jq` would print them, since the stub applies no filter.

here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
root="$(dirname "$here")"
me="$(basename "$0")"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/push-requeue-selftest.XXXXXX")" || exit 1
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin"

cat >"$tmp/bin/gh" <<'STUB'
#!/bin/sh
d="$STUB_DIR"
echo "gh $*" >>"$d/log"
get() { cat "$d/$1" 2>/dev/null; }
pr() {
	jq -n --arg q "$(get queued)" --arg a "$(get auto)" --arg dr "$(get draft)" --arg labels "$(get labels)" '
		{number: 42, id: "PR_42", state: "OPEN", isDraft: ($dr == "true"), headRefOid: "h1",
		 mergeQueueEntry: (if $q == "true" then {state: "QUEUED"} else null end),
		 autoMergeRequest: (if $a == "true" then {enabledAt: "x"} else null end),
		 labels: {nodes: ($labels | split(" ") | map(select(. != "") | {name: .}))}}'
}
case "$*" in
*dequeuePullRequest*) echo false >"$d/queued" ;;
*check-runs*)
	[ -z "$STUB_CHECKS_FAIL" ] || exit 1
	case "$*" in */commits/h1/*) ;; *) echo "stub gh: check runs asked for another head: $*" >&2; exit 1 ;; esac
	get lint || echo '[{"id":1,"status":"completed","conclusion":"success"}]'
	;;
"api repos/"*"/actions/runs?head_sha=g1"*) echo '{"workflow_runs":[{"id":7,"conclusion":"failure"},{"id":8,"conclusion":"success"}]}' ;;
"run view 7 --log-failed") get runlog ;;
*timelineItems*) get removal || echo ' ' ;;
"api graphql"*)
	[ -z "$STUB_GRAPHQL_FAIL" ] || exit 1
	if [ -e "$d/noPR" ]; then
		echo '{"data":{"repository":{"pullRequest":null,"pullRequests":{"nodes":[]}}}}'
	else
		jq -n --argjson p "$(pr)" '{data:{repository:{pullRequest:$p,pullRequests:{nodes:[$p]}}}}'
	fi
	;;
"pr merge"*) [ -n "$STUB_MERGE_NOOP" ] || echo true >"$d/queued" ;;
"pr comment"*) [ -z "$STUB_COMMENT_FAIL" ] || exit 1 ;;
*) echo "stub gh: unexpected: $*" >&2; exit 1 ;;
esac
STUB

cat >"$tmp/bin/git" <<'STUB'
#!/bin/sh
d="$STUB_DIR"
case "$1" in
symbolic-ref) echo lane/x ;;
rev-parse)
	case "$*" in
	*--show-toplevel*) echo "$d" ;;
	*--git-path*) echo "$d/hook" ;;
	*) echo aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ;;
	esac
	;;
--exec-path) echo "$d/bin" ;;
remote) echo "$d/origin" ;;
config) exit 1 ;;
push) echo "git $*" >>"$d/log" ;;
ls-remote) printf '%s\trefs/heads/lane/x\n' "${STUB_REMOTE:-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}" ;;
*) echo "stub git: unexpected: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$tmp/bin/gh" "$tmp/bin/git"

# helpers/arm.sh runs script/queue-conflicts. Every case but 10 replaces it with one that finds
# nothing, so no case reaches the network; case 10 runs the real check against a local fixture.
printf '#!/bin/sh\nexit 0\n' >"$tmp/check-clean"
printf '#!/bin/sh\necho "queue-conflicts: could not read the queue (offline); nothing was checked."\nexit 2\n' >"$tmp/check-unknown"
chmod +x "$tmp/check-clean" "$tmp/check-unknown"
ARM_QUEUE_CHECK="$tmp/check-clean"
export ARM_QUEUE_CHECK
# The lint over the queue (script/preflight-queue --arm-check) is stubbed the same way: green unless a
# case says otherwise. helpers/preflight-arm-check-selftest.sh runs the real one against a fixture.
printf '#!/bin/sh\necho "arm-check $*" >>"$STUB_DIR/log"\nexit 0\n' >"$tmp/lint-green"
printf '#!/bin/sh\necho "arm-check $*" >>"$STUB_DIR/log"\necho "#$1 makes lint red: the prose ratchet"\nexit 1\n' >"$tmp/lint-red"
printf '#!/bin/sh\necho "could not tell: the stack is red without it"\nexit 2\n' >"$tmp/lint-unknown"
chmod +x "$tmp/lint-green" "$tmp/lint-red" "$tmp/lint-unknown"
ARM_STACK_LINT="$tmp/lint-green"
export ARM_STACK_LINT

fail=0
# run <name> <want: 0|nonzero> <fragment of output or ''> -- args...   state comes from the caller
run() {
	name="$1"; want="$2"; frag="$3"; shift 4
	out="$(PATH="$tmp/bin:$PATH" STUB_DIR="$tmp" ARM_VERIFY_SLEEP=0 ARM_VERIFY_TRIES=2 NIFE_SKIP_QUEUE_CHECK=1 "$@" 2>&1)"
	got=$?
	if { [ "$want" = 0 ] && [ "$got" != 0 ]; } || { [ "$want" != 0 ] && [ "$got" = 0 ]; }; then
		echo "$me: $name: exit $got (want $want): $out" >&2; fail=1; return
	fi
	if [ -n "$frag" ] && ! printf '%s' "$out" | grep -qF -- "$frag"; then
		echo "$me: $name: output lacks '$frag': $out" >&2; fail=1
	fi
}
state() { # queued auto labels draft
	rm -f "$tmp/log" "$tmp/noPR"
	printf '%s\n' "$1" >"$tmp/queued"; printf '%s\n' "$2" >"$tmp/auto"
	printf '%s\n' "$3" >"$tmp/labels"; printf '%s\n' "$4" >"$tmp/draft"
	rm -f "$tmp/lint" "$tmp/removal" "$tmp/runlog"
}
logged() { grep -qF -- "$1" "$tmp/log" 2>/dev/null; }
check() { # name command-that-must-succeed
	name="$1"; shift
	"$@" || { echo "$me: $name" >&2; fail=1; }
}
STUB_MERGE_NOOP='' STUB_GRAPHQL_FAIL='' STUB_COMMENT_FAIL='' STUB_CHECKS_FAIL=''
export STUB_MERGE_NOOP STUB_GRAPHQL_FAIL STUB_COMMENT_FAIL STUB_CHECKS_FAIL
unset STUB_REMOTE

# 1. A queued pull request: dequeue, push, arm, in that order, and armed at the end.
state true false "ci" false
run "queued: --requeue re-arms" 0 "armed" -- "$root/script/push" --requeue
check "queued: dequeue, then push, then merge" awk '/dequeuePullRequest/{d=NR} /git push/{p=NR} /pr merge/{m=NR} END{exit !(d && d<p && p<m)}' "$tmp/log"
check "queued: pushed" logged "git push origin HEAD:refs/heads/lane/x"
check "queued: armed again" logged "pr merge 42 --auto --merge"
check "queued: ends queued" test "$(cat "$tmp/queued")" = true

# 1b. A pre-push hook runs before the dequeue, with the queued-branch refusal waived for that run
# alone (the pull request is still queued), so a failing check leaves it in the queue.
printf '#!/bin/sh\ncat >/dev/null\necho "hook queued-waived=$NIFE_PUSH_QUEUED" >>"%s/log"\n[ ! -e "%s/refuse" ]\n' "$tmp" "$tmp" >"$tmp/hook"
chmod +x "$tmp/hook"
state true false "ci" false
run "hook: --requeue re-arms" 0 "armed" -- "$root/script/push" --requeue
check "hook: hook, then dequeue, then push" awk '/^hook /{h=NR} /dequeuePullRequest/{d=NR} /git push/{p=NR} END{exit !(h && h<d && d<p)}' "$tmp/log"
check "hook: queued refusal waived for the early run" logged "hook queued-waived=1"
state true false "ci" false
: >"$tmp/refuse"
run "hook refuses: nothing dequeued" nonzero "NOT PUSHED" -- "$root/script/push" --requeue
check "hook refuses: no dequeue, no push" sh -c "! grep -q -e dequeuePullRequest -e 'git push' '$tmp/log'"
check "hook refuses: still queued" test "$(cat "$tmp/queued")" = true
rm -f "$tmp/hook" "$tmp/refuse"

# 2. Falsification: the re-arm step is a no-op, so the lane must hear it was NOT armed.
state true false "" false
STUB_MERGE_NOOP=1 run "no-op re-arm fails loudly" nonzero "NOT ARMED" -- "$root/script/push" --requeue

# 3. Every hold refuses, names the label, and never calls merge.
for hold in needs-architect held-by-lane held-for-red-trunk; do
	state true false "ci $hold" false
	run "hold $hold refuses" nonzero "$hold" -- "$root/script/push" --requeue
	check "hold $hold: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
done

# 4. Auto-merge on, not queued: re-armed. Not armed at all: stays unarmed.
state false true "" false
run "auto-merge on is kept armed" 0 "already armed" -- "$root/script/push" --requeue
state false false "" false
run "unarmed stays unarmed" 0 "stays unarmed" -- "$root/script/push" --requeue
check "unarmed: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"

# 5. The push does not land: no re-arm, and the lane is told.
state true false "" false
STUB_REMOTE=bbbb run "unlanded push does not re-arm" nonzero "NOT armed" -- "$root/script/push" --requeue
check "unlanded: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"

# 6. Without the flag nothing touches the pull request.
state true false "" false
run "no flag, no dequeue" 0 "" -- "$root/script/push"
check "no flag: gh never called" sh -c "! grep -q '^gh' '$tmp/log'"

# 6b. GitHub cannot be asked: --requeue pushes nothing and says so.
state true false "" false
STUB_GRAPHQL_FAIL=1 run "unreadable state stops before the push" nonzero "nothing was pushed" -- "$root/script/push" --requeue
check "unreadable: no push" sh -c "! grep -q 'git push' '$tmp/log'"

# 7. No pull request for the branch.
state false false "" false
: >"$tmp/noPR"
run "no pull request" 0 "no open pull request" -- "$root/script/push" --requeue

# 8. arm.sh alone: a draft is refused, an armed one is left alone.
state false false "" true
run "draft refused" nonzero "draft" -- "$root/helpers/arm.sh" 42
state true false "" false
run "already armed" 0 "already armed" -- "$root/helpers/arm.sh" 42
run "bad argument" nonzero "usage" -- "$root/helpers/arm.sh" nope

# 9. The holds are one definition: the drain sources hold-labels.sh and the jq list agrees.
check "merge-drain sources the hold labels" grep -q 'hold-labels.sh' "$root/helpers/merge-drain.sh"
check "merge-drain no longer defines them" sh -c "! grep -q '^HELD_LABEL=' '$root/helpers/merge-drain.sh'"
. "$here/hold-labels.sh"
want="[\"$HELD_LABEL\", \"$LANE_HELD_LABEL\", \"$RED_TRUNK_LABEL\"]"
got="$(sed -n 's/^def nm_held: \(.*\);$/\1/p' "$here/needs-maintainer.jq")"
check "needs-maintainer.jq nm_held is $want" test "$got" = "$want"

# 10. arm.sh refuses a pull request that conflicts with an entry ahead, milestone 1054 (an entry
# that conflicts with the queue is caught before it). The check is the real script/queue-conflicts,
# run with the real git against a local fixture: a bare origin with
# `main`, entry #7 ahead in the queue that rewrites line 2 of `shared`, a pull request head that
# rewrites the same line (conflict) and one that changes another file (clean). The wrapper takes
# the head from FIXTURE_HEAD and ignores the arguments arm.sh passes, since the fixture is not on
# GitHub.
fx="$tmp/fixture"
fxgit() { GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@example.invalid \
	GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@example.invalid git -C "$fx/work" "$@" >/dev/null 2>&1; }
fxcommit() { # branch file text -> prints the commit, pushed to the fixture origin as that branch
	fxgit checkout -q -B "$1" "$fxbase" && printf '%s' "$3" >"$fx/work/$2" && fxgit add -A &&
		fxgit commit -q -m "$1" && fxgit push -q origin "$1:refs/heads/$1" &&
		git -C "$fx/work" rev-parse HEAD
}
fxok=0
if mkdir -p "$fx" && git init -q --bare -b main "$fx/origin.git" >/dev/null 2>&1 &&
	git init -q -b main "$fx/work" >/dev/null 2>&1 &&
	printf 'one\ntwo\nthree\n' >"$fx/work/shared" && printf 'x\n' >"$fx/work/other" &&
	fxgit add -A && fxgit commit -q -m base && fxgit remote add origin "$fx/origin.git" &&
	fxgit push -q origin main && fxbase="$(git -C "$fx/work" rev-parse HEAD)" &&
	ahead="$(fxcommit ahead shared 'one
ahead
three
')" && conflicting="$(fxcommit mine shared 'one
mine
three
')" && clean="$(fxcommit clean other 'y
')"; then
	jq -n --arg h "$ahead" '{data:{repository:{mergeQueue:{entries:{nodes:[{position:1,state:"QUEUED",pullRequest:{number:7,headRefOid:$h}}]}}}}}' >"$fx/queue.json" && fxok=1
fi
if [ "$fxok" = 1 ]; then
	cat >"$tmp/check-fixture" <<CHECK
#!/bin/sh
cd "$fx/work" && PATH="$PATH" exec "$root/script/queue-conflicts" --pr 42 --head "\$FIXTURE_HEAD" \\
	--entries-json "$fx/queue.json" --remote "$fx/origin.git" --repo-dir "$fx/work/.git"
CHECK
	chmod +x "$tmp/check-fixture"
	state false false "" false
	FIXTURE_HEAD="$conflicting" ARM_QUEUE_CHECK="$tmp/check-fixture" run "conflict refused" nonzero "NOT ARMED" -- "$root/helpers/arm.sh" 42
	check "conflict: names the file" sh -c "printf '%s' \"\$1\" | grep -q 'shared  (changed by #7)'" _ "$out"
	check "conflict: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
	check "conflict: still unarmed" test "$(cat "$tmp/queued")" = false
	state false false "" false
	FIXTURE_HEAD="$clean" ARM_QUEUE_CHECK="$tmp/check-fixture" run "clean passes" 0 "armed" -- "$root/helpers/arm.sh" 42
	check "clean: merge called" logged "pr merge 42 --auto --merge"
	state false false "" false
	FIXTURE_HEAD="$conflicting" ARM_QUEUE_CHECK="$tmp/check-fixture" run "override arms" 0 "armed" -- "$root/helpers/arm.sh" 42 --despite-conflict "entry 7 is being dequeued"
	check "override: reason commented, then merge" awk '/pr comment 42/{c=NR} /entry 7 is being dequeued/{w=NR} /pr merge 42/{m=NR} END{exit !(c && w && c<m)}' "$tmp/log"
	check "override: ends queued" test "$(cat "$tmp/queued")" = true
	state false false "" false
	FIXTURE_HEAD="$conflicting" ARM_QUEUE_CHECK="$tmp/check-fixture" STUB_COMMENT_FAIL=1 run "override, comment fails" nonzero "could not post" -- "$root/helpers/arm.sh" 42 --despite-conflict "why"
	check "override, comment fails: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
	state true false "" false
	FIXTURE_HEAD="$conflicting" ARM_QUEUE_CHECK="$tmp/check-fixture" run "requeue over a conflict is not re-armed" nonzero "NOT re-armed" -- "$root/script/push" --requeue
	check "requeue over a conflict: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
else
	echo "$me: case 10: could not build the git fixture" >&2; fail=1
fi
state false false "" false
run "override needs a reason" nonzero "needs the reason" -- "$root/helpers/arm.sh" 42 --despite-conflict " "
run "unknown argument" nonzero "usage" -- "$root/helpers/arm.sh" 42 --force
state false false "" false
ARM_QUEUE_CHECK="$tmp/check-unknown" run "check could not tell: armed, and says so" 0 "armed unchecked" -- "$root/helpers/arm.sh" 42
check "check could not tell: merge called" logged "pr merge 42 --auto --merge"

# 12. Two ways into the queue that per-pull-request CI cannot see (the proposal "lint fails in the
# queue after passing on the pull request"). First the lint check on the head: a draft's run skips
# `lint` and GitHub counts the skip as passed, so arming refuses until a real lint has passed.
lint() { printf '%s\n' "$1" >"$tmp/lint"; }
S='{"id":1,"status":"completed","conclusion":"skipped"}'
state false false "" false; lint "[$S]"
run "lint skipped refused" nonzero "is skipped" -- "$root/helpers/arm.sh" 42
check "lint skipped: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
state false false "" false; lint "[$S,{\"id\":2,\"status\":\"in_progress\",\"conclusion\":null}]"
run "lint running after a skip refused" nonzero "still running" -- "$root/helpers/arm.sh" 42
check "lint running after a skip: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
state false false "" false; lint "[$S,{\"id\":2,\"status\":\"completed\",\"conclusion\":\"failure\"}]"
run "lint failed refused" nonzero "is failure" -- "$root/helpers/arm.sh" 42
state false false "" false; lint "[$S,{\"id\":2,\"status\":\"completed\",\"conclusion\":\"success\"}]"
run "lint passed after the skip arms" 0 "armed" -- "$root/helpers/arm.sh" 42
# Falsification of the order: the newest run decides, not the first. A pass followed by a skip refuses.
state false false "" false; lint '[{"id":1,"status":"completed","conclusion":"success"},{"id":2,"status":"completed","conclusion":"skipped"}]'
run "the newest lint decides" nonzero "is skipped" -- "$root/helpers/arm.sh" 42
# A head with no lint yet, or lint running with no skip beside it, arms: GitHub waits for the
# check, and that is how script/push --requeue re-arms right after its push.
state false false "" false; lint '[]'
run "no lint yet arms" 0 "armed" -- "$root/helpers/arm.sh" 42
state false false "" false; lint '[{"id":2,"status":"queued","conclusion":null}]'
run "lint running, no skip, arms" 0 "armed" -- "$root/helpers/arm.sh" 42
state true false "" false; lint '[]'
run "requeue after a push still re-arms" 0 "armed" -- "$root/script/push" --requeue
state false false "" false
STUB_CHECKS_FAIL=1 run "unreadable lint refuses" nonzero "could not read the lint check" -- "$root/helpers/arm.sh" 42

# Then the last queue removal: failed checks at this same head refuse; at an older head, or for
# another reason, arming goes ahead. --despite-failed-checks posts its reason first.
removal() { printf '%s\n' "$1" >"$tmp/removal"; }
state false false "" false; removal "failed_checks g1 h1"
run "re-arm at a failed head refused" nonzero "same head" -- "$root/helpers/arm.sh" 42
check "failed head: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
state false false "" false; removal "FAILED_CHECKS g1 h1"
run "the reason's case does not matter" nonzero "same head" -- "$root/helpers/arm.sh" 42
state true false "" false; removal "failed_checks g1 h1"
run "requeue at a failed head is not re-armed" nonzero "NOT re-armed" -- "$root/script/push" --requeue
state false false "" false; removal "failed_checks g1 h0"
run "failed at an older head arms" 0 "armed" -- "$root/helpers/arm.sh" 42
# A known flake re-arms without the override: a failed group job's log matches a signature. The list
# is a fixture, so the cases do not depend on what helpers/known-flakes.tsv holds today.
printf '# fixture\nprogenitor-oom\tthe progenitor is out of memory\tnotes/swish-check-flake.md\n' >"$tmp/flakes"
state false false "" false; removal "failed_checks g1 h1"; echo "swish-check: could not spawn (the progenitor is out of memory)" >"$tmp/runlog"
ARM_KNOWN_FLAKES="$tmp/flakes" run "failed head, a known flake: armed" 0 "known flake progenitor-oom" -- "$root/helpers/arm.sh" 42
check "known flake: the group's failed log was read" logged "run view 7 --log-failed"
check "known flake: merge called" logged "pr merge 42 --auto --merge"
state false false "" false; removal "failed_checks g1 h1"; echo "an assertion failed in vm_test" >"$tmp/runlog"
ARM_KNOWN_FLAKES="$tmp/flakes" run "failed head, no flake matches: refused" nonzero "NOT ARMED" -- "$root/helpers/arm.sh" 42
check "no flake matches: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
state false false "" false; removal "failed_checks g1 h1"; echo "the progenitor is out of memory" >"$tmp/runlog"
ARM_KNOWN_FLAKES="$tmp/no-such-list" run "failed head, no list: refused" nonzero "NOT ARMED" -- "$root/helpers/arm.sh" 42
state true false "" false; removal "failed_checks g1 h1"; echo "an assertion failed in vm_test" >"$tmp/runlog"
ARM_KNOWN_FLAKES="$tmp/flakes" run "requeue that leaves the failed head: not re-armed" nonzero "NOT re-armed" -- "$root/script/push" --requeue
check "every known flake names a record that exists" sh -c "grep '^[^#]' '$here/known-flakes.tsv' | while IFS='	' read -r n s r; do [ -n \"\$s\" ] && [ -e '$root/'\"\$r\" ] || { echo \"known-flakes.tsv: \$n: no signature, or no record at \$r\" >&2; exit 1; }; done"
state false false "" false; removal "manual g1 h1"
run "removed by hand at this head arms" 0 "armed" -- "$root/helpers/arm.sh" 42
state false false "" false; removal "failed_checks g1 h1"
run "failed-checks override arms" 0 "armed" -- "$root/helpers/arm.sh" 42 --despite-failed-checks "a runner died mid-job"
check "failed-checks override: reason commented, then merge" awk '/pr comment 42/{c=NR} /a runner died mid-job/{w=NR} /pr merge 42/{m=NR} END{exit !(c && w && c<m)}' "$tmp/log"
state false false "" false; removal "failed_checks g1 h1"
STUB_COMMENT_FAIL=1 run "failed-checks override, comment fails" nonzero "could not post" -- "$root/helpers/arm.sh" 42 --despite-failed-checks "why"
check "failed-checks override, comment fails: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
state false false "" false
run "failed-checks override needs a reason" nonzero "needs the reason" -- "$root/helpers/arm.sh" 42 --despite-failed-checks ""
run "failed-checks override needs an argument" nonzero "needs the reason" -- "$root/helpers/arm.sh" 42 --despite-failed-checks

# 13. Lint over the queue it joins (the architect's ruling of 2026-11-11 on question A): a red the
# pull request adds refuses, --despite-lint posts its reason and arms, and "could not tell" arms and
# says so.
state false false "" false
run "stack lint is asked about this pull request" 0 "armed" -- "$root/helpers/arm.sh" 42
check "stack lint: called with the number" logged "arm-check 42"
state false false "" false
ARM_STACK_LINT="$tmp/lint-red" run "red over the queue refused" nonzero "makes lint red on main plus the queue" -- "$root/helpers/arm.sh" 42
check "red over the queue: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
state false false "" false
ARM_STACK_LINT="$tmp/lint-red" run "lint override arms" 0 "armed" -- "$root/helpers/arm.sh" 42 --despite-lint "the entry ahead is being dequeued"
check "lint override: reason commented, then merge" awk '/pr comment 42/{c=NR} /pr merge 42/{m=NR} END{exit !(c && c<m)}' "$tmp/log"
state false false "" false
ARM_STACK_LINT="$tmp/lint-red" STUB_COMMENT_FAIL=1 run "lint override, comment fails" nonzero "could not post" -- "$root/helpers/arm.sh" 42 --despite-lint "why"
check "lint override, comment fails: merge never called" sh -c "! grep -q 'pr merge' '$tmp/log'"
state false false "" false
ARM_STACK_LINT="$tmp/lint-unknown" run "stack lint could not tell: armed, and says so" 0 "armed with that unchecked" -- "$root/helpers/arm.sh" 42
state true false "" false
ARM_STACK_LINT="$tmp/lint-red" run "requeue onto a red stack is not re-armed" nonzero "NOT re-armed" -- "$root/script/push" --requeue
state false false "" false
run "lint override needs a reason" nonzero "needs the reason" -- "$root/helpers/arm.sh" 42 --despite-lint " "

# 11. No instruction names the bare arming command, milestone 1052 (a push to a queued pull request
# re-arms it, and arming refuses a held one). The briefs, the skills, CONTRIBUTING.md, the hook's
# refusal and the drain's comments say helpers/arm.sh or script/push --requeue, so a person following
# them does not skip the holds and the conflict check. `gh pr merge --disable-auto` is not arming
# and does not match. The drain's own requeue_stuck_head is cut out first: it is code, not an
# instruction, and the one ruled exception (#1833) that helpers/needs-maintainer-selftest.sh
# section 5 guards.
bare_arms() { # files... -> each line naming `gh pr merge ... --auto`
	for f in "$@"; do
		awk '/^requeue_stuck_head\(\) \{/ { skip = 1 } !skip { print FILENAME ":" FNR ": " $0 } skip && /^\}/ { skip = 0 }' "$f"
	done | grep -E 'gh pr merge .*--auto' || true
}
scope="$(find "$root/briefs" "$root/notes/skills" -type f -name '*.md' | sort) $root/CONTRIBUTING.md $root/helpers/push-queued-check.sh $root/helpers/merge-drain.sh"
# shellcheck disable=SC2086
bare="$(bare_arms $scope)"
if [ -n "$bare" ]; then
	echo "$me: an instruction names the bare arming command; say helpers/arm.sh <N> or script/push --requeue:" >&2
	printf '  %s\n' "$bare" >&2
	fail=1
fi
# Falsification: one planted in a brief must be found.
printf 'Arm it with `gh pr merge 1 --auto --merge`.\n' >"$tmp/planted.md"
check "case 11 finds a planted bare arm" test -n "$(bare_arms "$tmp/planted.md")"

[ "$fail" = 0 ] && echo "$me: ok"
exit "$fail"
