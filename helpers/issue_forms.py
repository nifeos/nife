#!/usr/bin/env python3
"""Every issue form under .github/ISSUE_TEMPLATE/ is one GitHub will render.

    helpers/issue_forms.py              # script/lint: fail naming the file, the field and the rule
    helpers/issue_forms.py --selftest   # fixtures, no tree; script/lint runs it first

Milestone 207 (the four things GitHub asks for). The file name is provisional; naming is an architect's.

# WHY

GitHub validates an issue form only once it is on the default branch, and a form it rejects drops
out of the issue chooser with no red check anywhere. Blank issues are off here, so a broken
bug-report form is a stranger with no way to file a bug. This is the check GitHub runs, moved to
before the merge.

# THE RULES

The specification is GitHub's own three pages: "Syntax for issue forms" (top-level keys), "Syntax
for GitHub's form schema" (each body element's keys, attributes and validations), and "Common
validation errors when creating issue forms" (21 named errors). Each rule below cites the error it
enforces from that last page where there is one. Read 2026-10-19 (UTC).

SchemaStore's `github-issue-forms` JSON schema, run by `check-jsonschema`, was measured against the
same pages and not adopted: it passes duplicate ids, duplicate labels, labels that normalize alike,
a body of markdown only, the reserved option `None`, the word `password` in a label and a
whitespace-only name (eight of the 21 errors), and it rejects the documented `min_length`. JSON
Schema cannot express the uniqueness rules at all, so a check written here was needed whatever
validator ran beside it, and the validator would have been the tree's first Python package
dependency, a ruling under §46 (thin primitives or whole subsystems). The pull request that added
this file has the measurement.

# THE PARSER

`ruby -ryaml`, for helpers/workflow_parses.py's reason: Ruby's stdlib YAML is present on both the
runner image and macOS, and no Python YAML library is. It is also YAML 1.1, as GitHub's parser is,
which matters here: a bare `yes` in a dropdown's options is a boolean to both, and GitHub rejects
it (error "options must not include booleans"). A YAML 1.2 parser reads it as a string and passes
it. `safe_load` evaluates no tag, so nothing in a form executes.

# BUGS

- The rules are transcribed from documentation, and GitHub's validator is closed. A form this
  passes and GitHub rejects is the residue, and a key GitHub adds later fails here until it is
  added to the tables below. That direction is the safe one: a false red names the key.
- Two of the 21 errors are checked only in part. Checkbox option labels are compared with each
  other but not with other fields' labels, which GitHub also forbids. The forbidden-word rule
  names only "password" as an example, so only "password" is refused.
- "Labels are too similar" does not say how GitHub normalizes. This folds case and drops
  everything but letters and digits, which may be stricter or looser than GitHub.
- `labels` naming a label the repository lacks is not an error to GitHub (the label is skipped),
  and is not checked here.
- Only `.github/ISSUE_TEMPLATE/` in this repository is read. An organization's `.github`
  repository can supply default forms, and nothing here sees those.
"""
import io
import json
import os
import re
import subprocess
import sys
import tempfile

TEMPLATES = os.path.join(".github", "ISSUE_TEMPLATE")
CONFIG = "config.yml"

# One ruby process for the directory. Per file it prints one JSON line: the document, or the reason
# there is none. A key that is not a string (`yes:` parses to true) cannot survive into JSON as
# itself, and a key given twice has already lost one value by then, so both are reported here,
# where the parse still shows them.
LOAD = """\
require "json"
def bad_keys(node, path, out)
  case node
  when Hash
    node.each do |k, v|
      out << "#{path}: key #{k.inspect} is not a string (quote it)" unless k.is_a?(String)
      bad_keys(v, "#{path}.#{k}", out)
    end
  when Array
    node.each_with_index { |v, i| bad_keys(v, "#{path}[#{i}]", out) }
  end
  out
end
def dup_keys(node, out)
  if node.is_a?(Psych::Nodes::Mapping)
    keys = node.children.each_slice(2).map(&:first).grep(Psych::Nodes::Scalar).map(&:value)
    keys.select { |k| keys.count(k) > 1 }.uniq.each do |k|
      out << "key #{k.inspect} appears twice in one map (line #{node.start_line + 1}); YAML keeps the last"
    end
  end
  (node.children || []).each { |c| dup_keys(c, out) }
  out
end
ARGV.each do |path|
  begin
    src = File.read(path)
    stream = Psych.parse_stream(src)
    docs = stream.children.size
    if docs != 1
      puts JSON.generate({ "path" => path, "error" =>
        "holds #{docs} YAML documents; a `---` separator splits the form and GitHub reads " \\
        "\\"Body cannot be empty\\"" })
      next
    end
    doc = YAML.safe_load(src)
    keys = dup_keys(stream, []) + bad_keys(doc, "", [])
    if keys.empty?
      puts JSON.generate({ "path" => path, "doc" => doc })
    else
      puts JSON.generate({ "path" => path, "error" => keys.join("; ") })
    end
  rescue Psych::Exception => e
    puts JSON.generate({ "path" => path, "error" => e.message })
  end
end
"""


