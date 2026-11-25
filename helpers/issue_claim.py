#!/usr/bin/env python3
"""Claim a GitHub issue before work starts on it, refuse a claim on one that is held, and release.

    helpers/issue_claim.py status  --issue N [--repo OWNER/REPO]
    helpers/issue_claim.py claim   --issue N --branch B [--repo OWNER/REPO]   # a lane; script/claim --issue
    helpers/issue_claim.py link    --issue N --comment ID --pull-request URL [--repo OWNER/REPO]
    helpers/issue_claim.py release --issue N [--reason TEXT] [--repo OWNER/REPO]
    helpers/issue_claim.py drop    --issue N --comment ID [--repo OWNER/REPO]  # undo a claim never used
    helpers/issue_claim.py comment                                   # the workflow: /claim, /release
    helpers/issue_claim.py sweep   [--repo OWNER/REPO]               # the workflow: stale claims
    helpers/issue_claim.py --selftest                                # fixtures, no network

The proposal is nifeos/record:design/roadmap/proposals/an-issue-is-claimed-before-work-starts.md.
Name: provisional, 2026-11-25 (UTC), as are the `/claim` and `/release` commands, the marker
spelling and the stale limit. A python module under `helpers/`, outside `script/names`' scope, so
its provenance is this paragraph.

# WHAT A CLAIM IS

A claim is a comment on the issue that this helper writes, carrying a marker line a reader does not
see (`<!-- issue-claim: claim holder=<login> lane=<branch> -->`), and the holder's login among the
issue's assignees. The comment is the record and the assignee is the flag in the issue list. Every
agent here acts through one person's account, so the assignee alone cannot tell two lanes apart;
the comment names the lane and its branch, and later its draft pull request (`pull=owner/repo#N`
in the marker, once `link` writes it).

An issue is held when any one of these is true, which is the strict reading on purpose:

1. A claim comment that no release comment follows. The first such claim wins; a claim written
   while another one stands is ignored, so a race between two lanes has one winner.
2. Anyone is assigned, including a person assigned by hand with no comment at all.
3. An open pull request (draft or not) closes it with a keyword (`Closes #N`).

Only a marker written by a trusted author counts: `github-actions[bot]` (the workflow) or an author
whose association with the repository is OWNER, MEMBER or COLLABORATOR. An outsider who pastes a
marker into their own comment changes nothing.

# HOW ONE ENDS

The pull request that names the issue with a closing keyword merges and closes it; a `/release`
by the holder or a member; `release` by a lane; or the sweep, once the claim is `STALE_DAYS` old
and no open pull request closes the issue. A claim whose marker names a pull request that is still
open stands too, so a lane that edits `Closes #N` out of its draft does not lose a live claim. A
hand assignment with no claim comment is never
released by the sweep: a person made it, and a person undoes it.

# BUGS

- `sweep` reads the comments of every open issue that has any, two calls each. That is fine at
  tens of issues and slow at thousands; the search API could narrow it.
- The check before a claim and the claim itself are two calls, so two claimants can both pass the
  check. Both sides then post and read back: the first claim comment wins, a lane deletes its
  losing comment and refuses, and the workflow rewrites its losing comment into a refusal. The
  window costs a retry, never a shared issue, and it needs no concurrency group, which would let
  an unrelated comment cancel a pending `/claim` and leave it with no reply.
- The one-claim limit for an outsider reads the issues assigned to them through the REST issues
  list, not search, which lags and has a small rate limit. Two `/claim` comments on two issues in
  the same second can both pass the first count, so each run counts again after it assigns, and
  the claim that is not among the oldest OUTSIDER_LIMIT withdraws itself. A claim whose
  assignment GitHub refused is not assigned, so it does not count against the limit.
- A lane needs `gh api user` to name the account it claims as. A token that cannot answer it, such
  as an App installation token, makes `claim` refuse before it posts anything.
"""

import json
import os
import re
import subprocess
import sys
from datetime import datetime, timedelta, timezone

