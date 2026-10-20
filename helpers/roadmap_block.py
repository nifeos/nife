"""The milestone block's fields, read from frontmatter or from the prose form it replaces.

**Why this exists**: milestone 596 (the roadmap blocks get frontmatter too). calef ruled on
2026-09-26 (UTC) that the roadmap follows milestone 582 (a decision's status becomes a field, and
the index becomes generated) onto frontmatter, and the same day that the block carries the
dependency fields of §207 (the roadmap is a graph, and the block says so in fields a script can
walk) as keys, retiring the `**Gate:**` line. Six scripts read a block's status out of a sentence, each with
its own regex. They all read it through here now, in both forms, because three of them
(`script/metrics`, `script/catch-up`, `script/citations --moved`) read blobs from revisions that
predate the switch and will go on reading them for as long as the history is walked. The prose
reader here is therefore permanent, not transitional.

**The key names are calef's, ratified 2026-09-26 (UTC): "Yes to all."** With them he ratified
`decision_dependencies: unwritten` for a fork nobody has written up, `raised` on every block, a
`branch` for IN-PROGRESS, one format for proposals, and `superseded_by` and `refused_by` as lists
that may name a `§N`. Every key is spelled once, in `KEY` below, and every reader and the migrator
(`helpers/roadmap_migrate.py`) go through that table. `VALUE` does the same for the fixed words a
value may take.

**The format is 582's, exactly**: `---`, flat `key: value` lines, `---`. No lists and no nesting; a
list is one comma-separated value. A value may not contain `: ` or ` #`, because GitHub renders
frontmatter as YAML and either would make it something other than a string.

Name: provisional, minted by milestone 596's lane on 2026-09-26. A shared python module under
`helpers/`, which `script/names` puts out of its own scope, so this paragraph is the record. calef
names modules, and has not ratified it.
"""
import re

# ---- the schema, spelled once ------------------------------------------------------------------
# The role on the left is what the code says; the key on the right is what a block says. A ratified
# rename changes the right-hand side only.
KEY = {
    'status': 'status',
    'raised': 'raised',
    'built': 'built',
    'branch': 'branch',
    'promoted_from': 'promoted_from',
    'superseded_by': 'superseded_by',
    'refused_by': 'refused_by',
    'milestone_dependencies': 'milestone_dependencies',
    'decision_dependencies': 'decision_dependencies',
    'machine_requirements': 'machine_requirements',
    'specific_machine': 'specific_machine',
    'needs_person': 'needs_person',
}
# The order the migrator writes them in, which is the order a reader asks the questions: where does
# it stand, since when, what it came from, and what it waits on.
ORDER = list(KEY)
# §207's five, which travel together: a block carries all of them or none (`--unmodelled`).
DEPENDENCIES = ['milestone_dependencies', 'decision_dependencies', 'machine_requirements',
                'specific_machine', 'needs_person']
VALUE = {
    'none': 'none',            # an empty dependency field: §207 writes it out, silence means nothing
    'unwritten': 'unwritten',  # a decision dependency on a fork nobody has written up yet
    'yes': 'yes',
    'no': 'no',
}

# The status vocabulary, as `script/roadmap` defines it; `PROPOSED` belongs to proposals alone.
STATUSES = ["BUILT", "REMOVED", "SUPERSEDED", "PARTIAL", "IN-PROGRESS", "NOT-STARTED", "OPTIONAL",
            "REFUSED"]
PROPOSED = "PROPOSED"

DATE = re.compile(r"\d{4}-\d{2}-\d{2}")
_LINE = re.compile(r"([a-z_]+): (.+)")
# The prose form, as every reader spelled it before 596. Kept for old revisions.
PROSE_STATUS = re.compile(r"\*\*Status: ([A-Z-]+)\b")
PROSE_BUILT = re.compile(r"^\*\*Built:\*\*\s*(\d{4}-\d{2}-\d{2})\s*$", re.M)


def key(role):
    """The key a block spells for `role`."""
    return KEY[role]


