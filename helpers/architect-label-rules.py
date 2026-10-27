#!/usr/bin/env python3
"""The diff-reading half of `needs-architect` (§88): a predicate over a unified diff.

§88 (`needs-architect` as a required check, rather than as a script's restraint) built the
enforcement half -- `architect-hold.yml` fails while the label is present -- and named plainly what
it did not build: *"a check that reads the diff and demands the label when the syscall surface or
the dependency graph moved... the second is the real answer."* `.github/workflows/coe-architect-
label.yml` built that answer for one shape, a correction of error. This is the general one, wired
by `.github/workflows/architect-label.yml`.

This module is a pure predicate: it reads a unified diff (as `git diff --unified=1000000 base head`
produces, so every file's diff is one hunk holding the WHOLE file, changed lines marked) and reports
which of six rules fired, and where. It never runs `git` and never talks to GitHub; the workflow
does both and pipes the diff text in on stdin. That split is what makes `--selftest` below run
against literal fixture strings, no repository and no subprocess required, cheap enough for
`script/lint-fleet` to run on every invocation.

    python3 helpers/architect-label-rules.py --selftest        # fixtures; script/lint-fleet runs this
    git diff --unified=1000000 BASE HEAD | python3 helpers/architect-label-rules.py
        # prints one "rule: file: line" per match on stdout, exits 0 if anything fired, 1 if
        # nothing did, 2 on a usage error, 3 if it crashed. Nothing here reads BASE/HEAD; a caller
        # diffs first. A crash is 3 rather than Python's own 1 so that it can never read as
        # "nothing fired" (milestone 641 (a mechanism that reports clean says over how many units,
        # and zero is loud), provisional): the workflow routes 2 and 3 to a person.

# The six rules (the brief's wording; this file is the one place that has to agree with it)

1. **abi-surface**: an object type, method number or syscall number moved under `crates/abi/`.
   Detected as any changed `pub const NAME: <int type> = ...` line under that path. Broad on
   purpose: `crates/abi/src/lib.rs` also declares rights bitflags and the register convention's
   constants the same way, so this also fires on those (see BUGS). A false positive there costs a
   human one look; a syscall number landing unlabelled costs more. A removed `pub const A: T = V`
   paired with an added `pub const B: T = V` (same type, same value) is a rename and does not fire;
   a changed value, an added constant with no removed partner, and a deleted one still do.
2. **dependency**: a (name, version requirement) entered a `Cargo.toml` that the base tree does not
   already have. Computed per file as head's external deps minus base's, then dropped if the pair
   is in any manifest at the base revision (vendored ones included) or the name is locked in any
   base `Cargo.lock` at a version the requirement admits. So a dep already in the graph at that
   version does not fire (#1597), a features-only change does not, a `path = ` entry never counts,
   and a dep moved to a version nothing at the base uses DOES fire. The base comes from
   `--base-rev REV`; without it nothing is known and every changed dependency fires. See BUGS for
   what "external" cannot see: a multi-line inline table.
3. **format-crate**: a version constant, magic value, or documented on-disk layout moved in a crate
   that is classified, from its own head content, as a format crate: it declares a `pub const`
   ending in `VERSION` or `MAGIC`, or its module doc carries a markdown table with a `version`
   column (the two tells the brief names). Firing is on a changed `VERSION`/`MAGIC` constant line, or
   a changed doc or block comment line (`//!`, `///`, `/* */`) outside test code that states a
   layout (a markdown table row, `offset`, `width`, `bit N`, an `N..M` range). A plain `//`
   comment, a line under `#[test]` or `#[cfg(test)]`, and a removed/added pair with the same
   numbers and layout words (an identifier renamed in the sentence) do not fire.
4. **spawnproto**: a `pub const` changed in a file named `spawnproto.rs` (the wire layout
   `crates/grant_plan/src/spawnproto.rs` documents; matched by filename rather than the one path in
   the tree today, so a second one elsewhere is still caught). Same-value renames pair off as in
   rule 1.
5. **decisions**: `CLAUDE.md` changed at all, or a `design/decisions/*.md` file (excluding
   `README.md`, which says how the directory works and holds no decision) changed OUTSIDE its YAML frontmatter block (the first `---`-delimited
   section) by more than bookkeeping. A frontmatter-only edit (a status flip, a date) is
   provenance, not the decision's substance, and does not fire. Nor, since calef's ruling of
   2026-10-11 (UTC), does a bookkeeping edit: the body reads the same once link targets, backticked
   repository paths, wrapping and whitespace are normalized away (`rule_decisions` has the exact
   terms). A changed word, number, quotation, heading, list item or table cell fires, and so does
   a link added or removed rather than retargeted.
6. **syscall-error**: a changed code line in `kernel/src/syscall.rs` (or a future
   `kernel/src/syscall/`) that names an `Error::` variant: a method returns an error it did not, or
   stops returning one. Added by lane/architect-queue, 2026-10-06 (UTC), after #1745's forged-cursor
   refusal went unlabeled. A line removed and re-added verbatim (a move) pairs off; comments do
   not count. Measured over the 31 merges that touched the file from 2026-09-01 to 2026-10-06: it
   fires on 13 (#1678 #1665 #1644 #1630 #1360 #1418 #1383 #1378 #1351 #1373 #1366 #1372 #882), and
   5 of those were labeled at the time. calef ruled the breadth on #1792, 2026-10-06 (UTC): "Keep
   it broad." The other 8, classified by reading each fired diff, so a later narrowing is measured:
     changed what a caller can receive (4): #1351 (notification methods, new NotPermitted and
       WrongObject), #1373 (`RETYPE` takes a count; OutOfMemory on zero), #1372 (a per-client
       window refuses with NotPermitted), #882 (port-range methods, NotPermitted and BadMethod);
     false alarms, a refactor returning the same error on the same path (4): #1644
       (`current_cap` moved onto the revocation hold), #1630 (lookup code moved to `sched.rs`),
       #1366 (the capability-table-full path restructured), #1665 (`retype_page` hoisted out of
       a closure, still OutOfMemory).
   Narrowing to added lines would keep all 4 true ones and drop only #1630 of the false ones.

# Measured rate, 2026-10-04 UTC

The bot's own comments on the 70 most recent PRs labelled `needs-architect` or `architect-ruled`
(one count per PR per rule; "true" means a new or changed wire value, syscall number, dependency new
to the graph, or a decisions edit):

    rule          true  false  the false ones
    abi-surface      2      1  #1584 rename, value unchanged
    dependency       1      1  #1597 redox_syscall 0.9.0, already in three manifests
    format-crate     2      7  `//!` prose (#1289 #1369 #1374 #1443 #1511 #1527), a diagram (#1584); now quiet, #1402 still fires
    spawnproto       3      3  #1377 #1419 #1421: IMAGE_MAX_PAGES, not in those PRs' diffs (not reproduced)
    decisions       23      0  by definition; about 7 are link or rename sweeps (#1369 #1371 #1374 #1427 #1510 #1527 #1289)

Narrowed on this evidence: rules 1, 2, 3 and 4 (above). Decisions was left alone that day, on the
reasoning that a sweep is still an edit to a ratified record. calef overruled that for bookkeeping
on 2026-10-11 (UTC), after approving #1953 by hand: "I need to review and approve decisions, but I
don't think I need to approve minor bookkeeping edits like this one." The next section measures
the narrowing.

# decisions' narrowing, measured 2026-10-11 UTC

Rule 5 rerun, before and after, over every pull request merged from 2026-09-04 to 2026-10-11 that
touched `design/decisions/` or `CLAUDE.md` (75), plus #1953, each diffed from its merge base to its
head as the workflow does. The old rule fires on 73 of them (387 file firings), the new on 71 (364).
The 23 files that went quiet are in 6 pull requests, and a word diff of each shows nothing changed
but a path or a link target. The 2026-10-04 list, plus #1953:

    pr     before  after  what the edit was
    #1369  -       -      no decision file in its merged diff; stacked on #1427 when the bot
                          fired, it carried #1427's files (below), so it was not a sweep
    #1371  -       -      the same as #1369
    #1374  fires   fires  §229 (how a bare name at the prompt reaches an installed program)
                          amended with calef's ruling (true)
    #1427  fires   fires  §227 (how Tab reaches the shell), §228 (how a set of matched names
                          reaches the progenitor) and §229 new; §21 (the terminal is a userspace
                          component) amended (true)
    #1510  fires   fires  6 of 9 files quiet (links retargeted); 3 still fire on `crickertech/nife`
                          renamed `nifeos/nife`, an identifier rename sweep (false)
    #1527  fires   quiet  9 of 9 files quiet: proposal paths renumbered into the roadmap
    #1289  fires   fires  §212 (a prose budget) and §213 (writing standards) each gained a
                          rule on renames (true)
    #1953  fires   quiet  two `notes/timer-capability.md` citations linked to the split note

So the 2026-10-04 count of "about 7" sweeps was high: #1369 and #1371 were #1427's new sections,
and #1374, #1427 and #1289 were rulings. The other partly quiet ones are #1174 (3 of 7 files),
#1235 (1 of 3) and #958 (3 of 4). The firings that remain and are not substance, by class:

    class                                         files  pull requests
    identifier rename sweep                           6  #1510 (`crickertech` to `nifeos`), #1174
                                                         (`SHELL_CHECK_*` to `SWISH_CHECK_*`)
    emphasis sweep (bold removed)                    26  #1706
    a line-number citation made a heading anchor       3  #1943 (2), #1940 (1)
    a path not in backticks, or a bare file name       4  #1293

The first is the class the ruling leaves firing on purpose: a renamed identifier is a word, and
whether a rename touches a ruling's meaning is not decidable from text. The other three are outside
the ruling's three terms and were not widened into; each is in BUGS. 291 files fired because words
were added or removed, and those were not classified one by one. Among them are §-citation glosses
(#1174's §173 (narrowing the root of the shell's namespace) gained a milestone title, #1235's
§130 (how the kernel-resident CMOS RTC reaches the userspace clock service) a section title), which
fire by design.

Falsified the same day, each by hand against `--selftest`: `normalize_bookkeeping` returning its
input fires the two quiet bookkeeping fixtures (#1953's, and the reflow); joining each line alone
instead of each block fires the same two; dropping the base-against-head comparison fires those and
the frontmatter flip; and `is_repo_path` accepting any path makes the `/dev/kmsg` fixture go quiet.

# format-crate's second narrowing, measured 2026-10-08 UTC

calef on #1838's hold: "Please deal with the false positives." It fired on a `//` line beside a
test's assert (`... the 1900-to-1970 offset ...`); no constant, field or layout moved. Rule 3 rerun
over every merge from 2026-09-12 to 2026-10-07 (1,000 pull requests) plus #1838: 29 file firings.
13 fire on a `VERSION`/`MAGIC` constant line and are untouched. The other 16 fired on a comment
alone, classified by reading each diff:

    class                                        n  kept  the pull requests
    layout table edited (true)                   3     3  #1402 #1385 #1360 (manifest_note rows)
    true by coincidence (a const moved, the      3     2  kept #1727 (PUT_REFUSED) #1066 (PAGE_VA);
      comment that matched was unrelated)                 dropped #1352 (PAGE_VA, matched in a test)
    false, `//` comment or test code             3     0  #1838 #1569 #805
    false, identifier renamed in the sentence    3     0  #1255, #860 (two files)
    false, still firing                          4     4  #1088 (a cost table), #927 #853 #1067 (prose)

So 7 of 29 file firings drop (6 false, plus #1352's coincidental catch), every layout-table edit
still fires, and #1402's class is in the fixtures. #1352 shows the real gap, now in BUGS: a format
crate's other `pub const` values are not read at all.

# BUGS

- **decisions still fires on a rename sweep, an emphasis sweep, a line citation turned anchor, and
  an unbackticked path.** Measured 2026-10-11 (UTC) over 75 merges: 6, 26, 3 and 4 file firings
  (the section above names them). The first is deliberate: calef's 2026-10-11 ruling covered link
  targets, paths and wrapping, and a renamed identifier is a word. The other three are candidates
  for a later narrowing, each needing its own ruling.
- **decisions' repository paths are a fixed list of top-level entries** (`REPO_TOP_LEVEL`). A
  top-level directory added later and not listed makes its paths fire, the safe direction. A link
  whose text is a lone backticked path reads as that path, so turning `` `notes/a.md` `` into a link
  to `notes/a/b.md` is quiet (#1953); linking plain words still fires.
- **Renames pair by one-line value text.** `pub const A: u64 = 1;` against `pub const B: u64 = 1;`
  pairs; a `const` whose initializer spans lines, or whose type differs, never does (it fires). A
  pair of unrelated constants that swap to each other's spelling at equal values would pass; that is
  a rename by definition.
- **Rule 2's lock test is caret-only.** A requirement with a comma, operator or wildcard is never
  matched against a lockfile (it fires unless an identical manifest entry exists).
- **syscall-error reads one file.** An error moved into a helper the dispatcher calls (`revoke.rs`,
  `sched.rs`) is missed, and a refactor that rewrites a line holding `Error::` fires. Both measured
  at zero and unknown respectively on the 31 merges above; revisit on the first miss.
- **format-crate reads only `VERSION`/`MAGIC` constants.** A format crate's other wire values
  (#1352 and #1066 moved `PAGE_VA`, #1727 added `PUT_REFUSED`) fire only if an unrelated layout
  comment happens to change beside them, and #1352's no longer does. Unmeasured beyond those three;
  firing on every `pub const` in a format crate is the obvious widening and has not been tried.
- **The test-code mask counts braces.** A raw string or a multi-line string holding an unbalanced
  `{` or `}` misplaces where a test module ends. Leaving a test region late hides a layout comment
  after it; the fixtures cover one-line strings and char literals only.
- **format-crate's layout-comment test is a regex.** A layout described in a comment without a
  table row, offset, width, bit number or `N..M` range (say, a prose paragraph that moves a field)
  is missed unless a constant moves with it. Residual and unmeasured: the table-row form is the one
  this tree's layouts use (#1402); revisit if a layout change slips by.
"""
import re
import subprocess
import sys