# How long a claim may stand with no open pull request closing the issue (provisional, 2026-11-25).
STALE_DAYS = 14
# How many open claims a person without write access may hold in one repository (provisional).
OUTSIDER_LIMIT = 1
# An issue carrying one of these labels waits on a ruling and cannot be claimed.
UNCLAIMABLE_LABELS = ("needs-architect",)
TRUSTED_ASSOCIATIONS = ("OWNER", "MEMBER", "COLLABORATOR")
WORKFLOW_BOT = "github-actions[bot]"

MARKER = re.compile(r"^<!-- issue-claim: (claim|release)((?: [a-z]+=[^ ]+)*) -->$", re.M)
FIELD = re.compile(r" ([a-z]+)=([^ ]+)")
COMMAND = re.compile(r"^/(claim|release)\s*$", re.M)
# A login or a branch goes into a marker field, so neither may carry a space or end the comment.
SAFE_VALUE = re.compile(r"^[A-Za-z0-9._/#\[\]-]+$")
PULL_URL = re.compile(r"^https://github\.com/([A-Za-z0-9._-]+/[A-Za-z0-9._-]+)/pull/([0-9]+)$")


class Claim:
    """One claim comment: who holds it, the lane if a lane made it, when, and the comment's id."""

    def __init__(self, holder, lane, at, comment_id, pull=None):
        self.holder, self.lane, self.at, self.comment_id = holder, lane, at, comment_id
        self.pull = pull

    def describe(self):
        if self.lane:
            return f"lane `{self.lane}` (account {self.holder}), claimed {self.at:%Y-%m-%d}"
        return f"@{self.holder}, claimed {self.at:%Y-%m-%d}"


def parse_time(text):
    return datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)


def trusted(comment):
    user = (comment.get("user") or {}).get("login", "")
    return user == WORKFLOW_BOT or comment.get("author_association") in TRUSTED_ASSOCIATIONS


def standing_claim(comments):
    """The claim in force, or None. comments: REST issue comments, oldest first.

    The first trusted claim after the last trusted release wins. A claim that arrives while one
    stands is ignored rather than replacing it, so a second claimant never steals.
    """
    current = None
    for comment in comments:
        if not trusted(comment):
            continue
        for kind, fields in MARKER.findall(comment.get("body") or ""):
            values = dict(FIELD.findall(fields))
            if kind == "release":
                current = None
            elif current is None:
                current = Claim(values.get("holder", ""), values.get("lane"),
                                parse_time(comment["created_at"]), comment["id"],
                                values.get("pull"))
    return current


def holds(issue, comments, open_pulls):
    """Every reason the issue is held, as sentences; an empty list means anyone may claim it.

    issue: the REST issue. comments: its comments, oldest first. open_pulls: numbers of open pull
    requests that close it with a keyword.
    """
    reasons = []
    claim = standing_claim(comments)
    if claim:
        reasons.append(f"claimed by {claim.describe()}")
    claimed_logins = {claim.holder} if claim else set()
    others = [a["login"] for a in issue.get("assignees") or [] if a["login"] not in claimed_logins]
    if others:
        reasons.append("assigned to " + ", ".join("@" + login for login in others))
    if open_pulls:
        reasons.append("closed by open pull request " + ", ".join(f"#{n}" for n in open_pulls))
    return reasons


def refusal(issue, comments, open_pulls):
    """Why a new claim is refused, or None when the issue is free to claim."""
    if issue.get("state") != "open":
        return "this issue is closed"
    if issue.get("pull_request"):
        return "this is a pull request; claims are made on issues"
    for label in issue.get("labels") or []:
        if label.get("name") in UNCLAIMABLE_LABELS:
            return f"this issue carries `{label['name']}`, so it waits on a ruling"
    reasons = holds(issue, comments, open_pulls)
    if reasons:
        return "this issue is held: " + "; ".join(reasons)
    return None


