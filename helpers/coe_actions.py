"""A correction of error's action items: each one resolved to a file, and counted open or closed.

**Why this exists.** calef, reviewing #1513 on 2026-10-03 (UTC): *"How do we ensure these don't get
lost in the backlog so we prioritize them? Do we need to track open COE action items in the metrics
so that we can drive the count down?"* §210 (a correction of error, and its action items are
decisions, proposals or milestones) ruled what an action item may be, and proposed, without a
ruling, that a gate check it by reusing the `## Follow-on` vocabulary `script/roadmap` already
validates. Nothing built that gate, and the first two corrections wrote their action items as prose
under a `## The mechanism` heading, where no script could find them. This is the gate and the count,
in one place, so that `script/roadmap --check` (which fails a COE whose item resolves to nothing)
and `script/metrics` (which charts how many are still open each week) cannot disagree about what an
item is or whether it is done.

**The vocabulary is not copied.** A bullet is matched by `roadmap_block.DISPOSITION`, the regex the
`## Follow-on` check uses, so a word added there is a word a COE can use, and a COE bullet the regex
does not know is a failure rather than a silent pass. What differs from a milestone block is
resolution, and §210 named the differences:

- `**None.**` is refused. A correction that identified nothing to change has not finished.
- `**Outstanding.**` is refused. It is a PARTIAL block's word for its own scope, and a COE has none.
- Everything else resolves to something a reader can open, and that thing says whether it is done.

**Open or closed**, which is the whole of the count:

| opener | resolves to | open while |
|---|---|---|
| `**Milestone N.**` | `design/roadmap/N-*.md` | its status is not BUILT, REMOVED, SUPERSEDED or REFUSED |
| `**Proposed.**` | a file under `design/roadmap/proposals/` | it is still a proposal; once promoted (deleted, and a block says `promoted_from` its slug) it is that milestone |
| `**Decision.**` | a file under `design/decisions/` | its status is PROPOSED |
| `**Done.**`, `**Recorded.**`, `**Refused.**` | prose of at least 40 characters | never: these say the item is finished |

A promoted proposal is followed rather than reported as a dead link, because promotion deletes the
file (calef, 2026-09-18) and the promoting lane cannot be expected to know which corrections cite
it. The gate stays exact anyway: a `Proposed.` file that is gone and that no block was promoted from
is a failure.

**What a correction is**: every `notes/coes/*.md` except the README, and the same in the two places
corrections lived before that directory: `notes/corrections/` (the first, 2026-09-23) and a
`notes/coe-<date>-<slug>.md` file at the top of `notes/` (2026-09-29, a day). Each has exactly one
`## Action items` section. The older places are read so that `script/metrics` can count past weeks
from the tree as it stood, and so that a correction still filed there is gated like the rest.

Name: provisional, minted by the lane revising #1513 on 2026-10-03 (UTC). A shared python module
under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the record.
`coe` is the directory's own provisional abbreviation (`notes/coes/README.md`).

BUGS

- The per-opener resolution for `Recorded.`, `Done.` and `Refused.` (a 40-character floor) repeats
  `script/roadmap`'s `## Follow-on` loop rather than sharing it. §210 proposed factoring that loop
  and calling it twice; this module is the second caller written first, and folding the
  `## Follow-on` check onto it is left undone because that loop also carries milestone-only rules.
  An exception, and a foot gun: a change to the floor there must be made here too.
- Open means "the file it names says it is not done". A milestone marked BUILT that did not fix the
  correction's cause counts as closed. No script can tell, and §210 says so about itself.
"""
import re

import roadmap_block

ACTIONS_HEADING = "## Action items"
DIRECTORY = "notes/coes"
# Where corrections lived before DIRECTORY, oldest first; see the docstring.
OLDER_DIRECTORIES = ("notes/corrections",)
_OLDER_FILE = re.compile(r"^notes/coe-\d{4}-\d{2}-\d{2}-[a-z0-9-]+\.md$")
EXEMPT = ("README.md",)
PROPOSALS = "design/roadmap/proposals/"
DECISIONS = "design/decisions/"

# RECORDED is retired (calef, 2026-10-20 UTC) and no block carries it, but `script/metrics` runs this
# over past weeks' trees, where blocks did, and counted them closed.
MILESTONE_CLOSED = ("BUILT", "REMOVED", "SUPERSEDED", "REFUSED", "RECORDED")
DECISION_OPEN = ("PROPOSED",)
FINISHED = ("Done", "Recorded", "Refused")
PROSE_FLOOR = 40

_BLOCK = re.compile(r"^design/roadmap/(\d+)-[a-z0-9][a-z0-9-]*\.md$")
_DECISION = re.compile(r"^design/decisions/\d+[a-z]?-[a-z0-9-]+\.md$")
_TICKED = re.compile(r"`([^`]+)`")


def is_record(path):
    """True for a correction's own file, which is every markdown file in the directory but its README."""
    directory, _slash, name = path.rpartition("/")
    if _OLDER_FILE.match(path):
        return True
    return (directory in (DIRECTORY,) + OLDER_DIRECTORIES and name.endswith(".md")
            and name not in EXEMPT)


def _status(text):
    """A file's `status:` field, from frontmatter or from the roadmap's prose form."""
    status, _built = roadmap_block.status_and_built(text)
    return status


