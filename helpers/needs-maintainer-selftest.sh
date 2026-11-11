#!/bin/sh
#
# helpers/needs-maintainer-selftest.sh: the detector's decision (helpers/needs-maintainer.jq), fed
# GraphQL responses in the shape helpers/merge-drain.sh asks for, and the drain checked for still
# asking for every field the decision reads and for never arming or enqueueing. Milestone 727
# (a queue eviction goes to a maintainer session), number provisional. script/lint-fleet runs it.
#
# Two fixtures under helpers/needs-maintainer-fixtures/:
#
#   recorded-2026-10-03T23-54-15Z.json   the drain's query, run against the repository at that
#       moment and stored as returned, except that each `body` is emptied (they cite milestones,
#       which script/citations would hold this file to, and none had a `Blocked-by:` line, the
#       only thing the decision reads a body for). #1569 had just been ejected on a
#       `merge_conflict` and read CONFLICTING; everything else was a draft, armed, queued or held. Detection logic that
#       only runs against live GitHub is a defect, so the live shape is pinned here.
#   every-cause.json   one pull request per case, built from recorded event nodes (#1473's
#       `failed_checks` ejection, #1494's `merge_conflict`, #1530's manual dequeue, #1555's stale
#       entry) with the numbers changed so each case stands alone. Cases 20 to 29 are the two
#       causes added on 2026-10-04 (#1640 stacked and unlabelled; #1617 and #1653 red and unowned),
#       built in the same shape. Cases 30 to 37 are `stale-draft` (#1644, 2026-10-05),
#       and 38 and 39 its `parked` exemption (#1745, 2026-10-06), which leave out the
#       label-event alias only `red` reads. Case 40 is case 10 sent back to its lane
#       (`held-by-lane` beside `architect-ruled`, 2026-10-06). Cases 41 and 42 are `unmergeable`
#       (#1795 waiting behind #1745, 2026-10-07): an entry at position 4 reading UNMERGEABLE, and
#       a labeled one whose entry reads AWAITING_CHECKS again. Every entry carries the `position`
#       and head that cause reads.
#   orphan.json   the `orphan` cause (lane/orphan-work, 2026-10-06): one branch per shape in the
#       `refs` listing, and the drafts the drain opens for a branch that never had a pull request.
#       Built by hand in the query's shape; the shapes are the 2026-10-06 survey's (#1787).
#   architect.json   the three architect causes (lane/architect-queue, 2026-10-06): the invariant
#       that `needs-architect` sits on exactly the items with an open question, plus a diff the
#       architect-label rules fire on with no hold. Built by hand; the #1783 and #1745 shapes are
#       cases 202 and 212. The drafts keep every other cause quiet. The held nodes in the two
#       fixtures above (10, 26, 34 and the recorded #1564) gained an open ask comment that day, since
#       the recorded query fetched no comments and a hold with no ask is now itself a cause.
#
# The order of the checks is the order of the ruling: the causes, then clearing, then the
# drain's own shape.

set -e
here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
me="$(basename "$0")"
fx="$here/needs-maintainer-fixtures"

if ! command -v jq >/dev/null 2>&1; then
	echo "$me: jq is not installed, and the merge drain cannot run without it either." >&2
	exit 1
fi

program="$(cat "$here/queue-eligible.jq" "$here/open-question.jq" "$here/needs-maintainer.jq")"
# decide <fixture> <now, ISO> <blockers JSON>: one "number:action[:cause,cause]" per record.
decide() {
	jq -c --arg l needs-maintainer --argjson now "$(jq -n --arg t "$2" '$t | fromdateiso8601')" --argjson b "$3" \
		"$program"'[ nm_decide($l; $now; 30; $b)
			| "\(.number):\(.action)" + (if .action == "clear" then "" else ":" + (.causes | map(.cause) | join(",")) end) ]' "$1"
}
expect() {
	if [ "$2" != "$3" ]; then
		echo "$me: $1" >&2
		echo "  expected $3" >&2
		echo "  got      $2" >&2
		exit 1
	fi
}