# ---- parsing a `git diff --unified=<huge>` into one whole-file view per path -------------------
#
# With a big enough -U, git merges every hunk in a file into one, from the first changed line back
# to the top of the file and forward to the bottom, PROVIDED no unrelated content sits between two
# hunks past the context window; -U1000000 exceeds every file's line count in this tree, so in
# practice each file's diff is exactly one hunk holding the whole file, in order, each line tagged
# ' ' (context, i.e. unchanged), '+' (added) or '-' (removed). That is what lets `head_lines` /
# `base_lines` below reconstruct each side's full content by keeping/dropping only the other side's
# lines, with no separate `git show` call.

FILE_RE = re.compile(r'^diff --git a/(.+?) b/(.+)$')


def parse_diff(text):
    """A unified diff -> [{'path': str, 'lines': [(prefix, content), ...]}, ...].

    `prefix` is ' ', '+' or '-'. Extended headers (`index ...`, `new file mode ...`, `rename from
    ...`, `Binary files ... differ`) start with none of ' +-' and are silently dropped; so are the
    `--- a/x` / `+++ b/x` file markers and `@@ ... @@` hunk markers, since the file path already
    came from the `diff --git` line and this tool never needs a hunk's line numbers.
    """
    files = []
    cur = None
    for raw in text.splitlines():
        m = FILE_RE.match(raw)
        if m:
            if cur is not None:
                files.append(cur)
            # The b/ path names where the file ends up; a pure deletion still has SOME b/ path in
            # git's own header (historically `/dev/null` is only the +++ line, not this one), and
            # nothing here acts on a file with no b/ path anyway (no rule fires on a deleted file).
            cur = {'path': m.group(2), 'lines': []}
            continue
        if cur is None:
            continue
        if raw.startswith('--- ') or raw.startswith('+++ ') or raw.startswith('@@'):
            continue
        if raw[:1] in ('+', '-', ' '):
            cur['lines'].append((raw[0], raw[1:]))
        # else: extended header or "\ No newline at end of file"; not a content line, drop it.
    if cur is not None:
        files.append(cur)
    return files


def head_lines(fd):
    """This file's content at the diff's post-image (context + added, removed dropped)."""
    return [line for prefix, line in fd['lines'] if prefix != '-']


def base_lines(fd):
    """This file's content at the diff's pre-image (context + removed, added dropped)."""
    return [line for prefix, line in fd['lines'] if prefix != '+']


def changed_lines(fd):
    """(prefix, line) pairs for every added or removed line, in file order."""
    return [(p, line) for p, line in fd['lines'] if p in ('+', '-')]


# ---- rule 1: crates/abi -------------------------------------------------------------------------

ABI_CONST_RE = re.compile(
    r'^\s*pub const [A-Za-z_][A-Za-z0-9_]*\s*:\s*(u8|u16|u32|u64|usize|i8|i16|i32|i64|isize)\b')


CONST_DECL_RE = re.compile(r'^\s*pub const ([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([^=]+?)\s*=\s*(.*)$')