def context(texts):
    """What resolution needs from one tree, read once: milestone statuses by number, the milestone a
    promoted proposal became, and decision statuses by path. `texts` maps path to markdown text."""
    milestones, promoted, decisions = {}, {}, {}
    for path, text in texts.items():
        m = _BLOCK.match(path)
        if m:
            milestones[int(m.group(1))] = _status(text)
            fields, _start, _problems = roadmap_block.frontmatter(text.split("\n"))
            if fields and fields.get("promoted_from"):
                promoted[fields["promoted_from"]] = int(m.group(1))
        elif _DECISION.match(path):
            fields, _start, _problems = roadmap_block.frontmatter(text.split("\n"))
            decisions[path] = (fields or {}).get("status")
    return {"milestones": milestones, "promoted": promoted, "decisions": decisions}


def bullets(text):
    """[(line number, joined bullet)] under the one `## Action items` heading, and the problems
    with the section itself. A bullet is its first line plus every indented continuation line, the
    way `script/roadmap` reads `## Follow-on` (reading the first line alone was that check's own
    first defect)."""
    lines = text.split("\n")
    heads = [n for n, l in enumerate(lines, start=1) if l.rstrip() == ACTIONS_HEADING]
    if not heads:
        return [], [f"no '{ACTIONS_HEADING}' section. §210: an action item is a decision, a "
                    f"proposal or a milestone, and a correction with none has not finished"]
    if len(heads) > 1:
        return [], [f"{len(heads)} '{ACTIONS_HEADING}' sections; one is the list, a second is the "
                    f"copy that stops being updated"]
    out = []
    for n, l in enumerate(lines[heads[0]:], start=heads[0] + 1):
        if l.startswith("#"):
            break
        if l.startswith("- "):
            out.append([n, l])
        elif out and l.startswith(("  ", "\t")) and l.strip():
            out[-1][1] += " " + l.strip()
    if not out:
        return [], [f"'{ACTIONS_HEADING}' has no bullets"]
    return [(n, l) for n, l in out], []


def resolve(line, ctx, exists):
    """(state, problem) for one bullet: state is 'open' or 'closed', problem a string or None.
    `exists(path)` answers whether a tracked path exists in the tree being read."""
    dm = roadmap_block.DISPOSITION.match(line.rstrip())
    if not dm:
        return None, (f"{line[:60]!r} is not an action item. Each bullet opens '**Milestone N.**', "
                      f"'**Proposed.**', '**Decision.**', '**Done.**', '**Recorded.**' or "
                      f"'**Refused.**' (notes/coes/README.md)")
    kind, rest = dm.group(1).split(" ")[0], dm.group(3).strip()
    if kind == "None":
        return None, ("'**None.**' is refused in a correction (§210): one that identified nothing "
                      "to change has not finished")
    if kind == "Outstanding":
        return None, ("'**Outstanding.**' is a PARTIAL milestone's word for its own scope. Work a "
                      "correction still owes is '**Milestone N.**' or '**Proposed.**'")
    if kind == "Milestone":
        return _milestone(int(dm.group(2)), ctx)
    if kind == "Proposed":
        named = [p for p in _TICKED.findall(rest) if p.startswith(PROPOSALS)]
        for path in named:
            if exists(path):
                return "open", None
            slug = path[len(PROPOSALS):].removesuffix(".md")
            if slug in ctx["promoted"]:
                return _milestone(ctx["promoted"][slug], ctx)
        return None, (f"'**Proposed.**' names its file under {PROPOSALS} in backticks, and that file "
                      f"exists or was promoted to a milestone (found {named or 'no proposal'})")
    if kind == "Decision":
        named = [p for p in _TICKED.findall(rest) if p.startswith(DECISIONS)]
        real = [p for p in named if p in ctx["decisions"]]
        if not real:
            return None, (f"'**Decision.**' names its file under {DECISIONS} in backticks, and that "
                          f"file exists (found {named or 'no path'})")
        return ("open" if ctx["decisions"][real[0]] in DECISION_OPEN else "closed"), None
    if kind in FINISHED:
        if len(rest) < PROSE_FLOOR:
            return None, (f"'**{kind}.**' says what carried it or why, so a reader can go and look "
                          f"(found {rest!r})")
        return "closed", None
    return None, f"'**{kind}.**' is not handled here; teach helpers/coe_actions.py the new word"


def _milestone(number, ctx):
    status = ctx["milestones"].get(number)
    if status is None:
        return None, f"names milestone {number}, which has no block under design/roadmap/"
    return ("closed" if status in MILESTONE_CLOSED else "open"), None


def evaluate(texts, exists):
    """Every correction in `texts`: ({path: (open, closed)}, [(path, line, problem)])."""
    ctx = context(texts)
    counts, problems = {}, []
    for path in sorted(p for p in texts if is_record(p)):
        items, section_problems = bullets(texts[path])
        problems.extend((path, 0, p) for p in section_problems)
        opened = closed = 0
        for n, line in items:
            state, problem = resolve(line, ctx, exists)
            if problem:
                problems.append((path, n, problem))
            elif state == "open":
                opened += 1
            else:
                closed += 1
        counts[path] = (opened, closed)
    return counts, problems