def is_stale(comments, open_pulls, now, pull_is_open=lambda ref: False):
    """The standing claim when the sweep should release it, else None.

    pull_is_open(ref) answers for the pull request a lane's marker names (`owner/repo#N`), so a
    claim outlives an edit that drops `Closes #N` from a draft that is still open.
    """
    claim = standing_claim(comments)
    if not claim or open_pulls or now - claim.at <= timedelta(days=STALE_DAYS):
        return None
    if claim.pull and pull_is_open(claim.pull):
        return None
    return claim


def pull_reference(url):
    """`owner/repo#N` for a pull request's web address, or None."""
    found = PULL_URL.match(url.strip())
    return f"{found.group(1)}#{found.group(2)}" if found else None


def won(comments, comment_id):
    """True when the claim comment comment_id is the one in force after a read-back."""
    claim = standing_claim(comments)
    return claim is not None and claim.comment_id == comment_id


def over_limit(held, mine, limit):
    """True when the author's claim on issue mine is past the outsider limit and must withdraw.

    held: (issue number, claimed at or None) for every open issue assigned to the author, mine
    included; None is an assignment with no claim comment, such as one made by hand, and counts as
    oldest. The oldest `limit` stand and the rest withdraw. Every run that sees two claims orders
    them the same way, so a burst keeps exactly `limit`.
    """
    floor = datetime.min.replace(tzinfo=timezone.utc)
    order = sorted(held, key=lambda entry: (entry[1] or floor, entry[0]))
    return mine not in [number for number, _ in order[:limit]]


def marker(kind, **fields):
    for value in fields.values():
        if not SAFE_VALUE.match(value):
            raise SystemExit(f"issue_claim: refusing an unsafe marker value {value!r}")
    return f"<!-- issue-claim: {kind}" + "".join(f" {k}={v}" for k, v in fields.items()) + " -->"


def claim_body(holder, lane=None, pull_request=None):
    if lane:
        text = (f"Claimed by lane `{lane}`, written by an agent; the account shown is the person "
                f"running it. Branch `{lane}`.")
        text += f" Draft pull request: {pull_request}." if pull_request else " Draft pull request to follow."
        ref = pull_reference(pull_request) if pull_request else None
        if pull_request and not ref:
            raise SystemExit(f"issue_claim: {pull_request!r} is not a pull request's address")
        mark = marker("claim", holder=holder, lane=lane, **({"pull": ref} if ref else {}))
    else:
        text = (f"Claimed by @{holder}. Open a pull request whose description says `Closes #<this "
                f"issue>`; the claim is released if none is open {STALE_DAYS} days after this "
                f"comment. `/release` gives it up.")
        mark = marker("claim", holder=holder)
    return text + "\n\n" + mark


# The GitHub side. Everything below calls `gh`; everything above is pure and tested by --selftest.