def unpaired_const_changes(fd, const_re):
    """Changed `pub const` lines (matching `const_re`) left over after pairing renames.

    A removed `pub const A: T = V` and an added `pub const B: T = V` in the same file, same type and
    same value text, is a rename: the number on the wire did not move (§88's concern), only its
    spelling did, and a spelling is an architect's call that a rename PR has usually already carried
    (#1584). Each removed line pairs with at most one added line. What stays unpaired still fires: a
    changed value (the pair's value differs), an added constant with no removed partner, and a
    removed constant with no added partner (a deleted number is a surface change too). The value
    text is compared after dropping a trailing `;` and `//` comment, and a multi-line initializer
    never pairs (its line has no closing `;`), so it fires as before.
    """
    removed, added = [], []
    for prefix, line in changed_lines(fd):
        if not const_re.match(line):
            continue
        m = CONST_DECL_RE.match(line)
        key = None
        if m:
            value = re.sub(r'\s*//.*$', '', m.group(3)).rstrip()
            if value.endswith(';'):  # a one-line initializer; a multi-line one never pairs
                key = (m.group(2).strip(), value[:-1].rstrip())
        (removed if prefix == '-' else added).append((key, line))
    left = []
    for key, line in added:
        hit = next((i for i, (rk, _l) in enumerate(removed) if key is not None and rk == key), None)
        if hit is None:
            left.append(line)
        else:
            removed.pop(hit)
    left.extend(line for _k, line in removed)
    return left


def rule_abi_surface(fd, out):
    if not (fd['path'].startswith('crates/abi/') and fd['path'].endswith('.rs')):
        return
    left = unpaired_const_changes(fd, ABI_CONST_RE)
    if left:
        out.append(('abi-surface', fd['path'], left[0].strip()))  # one citation per file


# ---- rule 2: an external dependency entered a Cargo.toml -----------------------------------------

SECTION_RE = re.compile(r'^\[(.+)\]$')
DEP_KEY_RE = re.compile(r'^([A-Za-z0-9_.-]+)\s*=\s*(.*)$')


VERSION_RE = re.compile(r'\bversion\s*=\s*"([^"]*)"')
BARE_REQ_RE = re.compile(r'^"([^"]*)"')


def toml_external_deps(lines):
    """{(dep_name, requirement)} in a dependency section, on one line, with no `path = ` on it.

    The requirement is the `version = "..."` string (or the bare `name = "..."` string); a dep with
    neither (a git or `workspace = true` entry) uses its whole value text, so a changed rev still
    reads as a change. See the module docstring's BUGS entry for what a multi-line inline table does.
    """
    external = set()
    in_deps = False
    for raw in lines:
        stripped = raw.strip()
        m = SECTION_RE.match(stripped)
        if m:
            in_deps = m.group(1).endswith('dependencies')
            continue
        if not in_deps or not stripped or stripped.startswith('#'):
            continue
        km = DEP_KEY_RE.match(stripped)
        if not km:
            continue
        name, rest = km.group(1), km.group(2)
        if 'path' in rest:
            continue  # `path = "../crates/x"` somewhere in the value: an in-tree dependency
        vm = BARE_REQ_RE.match(rest) or VERSION_RE.search(rest)
        external.add((name, vm.group(1) if vm else rest.strip()))
    return external


def lock_versions(text):
    """{name: {version, ...}} from one Cargo.lock's `[[package]]` blocks."""
    out, name = {}, None
    for line in text.splitlines():
        if line.startswith('name = '):
            name = line.split('"')[1]
        elif line.startswith('version = ') and name is not None:
            out.setdefault(name, set()).add(line.split('"')[1])
            name = None
    return out


def req_matches(req, version):
    """Does a plain caret-style requirement (`1.2`, `^0.9.0`, `=0.4.1`, `~1.2`) admit `version`?

    Anything with a comma, comparison operator or wildcard answers False: the tool then fires, which
    is the direction it is allowed to be wrong in.
    """
    r = req.strip().lstrip('^~=').strip()
    if not re.fullmatch(r'\d+(\.\d+){0,2}', r):
        return False
    rp = [int(x) for x in r.split('.')]
    vp = [int(x) for x in re.split(r'[.+-]', version)[:3]]
    if len(vp) < 3 or len(rp) > len(vp):
        return False
    if req.strip().startswith('='):
        return rp == vp[:len(rp)]
    if vp[:len(rp)] < rp:
        return False  # the locked version is older than the requirement
    # Caret: the leftmost nonzero requirement component (or the last given one) must match.
    lead = next((i for i, x in enumerate(rp) if x != 0), len(rp) - 1)
    return vp[:lead + 1] == rp[:lead + 1]


class KnownDeps:
    """What the base tree already depends on: every non-path (name, requirement) in any manifest,
    and every (name, version) in any Cargo.lock. Empty by default, which makes rule 2 fire on any
    dependency the diff touches (the pre-narrowing behaviour, minus a pure features change)."""

    def __init__(self, manifests=(), locks=()):
        self.pairs = set()
        for text in manifests:
            self.pairs |= toml_external_deps(text.splitlines())
        self.locked = {}
        for text in locks:
            for name, vs in lock_versions(text).items():
                self.locked.setdefault(name, set()).update(vs)

    def has(self, name, req):
        if (name, req) in self.pairs:
            return True
        return any(req_matches(req, v) for v in self.locked.get(name, ()))


def known_deps_from_git(rev):
    """KnownDeps for every Cargo.toml / Cargo.lock at `rev` (vendored ones included).

    The one place this tool runs `git`, and only when the caller names a base with --base-rev;
    the selftest builds KnownDeps from literal strings instead.
    """
    names = subprocess.run(['git', 'ls-tree', '-r', '--name-only', rev], check=True,
                           capture_output=True, text=True).stdout.splitlines()
    def show(path):
        return subprocess.run(['git', 'show', f'{rev}:{path}'], check=True,
                              capture_output=True, text=True).stdout
    return KnownDeps(
        manifests=[show(n) for n in names if n.endswith('Cargo.toml')],
        locks=[show(n) for n in names if n.endswith('Cargo.lock')])


def rule_dependency(fd, out, known):
    if not fd['path'].endswith('Cargo.toml'):
        return
    changed = toml_external_deps(head_lines(fd)) - toml_external_deps(base_lines(fd))
    for name, req in sorted(changed):
        if not known.has(name, req):
            out.append(('dependency', fd['path'], f'{name} {req}'))


# ---- rule 3: a format crate's version, magic or documented layout --------------------------------

VERSION_OR_MAGIC_CONST_RE = re.compile(r'^\s*pub const \w*(VERSION|MAGIC)\w*\s*:')
VERSION_TABLE_RE = re.compile(r'^\s*//!.*\|\s*version\s*\|', re.IGNORECASE)


def is_format_crate_source(lines):
    for line in lines:
        if VERSION_OR_MAGIC_CONST_RE.match(line) or VERSION_TABLE_RE.match(line):
            return True
    return False


def is_comment_only(line):
    """A line that is only comment text: `//`, `///`, `//!`, or the inside or edges of a `/* */`."""
    t = line.strip()
    return t.startswith(('//', '/*', '*/')) or (t.startswith('*') and not t.startswith('*b"'))


# A comment that states a layout rather than explaining one: a markdown table row, or the words
# offset / width / `bit N`, or an `N..M` range. Chosen by running candidates over the 9 firings in
# the measured table: this matches #1402's table-row edit and none of the 7 prose or diagram ones
# (the bare word "bytes" was tried and matched 4 of those 7, so it is not here).
LAYOUT_COMMENT_RE = re.compile(
    r'^\s*(//[/!]?|/?\*)\s*\|.*\|\s*$|\boffsets?\b|\bbits?\s+\d|\bwidths?\b|\b\d+\.\.=?\w+',
    re.IGNORECASE)


# Test code, which documents no format: an item under `#[test]`, `#[cfg(test)]` or
# `#[cfg(all(test, ...))]`. `#[cfg(any(test, kani))]` is not here, since it compiles outside tests.
TEST_ATTR_RE = re.compile(r'^\s*#\[(test|cfg\(test\)|cfg\(all\(test\b)')
STRING_OR_CHAR_RE = re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])\'')


def test_code_mask(lines):
    """[bool] per line: is this line inside (or the header of) a test item?

    Brace counting over code text with string and char literals and `//` tails removed. A test
    attribute arms the next item: its first `{` opens the region and the matching `}` closes it,
    and a `;` before any `{` (`#[cfg(test)] mod tests;`) disarms it. See BUGS for what a raw string
    holding a brace does.
    """
    mask = []
    depth, armed, region = 0, False, None  # region: the depth the test item's `{` opened from
    for line in lines:
        inside = armed or region is not None
        if not is_comment_only(line):
            if region is None and TEST_ATTR_RE.match(line):
                armed = inside = True
            code = STRING_OR_CHAR_RE.sub('""', line).split('//')[0]
            for ch in code:
                if ch == '{':
                    if armed and region is None:
                        region, armed = depth, False
                    depth += 1
                elif ch == '}':
                    depth -= 1
                    if region is not None and depth <= region:
                        region = None
                elif ch == ';' and armed and region is None:
                    armed = False
        mask.append(inside)
    return mask