def load(cwd, rels):
    """({path: doc}, {path: error}, note): ruby's reading of each file. `note` is set only when
    the gate could not run at all."""
    try:
        run = subprocess.run(["ruby", "-ryaml", "-e", LOAD, "--"] + rels,
                             capture_output=True, text=True, cwd=cwd)
    except FileNotFoundError:
        return {}, {}, ("`ruby` is not on PATH. The parser is `ruby -ryaml` "
                        "(helpers/issue_forms.py); run script/bootstrap (it installs it), do not skip the gate")
    if run.returncode != 0:
        return {}, {}, (run.stderr.strip() or "ruby exited %d with no error text" % run.returncode)
    docs, errors = {}, {}
    for line in run.stdout.splitlines():
        rec = json.loads(line)
        if "error" in rec:
            errors[rec["path"]] = rec["error"]
        else:
            docs[rec["path"]] = rec["doc"]
    return docs, errors, None


# ---- the rules ----------------------------------------------------------------------------------

TOP_KEYS = {"name", "description", "body", "assignees", "labels", "title", "type", "projects"}
ELEMENT_KEYS = {"type", "id", "attributes", "validations"}
ID = re.compile(r"^[A-Za-z0-9_-]+$")
PROJECT = re.compile(r"^[^/\s]+/[0-9]+$")
URL = re.compile(r"^https?://\S+$")

# Per element type: (required attributes, optional attributes, permitted validations).
ELEMENTS = {
    "markdown": ({"value"}, set(), set()),
    "textarea": ({"label"}, {"description", "placeholder", "value", "render"},
                 {"required", "min_length"}),
    "input": ({"label"}, {"description", "placeholder", "value"}, {"required", "min_length"}),
    "dropdown": ({"label", "options"}, {"description", "multiple", "default"}, {"required"}),
    "checkboxes": ({"label", "options"}, {"description"}, {"required"}),
    "upload": ({"label"}, {"description"}, {"required", "accept"}),
}
STRING_ATTRS = {"value", "label", "description", "placeholder", "render"}


def text(v):
    """A string GitHub accepts: present, a string, and not only whitespace ("key must be a
    string")."""
    return isinstance(v, str) and v.strip() != ""


def is_int(v):
    return isinstance(v, int) and not isinstance(v, bool)


def string_list(v):
    """GitHub's "Array or comma-delimited string"."""
    return text(v) or (isinstance(v, list) and all(text(x) for x in v))


def check_top(doc, out):
    for k in sorted(set(doc) - TOP_KEYS):
        out.append("`%s` is not a permitted key" % k)
    for k in ("name", "description"):
        if k not in doc:
            out.append("required top level key `%s` is missing" % k)
        elif not text(doc[k]):
            out.append("`%s` must be a non-empty string" % k)
    for k in ("title", "type"):
        if k in doc and not text(doc[k]):
            out.append("`%s` must be a non-empty string" % k)
    for k in ("assignees", "labels", "projects"):
        if k in doc and not string_list(doc[k]):
            out.append("`%s` must be a list of strings or a comma-delimited string" % k)
    projects = doc.get("projects")
    if string_list(projects):
        items = projects.split(",") if isinstance(projects, str) else projects
        for p in items:
            if not PROJECT.match(p.strip()):
                out.append("project `%s` is not PROJECT-OWNER/PROJECT-NUMBER" % p.strip())