# 1. The recorded response: one ejected, conflicting pull request, and nothing else flagged.
expect "the recorded 2026-10-03T23:54:15Z response decided wrong" \
	"$(decide "$fx/recorded-2026-10-03T23-54-15Z.json" 2026-10-03T23:54:15Z '{}')" \
	'["1569:label:conflict,ejected"]'

# 2. Every cause, and every clearing, at 22:00 UTC with a 30-minute grace. #11, #22 and #33 name
#    an open blocker and #12 only resolved ones.
want='["1:label:ejected","2:clear","3:clear","4:label:conflict,ejected","5:clear","6:label:unarmed","7:label:unarmed","9:label:unarmed","12:label:unarmed","14:label:conflict","16:clear","20:label:off-main","24:label:red","27:label:red","29:label:red","30:label:stale-draft","32:label:stale-draft","36:keep:stale-draft","39:clear","41:label:unmergeable","42:clear","45:label:unarmed","47:label:unarmed","50:label:budget","51:keep:budget","52:clear","53:label:budget","1555:keep:stale","1556:clear"]'
got="$(decide "$fx/every-cause.json" 2026-10-03T22:00:00Z '{"11": ["OPEN", "MERGED"], "12": ["MERGED"], "22": ["OPEN"], "33": ["OPEN"]}')"
if [ "$got" != "$want" ]; then
	echo "$me: the needs-maintainer decision is wrong." >&2
	echo "  expected $want" >&2
	echo "  got      $got" >&2
	cat >&2 <<'EOF'
  The cases: 1 ejected (failed_checks) at the head it still has. 2 the same, head moved and armed:
  clear. 3 the same, back in the queue: clear. 4 a merge_conflict ejection still CONFLICTING:
  both causes. 5 the same, conflict fixed and armed: clear. 6 the same, fixed but never re-armed
  for hours: unarmed. 7 dequeued by hand an hour ago and left: unarmed. 8 ready 10 minutes ago:
  nothing yet. 9 ready 60 minutes, unarmed. 10 held needs-architect: nothing. 11 Blocked-by an
  open pull request: nothing. 12 Blocked-by merged ones: unarmed. 13 a conflicting draft: nothing.
  14 conflicting and unarmed: conflict only. 15 a fork: nothing. 16 labelled and queued: clear.
  17 an ejected draft: nothing. 18 armed: nothing. 19 opened ready 15 minutes ago: nothing.
  1555 a merged pull request still in the queue, already labelled: keep. 1556 labelled, merged,
  and out of the queue: clear. 20 ready an hour on another pull request's branch, unarmed:
  off-main (#1640). 21 the same, ready 10 minutes: nothing yet. 22 the same, Blocked-by an open
  pull request: nothing. 23 the same, armed into its base: nothing. 24 armed and ci-failing for an
  hour: red (#1653), keyed on that label and not on a later one. 25 the same, 10 minutes: nothing
  yet. 26 ci-failing but held needs-architect: nothing. 27 ci-failing and unarmed: red, and not
  unarmed beside it. 28 a ci-failing draft: nothing. 29 ci-failing on another base, its label
  older than the fetched events: red. 30 a stacked draft whose head commit is 10 hours old:
  stale-draft (#1644). 31 the same, 4 hours: nothing yet. 32 a draft holding only a 22-hour-old
  claim commit: stale-draft, not exempt. 33 an old draft Blocked-by an open pull request:
  nothing. 34 an old draft held needs-architect: nothing. 35 a fork's old draft: nothing. 36 an
  old draft already labelled: keep. 37 a ready, armed pull request with an old commit: nothing.
  38 an old draft labeled parked: nothing. 39 the same, already labeled: clear. 40 ready an hour,
  unarmed, sent back with held-by-lane and architect-ruled: nothing. 41 armed, queued at position
  4, its entry UNMERGEABLE: unmergeable (#1795). 42 labeled, its entry AWAITING_CHECKS: clear.
  50 an open near-budget issue (milestone 808 (every gate accounts for its time)), seen by both
  the issues list and the budget search: budget, once. 51 the same, already labeled: keep.
  52 a closed near-budget issue still labeled: clear. 53 an open near-budget issue older than the
  issues list reaches, seen only by the budget search: budget. 43 ready an hour, unarmed, its `CI`
  run IN_PROGRESS: nothing yet. 44 the same with `verify` QUEUED: nothing yet. 45 the same with CI and
  verify COMPLETED and only `architect hold` running: unarmed (briefs/gate-in-ci.md: the grace
  counts once the gating runs are idle). 46 the same, both finished 10 minutes ago: nothing yet,
  the grace restarts at the last finish. 47 finished 40 minutes ago: unarmed. The check-run shape is the live query's (checked on
  2026-10-16 against #2236 and #2253).
EOF
	exit 1
fi

# 2b. The orphan cause, at 22:00 UTC: a tip from 10:00 is past the two hours, one from 21:00 is not.
#     The cases: o1 work, no pull request, tip 12 hours old: adopt. o2 the same, an hour old:
#     nothing yet. 103 merged, then a commit pushed past it: orphan. 104 merged and nothing since:
#     nothing (a leftover is hygiene, not lost work). 105 closed unmerged with work: orphan. 106 the
#     same, `parked`: nothing (argon's, #1738). 107 has an open pull request: nothing. 110 closed,
#     already labeled: keep, on an `audit/*` branch, since every prefix is in scope. 111 labeled,
#     now merged whole: clear. o12 a comparison GitHub could not make: nothing. 113 the last of two
#     pull requests, closed: orphan on it, not on the merged #112. `main` and a `gh-readonly-queue/*`
#     candidate: nothing. 120 the drain's own draft, still at the head it adopted: orphan. 121 the
#     same, head moved: nothing. 122 made ready: nothing. 123 `parked`: nothing. 124 a draft with no
#     marker: nothing.
orphan_want='["103:label:orphan","105:label:orphan","110:keep:orphan","111:clear","113:label:orphan","120:label:orphan","null:adopt:orphan"]'
expect "the orphan cause decided wrong" "$(decide "$fx/orphan.json" 2026-10-03T22:00:00Z '{}')" "$orphan_want"
got=$(jq -c --argjson now "$(jq -n '"2026-10-03T22:00:00Z" | fromdateiso8601')" \
	"$program"'[ nm_decide("needs-maintainer"; $now; 30; {}) | select(.action == "adopt") | .branch, .causes[0].head ]' \
	"$fx/orphan.json")
expect "an orphan branch with no pull request was not handed to the shell to adopt, or at the wrong head" "$got" \
	'["lane/case-o1","1111111111111111111111111111111111111111"]'

# 2c. Falsification: each mutation removes one clause 2b depends on, and the decision must then come
#     out different. A clause the mutation cannot find, or an answer that does not move, means 2b
#     no longer tests that clause.
falsify() {
	mutated=$(printf '%s' "$program" | awk -v from="$1" -v to="$2" '
		{ n = index($0, from); if (n) { $0 = substr($0, 1, n - 1) to substr($0, n + length(from)); hit = 1 } print }
		END { exit !hit }') || {
		echo "$me: the falsification for $3 no longer finds its clause: $1" >&2
		exit 1
	}
	out=$(jq -c --arg l needs-maintainer --argjson now "$(jq -n '"2026-10-03T22:00:00Z" | fromdateiso8601')" --argjson b '{}' \
		"$mutated"'[ nm_decide($l; $now; 30; $b)
			| "\(.number):\(.action)" + (if .action == "clear" then "" else ":" + (.causes | map(.cause) | join(",")) end) ]' \
		"$fx/orphan.json")
	if [ "$out" = "$orphan_want" ]; then
		echo "$me: removing $3 left the orphan decision unchanged, so 2b does not test it." >&2
		exit 1
	fi
}
falsify 'select((.compare.behindBy // 0) > 0)' '.' 'the work-past-main test'
falsify 'def nm_orphan_hours: 2;' 'def nm_orphan_hours: 0;' 'the two-hour grace'
falsify 'select($oid == $pr.headRefOid)' '.' "the adopted draft's head check"
falsify 'select($last == null or (nm_parked([$last.labels.nodes[]?.name]) | not))' '.' 'the parked exemption'

# 2d. The architect causes, at 22:00 UTC with a 30-minute grace. 201 held with an open ask:
#     nothing. 202 held after a hand-written `## Ruling` (#1783): hold-no-ask. 203 the same, ruled
#     by script/record-ruling's marker. 204 labeled 10 minutes ago with no ask yet: nothing. 205
#     labeled two hours with no ask: hold-no-ask. 206 an ask, unlabeled: ask-no-hold. 207 the
#     same, 15 minutes old: nothing yet. 208 an ask answered by `architect-ruled` going on after
#     it: nothing. 209 a new ask after a send-back's labels: ask-no-hold. 210 held, and only a
#     partial ruling since the ask: nothing. 211 a "Nothing." section: nothing. 212 the rules fire
#     and no label (#1745): surface-no-hold. 213 the same under `architect-ruled`: nothing. 214 an
#     ask in the body: ask-no-hold. 215 the labeler's own comment quoting the heading: nothing. 216
#     its needs-maintainer cause gone: clear. 217 case 206 already wearing needs-maintainer:
#     clear, and still held. Issues 301 (held, no ask) and 302 (an ask in the body, unlabeled)
#     take the first two causes, and come back as issues.
#     The repair is asymmetric (calef, #1792, 2026-10-06 UTC): `+hold` marks the ADD of
#     needs-architect the drain makes itself (ask-no-hold, surface-no-hold), with no
#     needs-maintainer; hold-no-ask only flags. Section 5b holds the other half: the drain never
#     removes needs-architect.
arch_want='["202:pr:label:hold-no-ask","203:pr:label:hold-no-ask","205:pr:label:hold-no-ask","206:pr:hold+hold:ask-no-hold","209:pr:hold+hold:ask-no-hold","212:pr:hold+hold:surface-no-hold","214:pr:hold+hold:ask-no-hold","216:pr:clear","217:pr:clear+hold","301:issue:label:hold-no-ask","302:issue:hold+hold:ask-no-hold"]'
arch_decide() {
	jq -c --arg l needs-maintainer --argjson now "$(jq -n '"2026-10-03T22:00:00Z" | fromdateiso8601')" \
		"$1"'[ nm_decide($l; $now; 30; {})
			| "\(.number):\(.kind):\(.action)" + (if .hold then "+hold" else "" end)
				+ (if .action == "clear" then "" else ":" + (.causes | map(.cause) | join(",")) end) ]' \
		"$fx/architect.json"
}
expect "the architect causes decided wrong" "$(arch_decide "$program")" "$arch_want"
# Each mutation removes one clause 2d depends on; the decision must move.
arch_falsify() {
	mutated=$(printf '%s' "$program" | awk -v from="$1" -v to="$2" '
		{ n = index($0, from); if (n) { $0 = substr($0, 1, n - 1) to substr($0, n + length(from)); hit = 1 } print }
		END { exit !hit }') || {
		echo "$me: the falsification for $3 no longer finds its clause: $1" >&2
		exit 1
	}
	if [ "$(arch_decide "$mutated")" = "$arch_want" ]; then
		echo "$me: removing $3 left the architect decision unchanged, so 2d does not test it." >&2
		exit 1
	fi
}
arch_falsify 'if $ask != null and ($ask.at | oq_ts) > $ruled then $ask else null end' '$ask' 'a ruling closing an ask'
arch_falsify '(Ruling|Sent back)' '(NEVER-A-HEADING)' 'the hand-written ruling heading'
arch_falsify 'select(($labeled_at | nm_ts) <= $now - $minutes * 60)' 'select(true)' "hold-no-ask's grace"
arch_falsify '(oq_has_label("needs-architect") or oq_has_label("architect-ruled") or oq_has_label("held-by-lane")) | not' 'true' "surface-no-hold's label exemption"
arch_falsify '(.labelEvents.nodes[]? | select(.label.name == "architect-ruled" or .label.name == "held-by-lane") | .createdAt)' 'empty' 'a ruling label closing an ask'
arch_falsify '(Nothing|None)' '(NEVER-EMPTY)' 'the "Nothing." exemption'
arch_falsify 'def nm_hold_causes: ["ask-no-hold", "surface-no-hold"];' 'def nm_hold_causes: [];' 'the drain adding needs-architect itself'
arch_falsify 'def nm_hold_causes: ["ask-no-hold", "surface-no-hold"];' 'def nm_hold_causes: ["ask-no-hold", "surface-no-hold", "hold-no-ask"];' 'hold-no-ask staying flag-only'

# 2e. The `missing-check` cause (lane/drain-missing-check, 2026-10-07 UTC; names provisional), from
#     #1814: CI finished `failure` with every job it had green, and the required "cpu matrix" job was
#     never created, so the armed pull request sat BLOCKED with nothing red to report. Two halves.
#     helpers/missing-check.jq classifies a head from the required contexts, the checks present, the
#     runs and the workflow that owns each context (cases in missing-check-runs.json, #1814's
#     first). needs-maintainer.jq names the cause only once a rerun has been tried. Fixture
#     missing-check.json, at 22:00 UTC on the 7th: 1814 armed, rerun tried, absent: missing-check.
#     1815 the rerun is only now happening: nothing. 1816 nothing absent: nothing. 1817 a draft:
#     nothing. 1818 already labeled, still absent: keep. 1819 labeled, nothing absent now: clear.
mc_program="$(cat "$here/missing-check.jq")"
ncases=$(jq '.cases | length' "$fx/missing-check-runs.json")
i=0
while [ "$i" -lt "$ncases" ]; do
	got=$(jq -c ".cases[$i].input | $mc_program"'[ missing_checks ] | .[0]' "$fx/missing-check-runs.json")
	want=$(jq -c ".cases[$i].want" "$fx/missing-check-runs.json")
	expect "missing-check classification, case $i ($(jq -r ".cases[$i].name" "$fx/missing-check-runs.json"))" "$got" "$want"
	i=$((i + 1))
done
mc_want='["1814:label:missing-check","1818:keep:missing-check","1819:clear"]'
expect "the missing-check cause decided wrong" "$(decide "$fx/missing-check.json" 2026-10-07T22:00:00Z '{}')" "$mc_want"
mc_falsify() {
	mutated=$(printf '%s' "$program" | awk -v from="$1" -v to="$2" '
		{ n = index($0, from); if (n) { $0 = substr($0, 1, n - 1) to substr($0, n + length(from)); hit = 1 } print }
		END { exit !hit }') || {
		echo "$me: the falsification for $3 no longer finds its clause: $1" >&2
		exit 1
	}
	out=$(jq -c --arg l needs-maintainer --argjson now "$(jq -n '"2026-10-07T22:00:00Z" | fromdateiso8601')" --argjson b '{}' \
		"$mutated"'[ nm_decide($l; $now; 30; $b)
			| "\(.number):\(.action)" + (if .action == "clear" then "" else ":" + (.causes | map(.cause) | join(",")) end) ]' \
		"$fx/missing-check.json")
	if [ "$out" = "$mc_want" ]; then
		echo "$me: removing $3 left the missing-check decision unchanged, so 2e does not test it." >&2
		exit 1
	fi
}
mc_falsify 'and .missingChecks.tried == true' 'and true' 'the rerun-first rule'
mc_falsify 'select(.isDraft == false and .autoMergeRequest != null' 'select(true and .autoMergeRequest != null' "the draft exemption"
# 2f. The `stuck-head` cause (lane/drain-stuck-queue-head, 2026-10-07 UTC; names provisional), from
#     #1824: a queue entry AWAITING_CHECKS from 16:38 to 19:42 UTC although every run of its group
#     had finished by 17:01 (CI and verify `failure`, every listed job green). Two halves.
#     helpers/stuck-head.jq summarizes the recorded runs response (stuck-head-runs-1824.json) and
#     says nothing while a run is unfinished or none exists. The decision, at 19:30 UTC with a
#     10-minute threshold, reads fixture stuck-head.json: 1824 finished 2.5 hours ago: re-queue. 1850
#     finished 5 minutes ago: nothing yet. 1860 stuck, and the drain's own marker comment from an
#     earlier stall is on it: label (second stall, no re-queue). 1824 is the first stall: `requeue`
#     (calef ruled the drain recovers it, #1833), no label. 1851 runs still going (no verdict): nothing. 1852 not
#     AWAITING_CHECKS: nothing. 1853 labeled, no longer stuck: clear.
sh_got=$(jq -c -f "$here/stuck-head.jq" "$fx/stuck-head-runs-1824.json" | jq -c '{completedAt, names: (.runs | map(.name)), conclusions: (.runs | map(.conclusion))}')
expect "stuck-head.jq read #1824's recorded group wrong" "$sh_got" \
	'{"completedAt":"2026-10-07T17:01:00Z","names":["CI","verify","ready status","architect hold","empty diff"],"conclusions":["failure","failure","success","success","success"]}'
if [ -n "$(jq '.workflow_runs[0].status = "in_progress"' "$fx/stuck-head-runs-1824.json" | jq -c -f "$here/stuck-head.jq")" ] ||
	[ -n "$(printf '{"workflow_runs":[]}' | jq -c -f "$here/stuck-head.jq")" ]; then
	echo "$me: stuck-head.jq called a group with a running run, or no runs, finished." >&2
	exit 1
fi
stuck_want='["1824:requeue:stuck-head-requeue","1853:clear","1860:label:stuck-head"]'
stuck_decide() {
	jq -c --arg l needs-maintainer --argjson now "$(jq -n '"2026-10-07T19:30:00Z" | fromdateiso8601')" \
		"$1"'[ nm_decide($l; $now; 30; {})
			| "\(.number):\(.action)" + (if .action == "clear" then "" else ":" + (.causes | map(.cause) | join(",")) end) ]' \
		"$fx/stuck-head.json"
}
expect "the stuck-head cause decided wrong" "$(stuck_decide "$program")" "$stuck_want"
for m in 'def nm_stuck_head_minutes: 10;|def nm_stuck_head_minutes: 0;|the ten-minute threshold' \
	'.state == "AWAITING_CHECKS" and .stuckHead|.stuckHead|the AWAITING_CHECKS test' \
	'contains("needs-maintainer:stuck-head")|contains("NEVER-MATCHES")|the earlier-stall marker (a second stall would re-queue forever)' \
	'elif .requeue then "requeue"|elif false then "requeue"|the requeue action'; do
	mutated=$(printf '%s' "$program" | awk -v from="${m%%|*}" -v to="$(printf '%s' "$m" | cut -d'|' -f2)" '
		{ n = index($0, from); if (n) { $0 = substr($0, 1, n - 1) to substr($0, n + length(from)); hit = 1 } print }
		END { exit !hit }') || {
		echo "$me: the falsification for ${m##*|} no longer finds its clause." >&2
		exit 1
	}
	if [ "$(stuck_decide "$mutated")" = "$stuck_want" ]; then
		echo "$me: removing ${m##*|} left the stuck-head decision unchanged, so 2f does not test it." >&2
		exit 1
	fi
done

# 3. The episode keys the comment markers are built from, and the evidence the comment names.
got=$(jq -c --argjson now "$(jq -n '"2026-10-03T22:00:00Z" | fromdateiso8601')" \
	"$program"'[ nm_decide("needs-maintainer"; $now; 30; {}) | select(.number == 1 or .number == 1555 or .number == 9 or .number == 20 or .number == 24 or .number == 29 or .number == 30) | .causes[] | [.cause, .key] ]' \
	"$fx/every-cause.json")
expect "a cause's episode key came back wrong" "$got" \
	'[["ejected","2026-10-03T01:45:00Z"],["unarmed","2026-10-03T21:00:00Z"],["off-main","2026-10-03T21:00:00Z"],["red","2026-10-03T21:00:00Z"],["red","2026-10-03T00:00:00Z"],["stale-draft","2026-10-03T12:00:00Z"],["stale","2026-10-03T22:23:57Z"]]'
got=$(jq -c --argjson now "$(jq -n '"2026-10-03T22:00:00Z" | fromdateiso8601')" \
	"$program"'[ nm_decide("needs-maintainer"; $now; 30; {}) | select(.number == 1) | .causes[0] | .group, .head ]' \
	"$fx/every-cause.json")
expect "an ejection's group commit or head came back wrong" "$got" \
	'["47d8f11eee8260aa479d3905a50900008694fef2","4124d6390de36d200e2797616761b584e9dcd850"]'

# 3b. An UNMERGEABLE entry is keyed on its head, and names its position and every entry ahead of
#     it with that entry's head, which the comment's pairwise conflict check reads.
got=$(jq -c --argjson now "$(jq -n '"2026-10-03T22:00:00Z" | fromdateiso8601')" \
	"$program"'[ nm_decide("needs-maintainer"; $now; 30; {}) | select(.number == 41) | .causes[0] | .key, .position, .ahead ]' \
	"$fx/every-cause.json")
expect "an UNMERGEABLE entry's key, position or the entries ahead of it came back wrong" "$got" \
	'["H41",4,[{"number":3,"head":"4124d6390de36d200e2797616761b584e9dcd850"},{"number":16,"head":"H16"},{"number":1555,"head":"H1555"}]]'

# 4. The drain splices the decision and asks for every field it reads.
got=$(jq -c --argjson now "$(jq -n '"2026-10-03T22:00:00Z" | fromdateiso8601')" \
	"$program"'[ nm_decide("needs-maintainer"; $now; 30; {}) | select(.number >= 50 and .number <= 53) | "\(.number):\(.kind):" + (.causes | map(.key) | join(",")) ]' \
	"$fx/every-cause.json")
expect "a budget issue's kind or episode key came back wrong (milestone 808)" "$got" \
	'["50:issue:50@2026-10-03T06:00:00Z","51:issue:51@2026-10-02T06:00:00Z","52:issue:","53:issue:53@2026-09-20T06:00:00Z"]'
# The label is spelled in three places, checked from this side only (milestone 889 (the tree's gates
# stop depending on the fleet tooling)): helpers/ci_job_times.py, which opens the issue, is a tree
# tool and no longer reads the fleet's two files, so the exact spellings are compared here.
grep -qF 'def nm_budget_label: "near-budget";' "$here/needs-maintainer.jq" || {
	echo "$me: needs-maintainer.jq's nm_budget_label is no longer \"near-budget\"." >&2
	exit 1
}
for pair in 'merge-drain.sh|NM_BUDGET_LABEL="near-budget"' 'ci_job_times.py|LABEL = "near-budget"'; do
	where="$here/${pair%%|*}"
	if ! grep -qF "${pair#*|}" "$where"; then
		echo "$me: $where does not spell the near-budget label as ${pair#*|}, which needs-maintainer.jq's nm_budget_label holds." >&2
		exit 1
	fi
done

f="$here/merge-drain.sh"
if ! grep -qF '"$(cat "$ELIGIBLE_JQ" "$OQ_JQ" "$NM_JQ")"' "$f"; then
	echo "$me: merge-drain.sh does not splice queue-eligible.jq, open-question.jq and needs-maintainer.jq, so nothing decides." >&2
	exit 1
fi
for field in 'mergeQueue(branch: "main")' 'position enqueuedAt state' 'mergedAt headRefOid' enqueuedAt mergedAt isDraft baseRefName isCrossRepository \
	headRefName headRefOid createdAt mergeable body autoMergeRequest REMOVED_FROM_MERGE_QUEUE_EVENT \
	ADDED_TO_MERGE_QUEUE_EVENT READY_FOR_REVIEW_EVENT AUTO_MERGE_DISABLED_EVENT LABELED_EVENT committedDate statusCheckRollup completedAt beforeCommit \
	'parents(first: 2)' 'search(query: $labelled' 'refs(refPrefix: "refs/heads/"' 'compare(headRef: "main") { behindBy }' \
	associatedPullRequests 'comments(last: 30) { nodes { createdAt url body } }' 'issues(states: OPEN' \
	'budget: search(query: $budget' '... on Issue { number state' __typename \
	'headCommit { oid }' 'STUCK_JQ="$(dirname "$0")/stuck-head.jq"' 'architectSurface:' 'missingChecks:' 'RULES_PY="$(dirname "$0")/architect-label-rules.py"' 'OQ_JQ="$(dirname "$0")/open-question.jq"'; do
	if ! grep -qF -- "$field" "$f"; then
		echo "$me: merge-drain.sh's needs-maintainer query does not ask for $field, which the decision reads." >&2
		exit 1
	fi
done

# 5. calef's first ruling on #1564, held by a gate rather than by memory: the drain never arms
#    and never enqueues. Comments are skipped, so the history can still name what it did, and so
#    is a command quoted in a pull request comment (\`gh pr merge ...\`, \`helpers/arm.sh ...\`),
#    which tells a person what to type rather than typing it. A call to helpers/arm.sh arms too.
#
#    **One marked exception, by name, and a foot gun** (calef, #1833, 2026-10-07 UTC): the function
#    `requeue_stuck_head` may dequeue and enqueue, because a group whose runs all finished and
#    which GitHub neither merged nor ejected held #1824 at the head of the queue for 3 hours. Its
#    body is cut out before the grep, so the gate still forbids a re-arm in every other function,
#    and it must have exactly one caller. Do not widen the grep to admit anything else, and do not
#    copy the exception to a second cause: that is how the 2026-10-03 re-queue fights began.
armers_in() {
	awk '/^requeue_stuck_head\(\) \{/ { skip = 1 } !skip { print } skip && /^\}/ { skip = 0 }' "$1" |
		grep -v '^[[:space:]]*#' | grep -e 'gh pr merge' -e 'enqueuePullRequest' -e 'arm\.sh' |
		grep -v -e '\\`gh pr merge' -e '\\`helpers/arm\.sh' || true
}
armers=$(armers_in "$f")
if [ -n "$armers" ]; then
	echo "$me: merge-drain.sh arms or enqueues again, which calef ruled out on #1564 (2026-10-03):" >&2
	printf '  %s\n' "$armers" >&2
	echo "  A pull request a lane did not arm is labelled needs-maintainer instead." >&2
	echo "  The one exception is requeue_stuck_head (calef, #1833)." >&2
	exit 1
fi
callers=$(grep -v '^[[:space:]]*#' "$f" | grep -c 'requeue_stuck_head' || true)
if [ "$callers" != "2" ]; then
	echo "$me: requeue_stuck_head must be defined once and called once (found $callers mentions); it is the one exception to #1564." >&2
	exit 1
fi
# Falsification: a re-arm planted in any other function must trip the gate.
planted=$(mktemp)
sed 's|^dequeue_held() {|dequeue_held() {\
	w gh pr merge 1 --auto --merge|' "$f" >"$planted"
if [ -z "$(armers_in "$planted")" ]; then
	rm -f "$planted"
	echo "$me: section 5 no longer catches a re-arm planted in dequeue_held, so it guards nothing." >&2
	exit 1
fi
sed 's|^dequeue_held() {|dequeue_held() {\
	"$here/arm.sh" 1|' "$f" >"$planted"
if [ -z "$(armers_in "$planted")" ]; then
	rm -f "$planted"
	echo "$me: section 5 no longer catches a helpers/arm.sh call planted in dequeue_held, so it guards nothing." >&2
	exit 1
fi
rm -f "$planted"

# 5b. calef's ruling on #1792 (2026-10-06 UTC), the half no fixture can show: the drain adds
#     needs-architect and never removes it, since a wrong removal could merge a pull request without
#     his ruling. Comments are skipped, as in 5.
removers=$(grep -v '^[[:space:]]*#' "$f" | grep -e '--remove-label "\$HELD_LABEL"' -e '--remove-label needs-architect' -e 'labels/needs-architect' || true)
if [ -n "$removers" ]; then
	echo "$me: merge-drain.sh removes needs-architect, which calef ruled out on #1792 (2026-10-06):" >&2
	printf '  %s\n' "$removers" >&2
	echo "  hold-no-ask flags needs-maintainer instead; a maintainer checks before the label comes off." >&2
	exit 1
fi
if ! grep -qF -- '--add-label "$HELD_LABEL"' "$f"; then
	echo "$me: merge-drain.sh no longer adds needs-architect for ask-no-hold and surface-no-hold (calef, #1792)." >&2
	exit 1
fi

echo "needs maintainer: ejected, conflicting, stale, unarmed, off-main, red, stale-draft, orphan, stuck-head and unmergeable pull requests, the three architect-queue causes, and a CI job near its budget, labeled, each cleared when its cause goes; the drain splices it and never arms, except to re-queue a stuck head once (#1833)"