def changed_lines_outside_tests(fd):
    """changed_lines(fd), less those inside test code on their own side of the diff."""
    head_mask = iter(test_code_mask(head_lines(fd)))
    base_mask = iter(test_code_mask(base_lines(fd)))
    left = []
    for prefix, line in fd['lines']:
        in_head = next(head_mask) if prefix != '-' else False
        in_base = next(base_mask) if prefix != '+' else False
        if prefix == '+' and not in_head or prefix == '-' and not in_base:
            left.append((prefix, line))
    return left


def is_doc_or_block_comment(line):
    """`//!`, `///` or a `/* */` line: where a crate documents its layout. A plain `//` explains the
    code beside it (#1838: `// ... the 1900-to-1970 offset ...` over a test's assert)."""
    t = line.strip()
    return t.startswith(('//!', '///')) or not t.startswith('//')


def rule_format_crate(fd, out):
    if not (fd['path'].startswith('crates/') and '/src/' in fd['path']
            and fd['path'].endswith('.rs')):
        return
    if not is_format_crate_source(head_lines(fd)):
        return
    # A comment-only edit fires only if it reads as a layout statement (measured: 7 of 9 firings
    # were `//!` prose or a diagram that does not), sits in a doc or block comment, and is outside
    # test code (measured 2026-10-08: see "format-crate's second narrowing" above). A VERSION/MAGIC
    # code line fires wherever it is.
    for prefix, line in changed_lines(fd):
        if not is_comment_only(line) and VERSION_OR_MAGIC_CONST_RE.match(line):
            out.append(('format-crate', fd['path'], line.strip()))
            return
    # A removed and an added layout line with the same layout signature (every number, and every
    # layout match's text) pair off, as a constant rename does in rule 1: `[plausible]` becoming
    # `[is_plausible]` in a sentence about an offset moved no byte (#1255, #860). A table row's
    # match is the whole row, so a table row pairs only with itself.
    removed, added = [], []
    for prefix, line in changed_lines_outside_tests(fd):
        if (is_comment_only(line) and is_doc_or_block_comment(line)
                and LAYOUT_COMMENT_RE.search(line)):
            (removed if prefix == '-' else added).append((layout_signature(line), line))
    for key, line in added:
        hit = next((i for i, (rk, _l) in enumerate(removed) if rk == key), None)
        if hit is None:
            out.append(('format-crate', fd['path'], line.strip()))
            return
        removed.pop(hit)
    if removed:
        out.append(('format-crate', fd['path'], removed[0][1].strip()))


def layout_signature(line):
    return (tuple(re.findall(r'\d+', line)),
            tuple(m.group(0).strip().lower() for m in LAYOUT_COMMENT_RE.finditer(line)))


# ---- rule 4: spawnproto ---------------------------------------------------------------------------

SPAWNPROTO_CONST_RE = re.compile(r'^\s*pub const \w+\s*:')


def rule_spawnproto(fd, out):
    if not fd['path'].endswith('spawnproto.rs'):
        return
    left = unpaired_const_changes(fd, SPAWNPROTO_CONST_RE)
    if left:
        out.append(('spawnproto', fd['path'], left[0].strip()))


# ---- rule 5: CLAUDE.md and design/decisions/*.md (outside frontmatter, past bookkeeping) ---------
#
# calef, 2026-10-11 (UTC), after approving #1953 by hand (two link targets in §147 (a timer a
# userspace service cannot hold) retargeted to a split note, no word of the ruling changed): "I
# need to review and approve decisions, but I don't think I need to approve minor bookkeeping edits
# like this one." So a decision file's body is compared base against head after normalizing away
# exactly three things, and only a difference that survives fires:
#
#   (a) a markdown link's target: `[text](target)` and `[text](target#anchor)` keep their text and
#       lose their target, so retargeting is quiet and adding or removing a link is not;
#   (b) a backticked repository path: `` `notes/x.md` `` and `` `notes/y/z.md#h` `` read the same.
#       A link whose whole text is such a path reads as the bare path too, which is #1953's shape
#       (a backticked path became a link to its successor);
#   (c) wrapping and whitespace: the body is compared as blocks (a paragraph, a heading, a list
#       item, a table row, a quote or fence line), each block's lines joined and its runs of
#       whitespace collapsed, so a reflow pairs off and a block boundary still counts.
#
# The comparison is over the whole file (the diff is one hunk holding it), not line by line. Any
# other difference fires: a word, a number, a quotation, a heading, a list item, a table cell. A
# CLAUDE.md edit fires as before; the ruling covered decision files only.
#
# A "repository path" is one whose first segment, past any `./` or `../`, is a top-level entry of
# this tree. That keeps `/dev/kmsg`, `America/Los_Angeles` and a URL out: §-files cite all three,
# and changing one is substance. A top-level entry added later and missing below makes its paths
# fire, which is the safe direction (BUGS).

REPO_TOP_LEVEL = frozenset((
    '.agents', '.cargo', '.claude', '.githooks', '.github', 'AGENTS.md', 'ARCHITECTS.md',
    'CLAUDE.md', 'CODE_OF_CONDUCT.md', 'CONTRIBUTING.md', 'Cargo.lock', 'Cargo.toml', 'README.md',
    'SECURITY.md', 'art', 'bench', 'briefs', 'c_library', 'components', 'crates',
    'cryptography_exerciser', 'cryptography_provider', 'design', 'entropy_backend', 'fixtures',
    'fuzz', 'helpers', 'jig', 'kernel', 'notes', 'packages', 'patches', 'pinned_tls_client',
    'pinned_tls_exerciser', 'redoxfs_server', 'script', 'std_exerciser', 'system_tests', 'targets',
    'tools', 'uefi_loader', 'vendor', 'xtask'))

PATH_TEXT_RE = re.compile(r'[\w.\-/]+(?:#[\w.\-]*)?')
BACKTICK_SPAN_RE = re.compile(r'`([^`]+)`')
# `[text](target "title")`, with one level of parentheses allowed inside the target.
MD_LINK_RE = re.compile(r'(!?)\[([^\[\]]*)\]\((?:[^()\s]|\([^()\s]*\))*(?:\s+"[^"]*")?\)')
BLOCK_START_RE = re.compile(r'\s*(?:#{1,6}\s|[-*+]\s|\d+[.)]\s|\||>|```|~~~)')
PATH_TOKEN = '\x00path\x00'
TARGET_TOKEN = '\x00target\x00'


def is_repo_path(text):
    text = text.strip()
    if not PATH_TEXT_RE.fullmatch(text):
        return False
    path = text.split('#', 1)[0]
    while path.startswith('../') or path.startswith('./'):
        path = path[path.index('/') + 1:]
    return path.split('/', 1)[0] in REPO_TOP_LEVEL


def normalize_bookkeeping(text):
    """One block's joined text with (a), (b) and (c) above normalized away."""
    text = BACKTICK_SPAN_RE.sub(
        lambda m: PATH_TOKEN if is_repo_path(m.group(1)) else m.group(0), text)
    text = MD_LINK_RE.sub(
        lambda m: (PATH_TOKEN if m.group(2).strip() == PATH_TOKEN or is_repo_path(m.group(2))
                   else f"{m.group(1)}[{m.group(2)}]({TARGET_TOKEN})"), text)
    return ' '.join(text.split())


def decision_body(lines):
    """A decision file's lines past its frontmatter (the first `---`-delimited block)."""
    body, dash_count, in_frontmatter = [], 0, False
    for line in lines:
        if line.strip() == '---':
            dash_count += 1
            in_frontmatter = (dash_count == 1)
            continue
        if not in_frontmatter:
            body.append(line)
    return body


def bookkeeping_blocks(lines):
    """Body lines -> normalized blocks: split at blank lines and at each block-starting line."""
    blocks, cur = [], []
    for line in lines:
        if not line.strip() or (cur and BLOCK_START_RE.match(line)):
            if cur:
                blocks.append(cur)
            cur = [line] if line.strip() else []
            continue
        cur.append(line)
    if cur:
        blocks.append(cur)
    return [normalize_bookkeeping(' '.join(b)) for b in blocks]