def check_options(where, kind, attrs, out):
    options = attrs.get("options")
    if not isinstance(options, list) or not options:
        out.append("%s: `options` must be a non-empty list" % where)
        return
    if kind == "dropdown":
        if not all(isinstance(o, str) for o in options):
            out.append("%s: `options` must not include booleans or numbers; quote them" % where)
            return
        labels = options
        if any(o.strip().casefold() == "none" for o in options):
            out.append("%s: `options` must not include the reserved word, none" % where)
        default = attrs.get("default")
        if default is not None and not (is_int(default) and 0 <= default < len(options)):
            out.append("%s: `default` must index `options`" % where)
    else:
        labels = []
        for i, o in enumerate(options):
            if not isinstance(o, dict) or not text(o.get("label")):
                out.append("%s: options[%d] needs a string `label`" % (where, i))
                continue
            for k in sorted(set(o) - {"label", "required"}):
                out.append("%s: options[%d]: `%s` is not a permitted key" % (where, i, k))
            if "required" in o and not isinstance(o["required"], bool):
                out.append("%s: options[%d]: `required` must be a boolean" % (where, i))
            labels.append(o["label"])
    if len(set(labels)) != len(labels):
        out.append("%s: `options` must be unique" % where)


def check_attributes(where, kind, attrs, out):
    required, optional, _ = ELEMENTS[kind]
    for k in sorted(set(attrs) - required - optional):
        out.append("%s: `%s` is not a permitted attribute" % (where, k))
    for k in sorted(required - set(attrs)):
        out.append("%s: required attribute key `%s` is missing" % (where, k))
    for k in sorted(STRING_ATTRS & set(attrs)):
        if not text(attrs[k]):
            out.append("%s: `%s` must be a non-empty string" % (where, k))
    if "multiple" in attrs and not isinstance(attrs["multiple"], bool):
        out.append("%s: `multiple` must be a boolean" % where)
    if kind in ("input", "textarea") and "password" in str(attrs.get("label", "")).casefold():
        out.append("%s: `label` contains forbidden word, password" % where)
    if kind in ("dropdown", "checkboxes"):
        check_options(where, kind, attrs, out)


def check_validations(where, kind, v, out):
    if not isinstance(v, dict):
        out.append("%s: `validations` must be a map" % where)
        return
    for k in sorted(set(v) - ELEMENTS[kind][2]):
        out.append("%s: `%s` is not a permitted validation" % (where, k))
    if "required" in v and not isinstance(v["required"], bool):
        out.append("%s: `required` must be a boolean" % where)
    if "min_length" in v and not (is_int(v["min_length"]) and v["min_length"] >= 0):
        out.append("%s: `min_length` must be a non-negative integer" % where)
    if "accept" in v and not text(v["accept"]):
        out.append("%s: `accept` must be a non-empty string" % where)


def check_element(i, el, out):
    """The element's type, or None when it is not one GitHub knows."""
    where = "body[%d]" % i
    if not isinstance(el, dict):
        out.append("%s: must be a map" % where)
        return None
    for k in sorted(set(el) - ELEMENT_KEYS):
        out.append("%s: `%s` is not a permitted key" % (where, k))
    kind = el.get("type")
    if kind is None:
        out.append("%s: required key type is missing" % where)
        return None
    if kind not in ELEMENTS:
        out.append("%s: `%s` is not a valid input type" % (where, kind))
        return None
    if "id" in el and not (isinstance(el["id"], str) and ID.match(el["id"])):
        out.append("%s: `id` can only contain numbers, letters, -, _" % where)
    attrs = el.get("attributes")
    if not isinstance(attrs, dict):
        out.append("%s: required key attributes is missing or not a map" % where)
    else:
        check_attributes(where, kind, attrs, out)
    if "validations" in el:
        check_validations(where, kind, el["validations"], out)
    return kind


def fold(label):
    return re.sub(r"[^0-9a-z]", "", label.casefold())


def check_body(body, out):
    if not isinstance(body, list) or not body:
        out.append("Body cannot be empty")
        return
    kinds = [check_element(i, el, out) for i, el in enumerate(body)]
    if all(k in (None, "markdown") for k in kinds):
        out.append("Body must contain at least one non-markdown field")
    ids, labels = {}, {}
    for i, el in enumerate(body):
        if kinds[i] in (None, "markdown"):
            continue
        if isinstance(el.get("id"), str):
            ids.setdefault(el["id"], []).append(i)
        label = el.get("attributes", {}).get("label")
        # GitHub lets an `id` tell two labels apart, so only id-less fields collide.
        if text(label) and "id" not in el:
            labels.setdefault(fold(label), []).append(i)
    for key, at in sorted(ids.items()):
        if len(at) > 1:
            out.append("Body must have unique ids: `%s` at body%s" % (key, at))
    for key, at in sorted(labels.items()):
        if len(at) > 1:
            out.append("Body must have unique labels: body%s are too similar" % at)


