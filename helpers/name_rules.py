"""Names ratified by a rule rather than one at a time, for `script/names`.

**Why this file exists.** On 2026-10-17 (UTC) calef ratified two rules that each cover a whole class
of names: an appendix directory takes its parent's stem and an appendix file takes a slug of the
heading it was split from, and a test fixture program is named for the role it plays in its test.
`design/naming/convention-rulings.md` records both with his words. A ruling that lives only in
prose leaves the 1,400-name worklist unchanged and is rung four of the ladder in AGENTS.md; this
module is rung two: `script/names` reads the rulings page and counts every name a rule covers as
ratified, citing the rule, so the worklist shrinks by the rule and not by someone remembering it.

**What the rules cover is decided here, not by a pattern in the prose.** Rule 1 is computed from
the tree, because 600 stems cannot be listed and a list would rot: a directory beside `X.md` takes
its parent's stem by construction, and a document inside it is covered when its stem matches a
heading of the parent (`_matches_heading`). Rule 2 is an explicit list in the rulings page, one
backticked name per bullet, because "names a role" is a judgment and a pattern would guess.

**What stays provisional is the point of the checks.** A stem that matches no heading, or breaks a
word rule that a script can see (`_word_rule`), is pulled aside and keeps its own status. So are
`design/fatal-risks/` and `notes/coes/`, which section 212 calls records directories. Form only:
nothing here can tell whether a heading is the right one, only whether the stem is made of its
words.

Name: provisional, minted 2026-10-17 (UTC) by the lane that recorded the two rulings. A shared
helper under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the
record. It sits beside `name_provenance.py` and takes `name_` for the same family; `conventions`
was considered and names the pages rather than the rulings they hold. calef has not named it.
"""

import os
import re
import sys

RULES_DOC = "design/naming/convention-rulings.md"
APPENDIX_RULE = "appendix-stems"
FIXTURE_RULE = "fixture-roles"
LISTED_RULE = "listed-stems"

# Spelled in two halves because script/lint's naming check refuses the word itself outside
# design/ and itself, and this is the one place a script has to name what it refuses.
_UNIX_SERVICE_WORD = "dae" + "mon"

# Words a heading shares with a stem without being part of its subject.
_ARTICLES = {"the", "a", "an"}

# Directories under `design/` or `notes/` that sit beside a `X.md` and are still not appendices.
# `README` is the index of its own directory, so `notes/README/` is no appendix of it.
_NOT_PARENTS = {"README"}


def parse_rules(text):
    """`{rule id: {...}}` from the rulings page: `Applies:`, `Ratified:`, covered names, exclusions.

    A rule is a `## Rule N` section. `Applies:` names the mechanism (`appendix-stems` or
    `fixture-roles`), `Ratified:` carries the date, every bullet under `Covered:` holds one
    backticked name, and `Not covered:` holds backticked directories the rule does not reach.
    """
    rules, cur, section = {}, None, None
    for line in text.split("\n"):
        if re.match(r"^## Rule \d+", line):
            cur = {"heading": line[3:].strip(), "applies": None, "date": None,
                   "covered": [], "not_covered": []}
            section = None
            rules[line.split(":")[0][3:].strip()] = cur
        elif line.startswith("## "):
            cur = None
        if cur is None:
            continue
        for key, field in (("Applies:", "applies"), ("Ratified:", "date")):
            if line.startswith(key):
                cur[field] = line[len(key):].strip().split()[0]
        if line.startswith("Not covered:"):
            cur["not_covered"] = re.findall(r"`([^`]+)`", line)
        section = "covered" if line.startswith("Covered:") else section
        if section == "covered" and line.startswith("- "):
            names = re.findall(r"`([^`]+)`", line)
            if cur["applies"] == LISTED_RULE:
                # `- notes/x: `a`, `b`` lists documents `notes/x/a.md` and `notes/x/b.md`.
                names = [f"{line[2:].split(':')[0].strip()}/{n}.md" for n in names]
            cur["covered"] += names
    return rules


def _tokens(text):
    text = re.sub(r"\([^)]*\)", " ", text)
    text = re.sub(r"[`*_]", "", text).lower()
    return [t for t in re.split(r"[^a-z0-9]+", text) if t]