def frontmatter(lines):
    """(fields by ROLE, index of the first body line, [(line number, problem)]), or (None, 0, [])
    when the text does not open with `---`, which is the prose form.

    Fields come back keyed by role, not by spelling, so a caller never sees a key name. A key that is
    not in the schema is a problem and is dropped; so is a key given twice.
    """
    if not lines or lines[0] != "---":
        return None, 0, []
    try:
        end = lines.index("---", 1)
    except ValueError:
        return {}, len(lines), [(1, "the frontmatter is never closed by a second '---'")]
    role_of = {v: k for k, v in KEY.items()}
    fields, problems = {}, []
    for n, line in enumerate(lines[1:end], start=2):
        m = _LINE.fullmatch(line)
        if not m:
            problems.append((n, f"{line[:40]!r} is not '<key>: <value>'. The frontmatter is flat "
                                f"key-value pairs and nothing else: no lists, no nesting"))
            continue
        spelled, value = m.group(1), m.group(2).strip()
        role = role_of.get(spelled)
        if role is None:
            problems.append((n, f"{spelled!r} is not in the schema ({', '.join(KEY.values())}). "
                                f"The schema is calef's and a lane does not extend it"))
        elif role in fields:
            problems.append((n, f"{spelled!r} twice. One fact, one field"))
        elif ": " in value or " #" in value:
            problems.append((n, f"{spelled}'s value carries ': ' or ' #', which YAML would read "
                                f"as something other than a string"))
            fields[role] = value
        else:
            fields[role] = value
    # One blank line after the closing fence, as design/decisions/ writes it, is layout.
    start = end + 1
    while start < len(lines) and not lines[start].strip():
        start += 1
    return fields, start, problems


def tokens(value):
    """A comma-separated value as a list; `none` is the empty list."""
    if value is None or value == VALUE['none']:
        return []
    return [t.strip() for t in value.split(",") if t.strip()]


def render(fields):
    """The frontmatter lines for `fields` (by role), in ORDER."""
    return ["---"] + [f"{KEY[r]}: {fields[r]}" for r in ORDER if fields.get(r) is not None] + ["---"]


def status_and_built(text):
    """(status, built) of a numbered block in either form, or (None, '') if neither parses.

    For the readers that want two facts and no validation: `script/catch-up`, `script/citations
    --moved`, `script/metrics` and `script/journeys`. `script/roadmap --check` is what judges a
    block; these read what it passed, at whatever revision they were asked about.
    """
    lines = text.split("\n")
    fields, start, _ = frontmatter(lines)
    if fields is not None:
        return fields.get('status'), fields.get('built', '') or ''
    first = next((l for l in lines[1:] if l.strip()), "")
    sm = PROSE_STATUS.match(first)
    if not sm:
        return None, ''
    bm = PROSE_BUILT.search(text)
    return sm.group(1), bm.group(1) if bm else ''


def body(text):
    """The lines after the frontmatter, or all of them for the prose form. Line 1 is the H1."""
    lines = text.split("\n")
    fields, start, _ = frontmatter(lines)
    return lines[start:] if fields is not None else lines


# ---- the prose form's field tokens, which are syntax and not sentences --------------------------
# Every bold marker `script/roadmap` reads out of a block's prose, spelled once here and imported by
# it. `helpers/prose_ratchet.py` reads the same list to measure a block's prose without them: a
# `**Status: BUILT.**` is a field written in a sentence's clothes, and counted as a two-word sentence
# it held down the median of every block that carried one, so removing it in milestone 596's switch
# read as 118 blocks getting worse. The patterns are literals on purpose, so a scanner that derives
# markers from the parsers' source (as #1311's does for bold) sees them.
# RECORDED is retired (calef, 2026-10-20 UTC) and stays in this token and in PHRASE because
# `script/citations` reads old revisions, whose prose-form blocks spell it.
STATUS_TOKEN = re.compile(r"^\*\*Status: (BUILT|REMOVED|SUPERSEDED|PARTIAL|IN-PROGRESS|"
                          r"NOT-STARTED|OPTIONAL|RECORDED|REFUSED|PROPOSED)\b([^*]*)\*\*[ \t]*", re.M)