def check_form(doc):
    out = []
    if not isinstance(doc, dict):
        return ["a form must be a map with `name`, `description` and `body`"]
    check_top(doc, out)
    if "body" not in doc:
        out.append("required top level key `body` is missing")
    else:
        check_body(doc["body"], out)
    return out


def check_config(doc):
    if doc is None:
        return []
    if not isinstance(doc, dict):
        return ["config.yml must be a map"]
    out = ["`%s` is not a permitted key" % k
           for k in sorted(set(doc) - {"blank_issues_enabled", "contact_links"})]
    if "blank_issues_enabled" in doc and not isinstance(doc["blank_issues_enabled"], bool):
        out.append("`blank_issues_enabled` must be a boolean")
    links = doc.get("contact_links", [])
    if not isinstance(links, list):
        return out + ["`contact_links` must be a list"]
    for i, link in enumerate(links):
        if not isinstance(link, dict):
            out.append("contact_links[%d] must be a map" % i)
            continue
        for k in sorted(set(link) - {"name", "url", "about"}):
            out.append("contact_links[%d]: `%s` is not a permitted key" % (i, k))
        for k in ("name", "url", "about"):
            if not text(link.get(k)):
                out.append("contact_links[%d]: `%s` must be a non-empty string" % (i, k))
        if text(link.get("url")) and not URL.match(link["url"]):
            out.append("contact_links[%d]: `url` must be an http(s) URL" % i)
    return out


# ---- the tree -----------------------------------------------------------------------------------

def judge(docs, errors):
    """{path: [problem]} for every file with one. Names are compared across forms, since GitHub
    requires a form's name to be unique among every template."""
    found = {p: [e] for p, e in errors.items()}
    names = {}
    for path, doc in sorted(docs.items()):
        problems = check_config(doc) if os.path.basename(path) == CONFIG else check_form(doc)
        if problems:
            found[path] = problems
        if os.path.basename(path) != CONFIG and isinstance(doc, dict) and text(doc.get("name")):
            names.setdefault(doc["name"].strip(), []).append(path)
    for name, paths in names.items():
        if len(paths) > 1:
            for p in paths:
                found.setdefault(p, []).append("`name` %r is shared with another form" % name)
    return found


def check_tree(root):
    d = TEMPLATES
    names = sorted(f for f in os.listdir(os.path.join(root, d)) if f.endswith((".yml", ".yaml")))
    forms = [f for f in names if f != CONFIG]
    # A gate that examined nothing must not report clean
    # (design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md).
    if not forms:
        print("issue forms: %s holds no issue form, so this examined nothing" % d, file=sys.stderr)
        return 1
    docs, errors, note = load(root, [os.path.join(d, f) for f in names])
    if note:
        print("issue forms: " + note, file=sys.stderr)
        return 1
    found = judge(docs, errors)
    if found:
        for path in sorted(found):
            for problem in found[path]:
                print("  %s: %s" % (path, problem), file=sys.stderr)
        print("issue forms: %d of %d file(s) would not render. GitHub drops such a form from the "
              "issue chooser without any red check; see helpers/issue_forms.py."
              % (len(found), len(names)), file=sys.stderr)
        return 1
    print("issue forms: %d form(s) and %s, each passes GitHub's documented form schema"
          % (len(forms), "config.yml" if CONFIG in names else "no config.yml"))
    return 0


# ---- fixtures -----------------------------------------------------------------------------------

GOOD = """\
name: Bug report
description: Something wrong.
labels: ["bug"]
body:
  - type: markdown
    attributes:
      value: Read this first.
  - type: input
    id: sha
    attributes:
      label: Commit sha
    validations:
      required: true
      min_length: 7
  - type: dropdown
    id: arch
    attributes:
      label: Architecture
      options: [aarch64, riscv64]
  - type: checkboxes
    attributes:
      label: Checked
      options:
        - label: I searched
          required: true
  - type: upload
    attributes:
      label: Logs
"""

