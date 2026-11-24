#!/usr/bin/env python3
"""Which of one day's `calef`-account steps on GitHub an agent session can claim.

**Why this exists.** calef's `gh` token is used by calef and by every agent session on patagonia,
so GitHub credits both to `calef` and records no difference (measured 2026-10-03 UTC: the
`performed_via_github_app` field is null on every `calef` event and comment, because `gh` is an
OAuth App and not a GitHub App). The 2026-10-03 correction of error (the queue judged one pull
request at a time) could not say which dequeues were calef's own for that reason. Milestone 642
(the record should say whether a person or the machinery took a step) is the fix going forward,
and a new identity cannot attribute anything retroactively. Session transcripts can: each Claude
Code session writes every Bash call it made, with a timestamp, under `~/.claude/projects/`.

**What it does.** It lists the day's deliberate `calef`-actor issue events (dequeues, arming,
ready, labels, drafts, renames) on the repository, finds every transcript Bash call that ran a
mutating `gh` command naming the same pull request, and reports each event as claimed by the
session whose call came at most `--window` seconds before it, or as `unmatched`.

**What a result means, which is less than it looks.** A match is evidence and not proof: a session
that touched #N for another reason inside the window claims an event it did not cause. An
`unmatched` event is the weaker half again: it is calef by hand, OR a session whose transcript is
gone, on another machine, or not Claude Code. This is the positive half only, which is why
milestone 642's block refuses a local log as the answer and keeps it as a complement.

Name provisional (milestone 642's lane); `nifeos/record:design/naming.md` is the rule.

EXAMPLES

    python3 helpers/session_steps.py --date 2026-10-03
    python3 helpers/session_steps.py --date 2026-10-03 --kinds removed_from_merge_queue
    python3 helpers/session_steps.py --selftest

On 2026-10-03 the first reported 229 of 250 deliberate `calef` steps claimed by a session. The 10
unmatched dequeues include all four calef recalls making by hand that day (#1530 once, #1534,
#1547, #1557). It also claims three #1530 dequeues for the session where that day's record has
two, which is the coincidence limit below showing itself on real data.

BUGS

- A session that ran any mutating `gh` command on #N inside the window claims every `calef` event
  on #N in it, including one calef took by hand at the same moment.
- Only Claude Code transcripts are read. A session run under opencode (the z.ai week) claims
  nothing, and its steps read as `unmatched`.
- Transcripts age out (Claude Code's `cleanupPeriodDays`, 30 days by default, recalled rather
  than measured), and they exist only on the machine that ran the session.
- Pull request numbers are found by pattern in the command text. A command that computes the
  number at run time (`xargs`, a shell variable) names no number and claims nothing.
- The repository-wide issue events endpoint pages back about 90 days, and further only per issue.
"""

import argparse
import glob
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timedelta

# The events a person or a session chooses to take. `merged`, `closed`, `added_to_merge_queue` and
# `head_ref_deleted` are left out: GitHub credits them to whoever armed the pull request, so they
# follow an earlier step rather than being one.
KINDS = (
    'removed_from_merge_queue', 'auto_merge_enabled', 'auto_merge_disabled', 'ready_for_review',
    'convert_to_draft', 'labeled', 'unlabeled', 'renamed',
)

MUTATING = re.compile(
    r'\bgh\b[^\n]*?\b(?:pr (?:merge|ready|comment|edit|close|create)'
    r'|api[^\n]*?(?:dequeuePullRequest|enqueuePullRequest|enablePullRequestAutoMerge'
    r'|disablePullRequestAutoMerge|convertPullRequestToDraft|/labels|/comments)'
    r'|issue (?:comment|edit))'
)
PR_NUMBER = re.compile(r'(?:#|\bpr \w+ |pull/|issues/|pulls/|number[:=]\s*)(\d{2,5})\b')


def parse_time(s):
    return datetime.fromisoformat(s.replace('Z', '+00:00'))