def rule_decisions(fd, out):
    path = fd['path']
    if path == 'CLAUDE.md':
        if changed_lines(fd):
            out.append(('decisions', path, '(CLAUDE.md edited)'))
        return
    if not (path.startswith('design/decisions/') and path.endswith('.md')):
        return
    if path == 'design/decisions/README.md':
        # Not a decision. It held the generated index table until calef retired it on 2026-10-12
        # (UTC), and since then it holds only how the directory works, so neither its prose nor that
        # table's removal is a ruling to review.
        return
    base_body = decision_body(base_lines(fd))
    head_body = decision_body(head_lines(fd))
    if bookkeeping_blocks(base_body) == bookkeeping_blocks(head_body):
        return  # frontmatter only, or bookkeeping (calef, 2026-10-11 UTC)
    dash_count = 0
    in_frontmatter = False
    for prefix, line in fd['lines']:
        if line.strip() == '---':
            dash_count += 1
            in_frontmatter = (dash_count == 1)
            continue
        if in_frontmatter:
            continue
        if prefix in ('+', '-') and line.strip():
            out.append(('decisions', path, line.strip()))
            return
    out.append(('decisions', path, '(a blank line moved a block boundary)'))


# ---- rule 6: syscall-error -----------------------------------------------------------------------
#
# #1745 (2026-10-06): `AddressSpace::LIST` began refusing a forged cursor with `BadPointer`, a new
# answer from an existing method, and no rule fired, because the constants were untouched and the
# change was seven lines of `kernel/src/syscall.rs`. A method's errors are part of what a program is
# written against (§10 (the capability-based microkernel process model), §16 (object revocation)),
# so a changed line there naming an `Error::` variant fires. A line removed and added back verbatim
# is a move, not a change, and pairs off.

SYSCALL_ERROR_RE = re.compile(r'\bError::[A-Z]\w*')


def rule_syscall_error(fd, out):
    path = fd['path']
    if not (path == 'kernel/src/syscall.rs' or path.startswith('kernel/src/syscall/')):
        return
    added, removed = [], []
    for prefix, line in changed_lines(fd):
        if is_comment_only(line) or not SYSCALL_ERROR_RE.search(line.split('//')[0]):
            continue
        (added if prefix == '+' else removed).append(line.strip())
    for line in added:
        if line in removed:
            removed.remove(line)
            continue
        out.append(('syscall-error', path, line))
        return
    if removed:
        out.append(('syscall-error', path, removed[0]))


RULES = (rule_abi_surface, rule_format_crate, rule_spawnproto, rule_decisions, rule_syscall_error)


# ---- the question each finding puts to calef ------------------------------------------------------
#
# calef, 2026-10-08 (UTC), on #1838's hold: "false positive for what? The label doesn't make sense
# on its own." A hold is released only by his ruling, so the bot's comment has to say what the
# ruling is about. The question lives here, beside the rule it belongs to, so a new rule cannot be
# added without one (`question` raises on an unknown rule, and the selftest asks every rule's).
# helpers/architect-label-comment.sh and helpers/merge-drain.sh's `surface-no-hold` both print it
# through `--ask`. The dependency question cites §46 (thin primitives or whole subsystems), the
# decision every new dependency is ruled under.

def _crate(path):
    parts = path.split('/')
    return parts[1] if len(parts) > 2 and parts[0] == 'crates' else path


def _section(path):
    m = re.match(r'design/decisions/0*(\d+)-(.*)\.md$', path)
    return f"§{m.group(1)} ({m.group(2).replace('-', ' ')})" if m else path


QUESTIONS = {
    'abi-surface': lambda p, d: (
        f"Does this add or change a syscall number, object type or method (`{d}`)?"),
    'dependency': lambda p, d: (
        f"Do you approve taking `{d}` as a dependency (§46), in `{p}`?"),
    'format-crate': lambda p, d: (
        f"Does this change the bytes of `{_crate(p)}`'s format (a version, magic value or field "
        "layout) that another program reads?"),
    'spawnproto': lambda p, d: (
        f"Does this change a spawn-protocol value another program sends or reads (`{d}`)?"),
    'decisions': lambda p, d: (
        "Do you approve this edit to CLAUDE.md?" if p == 'CLAUDE.md'
        else f"Do you approve this edit to {_section(p)}? It changes more than link targets, "
        "repository paths or wrapping, which no longer need you (your ruling, 2026-10-11 UTC)."),
    'syscall-error': lambda p, d: (
        f"Does this change which error a syscall method can return (`{d}`)?"),
}

UNEXAMINED_QUESTION = (
    "The rules could not read this diff. Does any of it need your ruling (the syscall surface, a "
    "dependency, a wire format or a decision)?")


def question(rule, path, detail):
    return QUESTIONS[rule](path, detail.replace('`', ''))


def ask(report):
    """Report lines (`rule: file: detail`, as `main` prints) -> one markdown list item each, the
    question first and the match after it. A line this file did not write passes through as is."""
    items = []
    for line in report.splitlines():
        if not line.strip():
            continue
        if line.startswith('unexamined:'):
            items.append(f"- **{UNEXAMINED_QUESTION}**\n  ({line.strip()})")
            continue
        rule, _, rest = line.partition(': ')
        path, _, detail = rest.partition(': ')
        if rule not in QUESTIONS:
            items.append(f"- {line.strip()}")
            continue
        items.append(f"- **{question(rule, path, detail)}**\n"
                     f"  ({rule}, `{path}`, matched `{detail.replace('`', '')}`)")
    return '\n'.join(items)


def evaluate(diff_text, known=None):
    """The diff text -> [(rule, path, detail), ...], one entry per file per rule that fired."""
    known = known if known is not None else KnownDeps()
    out = []
    for fd in parse_diff(diff_text):
        for rule in RULES:
            rule(fd, out)
        rule_dependency(fd, out, known)
    return out


# ---- the selftest ---------------------------------------------------------------------------------
#
# Every fixture is a literal `diff --git` block, the shape `git diff --unified=1000000` produces
# (each fixture is deliberately small, well under any real file's line count, so the "one hunk holds
# the whole file" property the docstring argues for holds trivially here too). Each names the rule
# it is meant to prove, or `None` for a negative case that must stay quiet.

# What the fixtures' pretend base tree already depends on (one manifest, one lock).
KNOWN_BASE = KnownDeps(
    manifests=['[dependencies]\nredox_syscall = { version = "0.9.0", default-features = false }\n'
               'getrandom = "0.4"\n'],
    locks=['[[package]]\nname = "libc"\nversion = "0.2.150"\n'])