GOOD_CONFIG = """\
blank_issues_enabled: false
contact_links:
  - name: Security
    url: https://example.org/security
    about: Privately.
"""

# (name, {file: text} added to or replacing the good tree, the problem it must report or None for
# a tree that must pass). Matching the problem, not only the exit, keeps a fixture from passing on
# an unrelated failure while the rule it names has stopped firing.
CASES = [
    ("the shipped shape: forms and a config", {}, None),
    ("a form with no name", {"a.yml": GOOD.replace("name: Bug report\n", "")},
     "key `name` is missing"),
    ("a type GitHub does not know", {"a.yml": GOOD.replace("type: upload", "type: textbox")},
     "not a valid input type"),
    ("two fields sharing an id", {"a.yml": GOOD.replace("id: arch", "id: sha")},
     "unique ids"),
    ("two id-less labels that differ only in case",
     {"a.yml": GOOD + "  - type: textarea\n    attributes:\n      label: CHECKED\n"},
     "unique labels"),
    ("a body of markdown only",
     {"a.yml": "name: x\ndescription: y\nbody:\n  - type: markdown\n    attributes:\n"
               "      value: hi\n"}, "at least one non-markdown field"),
    ("a bare yes among dropdown options, a boolean to YAML 1.1",
     {"a.yml": GOOD.replace("[aarch64, riscv64]", "[aarch64, yes]")}, "must not include booleans"),
    ("the reserved option None", {"a.yml": GOOD.replace("[aarch64, riscv64]", "[aarch64, None]")},
     "reserved word, none"),
    ("password in an input's label", {"a.yml": GOOD.replace("Commit sha", "Commit password")},
     "forbidden word, password"),
    ("an attribute the type does not take",
     {"a.yml": GOOD.replace("      label: Logs", "      label: Logs\n      placeholder: x")},
     "`placeholder` is not a permitted attribute"),
    ("a body after a --- separator", {"a.yml": GOOD.replace("body:", "---\nbody:")},
     "2 YAML documents"),
    ("one key given twice", {"a.yml": GOOD.replace("    id: sha\n", "    id: sha\n    id: sha2\n")},
     "appears twice"),
    ("a bare yes as a key", {"a.yml": GOOD.replace("labels:", "yes: 1\nlabels:")},
     "key true is not a string"),
    ("two forms with one name", {"b.yml": GOOD}, "shared with another form"),
    ("config: blank_issues_enabled as a string",
     {CONFIG: GOOD_CONFIG.replace("false", '"no"')}, "must be a boolean"),
    ("config: a contact link with no url",
     {CONFIG: GOOD_CONFIG.replace("    url: https://example.org/security\n", "")},
     "`url` must be a non-empty string"),
    ("a directory holding only config.yml", {"a.yml": None}, "examined nothing"),
]


def run_captured(root):
    """(exit code, everything check_tree printed) over a throwaway tree."""
    saved = sys.stdout, sys.stderr
    sys.stdout = sys.stderr = io.StringIO()
    try:
        return check_tree(root), sys.stdout.getvalue()
    finally:
        sys.stdout, sys.stderr = saved


def selftest():
    failed = 0
    for name, files, want in CASES:
        tree = {"a.yml": GOOD, CONFIG: GOOD_CONFIG}
        tree.update(files)
        with tempfile.TemporaryDirectory() as tmp:
            d = os.path.join(tmp, TEMPLATES)
            os.makedirs(d)
            for f, body in tree.items():
                if body is not None:
                    with open(os.path.join(d, f), "w") as fh:
                        fh.write(body)
            rc, said = run_captured(tmp)
        ok = rc == 0 if want is None else (rc == 1 and want in said)
        if not ok:
            failed += 1
            print("issue forms selftest: %s: want %s, got exit %d:\n%s"
                  % (name, "a pass" if want is None else repr(want), rc, said), file=sys.stderr)
    if failed:
        return 1
    print("issue forms selftest: %d fixtures, each failing case fails on its own rule" % len(CASES))
    return 0


def main(argv):
    if argv and argv[0] == "--selftest":
        return selftest()
    if argv:
        print("usage: helpers/issue_forms.py [--selftest]", file=sys.stderr)
        return 2
    root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
    return check_tree(root)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
