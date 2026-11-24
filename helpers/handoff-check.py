#!/usr/bin/env python3
# A Claude Code Stop hook: before an agent's turn ends, read what it is about to hand the person it
# is working with, and if that reads like fixable work being reported rather than done, send it back
# once to sort the items.
#
# Why this exists, and why at this rung. AGENTS.md's "We are all owners" section says owning is not
# recording: a cheap, reversible fix reported instead of made is an evasion. That paragraph was
# written on 2026-09-23 after one violation and broken twice more on 2026-09-24, both times by a
# maintainer session on a freshly cleared context: it relayed a lane report as "things worth your
# eye" (a stale line, two pull requests to sequence) instead of doing them, and when corrected it
# saved a memory restating the rule instead of applying it. Three strikes is partsfoundry/record:notes/rule-violations.md's
# threshold for moving a rule up the ladder. Prose in an 11,000-word constitution is rung four; the
# rule was in context both times and lost on salience. A hook is the harness firing it every turn,
# which is the nearest thing to rung two an agent's behaviour has.
#
# It fires at most once per turn: `stop_hook_active` is set when the agent is already continuing
# because of a Stop hook, and blocking again would loop. So a false positive costs one extra look,
# and the agent may answer it by saying every item really is an architect's call.
#
# BUGS. The phrase list is a heuristic over prose, so it misses a handoff worded some other way and
# will occasionally fire on a message that correctly hands an architect a decision. It cannot tell
# a reversible fix from a naming or system-call-surface question; the agent does that sorting when
# prompted, which is the point. It reads only the final turn's text, not what the agent did with
# tools.
#
# The text names the role, not the person: .claude/settings.json is checked in, so every
# contributor's sessions run this hook, and the needs-architect label's rule applies (calef,
# 2026-09-24, on #1216).
#
# Name: provisional. Minted 2026-09-24 by a maintainer session; no architect has ratified it.
import json
import re
import sys

HANDOFF = re.compile(
    r"worth (?:your|a) (?:eye|look)"
    r"|wants? (?:a |an )?(?:one-line |small |quick )?(?:fix|lane|owner)"
    r"|(?:someone|somebody|whoever) (?:should|could|needs to)"
    r"|needs? (?:a )?(?:one-line|small|quick|trivial) (?:fix|change|edit)"
    r"|\bI (?:left|did not fix|didn't fix|have not fixed|haven't fixed|did not touch|didn't touch)\b"
    r"|\bI (?:would )?recommend (?:sequencing|landing|merging|rebasing|fixing|enqueueing)"
    r"|(?:could|should|can) be fixed"
    r"|left (?:it|them|that) (?:alone|for)"
    r"|(?:a |one )(?:cheap|one-line|trivial) fix",
    re.IGNORECASE,
)

REASON = (
    "Before ending: your message hands the person you are working with work that is yours. "
    "AGENTS.md, 'We are all owners': owning is not recording. Sort every item you are reporting "
    "into (a) reversible and within your authority (doc fixes, enqueueing green pull requests, "
    "sequencing, rebases, dispatching a lane): do it or dispatch it now, then report it as done; or "
    "(b) an architect's call (a name, a wire format, the system call surface, a dependency, a design "
    "fork): present it as a decision with a recommendation. If everything is already (b), say so "
    "in one line and stop."
)


def final_turn_text(path):
    texts = []
    with open(path) as f:
        for line in f:
            try:
                entry = json.loads(line)
            except ValueError:
                continue
            msg = entry.get("message") or {}
            content = msg.get("content")
            if entry.get("type") == "user":
                # A tool result also arrives as a user entry; only a person's message starts a turn.
                if isinstance(content, str) or not any(
                        isinstance(c, dict) and c.get("type") == "tool_result" for c in content or []):
                    texts = []
            elif entry.get("type") == "assistant" and isinstance(content, list):
                texts += [c.get("text", "") for c in content
                          if isinstance(c, dict) and c.get("type") == "text"]
    return "\n".join(texts)


def main():
    event = json.load(sys.stdin)
    if event.get("stop_hook_active"):
        return
    path = event.get("transcript_path")
    if not path:
        return
    try:
        text = final_turn_text(path)
    except OSError:
        return
    if HANDOFF.search(text):
        json.dump({"decision": "block", "reason": REASON}, sys.stdout)


if __name__ == "__main__":
    main()