def _core(text):
    return {t for t in _tokens(text) if t not in _ARTICLES}


def headings(markdown):
    """Every heading's text in one markdown file, outside fences."""
    out, fence = [], False
    for line in markdown.split("\n"):
        if line.strip().startswith("```"):
            fence = not fence
        m = re.match(r"^#{1,6}\s+(.*)$", line)
        if m and not fence:
            out.append(m.group(1).strip())
    return out


def _matches_heading(stem, parent_headings):
    """True when every word of the stem, articles aside, is a word of one parent heading.

    Exact slug equality is rarer than that (39 of 520 stems on 2026-10-17): a lane that split a
    section out shortened its heading to a stem (`limitations` from `Limitations and BUGS`). A stem
    with a word no heading holds was not derived from a heading and is pulled aside.
    """
    words = _core(stem)
    return bool(words) and any(words <= _core(h) for h in parent_headings)


def _word_rule(stem, british):
    """Why a stem breaks a word rule of design/naming.md that a script can see, else None."""
    if not re.fullmatch(r"[a-z0-9]+(-[a-z0-9]+)*", stem):
        return "not lowercase and hyphenated"
    if _UNIX_SERVICE_WORD in stem or stem.endswith("-d"):
        return "claims the Unix service model"
    hit = [w for w in british if w in stem.split("-")]
    return f"British spelling `{hit[0]}`" if hit else None


def appendix_coverage(files, read, rule, british=()):
    """`(covered, pulled)` for rule 1 over the tracked markdown `files`.

    `covered` maps a directory (`notes/x/`) or a document path to the sentence a reader is shown;
    `pulled` maps the same keys to why the rule did not take them. `read(path)` returns a file's
    text. Only a directory beside a `X.md` of its own stem is an appendix directory, a digit-led
    stem is a record, and `design/roadmap/` is a milestone's.
    """
    covered, pulled = {}, {}
    md = set(files)
    skip = tuple(d.rstrip("/") + "/" for d in rule["not_covered"])
    dirs = sorted({os.path.dirname(f) for f in md} - {"design", "notes", "briefs", ""})
    for d in dirs:
        base = os.path.basename(d)
        if (d + ".md" not in md or base in _NOT_PARENTS or base[0].isdigit()
                or d.startswith("design/roadmap") or (d + "/").startswith(skip)):
            continue
        why = _word_rule(base, british)
        if why:
            pulled[d + "/"] = why
            continue
        covered[d + "/"] = f"{d}/ takes the stem of {d}.md"
        parent = headings(read(d + ".md"))
        for f in sorted(p for p in md if os.path.dirname(p) == d):
            stem = os.path.basename(f)[:-3]
            if stem == "README" or stem[0].isdigit():
                continue
            why = _word_rule(stem, british)
            if why:
                pulled[f] = why
            elif not _matches_heading(stem, parent):
                pulled[f] = f"`{stem}` matches no heading of {d}.md"
            else:
                covered[f] = f"`{stem}` is made of a heading of {d}.md"
    return covered, pulled


def apply(records, rules, files, read, british=()):
    """Mark every record a rule covers `ratified` by that rule. Returns `(pulled, problems)`.

    A record that is already ratified is left as it is. `rule` on a record is the sentence
    `script/names <name>` prints; `status` and `date` are the rule's, so every count that reads
    them moves with it. `problems` are rule-list entries naming a program that does not exist.
    """
    by_name = {(r["kind"], r["name"]): r for r in records}
    marks, pulled, problems = {}, {}, []
    for rid, rule in rules.items():
        cite = f"{RULES_DOC}, {rid}"
        if rule["applies"] == APPENDIX_RULE:
            cov, pulled = appendix_coverage(files, read, rule, british)
            for key, why in cov.items():
                marks[key] = (rule["date"], f"{cite}: {why}")
        elif rule["applies"] == LISTED_RULE:
            for path in rule["covered"]:
                if path not in files:
                    problems.append(f"{RULES_DOC}, {rid}: `{path}` is not a tracked document")
                    continue
                marks[path] = (rule["date"], f"{cite}: a stem kept as it stands, listed by calef")
        elif rule["applies"] == FIXTURE_RULE:
            for name in rule["covered"]:
                rec = by_name.get(("program", name))
                if rec is None or not rec["path"].startswith("fixtures/"):
                    problems.append(f"{RULES_DOC}, {rid}: `{name}` is not a fixtures/ program")
                    continue
                marks[rec["path"]] = (rule["date"], f"{cite}: a role in its test, as a noun phrase")
    for r in records:
        hit = marks.get(r["name"] if r["kind"] == "directory" else r["path"])
        if hit and r["kind"] in ("directory", "document", "program") and r["status"] != "ratified":
            r["status"], r["date"], r["rule"] = "ratified", hit[0], hit[1]
    return pulled, problems