FIXTURES = [
    ("abi: a new object type constant fires", """\
diff --git a/crates/abi/src/lib.rs b/crates/abi/src/lib.rs
--- a/crates/abi/src/lib.rs
+++ b/crates/abi/src/lib.rs
@@ -1,4 +1,5 @@
 pub mod objtype {
     pub const RENDEZVOUS: u64 = 1;
+    pub const NOTIFICATION: u64 = 8;
     pub const ADDRESS_SPACE: u64 = 2;
 }
""", 'abi-surface'),

    ("abi: a doc comment change alone stays quiet", """\
diff --git a/crates/abi/src/lib.rs b/crates/abi/src/lib.rs
--- a/crates/abi/src/lib.rs
+++ b/crates/abi/src/lib.rs
@@ -1,3 +1,3 @@
-/// old wording
+/// new wording, no number moved
 pub const SYS_EXIT: u64 = 0;
""", None),

    ("abi: outside crates/abi, the same const shape stays quiet", """\
diff --git a/crates/other/src/lib.rs b/crates/other/src/lib.rs
--- a/crates/other/src/lib.rs
+++ b/crates/other/src/lib.rs
@@ -1,1 +1,2 @@
 pub const EXISTING: u64 = 1;
+pub const NEW_THING: u64 = 2;
""", None),

    ("abi: a rename with the same type and value stays quiet", """\
diff --git a/crates/abi/src/lib.rs b/crates/abi/src/lib.rs
--- a/crates/abi/src/lib.rs
+++ b/crates/abi/src/lib.rs
@@ -1,3 +1,3 @@
 pub mod method {
-    pub const RECV: u64 = 1;
+    pub const RECEIVE: u64 = 1;
 }
""", None),

    ("abi: a renamed constant whose value also changed fires", """\
diff --git a/crates/abi/src/lib.rs b/crates/abi/src/lib.rs
--- a/crates/abi/src/lib.rs
+++ b/crates/abi/src/lib.rs
@@ -1,3 +1,3 @@
 pub mod method {
-    pub const RECV: u64 = 1;
+    pub const RECEIVE: u64 = 2;
 }
""", 'abi-surface'),

    ("abi: a changed value under the same name fires", """\
diff --git a/crates/abi/src/lib.rs b/crates/abi/src/lib.rs
--- a/crates/abi/src/lib.rs
+++ b/crates/abi/src/lib.rs
@@ -1,3 +1,3 @@
 pub mod method {
-    pub const SEND: u64 = 0;
+    pub const SEND: u64 = 9;
 }
""", 'abi-surface'),

    ("abi: a rename paired with an extra added constant still fires for the extra", """\
diff --git a/crates/abi/src/lib.rs b/crates/abi/src/lib.rs
--- a/crates/abi/src/lib.rs
+++ b/crates/abi/src/lib.rs
@@ -1,3 +1,4 @@
 pub mod method {
-    pub const RECV: u64 = 1;
+    pub const RECEIVE: u64 = 1;
+    pub const POLL: u64 = 1;
 }
""", 'abi-surface'),

    ("abi: a removed constant with no added partner fires", """\
diff --git a/crates/abi/src/lib.rs b/crates/abi/src/lib.rs
--- a/crates/abi/src/lib.rs
+++ b/crates/abi/src/lib.rs
@@ -1,3 +1,2 @@
 pub mod method {
-    pub const RECV: u64 = 1;
 }
""", 'abi-surface'),

    ("spawnproto: a rename with the same value stays quiet", """\
diff --git a/crates/grant_plan/src/spawnproto.rs b/crates/grant_plan/src/spawnproto.rs
--- a/crates/grant_plan/src/spawnproto.rs
+++ b/crates/grant_plan/src/spawnproto.rs
@@ -1,1 +1,1 @@
-pub const RUN_UNVOUCHED_SLOT: u64 = 22;
+pub const RUN_UNVOUCHED_ENDPOINT_SLOT: u64 = 22;
""", None),

    ("spawnproto: a new wire value with no removed partner fires", """\
diff --git a/crates/grant_plan/src/spawnproto.rs b/crates/grant_plan/src/spawnproto.rs
--- a/crates/grant_plan/src/spawnproto.rs
+++ b/crates/grant_plan/src/spawnproto.rs
@@ -1,1 +1,2 @@
 pub const SPAWN_DISPLAY: u64 = u64::MAX - 3;
+pub const SPAWN_NO_DISPLAY: u64 = u64::MAX - 4;
""", 'spawnproto'),

    ("dependency: a brand-new external crate fires", """\
diff --git a/entropy_backend/Cargo.toml b/entropy_backend/Cargo.toml
--- a/entropy_backend/Cargo.toml
+++ b/entropy_backend/Cargo.toml
@@ -1,3 +1,4 @@
 [dependencies]
 getrandom = "0.4"
+rustls-rustcrypto = "0.1"
""", 'dependency'),

    ("dependency: an in-tree path dependency stays quiet", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+calendar = { version = "0.1.0", path = "../crates/calendar" }
""", None),

    ("dependency: an existing dep moved to a version nothing in the base uses fires", """\
diff --git a/entropy_backend/Cargo.toml b/entropy_backend/Cargo.toml
--- a/entropy_backend/Cargo.toml
+++ b/entropy_backend/Cargo.toml
@@ -1,2 +1,2 @@
 [dependencies]
-getrandom = "0.4"
+getrandom = "0.5"
""", 'dependency', KNOWN_BASE),

    ("dependency: an existing dep at a version the base already uses stays quiet", """\
diff --git a/fuzz/Cargo.toml b/fuzz/Cargo.toml
--- a/fuzz/Cargo.toml
+++ b/fuzz/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+redox_syscall = { version = "0.9.0", default-features = false }
""", None, KNOWN_BASE),

    ("dependency: a crate the base lock holds at a compatible version stays quiet", """\
diff --git a/fuzz/Cargo.toml b/fuzz/Cargo.toml
--- a/fuzz/Cargo.toml
+++ b/fuzz/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+libc = "0.2"
""", None, KNOWN_BASE),

    ("dependency: a crate the base lock holds only at an incompatible version fires", """\
diff --git a/fuzz/Cargo.toml b/fuzz/Cargo.toml
--- a/fuzz/Cargo.toml
+++ b/fuzz/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+libc = "0.3"
""", 'dependency', KNOWN_BASE),

    ("dependency: a crate absent from every base manifest and lock fires", """\
diff --git a/fuzz/Cargo.toml b/fuzz/Cargo.toml
--- a/fuzz/Cargo.toml
+++ b/fuzz/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+brand_new = "1.0"
""", 'dependency', KNOWN_BASE),

    ("dependency: a features-only change to a known dep stays quiet", """\
diff --git a/fuzz/Cargo.toml b/fuzz/Cargo.toml
--- a/fuzz/Cargo.toml
+++ b/fuzz/Cargo.toml
@@ -1,2 +1,2 @@
 [dependencies]
-redox_syscall = { version = "0.9.0" }
+redox_syscall = { version = "0.9.0", features = ["std"] }
""", None, KNOWN_BASE),

    ("dependency: a target-specific dependency section is still read", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [target.'cfg(target_arch = "aarch64")'.dependencies]
+aarch64-cpu = "9.0"
""", 'dependency'),

    ("format-crate: a MAGIC constant change in a classified crate fires", """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,2 +1,2 @@
 //! magic   "CRKR0002"   (8 bytes)
-pub const MAGIC: [u8; 8] = *b"CRKR0002";
+pub const MAGIC: [u8; 8] = *b"CRKR0003";
""", 'format-crate'),

    ("format-crate: a layout doc-comment edit (a range) fires even with the const untouched", """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,3 +1,4 @@
 //! # The layout
-//! blocks 0..DIR_BLOCKS   the superblock and directory
+//! blocks 0..DIR_BLOCKS+1 the superblock and directory
 pub const MAGIC: [u8; 8] = *b"CRKR0002";
""", 'format-crate'),

    ("format-crate: #1402's shape, a layout table row changed in a comment, fires", """\
diff --git a/crates/capability_manifest_note/src/lib.rs b/crates/capability_manifest_note/src/lib.rs
--- a/crates/capability_manifest_note/src/lib.rs
+++ b/crates/capability_manifest_note/src/lib.rs
@@ -1,3 +1,3 @@
 //! | version | field | meaning |
-//! | 4 | 1 | `arg` | 0 forbidden, 1 required, 2 words |
+//! | 4 | 1 | `arg` | 0 forbidden, 1 required; hears words: 2 read-only, 3 read-write |
 pub fn parse() {}
""", 'format-crate'),

    ("format-crate: a doc-comment prose change in a classified crate stays quiet", """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,3 +1,3 @@
-//! Completion needs the namespace.
+//! Completion needs the command namespace.
 pub const MAGIC: [u8; 8] = *b"CRKR0002";
""", None),

    ("format-crate: a diagram edited inside a comment stays quiet", """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,3 +1,3 @@
-//!   chatty ──CALL──► SVC ◄──RECV_CAP── editor
+//!   chatty ──CALL──► SVC ◄──RECEIVE_CAP── editor
 /// the magic
 pub const MAGIC: [u8; 8] = *b"CRKR0002";
""", None),

    ("format-crate: a code line changed in a classified crate fires", """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,3 +1,3 @@
 //! unchanged prose
-pub const VERSION: u32 = 1;
+pub const VERSION: u32 = 2;
""", 'format-crate'),

    ("format-crate: an ordinary crate with no VERSION/MAGIC/version-table stays quiet", """\
diff --git a/crates/glob/src/lib.rs b/crates/glob/src/lib.rs
--- a/crates/glob/src/lib.rs
+++ b/crates/glob/src/lib.rs
@@ -1,2 +1,2 @@
-//! matches a glob pattern
+//! matches a shell glob pattern
 pub fn matches(pattern: &str, name: &str) -> bool { true }
""", None),

    ("format-crate: classified by a doc version table, a code change in it fires", """\
diff --git a/crates/capability_manifest_note/src/lib.rs b/crates/capability_manifest_note/src/lib.rs
--- a/crates/capability_manifest_note/src/lib.rs
+++ b/crates/capability_manifest_note/src/lib.rs
@@ -1,3 +1,3 @@
 //! | version | field |
-//! | 1       | a     |
-pub fn parse() {}
+pub const MAGIC: u32 = 7;
""", 'format-crate'),

    ("format-crate: #1838's shape, a `//` line naming an offset beside a test assert, stays quiet",
     """\
diff --git a/crates/network_time_protocol/src/lib.rs b/crates/network_time_protocol/src/lib.rs
--- a/crates/network_time_protocol/src/lib.rs
+++ b/crates/network_time_protocol/src/lib.rs
@@ -1,9 +1,11 @@
 pub const VERSION: u8 = 4;
 #[cfg(test)]
 mod tests {
     #[test]
     fn the_pivot_holds() {
+        // 2^32 seconds less the 1900-to-1970 offset, plus the half-era the pivot grants, is
+        assert_eq!(super::pivot(), 1);
     }
 }
""", None),

    ("format-crate: a `///` layout line inside a test module stays quiet (#1569's shape)", """\
diff --git a/crates/package_archive/src/lib.rs b/crates/package_archive/src/lib.rs
--- a/crates/package_archive/src/lib.rs
+++ b/crates/package_archive/src/lib.rs
@@ -1,4 +1,5 @@
 pub const MAGIC: [u8; 8] = *b"NIFEPKG1";
 #[cfg(test)]
 mod tests {
+    /// a party writing this format from the crate docs lands on exactly these offsets.
 }
""", None),

    ("format-crate: a table row after a closed test module still fires", """\
diff --git a/crates/capability_manifest_note/src/lib.rs b/crates/capability_manifest_note/src/lib.rs
--- a/crates/capability_manifest_note/src/lib.rs
+++ b/crates/capability_manifest_note/src/lib.rs
@@ -1,8 +1,8 @@
 pub const VERSION: u32 = 1;
 #[cfg(test)]
 mod tests {
     #[test]
     fn f() { let s = "}"; }
 }
-/// | 53 | 3 | zero | |
+/// | 53 | 1 | `machine` | 0 or 1 |
 pub fn parse() {}
""", 'format-crate'),

    ("format-crate: a plain `//` line naming an offset in library code stays quiet", """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,2 +1,3 @@
 pub const MAGIC: [u8; 8] = *b"CRKR0002";
+// the reader takes its offsets from the same header the builder wrote
 pub fn read() {}
""", None),

    ("format-crate: a doc layout line whose number moved fires", """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,3 +1,3 @@
-/// The name sits at offset 12.
+/// The name sits at offset 16.
 pub const MAGIC: [u8; 8] = *b"CRKR0002";
""", 'format-crate'),

    ("format-crate: an identifier renamed in a layout sentence stays quiet (#1255, #860)", """\
diff --git a/crates/clock_contract/src/lib.rs b/crates/clock_contract/src/lib.rs
--- a/crates/clock_contract/src/lib.rs
+++ b/crates/clock_contract/src/lib.rs
@@ -1,3 +1,3 @@
-/// Where the opcode sits: bits 63:56, the same position `filesystem_proto` uses.
+/// Where the opcode sits: bits 63:56, the same position `filesystem_contract` uses.
 pub const VERSION: u32 = 1;
""", None),

    ("format-crate: a VERSION constant inside a test module still fires", """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,3 +1,4 @@
 pub const MAGIC: [u8; 8] = *b"CRKR0002";
 #[cfg(test)]
 mod tests {
+    pub const OLD_VERSION: u32 = 1;
 }
""", 'format-crate'),

    ("spawnproto: a changed slot constant fires", """\
diff --git a/crates/grant_plan/src/spawnproto.rs b/crates/grant_plan/src/spawnproto.rs
--- a/crates/grant_plan/src/spawnproto.rs
+++ b/crates/grant_plan/src/spawnproto.rs
@@ -1,1 +1,1 @@
-pub const RUN_UNVOUCHED_SLOT: u64 = 22;
+pub const RUN_UNVOUCHED_SLOT: u64 = 23;
""", 'spawnproto'),

    ("spawnproto: prose-only doc change in the same file stays quiet", """\
diff --git a/crates/grant_plan/src/spawnproto.rs b/crates/grant_plan/src/spawnproto.rs
--- a/crates/grant_plan/src/spawnproto.rs
+++ b/crates/grant_plan/src/spawnproto.rs
@@ -1,1 +1,1 @@
-//! old description
+//! clearer description, no constant moved
""", None),

    ("spawnproto: a const change in a same-named file elsewhere still fires", """\
diff --git a/crates/other/src/spawnproto.rs b/crates/other/src/spawnproto.rs
--- a/crates/other/src/spawnproto.rs
+++ b/crates/other/src/spawnproto.rs
@@ -1,1 +1,1 @@
-pub const SLOT: u64 = 1;
+pub const SLOT: u64 = 2;
""", 'spawnproto'),

    ("decisions: any CLAUDE.md edit fires", """\
diff --git a/CLAUDE.md b/CLAUDE.md
--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,1 +1,1 @@
-old sentence
+new sentence
""", 'decisions'),

    ("decisions: a substantive body edit to a decision file fires", """\
diff --git a/design/decisions/0088-needs-architect-as-a-check.md b/design/decisions/0088-needs-architect-as-a-check.md
--- a/design/decisions/0088-needs-architect-as-a-check.md
+++ b/design/decisions/0088-needs-architect-as-a-check.md
@@ -1,6 +1,6 @@
 ---
 status: DECIDED
 ---
-**What.** old text.
+**What.** new text, a different rule.
""", 'decisions'),

    ("decisions: a frontmatter-only status flip stays quiet", """\
diff --git a/design/decisions/0088-needs-architect-as-a-check.md b/design/decisions/0088-needs-architect-as-a-check.md
--- a/design/decisions/0088-needs-architect-as-a-check.md
+++ b/design/decisions/0088-needs-architect-as-a-check.md
@@ -1,4 +1,4 @@
 ---
-status: PROPOSED
+status: DECIDED
 ---
 **What.** unchanged text.
""", None),

    ("decisions: #1953's §147 edit, two link targets moved to a split note, stays quiet", """\
diff --git a/design/decisions/0147-a-timer-a-userspace-service-cannot-hold.md b/design/decisions/0147-a-timer-a-userspace-service-cannot-hold.md
--- a/design/decisions/0147-a-timer-a-userspace-service-cannot-hold.md
+++ b/design/decisions/0147-a-timer-a-userspace-service-cannot-hold.md
@@ -1,14 +1,15 @@
 ---
 status: AMENDED
 ---
 ## Why the premise failed
 
-The full working, with the specification citations, is `notes/timer-capability.md`, and the corrected
+The full working, with the specification citations, is
+[`notes/timer-capability/specifications.md`](../../notes/timer-capability/specifications.md), and the corrected
 per-architecture table is in `design/roadmap/0263-can-a-timer-be-a-capability.md`. The short form:
 
 which says *"There is no precedent
 in this tree for a per-thread system-register bit maintained across a context switch."* Milestones 229
 and 237 built one, so that sentence is stale inside its own decision. The correction is recorded in
-`notes/timer-capability.md` and in 263's block; amending a decision section is not a lane's to do.
+[`notes/timer-capability/specifications.md`](../../notes/timer-capability/specifications.md#aarch64-the-blocks-negative-is-refuted) and in 263's block; amending a decision section is not a lane's to do.
""", None),

    ("decisions: a reflowed paragraph with one link retargeted stays quiet", """\
diff --git a/design/decisions/0088-needs-architect-as-a-check.md b/design/decisions/0088-needs-architect-as-a-check.md
--- a/design/decisions/0088-needs-architect-as-a-check.md
+++ b/design/decisions/0088-needs-architect-as-a-check.md
@@ -1,8 +1,7 @@
 ---
 status: DECIDED
 ---
-The weakness is in [the merge queue's BUGS](../../notes/merge-queue.md#bugs), which
-has said so since the script was
-written, in the tree's own words.
+The weakness is in [the merge queue's BUGS](../../notes/merge-queue/bugs.md#the-label-is-by-hand),
+which has said so since the script was written, in the tree's own words.
 
 - A list item that did not move.
""", None),

    ("decisions: a retargeted link whose text also lost a word fires", """\
diff --git a/design/decisions/0088-needs-architect-as-a-check.md b/design/decisions/0088-needs-architect-as-a-check.md
--- a/design/decisions/0088-needs-architect-as-a-check.md
+++ b/design/decisions/0088-needs-architect-as-a-check.md
@@ -1,4 +1,4 @@
 ---
 status: DECIDED
 ---
-The weakness is in [the merge queue's BUGS](../../notes/merge-queue.md#bugs), as written.
+The weakness is in [the merge queue's](../../notes/merge-queue/bugs.md), as written.
""", 'decisions'),

    ("decisions: a retargeted link with one word of its sentence changed fires", """\
diff --git a/design/decisions/0088-needs-architect-as-a-check.md b/design/decisions/0088-needs-architect-as-a-check.md
--- a/design/decisions/0088-needs-architect-as-a-check.md
+++ b/design/decisions/0088-needs-architect-as-a-check.md
@@ -1,4 +1,4 @@
 ---
 status: DECIDED
 ---
-The weakness is in [the merge queue's BUGS](../../notes/merge-queue.md#bugs), as written.
+The weakness is in [the merge queue's BUGS](../../notes/merge-queue/bugs.md), as amended.
""", 'decisions'),

    ("decisions: a link added around words already there fires", """\
diff --git a/design/decisions/0088-needs-architect-as-a-check.md b/design/decisions/0088-needs-architect-as-a-check.md
--- a/design/decisions/0088-needs-architect-as-a-check.md
+++ b/design/decisions/0088-needs-architect-as-a-check.md
@@ -1,4 +1,4 @@
 ---
 status: DECIDED
 ---
-The weakness is in the merge queue's BUGS, as written.
+The weakness is in [the merge queue's BUGS](../../notes/merge-queue.md#bugs), as written.
""", 'decisions'),

    ("decisions: a link removed, its words kept, fires", """\
diff --git a/design/decisions/0088-needs-architect-as-a-check.md b/design/decisions/0088-needs-architect-as-a-check.md
--- a/design/decisions/0088-needs-architect-as-a-check.md
+++ b/design/decisions/0088-needs-architect-as-a-check.md
@@ -1,4 +1,4 @@
 ---
 status: DECIDED
 ---
-The weakness is in [the merge queue's BUGS](../../notes/merge-queue.md#bugs), as written.
+The weakness is in the merge queue's BUGS, as written.
""", 'decisions'),

    ("decisions: a changed number fires, even beside a retargeted path", """\
diff --git a/design/decisions/0088-needs-architect-as-a-check.md b/design/decisions/0088-needs-architect-as-a-check.md
--- a/design/decisions/0088-needs-architect-as-a-check.md
+++ b/design/decisions/0088-needs-architect-as-a-check.md
@@ -1,4 +1,4 @@
 ---
 status: DECIDED
 ---
-The hold expires after 30 minutes (`notes/merge-queue.md`).
+The hold expires after 60 minutes (`notes/merge-queue/bugs.md`).
""", 'decisions'),

    ("decisions: a backticked path outside the tree (`/dev/kmsg`) is substance and fires", """\
diff --git a/design/decisions/0088-needs-architect-as-a-check.md b/design/decisions/0088-needs-architect-as-a-check.md
--- a/design/decisions/0088-needs-architect-as-a-check.md
+++ b/design/decisions/0088-needs-architect-as-a-check.md
@@ -1,4 +1,4 @@
 ---
 status: DECIDED
 ---
-The log is read from `/dev/kmsg`.
+The log is read from `/dev/kmesg`.
""", 'decisions'),

    ("decisions: a CLAUDE.md link retarget still fires (the ruling covered decisions only)", """\
diff --git a/CLAUDE.md b/CLAUDE.md
--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,1 +1,1 @@
-See [the note](notes/a.md).
+See [the note](notes/a/b.md).
""", 'decisions'),

    ("decisions: design/decisions/README.md is excluded even on a substantive edit", """\
diff --git a/design/decisions/README.md b/design/decisions/README.md
--- a/design/decisions/README.md
+++ b/design/decisions/README.md
@@ -1,1 +1,1 @@
-88. old title
+88. a completely different title
""", None),

    ("decisions: removing the retired index table from design/decisions/README.md stays quiet", """\
diff --git a/design/decisions/README.md b/design/decisions/README.md
--- a/design/decisions/README.md
+++ b/design/decisions/README.md
@@ -1,6 +1,1 @@
-## The decisions
-
-| # | Status | Decision |
-|---|---|---|
-| 1 | DECIDED | [Target architecture: aarch64](0001-target-architecture.md) |
 ## Reading
""", None),

    ("decisions: an unrelated design/ file stays quiet", """\
diff --git a/design/naming.md b/design/naming.md
--- a/design/naming.md
+++ b/design/naming.md
@@ -1,1 +1,1 @@
-old rule
+new rule
""", None),

    ("syscall-error: a method's new refusal fires (#1745's shape)", """\
diff --git a/kernel/src/syscall.rs b/kernel/src/syscall.rs
--- a/kernel/src/syscall.rs
+++ b/kernel/src/syscall.rs
@@ -1,3 +1,6 @@
 fn address_space_list(frame: &mut TrapFrame, name: u64, cursor: u64) -> Result<i64, Error> {
+    if cursor != 0 && !crate::revoke::cursor_is_ours(root, cursor) {
+        return Err(Error::BadPointer);
+    }
     Ok(0)
""", 'syscall-error'),

    ("syscall-error: an error a method stops returning fires too", """\
diff --git a/kernel/src/syscall.rs b/kernel/src/syscall.rs
--- a/kernel/src/syscall.rs
+++ b/kernel/src/syscall.rs
@@ -1,3 +1,2 @@
 fn f() -> Result<i64, Error> {
-    if x { return Err(Error::Busy); }
     Ok(0)
""", 'syscall-error'),

    ("syscall-error: a moved line, a comment, and a line with no error stay quiet", """\
diff --git a/kernel/src/syscall.rs b/kernel/src/syscall.rs
--- a/kernel/src/syscall.rs
+++ b/kernel/src/syscall.rs
@@ -1,5 +1,6 @@
-    let a = lookup(cap).ok_or(Error::InvalidCapability)?;
     let b = 2;
+    let a = lookup(cap).ok_or(Error::InvalidCapability)?;
+    // a forged cursor is refused with Error::BadPointer
+    let c = 3;
     Ok(0)
""", None),

    ("syscall-error: Error:: in a kernel file outside the dispatcher stays quiet", """\
diff --git a/kernel/src/revoke.rs b/kernel/src/revoke.rs
--- a/kernel/src/revoke.rs
+++ b/kernel/src/revoke.rs
@@ -1,1 +1,2 @@
 fn g() {}
+fn h() -> Result<(), Error> { Err(Error::Busy) }
""", None),
]


