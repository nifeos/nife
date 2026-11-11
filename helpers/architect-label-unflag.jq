# helpers/architect-label-unflag.jq: may the architect labeler take back a `needs-architect` it put
# on? Spliced after helpers/open-question.jq, which supplies oq_asks and oq_rulings. Read by
# helpers/architect-label-unflag.sh, which the labeler (.github/workflows/architect-label.yml) runs
# only after helpers/architect-label-rules.py found nothing against every merge base and no other
# pull request's ruling covered a finding. Provisional name.
#
# The input is the pull request node helpers/architect-label-unflag.sh asks for. The output is one
# object: `remove` (true only when every condition holds), `why` (the first condition that failed,
# or "" when none did), `head`, `at` and `by` (when and by whom the label went on), and `rules`, the
# "rule: file" pairs the labeler's own finding comments named.
#
# A hold is calef's alone, so this undoes only the flagger's own false flag. Each condition below
# keeps the label, and a missing field keeps it too:
#   - `needs-architect` is not on, or `architect-ruled` or `held-by-lane` is;
#   - the newest time it went on was not a bot's doing (a person added it, or no event is in the
#     window), since a person's hold means a person has a question;
#   - any ruling was ever recorded here, partial ones included, or any ask was ever posted, answered
#     or not (helpers/open-question.jq's own readers);
#   - the pull request adds a correction of error, which coe-architect-label.yml flags as a bot
#     too and these rules do not read;
#   - the changed-file list is longer than one page, so that question could not be answered.

def unflag_partial:
  test("<!-- architect-ruling partial -->") or test("(?m)^#{2,3}[ \t]*Partial ruling\\b");

def unflag_coe_record:
  test("^notes/(coes|corrections)/[0-9]{4}-[0-9]{2}-[0-9]{2}-[^/]+\\.md$");

# The "rule: file" pairs of the newest finding comment a bot posted: the labeler's own, or the merge
# drain's when it flagged a surface the labeler missed.
def unflag_rules:
  [ .comments.nodes[]?
    | select(.author.__typename == "Bot")
    | .body // ""
    | select(contains("architect-label-rules-finding") or contains("AN ARCHITECT'S SURFACE MOVED")) ]
  | last // ""
  | [ scan("\\(([a-z][a-z_-]*), `([^`]+)`") | "\(.[0]): \(.[1])" ] | unique;

def unflag:
  ([.labels.nodes[]?.name]) as $labels
  | ([.labelEvents.nodes[]? | select(.label.name == "needs-architect")] | last) as $added
  | { head: .headRefOid, at: ($added.createdAt // null), by: ($added.actor.login // null),
      rules: unflag_rules }
  + (if ($labels | index("needs-architect")) == null then {why: "needs-architect is not on"}
     elif ($labels | index("architect-ruled")) != null then {why: "architect-ruled is on"}
     elif ($labels | index("held-by-lane")) != null then {why: "held-by-lane is on"}
     elif $added == null then {why: "no event in the window shows who added needs-architect"}
     elif $added.actor.__typename != "Bot" then {why: "a person (\($added.actor.login // "unknown")) added needs-architect"}
     elif (oq_rulings | length) > 0 then {why: "a ruling is recorded"}
     elif ([.comments.nodes[]? | .body // "" | select(unflag_partial)] | length) > 0 then {why: "a partial ruling is recorded"}
     elif (oq_asks | length) > 0 then {why: "a question for calef was asked here"}
     elif .files.pageInfo.hasNextPage != false then {why: "the changed-file list could not be read whole"}
     elif ([.files.nodes[]? | select(.changeType == "ADDED") | .path | select(unflag_coe_record)] | length) > 0
       then {why: "it adds a correction of error, which coe-architect-label.yml flags"}
     else {why: ""} end)
  | .remove = (.why == "");

unflag