def claims(events, calls, window):
    """Pair each (time, kind, number) event with the latest call on the same number before it.

    A call is (time, number, session). The call must come first: the transcript stamps the moment
    the model issued the command, and GitHub stamps the moment the command took effect.
    """
    out = []
    for t, kind, number in events:
        best = None
        for ct, cn, session in calls:
            lag = (t - ct).total_seconds()
            if cn == number and 0 <= lag <= window and (best is None or ct > best[0]):
                best = (ct, session)
        out.append((t, kind, number, best[1] if best else None))
    return out


def transcript_calls(day):
    calls = []
    root = os.path.expanduser('~/.claude/projects')
    since = (day - timedelta(days=1)).timestamp()
    prefix = day.strftime('%Y-%m-%d')
    for path in glob.glob(os.path.join(root, '**', '*.jsonl'), recursive=True):
        if os.path.getmtime(path) < since:
            continue
        with open(path, errors='ignore') as f:
            for line in f:
                if '"tool_use"' not in line or 'gh ' not in line:
                    continue
                try:
                    d = json.loads(line)
                except ValueError:
                    continue
                if not d.get('timestamp', '').startswith(prefix):
                    continue
                for c in (d.get('message') or {}).get('content') or []:
                    if not (isinstance(c, dict) and c.get('type') == 'tool_use'
                            and c.get('name') == 'Bash'):
                        continue
                    cmd = c.get('input', {}).get('command', '')
                    if not MUTATING.search(cmd):
                        continue
                    for n in set(PR_NUMBER.findall(cmd)):
                        calls.append((parse_time(d['timestamp']), int(n),
                                      (d.get('sessionId') or '?')[:8]))
    return calls


def github_events(repo, day, actor, kinds):
    prefix = day.strftime('%Y-%m-%d')
    out = subprocess.run(
        ['gh', 'api', '--paginate', f'repos/{repo}/issues/events?per_page=100',
         '--jq', '.[] | [.created_at, .actor.login, .event, .issue.number] | @tsv'],
        check=True, capture_output=True, text=True).stdout
    events = []
    for line in out.splitlines():
        ts, who, kind, number = line.split('\t')
        if ts.startswith(prefix) and who == actor and kind in kinds:
            events.append((parse_time(ts), kind, int(number)))
    return sorted(events)


def selftest():
    t0 = parse_time('2026-10-03T20:00:00Z')
    s = lambda n: t0 + timedelta(seconds=n)
    calls = [(s(0), 1530, 'aaaaaaaa'), (s(100), 1530, 'bbbbbbbb'), (s(0), 1534, 'cccccccc')]
    got = claims([
        (s(120), 'removed_from_merge_queue', 1530),  # the later of two calls claims it
        (s(-10), 'removed_from_merge_queue', 1530),  # before any call: a session cannot claim it
        (s(700), 'labeled', 1534),                   # outside the window
        (s(30), 'labeled', 1547),                    # no call names this pull request
    ], calls, 300)
    want = ['bbbbbbbb', None, None, None]
    if [g[3] for g in got] != want:
        sys.exit(f'session_steps selftest: got {[g[3] for g in got]}, want {want}')


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--date', help='UTC day, YYYY-MM-DD')
    ap.add_argument('--repo', default='nifeos/nife')
    ap.add_argument('--actor', default='calef')
    ap.add_argument('--window', type=int, default=300, help='seconds from call to event')
    ap.add_argument('--kinds', default=','.join(KINDS))
    ap.add_argument('--selftest', action='store_true')
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return
    if not a.date:
        ap.error('--date is required')
    day = datetime.strptime(a.date, '%Y-%m-%d')
    events = github_events(a.repo, day, a.actor, set(a.kinds.split(',')))
    rows = claims(events, transcript_calls(day), a.window)
    tally = {}
    for t, kind, number, session in rows:
        print(f"{t:%Y-%m-%dT%H:%M:%SZ}\t{kind}\t#{number}\t{session or 'unmatched'}")
        hit, total = tally.get(kind, (0, 0))
        tally[kind] = (hit + (session is not None), total + 1)
    for kind, (hit, total) in sorted(tally.items()):
        print(f'# {kind}: {hit} of {total} claimed by a session', file=sys.stderr)


if __name__ == '__main__':
    main()