def gh(*args, data=None):
    proc = subprocess.run(["gh", *args], input=json.dumps(data) if data is not None else None,
                          capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit(f"issue_claim: gh {' '.join(args[:3])} failed: {proc.stderr.strip()}")
    return json.loads(proc.stdout) if proc.stdout.strip() else None


def gh_list(endpoint):
    """Every item of a paged list endpoint, one `--jq` line each (older `gh` has no --slurp)."""
    proc = subprocess.run(["gh", "api", "--paginate", "--jq", ".[] | tojson", endpoint],
                          capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit(f"issue_claim: gh api {endpoint} failed: {proc.stderr.strip()}")
    return [json.loads(line) for line in proc.stdout.splitlines() if line.strip()]


def fetch(repo, number):
    issue = gh("api", f"repos/{repo}/issues/{number}")
    comments = gh_list(f"repos/{repo}/issues/{number}/comments")
    owner, name = repo.split("/", 1)
    query = ("query($o:String!,$n:String!,$i:Int!){repository(owner:$o,name:$n){issue(number:$i)"
             "{closedByPullRequestsReferences(first:20,includeClosedPrs:false)"
             "{nodes{number state}}}}}")
    graph = gh("api", "graphql", "-f", f"query={query}", "-f", f"o={owner}", "-f", f"n={name}",
               "-F", f"i={number}")
    nodes = graph["data"]["repository"]["issue"]["closedByPullRequestsReferences"]["nodes"]
    return issue, comments, [n["number"] for n in nodes if n["state"] == "OPEN"]


def post(repo, number, body):
    return gh("api", "-X", "POST", f"repos/{repo}/issues/{number}/comments", "--input", "-",
              data={"body": body})


def assign(repo, number, login):
    gh("api", "-X", "POST", f"repos/{repo}/issues/{number}/assignees", "--input", "-",
       data={"assignees": [login]})


def unassign(repo, number, login):
    gh("api", "-X", "DELETE", f"repos/{repo}/issues/{number}/assignees", "--input", "-",
       data={"assignees": [login]})


def whoami():
    """The login a lane claims as. Refuses, before anything is posted, when the token cannot say."""
    proc = subprocess.run(["gh", "api", "user", "--jq", ".login"], capture_output=True, text=True)
    login = proc.stdout.strip()
    if proc.returncode != 0 or not SAFE_VALUE.match(login):
        raise SystemExit("issue_claim: cannot tell which account this lane acts as (`gh api user`: "
                         f"{proc.stderr.strip() or 'no login'}). An App installation token cannot "
                         "answer it; claim with a user's token. Nothing was posted.")
    return login


def pull_is_open(ref):
    repo, number = ref.split("#", 1)
    return gh("api", f"repos/{repo}/pulls/{number}").get("state") == "open"


def assigned_claims(repo, author):
    """(issue number, claimed at or None) for every open issue in repo assigned to author."""
    held = []
    for issue in gh_list(f"repos/{repo}/issues?state=open&assignee={author}&per_page=100"):
        if issue.get("pull_request"):
            continue
        _, comments, _ = fetch(repo, issue["number"])
        claim = standing_claim(comments)
        held.append((issue["number"], claim.at if claim and claim.holder == author else None))
    return held


def current_repo():
    return os.environ.get("GITHUB_REPOSITORY") or gh("repo", "view", "--json", "nameWithOwner")["nameWithOwner"]


def command_status(repo, number):
    issue, comments, pulls = fetch(repo, number)
    reason = refusal(issue, comments, pulls)
    print(f"{repo}#{number}: {reason or 'free to claim'}")
    return 1 if reason else 0


def command_claim(repo, number, branch):
    """A lane's claim. Prints the claim comment's id on stdout; exits 1 when the issue is held."""
    issue, comments, pulls = fetch(repo, number)
    reason = refusal(issue, comments, pulls)
    if reason:
        print(f"issue_claim: {repo}#{number}: {reason}. Not claimed; take another issue.", file=sys.stderr)
        return 1
    login = whoami()
    mine = post(repo, number, claim_body(login, lane=branch))
    try:
        # Read back: if another claim landed first, ours is the one ignored, so take ours away.
        _, comments, _ = fetch(repo, number)
        if not won(comments, mine["id"]):
            gh("api", "-X", "DELETE", f"repos/{repo}/issues/comments/{mine['id']}")
            winner = standing_claim(comments)
            held = winner.describe() if winner else "a release that landed at the same moment"
            print(f"issue_claim: {repo}#{number}: lost a race to {held}. Not claimed.",
                  file=sys.stderr)
            return 1
        assign(repo, number, login)
    except BaseException:
        # Anything that stops us between the post and the printed id would orphan the comment.
        subprocess.run(["gh", "api", "-X", "DELETE", f"repos/{repo}/issues/comments/{mine['id']}"],
                       capture_output=True)
        raise
    print(mine["id"])
    return 0


def command_link(repo, number, comment_id, pull_request):
    comment = gh("api", f"repos/{repo}/issues/comments/{comment_id}")
    found = MARKER.search(comment["body"])
    values = dict(FIELD.findall(found.group(2))) if found else {}
    if not values.get("lane"):
        raise SystemExit(f"issue_claim: comment {comment_id} is not a lane's claim")
    body = claim_body(values["holder"], lane=values["lane"], pull_request=pull_request)
    gh("api", "-X", "PATCH", f"repos/{repo}/issues/comments/{comment_id}", "--input", "-",
       data={"body": body})
    return 0


def command_release(repo, number, reason):
    _, comments, _ = fetch(repo, number)
    claim = standing_claim(comments)
    if claim is None:
        print(f"issue_claim: {repo}#{number} has no standing claim to release", file=sys.stderr)
        return 1
    post(repo, number, f"Released ({reason}). Anyone may claim this issue again.\n\n"
         + marker("release", holder=claim.holder))
    unassign(repo, number, claim.holder)
    return 0


def command_drop(repo, number, comment_id):
    """Undo a lane's claim that never reached a draft: delete its comment and its assignment."""
    comment = gh("api", f"repos/{repo}/issues/comments/{comment_id}")
    found = MARKER.search(comment["body"])
    gh("api", "-X", "DELETE", f"repos/{repo}/issues/comments/{comment_id}")
    if found:
        unassign(repo, number, dict(FIELD.findall(found.group(2))).get("holder", ""))
    return 0


def decide_comment(issue, comments, pulls, author, association, command, author_open_claims):
    """What the workflow does with one `/claim` or `/release`: (reply, assign, unassign).

    Pure, so --selftest can hold each branch. author_open_claims counts open issues already
    assigned to the author in this repository.
    """
    member = association in TRUSTED_ASSOCIATIONS
    claim = standing_claim(comments)
    if command == "claim":
        if claim and claim.holder == author and not claim.lane:
            return (f"@{author}, this issue is already yours.", None, None)
        reason = refusal(issue, comments, pulls)
        if reason:
            return (f"@{author}, not claimed: {reason}. Pick another issue, or ask the holder "
                    f"here whether they are still on it.", None, None)
        if not member and author_open_claims >= OUTSIDER_LIMIT:
            return (f"@{author}, not claimed: you already hold {author_open_claims} open "
                    f"claim(s) here, and the limit is {OUTSIDER_LIMIT} until one closes or is "
                    f"released.", None, None)
        return (claim_body(author), author, None)
    if claim is None:
        return (f"@{author}, there is no claim on this issue to release.", None, None)
    if claim.holder != author and not member:
        return (f"@{author}, only the holder or a maintainer can release this claim.", None, None)
    return (f"Released by @{author}. Anyone may claim this issue again.\n\n"
            + marker("release", holder=claim.holder), None, claim.holder)


def command_comment():
    """The workflow's entry point. Every input arrives in the environment, never in a shell line."""
    event = json.load(open(os.environ["GITHUB_EVENT_PATH"]))
    repo = os.environ["GITHUB_REPOSITORY"]
    comment, number = event["comment"], event["issue"]["number"]
    if (comment.get("user") or {}).get("type") == "Bot" or event["issue"].get("pull_request"):
        return 0
    found = COMMAND.search(comment.get("body") or "")
    if not found:
        return 0
    author = comment["user"]["login"]
    if not SAFE_VALUE.match(author):
        return 0
    issue, comments, pulls = fetch(repo, number)
    # The command comment itself is not a marker, so leave it in: it changes nothing.
    member = comment.get("author_association") in TRUSTED_ASSOCIATIONS
    open_claims = 0
    if found.group(1) == "claim" and not member:
        open_claims = sum(1 for n, _ in assigned_claims(repo, author) if n != number)
    reply, to_assign, to_unassign = decide_comment(issue, comments, pulls, author,
                                                   comment.get("author_association"),
                                                   found.group(1), open_claims)
    if to_unassign:
        unassign(repo, number, to_unassign)
    mine = post(repo, number, reply)
    if not to_assign:
        return 0
    # Read back, as a lane does: a lane or another run may have claimed in the same moment.
    _, comments, _ = fetch(repo, number)
    if not won(comments, mine["id"]):
        winner = standing_claim(comments)
        held = winner.describe() if winner else "a claim made at the same moment"
        withdraw(repo, number, mine["id"], f"@{author}, not claimed: {held} came first.")
        return 0
    # They commented, so GitHub lets them be assigned; the comment above is the record either way.
    assign(repo, number, to_assign)
    if not member and over_limit(assigned_claims(repo, author), number, OUTSIDER_LIMIT):
        unassign(repo, number, author)
        withdraw(repo, number, mine["id"],
                 f"@{author}, not claimed: an older claim of yours is still open here, and the "
                 f"limit is {OUTSIDER_LIMIT} until one closes or is released.")
    return 0


def withdraw(repo, number, comment_id, text):
    """Turn the workflow's own claim comment into a refusal, so its marker no longer counts."""
    gh("api", "-X", "PATCH", f"repos/{repo}/issues/comments/{comment_id}", "--input", "-",
       data={"body": text})


def command_sweep(repo, now=None):
    now = now or datetime.now(timezone.utc)
    released = 0
    for issue in gh_list(f"repos/{repo}/issues?state=open&per_page=100"):
        if issue.get("pull_request") or not issue.get("comments"):
            continue
        _, comments, pulls = fetch(repo, issue["number"])
        claim = is_stale(comments, pulls, now, pull_is_open)
        if claim:
            command_release(repo, issue["number"],
                            f"claimed {claim.at:%Y-%m-%d}, and no open pull request closes this "
                            f"issue after {STALE_DAYS} days")
            released += 1
            print(f"released {repo}#{issue['number']}: {claim.describe()}")
    print(f"sweep: {released} stale claim(s) released in {repo}")
    return 0


def selftest():
    t0 = "2026-11-01T00:00:00Z"

    def c(cid, body, login=WORKFLOW_BOT, assoc="NONE", at=t0):
        return {"id": cid, "body": body, "user": {"login": login}, "author_association": assoc,
                "created_at": at}

    issue = {"state": "open", "assignees": [], "labels": []}
    lane = c(1, claim_body("calef", lane="lane/a"), "calef", "OWNER")
    lane2 = c(2, claim_body("calef", lane="lane/b"), "calef", "OWNER")
    outsider = c(3, claim_body("stranger"))
    release = c(4, "x\n\n" + marker("release", holder="calef"), "calef", "OWNER")
    forged = c(5, marker("release", holder="calef"), "mallory", "NONE")

    assert refusal(issue, [], []) is None, "a fresh issue is free"
    assert standing_claim([lane, lane2]).lane == "lane/a", "the first claim wins a race"
    assert "lane `lane/a`" in refusal(issue, [lane], []), "a second lane is refused"
    assert standing_claim([lane, forged]).lane == "lane/a", "an outsider cannot forge a release"
    assert standing_claim([lane, release, lane2]).lane == "lane/b", "a release frees the issue"
    assert refusal(dict(issue, assignees=[{"login": "x"}]), [], []), "a hand assignment holds"
    assert refusal(issue, [], [7]), "an open pull request that closes it holds"
    assert refusal(dict(issue, state="closed"), [], []), "a closed issue is refused"
    assert refusal(dict(issue, labels=[{"name": "needs-architect"}]), [], []), "a held ruling"
    now = parse_time(t0) + timedelta(days=STALE_DAYS + 1)
    assert is_stale([outsider], [], now), "an old claim with no pull request is stale"
    assert not is_stale([outsider], [9], now), "an open pull request keeps a claim"
    assert not is_stale([outsider], [], parse_time(t0) + timedelta(days=1)), "a new claim stands"
    assert not is_stale([], [], now), "a hand assignment is never swept"
    url = "https://github.com/nifeos/nife/pull/4100"
    assert pull_reference(url) == "nifeos/nife#4100" and pull_reference("x/pull/1") is None
    linked = c(6, claim_body("calef", lane="lane/a", pull_request=url), "calef", "OWNER")
    assert standing_claim([linked]).pull == "nifeos/nife#4100", "link records the draft"
    assert not is_stale([linked], [], now, lambda ref: ref == "nifeos/nife#4100"), \
        "an open draft keeps a claim whose body lost Closes"
    assert is_stale([linked], [], now, lambda ref: False), "a closed draft does not"
    assert won([lane, lane2], 1) and not won([lane, lane2], 2), "a read-back finds the winner"
    assert not won([outsider, lane], 1), "a lane loses to an earlier comment claim"
    t1, t2 = parse_time(t0), parse_time(t0) + timedelta(seconds=1)
    assert not over_limit([(8, t1)], 8, 1), "a first claim stands"
    assert over_limit([(8, t1), (9, t2)], 9, 1), "the newer of a burst withdraws"
    assert not over_limit([(8, t1), (9, t2)], 8, 1), "the older of a burst stands"
    assert over_limit([(8, None), (9, t1)], 9, 1), "a hand assignment counts first"
    assert not over_limit([(8, t1), (9, t1)], 8, 1), "a tie keeps the lower issue"
    assert over_limit([(8, t2), (9, t1)], 8, 1), "age decides, not the issue number"

    reply, add, drop = decide_comment(issue, [], [], "stranger", "NONE", "claim", 0)
    assert add == "stranger" and "issue-claim: claim holder=stranger" in reply
    reply, add, _ = decide_comment(issue, [lane], [], "stranger", "NONE", "claim", 0)
    assert add is None and "held" in reply, "an outsider cannot steal a lane's claim"
    reply, add, _ = decide_comment(issue, [], [], "stranger", "NONE", "claim", 1)
    assert add is None and "limit" in reply, "one open claim per outsider"
    _, add, _ = decide_comment(issue, [], [], "calef", "OWNER", "claim", 5)
    assert add == "calef", "a member has no limit"
    _, _, drop = decide_comment(issue, [lane], [], "stranger", "NONE", "release", 0)
    assert drop is None, "an outsider cannot release someone else's claim"
    _, _, drop = decide_comment(issue, [outsider], [], "stranger", "NONE", "release", 0)
    assert drop == "stranger", "the holder releases their own claim"
    _, _, drop = decide_comment(issue, [outsider], [], "calef", "MEMBER", "release", 0)
    assert drop == "stranger", "a maintainer releases a claim"
    assert COMMAND.search("thanks\n/claim\n") and not COMMAND.search("I will /claim this")
    try:
        marker("claim", holder="a -->")
        raise AssertionError("an unsafe marker value is refused")
    except SystemExit:
        pass
    print("issue_claim: selftest passed")
    return 0


def main(argv):
    if argv[:1] == ["--selftest"]:
        return selftest()
    if not argv:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    command, rest = argv[0], argv[1:]
    opts = {}
    while rest:
        if len(rest) < 2 or not rest[0].startswith("--"):
            print(f"issue_claim: bad arguments {rest}", file=sys.stderr)
            return 2
        opts[rest[0][2:]] = rest[1]
        rest = rest[2:]
    if command == "comment":
        return command_comment()
    repo = opts.get("repo") or current_repo()
    if command == "sweep":
        return command_sweep(repo)
    if "issue" not in opts or not opts["issue"].isdigit():
        print("issue_claim: --issue needs an issue number", file=sys.stderr)
        return 2
    number = int(opts["issue"])
    if command == "status":
        return command_status(repo, number)
    if command == "claim" and "branch" in opts:
        return command_claim(repo, number, opts["branch"])
    if command == "link" and {"comment", "pull-request"} <= opts.keys():
        return command_link(repo, number, opts["comment"], opts["pull-request"])
    if command == "release":
        return command_release(repo, number, opts.get("reason", "released by its holder"))
    if command == "drop" and "comment" in opts:
        return command_drop(repo, number, opts["comment"])
    print(f"issue_claim: unknown command or missing argument: {command}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