# (report line, a phrase its question must contain): one per rule, plus the unexamined case.
ASK_FIXTURES = [
    ("abi-surface: crates/abi/src/lib.rs: pub const NOTIFICATION: u64 = 8;", "syscall number"),
    ("dependency: fuzz/Cargo.toml: brand_new 1.0", "taking `brand_new 1.0` as a dependency"),
    ("format-crate: crates/network_time_protocol/src/lib.rs: // the offset",
     "the bytes of `network_time_protocol`'s format"),
    ("spawnproto: crates/grant_plan/src/spawnproto.rs: pub const SLOT: u64 = 2;", "spawn-protocol"),
    ("decisions: design/decisions/0088-needs-architect-as-a-check.md: **What.**",
     "this edit to §88 (needs architect as a check)"),
    ("decisions: CLAUDE.md: (CLAUDE.md edited)", "this edit to CLAUDE.md"),
    ("syscall-error: kernel/src/syscall.rs: return Err(Error::BadPointer);", "which error"),
    ("unexamined: no base to diff against, so no rule read this pull request", "could not read"),
]


def selftest():
    bad = []
    asked = {line.split(':')[0] for line, _w in ASK_FIXTURES}
    for rule in [r.__name__[len('rule_'):].replace('_', '-') for r in RULES] + ['dependency']:
        if rule not in asked or rule not in QUESTIONS:
            bad.append(f"rule {rule} has no question, or no ask fixture")
    for line, want in ASK_FIXTURES:
        got = ask(line)
        ok = want in got
        print(f"  {'ok  ' if ok else 'FAIL'}  ask: {line.split(':')[0]} states its question")
        if not ok:
            bad.append(f"ask {line!r}: wanted {want!r} in {got!r}")
    for name, diff_text, want_rule, *rest in FIXTURES:
        got = evaluate(diff_text, rest[0] if rest else None)
        got_rules = {r for r, _p, _d in got}
        ok = (want_rule in got_rules) if want_rule else (not got_rules)
        print(f"  {'ok  ' if ok else 'FAIL'}  {name}")
        if not ok:
            bad.append(f"{name}: wanted {want_rule!r}, got {sorted(got_rules)!r}")
    print()
    if bad:
        for b in bad:
            print(f"architect-label-rules: selftest: {b}", file=sys.stderr)
        return 1
    print(f"architect-label-rules: {len(FIXTURES)} fixtures, every rule fires and every "
          f"near-miss stays quiet; {len(ASK_FIXTURES)} findings each state their question")
    return 0


USAGE = (
    "usage: python3 helpers/architect-label-rules.py [--selftest]\n"
    "  --selftest   run the fixtures above; no stdin, no git, no repository\n"
    "  --base-rev REV   with a diff on stdin: rule 2 skips a dependency REV already has\n"
    "  --ask        read this tool's own report on stdin, print each finding's question for calef\n"
    "  (no args)    read a unified diff on stdin, print each rule that fired, "
    "exit 0 if any did"
)


def main(argv):
    if argv and argv[0] == '--selftest':
        return selftest()
    if argv == ['--ask']:
        print(ask(sys.stdin.read()))
        return 0
    known = None
    if len(argv) == 2 and argv[0] == '--base-rev':
        known = known_deps_from_git(argv[1])
    elif argv:
        print(USAGE, file=sys.stderr)
        return 2
    diff_text = sys.stdin.read()
    matches = evaluate(diff_text, known)
    for rule, path, detail in matches:
        print(f"{rule}: {path}: {detail}")
    return 0 if matches else 1


if __name__ == '__main__':
    try:
        sys.exit(main(sys.argv[1:]))
    except Exception:
        import traceback
        traceback.print_exc()
        sys.exit(3)
