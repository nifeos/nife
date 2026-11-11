# helpers/open-question.jq: does a pull request or issue hold a question for calef that no ruling
# has answered yet?
#
# lane/architect-queue, 2026-10-06 (UTC). calef: "I need a reliable work queue just like your
# agents." His queue is the `needs-architect` label, and that day it failed both ways: #1783 kept the
# label after he had ruled ("But the label says that the PR needs my attention"), and #1745 changed
# the syscall surface without it. The invariant this file makes checkable: an item carries
# `needs-architect` if and only if it has an open question. Two readers splice it in front of their
# own program, the way helpers/queue-eligible.jq is spliced (jq cannot compose `-f` with an inline
# program): helpers/needs-maintainer.jq's three architect causes, and script/architect-queue.
#
# An ASK is a `## What I need from you` heading (the decisions skill's convention; `###` too) at the
# start of a line, in a comment or in the body. The body counts from the item's creation, since
# GitHub records no time for a body edit. A section whose first line is just "Nothing." or "None."
# (#1651) asks nothing. A heading quoted mid-line in backticks, as the labelers' own comments do, is
# not an ask.
#
# **A task for a person that needs the merge first goes under `## After this merges`** (name
# provisional, lane/after-this-merges-asks-nothing, 2026-11-11 UTC), and no reader here counts it.
# #3476's body asked for a bench session that needs its own merge, under `## What I need from
# you`; the drain read that as an open question and added `needs-architect`, which blocked the merge
# the session waits for. The drain still counts every ask heading, whatever follows it: a question
# it missed could merge without a ruling, and a wrong add costs calef a glance (#1792). What stops
# the wrong add is earlier. `oq_unasked_sections` finds an ask section with no question mark in it
# that is not a "Nothing." section, and helpers/ask-section-check.sh refuses a pull request body
# that has one (.github/workflows/pr-text-ask-check.yml), naming `## After this merges` as where a
# post-merge task goes.
#
# A RULING closes every ask older than it. It is any of:
#   - a comment carrying `<!-- architect-ruling -->`, which script/record-ruling writes;
#   - a comment with a `## Ruling` or `## Sent back` heading, the hand-written forms of 2026-10-06
#     (#1781, #1785, #1786, #1783);
#   - `architect-ruled` or `held-by-lane` being applied, which is how every older ruling was
#     recorded.
# `<!-- architect-ruling partial -->` (script/record-ruling --partial) and a `## Partial ruling`
# heading are records that close nothing.
#
# The input is a node in the shape helpers/merge-drain.sh's `NM_QUERY` asks for: `body`,
# `createdAt`, `url`, `comments.nodes[] {createdAt, url, body}` and `labelEvents.nodes[] {createdAt,
# label {name}}`. A missing field reads as empty, so an older fixture has no ask.
#
# BUGS
#   - A hand-written `## Ruling on Fork 1` closes the forks still unruled beside it (#1785 ruled two
#     forks in two comments). Use `script/record-ruling --partial` for all but the last.
#   - A comment that rules and asks again at once counts as a ruling: the ask is not newer than it.
#     Ask in a comment of its own.
#   - A body edited to add an ask after a ruling is not seen. Ask in a comment.
#   - Only the newest 30 comments and 20 label events are read (the query's window). An ask older
#     than that on a busy pull request is missed.

def oq_ts: if . == null or . == "" then 0 else fromdateiso8601 end;

def oq_heading_re: "(?m)^#{2,3}[ \t]*What I need from you[^\n]*\n";

# The first non-empty line after the heading, or "" when the heading is the text's last line.
def oq_first_line:
  (capture(oq_heading_re + "\\s*(?<l>[^\n]*)").l // "")
  | gsub("^\\s+|\\s+$"; "");

def oq_is_ask:
  (. // "") as $t
  | ($t | test(oq_heading_re))
    and (($t | oq_first_line) | test("^(\\*\\*)?(Nothing|None)\\.?(\\*\\*)?$"; "i") | not);

# Each ask section of a text, as {heading: its line number, first: its first non-empty line, text}:
# from the heading to the next heading of level 1 to 3, or the end. A `####` inside stays in it.
def oq_ask_sections:
  (. // "") | split("\n")
  | reduce to_entries[] as $e ({ out: [], cur: null };
      if ($e.value | test("^#{2,3}[ \t]*What I need from you")) then
        (if .cur != null then .out += [.cur] else . end) | .cur = { heading: ($e.key + 1), lines: [] }
      elif ($e.value | test("^#{1,3}[ \t]")) then
        (if .cur != null then .out += [.cur] else . end) | .cur = null
      elif .cur != null then .cur.lines += [$e.value]
      else . end)
  | (if .cur != null then .out + [.cur] else .out end)
  | map({ heading, text: (.lines | join("\n")),
          first: ([.lines[] | gsub("^\\s+|\\s+$"; "") | select(. != "")] | first // "") });

# The ask sections that ask nothing: no question mark anywhere in the section, and not the
# "Nothing." form. A request for something to happen after the merge reads this way, and belongs
# under `## After this merges`.
def oq_unasked_sections:
  [ oq_ask_sections[]
    | select((.first | test("^(\\*\\*)?(Nothing|None)\\.?(\\*\\*)?$"; "i")) | not)
    | select(.text | contains("?") | not) ];

def oq_is_ruling:
  test("<!-- architect-ruling -->")
  or test("(?m)^#{2,3}[ \t]*(Ruling|Sent back)\\b");

def oq_asks:
  [ (select(.body | oq_is_ask) | { at: .createdAt, url: .url, line: (.body | oq_first_line) }),
    (.comments.nodes[]? | select(.body | oq_is_ask) | { at: .createdAt, url: .url, line: (.body | oq_first_line) }) ];

def oq_rulings:
  [ (.comments.nodes[]? | select(.body // "" | oq_is_ruling) | .createdAt),
    (.labelEvents.nodes[]? | select(.label.name == "architect-ruled" or .label.name == "held-by-lane") | .createdAt) ];

# The newest ask, if no ruling is as new as it; otherwise null.
def oq_open:
  (oq_asks | max_by(.at | oq_ts)) as $ask
  | (oq_rulings | map(oq_ts) | max // 0) as $ruled
  | if $ask != null and ($ask.at | oq_ts) > $ruled then $ask else null end;

# When the newest ruling was recorded, or null.
def oq_last_ruling: (oq_rulings | max_by(oq_ts));

def oq_has_label($name): ([.labels.nodes[]?.name] | index($name)) != null;
