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

**A table, and both directions.** A correction may write its action items as one table instead of
bullets, with the columns `#`, `what`, `status` and `record`. The record cell opens with the same
opener a bullet does, and the row resolves exactly as that bullet would. The status cell says
`delivered` or `proposed`, and the gate holds it to the resolution: a delivered row resolves closed
and a proposed row open. A table-form correction is also held to the other direction. Each
`Proposed.` row names a file that carries `COE action item: <correction>, action <N>`, and each
`Milestone N.` row a block that carries it. Every file under `design/roadmap/` that carries such a
line names a correction that exists, with a row of that number pointing back at it. A promoted
proposal still matches, through its `promoted_from`. Bullet-form corrections keep only the first
direction: their proposals predate the line, and the gate does not reach back for them.

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
# The back reference a proposal or a milestone block carries, one line of its own (2026-11-04 UTC).
BACK_REFERENCE = re.compile(r"^COE action item: `?(notes/coes/[a-z0-9-]+\.md)`?, action (\d+)\.?[ \t]*$",
                            re.M)
DELIVERED, PROPOSED_STATUS = "delivered", "proposed"


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
        row = _row(l)
        if row:
            out.append([n, row[3]])
        elif l.startswith("- "):
            out.append([n, l])
        elif out and l.startswith(("  ", "\t")) and l.strip():
            out[-1][1] += " " + l.strip()
    if not out:
        return [], [f"'{ACTIONS_HEADING}' has no bullets"]
    return [(n, l) for n, l in out], []


def _row(line):
    """(number, what, status, the row as a bullet) for one action-table row, or None for any other
    line, the header and the separator included. The bullet is the record cell, then the what cell,
    so `resolve` reads the opener first and counts both cells toward the prose floor."""
    if not line.startswith("|"):
        return None
    cells = [c.strip() for c in line.strip().strip("|").split("|")]
    if len(cells) != 4 or not cells[0].isdigit():
        return None
    number, what, status, record = cells
    return int(number), what, status.lower(), f"- {record} {what}"


def rows(text):
    """[(line number, action number, status, bullet)] for a table-form `## Action items`, or [] for
    a bullet-form one."""
    lines = text.split("\n")
    heads = [n for n, l in enumerate(lines, start=1) if l.rstrip() == ACTIONS_HEADING]
    out = []
    for n, l in enumerate(lines[heads[0]:] if len(heads) == 1 else [], start=(heads or [0])[0] + 1):
        if l.startswith("#"):
            break
        row = _row(l)
        if row:
            out.append((n, row[0], row[2], row[3]))
    return out


def _target(bullet, ctx):
    """What a row points at that can carry a back reference: ('proposal', path), ('milestone', N)
    or None. A promoted proposal points at its milestone."""
    dm = roadmap_block.DISPOSITION.match(bullet.rstrip())
    if not dm:
        return None
    kind = dm.group(1).split(" ")[0]
    if kind == "Milestone":
        return ("milestone", int(dm.group(2)))
    if kind == "Proposed":
        for path in _TICKED.findall(dm.group(3)):
            if path.startswith(PROPOSALS):
                slug = path[len(PROPOSALS):].removesuffix(".md")
                if slug in ctx["promoted"]:
                    return ("milestone", ctx["promoted"][slug])
                return ("proposal", path)
    return None


def _carriers(texts):
    """{(correction, action number): [the design/roadmap/ files carrying that back reference]}."""
    out = {}
    for path, text in texts.items():
        if path.startswith("design/roadmap/") and not path.endswith("README.md"):
            for m in BACK_REFERENCE.finditer(text):
                out.setdefault((m.group(1), int(m.group(2))), []).append(path)
    return out


def _milestone_path(number, texts):
    for path in texts:
        m = _BLOCK.match(path)
        if m and int(m.group(1)) == number:
            return path
    return None


def back_references(texts, ctx):
    """[(path, line, problem)] for both directions between table-form corrections and the files
    their rows name. See the docstring's "A table, and both directions"."""
    problems = []
    carriers = _carriers(texts)
    pointed = {}
    for path in sorted(p for p in texts if is_record(p)):
        for n, number, status, bullet in rows(texts[path]):
            target = _target(bullet, ctx)
            if target is None:
                continue
            want = (path, number)
            pointed[want] = target
            if target[0] == "proposal":
                carrier = target[1]
            else:
                carrier = _milestone_path(target[1], texts)
            if carrier is None or carrier not in texts:
                continue
            if carrier not in carriers.get(want, []):
                problems.append((path, n, f"action {number} names {carrier}, which does not carry "
                                          f"'COE action item: {path}, action {number}' on a line of "
                                          f"its own"))
    for (coe, number), files in sorted(carriers.items()):
        for f in files:
            if coe not in texts:
                problems.append((f, 0, f"'COE action item: {coe}, action {number}' names a "
                                       f"correction that does not exist"))
                continue
            target = pointed.get((coe, number))
            mine = ("proposal", f) if f.startswith(PROPOSALS) else None
            m = _BLOCK.match(f)
            if m:
                mine = ("milestone", int(m.group(1)))
            if target != mine:
                problems.append((f, 0, f"'COE action item: {coe}, action {number}': that "
                                       f"correction's action {number} does not point back at this "
                                       f"file"))
    return problems


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
        statuses = {n: status for n, _number, status, _bullet in rows(texts[path])}
        for n, line in items:
            state, problem = resolve(line, ctx, exists)
            status = statuses.get(n)
            if not problem and status is not None:
                if status not in (DELIVERED, PROPOSED_STATUS):
                    problem = (f"status {status!r} is not '{DELIVERED}' or '{PROPOSED_STATUS}'")
                elif (status == DELIVERED) != (state == "closed"):
                    problem = (f"status says {status}, but the record resolves "
                               f"{'closed' if state == 'closed' else 'open'}")
            if problem:
                problems.append((path, n, problem))
            elif state == "open":
                opened += 1
            else:
                closed += 1
        counts[path] = (opened, closed)
    problems.extend(back_references(texts, ctx))
    return counts, problems