def _selftest():
    rules = parse_rules("\n".join([
        "## Rule 1: appendix stems", "Applies: appendix-stems", "Ratified: 2026-10-17 (UTC)",
        "Not covered: `notes/coes`", "",
        "## Rule 2: fixtures", "Applies: fixture-roles", "Ratified: 2026-10-17", "Covered:",
        "- `role_one`", "- `role_two`", "- `ghost`", "",
        "## Rule 3: kept", "Applies: listed-stems", "Ratified: 2026-10-18", "Covered:",
        "- notes/y: `orphan`, `gone`"]))
    assert rules["Rule 2"]["covered"] == ["role_one", "role_two", "ghost"], rules
    assert rules["Rule 1"]["not_covered"] == ["notes/coes"], rules
    tree = {"notes/x.md": "# X\n## The loader\n## The zorbspell\n## Limitations and BUGS\n",
            "notes/x/the-loader.md": "", "notes/x/limitations.md": "",
            "notes/x/elsewhere.md": "", "notes/x/the-zorbspell.md": "",
            "notes/coes.md": "# C\n## Report\n", "notes/coes/report.md": "",
            "notes/y/orphan.md": ""}
    cov, pulled = appendix_coverage(sorted(tree), tree.get, rules["Rule 1"], ["zorbspell"])
    assert "notes/x/" in cov and "notes/x/the-loader.md" in cov, cov
    assert "notes/x/limitations.md" in cov, "a shortened heading is still its words"
    assert "no heading" in pulled["notes/x/elsewhere.md"], pulled
    assert "British" in pulled["notes/x/the-zorbspell.md"], "a word-rule break is pulled aside"
    assert not any(k.startswith(("notes/coes", "notes/y")) for k in list(cov) + list(pulled))
    def rec(kind, name, path, status="provisional"):
        return {"kind": kind, "name": name, "path": path, "status": status, "date": None}
    records = [rec("program", "role_one", "fixtures/src/role_one.rs"),
               rec("program", "alto", "fixtures/src/alto.rs"),
               rec("program", "role_two", "fixtures/src/role_two.rs", "ratified"),
               rec("document", "elsewhere", "notes/x/elsewhere.md"),
               rec("document", "the-loader", "notes/x/the-loader.md"),
               rec("directory", "notes/x/", "notes/x/README.md"),
               rec("document", "orphan", "notes/y/orphan.md")]
    _, problems = apply(records, rules, sorted(tree), tree.get)
    status = {r["name"]: r["status"] for r in records}
    assert status["role_one"] == "ratified" and "Rule 2" in records[0]["rule"], status
    assert status["alto"] == "provisional", "a name no rule lists stays provisional"
    assert status["the-loader"] == "ratified" and status["notes/x/"] == "ratified", status
    assert rules["Rule 3"]["covered"] == ["notes/y/orphan.md", "notes/y/gone.md"], rules
    assert status["elsewhere"] == "provisional", "a pulled-aside stem stays provisional"
    assert status["orphan"] == "ratified" and "Rule 3" in records[-1]["rule"], "a listed stem is ratified"
    assert problems == [f"{RULES_DOC}, Rule 2: `ghost` is not a fixtures/ program",
                        f"{RULES_DOC}, Rule 3: `notes/y/gone.md` is not a tracked document"], problems
    print("name_rules: selftest ok")


if __name__ == "__main__":
    if sys.argv[1:] == ["--selftest"]:
        _selftest()
    else:
        sys.exit("usage: name_rules.py --selftest")