BUILT_TOKEN = re.compile(r"^\*\*Built:\*\* \d{4}-\d{2}-\d{2}[ \t]*$", re.M)
GATE_TOKEN = re.compile(r"^\*\*Gate: [A-Z0-9§, ]+\.\*\*[ \t]*", re.M)
# The `## Follow-on` and `## Revisit` bullet tags, with the capture groups `script/roadmap` reads.
DISPOSITION = re.compile(
    r"^- \*\*(None|Recorded|Refused|Decision|Proposed|Done|Outstanding|Milestone (\d+))"
    r"\.\*\*(.*)$")
REVISIT = re.compile(r"^- \*\*(Condition|Nothing|Unstated)\.\*\*(.*)$")


# What the status word becomes when it leaves a sentence it was part of. "**Status: SUPERSEDED.**
# 2026-09-15, by milestone 166 (one boot loader)" still needs a subject, and "**Status: PARTIAL
# 2026-09-04.**" says when, which the frontmatter does not.
PHRASE = {"BUILT": "Built", "PARTIAL": "Partial as of", "REMOVED": "Removed", "REFUSED": "Refused",
          "SUPERSEDED": "Superseded", "RECORDED": "Recorded", "OPTIONAL": "Optional",
          "NOT-STARTED": "Not started", "IN-PROGRESS": "In progress"}


def status_lead(token, inside, following, built):
    """What replaces a status token: nothing, or the words it carried that the frontmatter does not.

    `inside` is what the bold held after the token, `following` the first character of the prose
    after it in the same paragraph ('' at the paragraph's end), `built` the block's Built date. A
    date inside the bold that equals `built` is the field and goes; anything else stays as prose.
    """
    if token == PROPOSED:
        return ""  # its one date becomes `raised`
    inside = inside.strip()
    bare = inside.strip(" .")
    lead = PHRASE[token] + ("" if not bare else
                            inside.rstrip(".") if inside[0] in ",;" else " " + inside.rstrip("."))
    continues = following and (following in "(,;:" or
                               ((not inside.endswith(".") or not bare)
                                and (following.isdigit() or following.islower())))
    if continues:
        return lead + ("" if following in ",;:" else " ")
    if not bare or bare == built:
        return ""
    return lead + (". " if following else ".")


def without_fields(text):
    """`text` with its status token, `**Built:**` lines and gate tokens taken out of the prose,
    and the words the status token carried kept (`status_lead`). This is the migration's rewrite
    of the body and the prose ratchet's reading of a prose-form block, which is what makes the two
    measure the same."""
    bm = PROSE_BUILT.search(text)
    built = bm.group(1) if bm else ""
    # Only the status line a block opens with: the first non-blank line after the H1. A phase
    # further down may open "**Status: PROPOSED, 2026-...**" about itself (milestone 126 (the
    # `procps` package) does), and that is prose.
    h1 = re.search(r"^# .*\n(?:[ \t]*\n)*", text, re.M)
    m = STATUS_TOKEN.match(text, h1.end()) if h1 else None
    if m:
        rest = text[m.end():]
        nxt = re.match(r"[ \t]*(\n[ \t]*)?(\S?)", rest)
        following = nxt.group(2) if nxt else ""
        lead = status_lead(m.group(1), m.group(2), following, built)
        if rest.startswith("\n"):
            lead = lead.rstrip()
        text = text[:m.start()] + lead + rest
    text = BUILT_TOKEN.sub("", text)
    return GATE_TOKEN.sub("", text)


def without_field_tokens(text):
    """`without_fields`, and the tag opening a Follow-on or Revisit bullet removed too: what the
    prose ratchet measures of a roadmap document in either form."""
    text = without_fields(text)
    out = []
    for line in text.split("\n"):
        m = DISPOSITION.match(line) or REVISIT.match(line)
        out.append("- " + m.group(m.lastindex).lstrip() if m else line)
    return "\n".join(out)
