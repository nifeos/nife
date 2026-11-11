#!/usr/bin/env python3
"""The diff-reading half of `needs-architect` (§88): a predicate over a unified diff.

§88 (`needs-architect` as a required check, rather than as a script's restraint) built the
enforcement half -- `architect-hold.yml` fails while the label is present -- and named plainly what
it did not build: *"a check that reads the diff and demands the label when the syscall surface or
the dependency graph moved... the second is the real answer."* `.github/workflows/coe-architect-
label.yml` built that answer for one shape, a correction of error. This is the general one, wired
by `.github/workflows/architect-label.yml`.

This module is a predicate: it reads a unified diff (as `git diff --unified=1000000 base head`
produces, so every file's diff is one hunk holding the WHOLE file, changed lines marked) and reports
which of six rules fired, and where. With a diff on stdin it never talks to GitHub and runs `git`
only for `--base-rev`; that split is what makes `--selftest` below run against literal fixture
strings, no repository and no subprocess required, cheap enough for `script/lint-fleet` to run on
every invocation. `--against MAIN HEAD` makes the diffs itself, one from each merge base, and is
what the workflow and the merge drain call.

    python3 helpers/architect-label-rules.py --selftest        # fixtures; script/lint-fleet runs this
    python3 helpers/architect-label-rules.py --against origin/main HEAD [--ruled N:SHA]...
    git diff --unified=1000000 BASE HEAD | python3 helpers/architect-label-rules.py
        # prints one "rule: file: line" per match on stdout, exits 0 if anything fired, 1 if
        # nothing did, 2 on a usage error, 3 if it crashed, 4 when --against finds no merge base.
        # A crash is 3 rather than Python's own 1 so that it can never read as
        # "nothing fired" (milestone 641 (a mechanism that reports clean says over how many units,
        # and zero is loud), provisional): the workflow routes 2, 3 and 4 to a person.

Two things hold for every rule. A file whose code is the same at base and head, comments dropped
and each identifier the diff renames at its declaration read under its new name (`code_unchanged`,
`diff_renames`), moved no number, error or byte, so rules 1, 3, 4 and 6 are quiet on it (rule 3
on its whole crate). And `--against` keeps a finding only when the diff from every merge base
yields it, and drops one in a file that an open pull request carrying `architect-ruled` (and not
`held-by-lane`) holds byte for byte at the same base (`drop_ruled`).

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
   what "external" cannot see: a multi-line inline table. Since calef's ruling of 2026-11-09
   (UTC), a dependency on one of our own crates is approved in advance and does not fire when it is
   a git dependency whose URL is `https://github.com/nifeos/<repository>` (a trailing `.git` is
   allowed) with `rev` a full 40-hexadecimal-digit commit and no `branch` or `tag`. A branch, a tag,
   a short rev, no rev, another organization, or a look-alike such as `nifeos-fork` still fires.
   Third-party dependencies are unchanged (§46). Known gaps, both deliberate: (a) a crates.io
   dependency whose name is a first-party crate still fires, because no reliable list exists (the
   organization's repositories include non-crates, `packages/homes.toml` names homes rather than
   crates, and nothing is published to crates.io yet); (b) the rule cannot tell that the commit is
   on that repository's main, which the ruling requires. Reviewers check it until a lint does.
3. **format-crate**: a version constant, magic value, or documented on-disk layout moved in a crate
   that is classified, from its own head content, as a format crate: it declares a `pub const`
   ending in `VERSION` or `MAGIC`, or its module doc carries a markdown table with a `version`
   column (the two tells the brief names). A crate whose code did not move does not fire at all.
   Otherwise firing is on a changed `VERSION`/`MAGIC` constant line, or a changed doc or block
   comment line (`//!`, `///`, `/* */`) outside test code that states a layout (a markdown table
   row, `offset`, `width`, `bit N`, an `N..M` range). A plain `//` comment, a line under `#[test]`
   or `#[cfg(test)]`, and a removed/added pair (in one file or across the diff) with the same
   constant or the same numbers and layout words do not fire: an identifier renamed in the
   sentence, a constant moved to another crate, `pub(crate)` widened to `pub`.
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
    spawnproto       3      3  #1377 #1419 #1421: IMAGE_MAXIMUM_PAGES, not in those PRs' diffs (not reproduced)
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

# The third narrowing, measured 2026-11-11 UTC

Every pull request created from 2026-10-14 to 2026-11-11 (1,264) was read for `needs-architect`:
221 received it, 145 from this file (the labeler's or the drain's finding comment), the rest by
hand. Each of the 145 was rerun at its final head, and the drain's 34 firings at the head and main
it saw. Real means the rules' own test: a wire or persisted format another program reads, the
syscall surface, a dependency new to the graph, a decision, or a published number or name.

    rule           PRs  real  false  the false ones, by class
    format-crate    45    18     27  comment-only edit, no code moved (13); identifier rename (8);
                                     constant moved to another crate (3); verbatim proof-module
                                     move (1); drain base race (2)
    dependency      24    20      4  a stacked branch carrying a ruled dependency (2); an own-crate
                                     pin before the 2026-11-09 approval, now exempt (2, counted real)
    decisions       70    61      9  criss-cross merge base (6); a split or stacked branch (3)
    syscall-error    7     2      5  rename (2); stacked parts inheriting #2809 (2); one of #2965's
    spawnproto       5     4      1  a value naming a renamed constant (#2792)
    abi-surface      6     6      0

Now quiet (22 pull requests, every one ruled a false positive): #2294 #2315 #2316 #2341 #2612
#2622 #2623 #2631 #2632 #2633 #2634 #2635 #2636 #2708 #2736 #2779 #2791 #2792 #2793 #3042 #3094
#3282 at their heads, and #2838 #3167 #3171 #3173 #3224 #3416 at the drain's (two merge bases each).
Every finding ruled real still fires, among them #3425 and #3427 (in-image formats, ruled
provisional), #2416 and #2512 (pages read across builds), #2836 (a value that gained a term) and
#1402, #1385, #1360, #1727 and #1066 from the earlier narrowings. The in-image class keeps firing
by design: #2416's handoff crosses a live swap and #2512's page is read by published binaries, so
"one image, one commit" is not visible in a diff; the question names that answer instead.

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
- **Code that moved without moving a byte still fires.** A proof module moved verbatim to a new
  file (#2181), a refactor (#2540, #2897), assertions added (#2867), doc attributes added (#2762),
  and a crate split (#2749, #2997) change code tokens, so the crate is not quiet. Measured at 8 of
  the 27 format-crate false positives above.
- **One identifier renamed two ways is left out of the rename map.** #2809 renamed the module
  `cap` to `capability_object` and the variable `cap` to `capability`; with no scopes the map
  drops `cap`, so that sweep still fires (the restrictive direction).
- **A ruled pull request covers a whole file only.** `drop_ruled` compares blobs, so a branch that
  carries a ruled hunk and edits the same file elsewhere fires on that file again.
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
            cur = {'path': m.group(2), 'old_path': m.group(1), 'lines': []}
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


# ---- code tokens, and the renames a diff performs -------------------------------------------------
#
# A format's bytes, a syscall's errors and a protocol's values are made by code, never by comments.
# So the rules below ask first whether a file's code moved at all: its tokens with every comment
# dropped, base against head. A file whose code is identical, or identical once each identifier the
# diff renames at its declaration is read under its new name, moved no byte, and those rules stay
# quiet on it. Measured 2026-11-11 (UTC): see "the third narrowing" in the module docstring.

TOKEN_RE = re.compile(
    r'b?r(#*)"(?:.|\n)*?"\1'        # raw string or raw byte string
    r'|b?"(?:\\[\s\S]|[^"\\])*"'     # string or byte string, a `\` line continuation included
    r"|b?'(?:\\(?:u\{[0-9A-Fa-f_]+\}|x[0-9A-Fa-f]{2}|.)|[^'\\\n])'"  # char or byte char
    r"|'[A-Za-z_]\w*"                # lifetime
    r'|[A-Za-z_]\w*'                 # identifier or keyword
    r'|0[xob][0-9A-Fa-f_]+\w*|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?\w*'  # number
    r'|::|->|=>|==|!=|<=|>=|&&|\|\||<<|>>|\.\.=|\.\.|\S')
IDENT_RE = re.compile(r'[A-Za-z_]\w*\Z')
KEYWORDS = frozenset((
    'as', 'async', 'await', 'break', 'const', 'continue', 'crate', 'dyn', 'else', 'enum', 'extern',
    'false', 'fn', 'for', 'if', 'impl', 'in', 'let', 'loop', 'match', 'mod', 'move', 'mut', 'pub',
    'ref', 'return', 'self', 'Self', 'static', 'struct', 'super', 'trait', 'true', 'type', 'union',
    'unsafe', 'use', 'where', 'while'))


def strip_comments(text):
    """Rust source text with `//` and `/* */` comments removed, strings and chars kept.

    Walks token by token (`TOKEN_RE`), so a quote beside an identifier or a
    comment marker inside a string never starts what it does not start."""
    out, i, n = [], 0, len(text)
    while i < n:
        if text.startswith('//', i):
            j = text.find('\n', i)
            i = n if j < 0 else j
            continue
        if text.startswith('/*', i):
            depth, i = 1, i + 2
            while i < n and depth:
                if text.startswith('/*', i):
                    depth, i = depth + 1, i + 2
                elif text.startswith('*/', i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
            out.append(' ')
            continue
        if text[i].isspace():
            out.append(text[i])
            i += 1
            continue
        m = TOKEN_RE.match(text, i)
        out.append(m.group(0) + ' ')
        i = m.end()
    return ''.join(out)


def code_tokens(lines):
    """A Rust file's lines -> its code tokens, every comment dropped, and a trailing comma before a
    closing bracket dropped too (rustfmt adds or removes one when a rename changes a line's width,
    and it changes nothing the compiler emits)."""
    toks = [m.group(0) for m in TOKEN_RE.finditer(strip_comments('\n'.join(lines)))]
    return [t for i, t in enumerate(toks)
            if not (t == ',' and i + 1 < len(toks) and toks[i + 1] in (')', ']', '}'))]


def line_tokens(line):
    return [m.group(0) for m in TOKEN_RE.finditer(strip_comments(line))]


# Where a line declares names: an item (`fn`, `const`, `struct` and the rest), a `let`, a struct
# field (`pub size: u64,`), an enum variant (`Busy,`, `Busy = 3,`, `Busy(u8),`) and a function's
# parameters (`fn f(frame: &mut TrapFrame, cap: Capability)`).
DECLARING = frozenset(('fn', 'const', 'static', 'struct', 'enum', 'trait', 'type', 'mod', 'union'))


def _is_name(token):
    return bool(IDENT_RE.match(token)) and token not in KEYWORDS


def declared_positions(tokens):
    """Indices of the names this line's tokens declare, in order (empty for most lines)."""
    found = []
    i = 0
    if tokens[:1] == ['pub']:
        i = 1
        if tokens[1:2] == ['(']:
            if ')' not in tokens[1:]:
                return []
            i = tokens.index(')', 1) + 1
    while i + 1 < len(tokens) and tokens[i] in ('unsafe', 'async', 'extern', 'const') and \
            tokens[i + 1] in DECLARING | {'unsafe', 'async', 'extern'}:
        i += 1
    if i < len(tokens) and tokens[i] in DECLARING | {'let'}:
        j = i + 1
        if tokens[j:j + 1] == ['mut']:
            j += 1
        if j < len(tokens) and _is_name(tokens[j]):
            found.append(j)
    elif i + 1 < len(tokens) and _is_name(tokens[i]) and tokens[i + 1] in (':', ',', '=', '('):
        found.append(i)
    if 'fn' in tokens:
        for j in range(1, len(tokens) - 1):
            if tokens[j + 1] == ':' and tokens[j - 1] in ('(', ',') and _is_name(tokens[j]):
                found.append(j)
            elif (tokens[j - 1] == 'mut' and j > 1 and tokens[j - 2] in ('(', ',')
                  and tokens[j + 1] == ':' and _is_name(tokens[j])):
                found.append(j)
    return sorted(set(found))


CARGO_NAME_RE = re.compile(r'^\s*name\s*=\s*"([^"]+)"')


def diff_renames(files):
    """{old identifier: new identifier} for each rename this diff performs where the name is made.

    A pair enters the map only from a declaration: a removed line and an added line that declare
    names and read the same token for token once the declared names (and the pairs already in the
    map) are set aside. Lines pair by position inside one block of changed lines (a `-` run and the
    `+` run after it, which is how a diff shows a line rewritten), or, failing that, anywhere in
    the diff when the shape has one name on each side (a declaration moved to another file). A
    crate's own name enters from its directory (`crates/a/` to `crates/b/`) or from its manifest's
    `name = "..."`, hyphens read as underscores. A name seen renamed two different ways is left out.
    Nothing else enters: a token that changed anywhere but a declared name is a change, not a
    rename, so a constant re-pointed from one existing value to another still fires. Iterated, so a
    declaration whose type or value names an identifier renamed elsewhere pairs too.
    """
    fixed, conflicted = {}, set()

    def note(pairs, into):
        for a, b in pairs:
            if a == b or a in conflicted:
                continue
            if into.get(a, b) != b:
                conflicted.add(a)
                into.pop(a, None)
            else:
                into[a] = b

    for fd in files:
        path, old_path = fd['path'], fd.get('old_path', fd['path'])
        if old_path != path:
            note([(a.replace('-', '_'), b.replace('-', '_'))
                  for a, b in zip(old_path.split('/'), path.split('/'))
                  if a != b and IDENT_RE.match(a.replace('-', '_'))
                  and IDENT_RE.match(b.replace('-', '_'))], fixed)
        if path.endswith('Cargo.toml'):
            names = {'-': [], '+': []}
            for prefix, line in changed_lines(fd):
                m = CARGO_NAME_RE.match(line)
                if m:
                    names[prefix].append(m.group(1).replace('-', '_'))
            if len(names['-']) == 1 and len(names['+']) == 1:
                note([(names['-'][0], names['+'][0])], fixed)
    # Declaration lines, grouped into change blocks.
    blocks = []
    for fd in files:
        if not fd['path'].endswith('.rs'):
            continue
        cur = None
        for prefix, line in fd['lines']:
            if prefix == ' ':
                cur = None
                continue
            if cur is None or (prefix == '-' and cur['+']):
                cur = {'-': [], '+': []}
                blocks.append(cur)
            toks = line_tokens(line)
            ks = declared_positions(toks) if toks else []
            if ks:
                cur[prefix].append((toks, ks))

    def shape(toks, ks, renames, side):
        mapped = [renames.get(t, t) for t in toks] if side == '-' else list(toks)
        for k in ks:
            mapped[k] = '\x00name'
        return tuple(mapped)

    renames = dict(fixed)
    for _round in range(6):
        found = {}
        leftovers = {'-': {}, '+': {}}
        for block in blocks:
            rem, add = list(block['-']), list(block['+'])
            used_add = set()
            for rt, rk in rem:
                key = shape(rt, rk, renames, '-')
                hit = next((j for j, (at, ak) in enumerate(add)
                            if j not in used_add and len(ak) == len(rk)
                            and shape(at, ak, renames, '+') == key), None)
                if hit is None:
                    leftovers['-'].setdefault(key, []).append(tuple(rt[k] for k in rk))
                    continue
                used_add.add(hit)
                at, ak = add[hit]
                note(list(zip((rt[k] for k in rk), (at[k] for k in ak))), found)
            for j, (at, ak) in enumerate(add):
                if j not in used_add:
                    leftovers['+'].setdefault(shape(at, ak, renames, '+'), []).append(
                        tuple(at[k] for k in ak))
        for key, olds in leftovers['-'].items():
            news = leftovers['+'].get(key, [])
            if len(set(olds)) == 1 and len(set(news)) == 1 and len(olds) == len(news):
                note(list(zip(olds[0], news[0])), found)
        merged = dict(fixed)
        for a, b in found.items():
            if a not in conflicted and merged.get(a, b) == b:
                merged[a] = b
        if merged == renames:
            break
        renames = merged
    return {a: b for a, b in renames.items() if a not in conflicted}


def rename_equivalent(base_tokens, head_tokens, renames):
    """Do two token lists match once each old name in `renames` reads as its new one?"""
    if len(base_tokens) != len(head_tokens):
        return False
    return all(a == b or renames.get(a) == b for a, b in zip(base_tokens, head_tokens))


def code_unchanged(fd, renames):
    """True when this file's code, comments dropped, is the same at base and head (renames read
    under their new names). A file added or deleted by the diff never counts as unchanged."""
    base, head = base_lines(fd), head_lines(fd)
    if not base or not head:
        return False
    return rename_equivalent(code_tokens(base), code_tokens(head), renames)


class Context:
    """What a rule may need from the whole diff rather than one file: every file, and the renames
    the diff performs (`diff_renames`)."""

    def __init__(self, files):
        self.files = files
        self.renames = diff_renames(files)


# ---- rule 1: crates/abi -------------------------------------------------------------------------

ABI_CONST_RE = re.compile(
    r'^\s*pub const [A-Za-z_][A-Za-z0-9_]*\s*:\s*(u8|u16|u32|u64|usize|i8|i16|i32|i64|isize)\b')


CONST_DECL_RE = re.compile(r'^\s*pub const ([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([^=]+?)\s*=\s*(.*)$')


def unpaired_const_changes(fd, const_re, renames=None):
    """Changed `pub const` lines (matching `const_re`) left over after pairing renames.

    A removed `pub const A: T = V` and an added `pub const B: T = V` in the same file, same type and
    same value text, is a rename: the number on the wire did not move (§88's concern), only its
    spelling did, and a spelling is an architect's call that a rename PR has usually already carried
    (#1584). Each removed line pairs with at most one added line. What stays unpaired still fires: a
    changed value (the pair's value differs), an added constant with no removed partner, and a
    removed constant with no added partner (a deleted number is a surface change too). The type and
    value are compared as code tokens after dropping a trailing `;` and comment, with each
    identifier the diff renames at its declaration (`renames`, from `diff_renames`) read under its
    new name, so `128 + IMAGE_MAX_PAGES` pairs with `128 + IMAGE_MAXIMUM_PAGES` only when the diff
    renames that constant. A multi-line initializer never pairs (its line has no closing `;`), so
    it fires as before.
    """
    renames = renames or {}
    removed, added = [], []
    for prefix, line in changed_lines(fd):
        if not const_re.match(line):
            continue
        m = CONST_DECL_RE.match(line)
        key = None
        if m:
            value = strip_comments(m.group(3)).rstrip()
            if value.endswith(';'):  # a one-line initializer; a multi-line one never pairs
                toks = tuple(line_tokens(m.group(2))) + ('=',) + tuple(line_tokens(value[:-1]))
                if prefix == '-':
                    toks = tuple(renames.get(t, t) for t in toks)
                key = toks
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


def rule_abi_surface(fd, out, ctx):
    if not (fd['path'].startswith('crates/abi/') and fd['path'].endswith('.rs')):
        return
    if code_unchanged(fd, ctx.renames):
        return
    left = unpaired_const_changes(fd, ABI_CONST_RE, ctx.renames)
    if left:
        out.append(('abi-surface', fd['path'], left[0].strip()))  # one citation per file


# ---- rule 2: an external dependency entered a Cargo.toml -----------------------------------------

SECTION_RE = re.compile(r'^\[(.+)\]$')
DEP_KEY_RE = re.compile(r'^([A-Za-z0-9_.-]+)\s*=\s*(.*)$')


GIT_URL_RE = re.compile(r'\bgit\s*=\s*"([^"]*)"')
REV_RE = re.compile(r'\brev\s*=\s*"([^"]*)"')
FIRST_PARTY_GIT_RE = re.compile(r'https://github\.com/nifeos/[A-Za-z0-9_.-]+')
FULL_COMMIT_RE = re.compile(r'[0-9a-fA-F]{40}')


def is_pinned_first_party_git(rest):
    """True for a git dependency on a crate of the `nifeos` organization at a full commit.

    The URL must be `https://github.com/nifeos/<repository>` (optionally ending `.git`) and `rev`
    must be 40 hexadecimal digits. A `branch`, a `tag`, a short or missing `rev` is not pinned, so
    it is not exempt. Whether the commit is on that repository's main is NOT checked here (see the
    module docstring's follow-up note).
    """
    gm, rm = GIT_URL_RE.search(rest), REV_RE.search(rest)
    if not gm or not rm or re.search(r'\b(branch|tag)\s*=', rest):
        return False
    url = gm.group(1)
    if url.endswith('.git'):
        url = url[:-4]
    return bool(FIRST_PARTY_GIT_RE.fullmatch(url) and FULL_COMMIT_RE.fullmatch(rm.group(1)))


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
        if is_pinned_first_party_git(rest):
            continue  # our own crate at a full commit: approved in advance (2026-11-09 UTC)
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
# The same, at any visibility: a constant moved to another crate often widens `pub(crate)` to `pub`.
VERSION_OR_MAGIC_ANY_RE = re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?const \w*(VERSION|MAGIC)\w*\s*:')
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


def _format_crate_of(path):
    """`crates/<name>/src/....rs` -> `<name>`, else None."""
    parts = path.split('/')
    if len(parts) > 3 and parts[0] == 'crates' and parts[2] == 'src' and path.endswith('.rs'):
        return parts[1]
    return None


def _apply_renames_to_words(line, renames):
    return re.sub(r'[A-Za-z_]\w*', lambda m: renames.get(m.group(0), m.group(0)), line)


def _const_key(line, renames, side):
    """A VERSION/MAGIC constant line as tokens, its visibility dropped (`pub(crate) const` and
    `pub const` hold the same bytes) and, on the removed side, renames read under the new name."""
    toks = line_tokens(line)
    if toks[:1] == ['pub']:
        toks = toks[1:]
        if toks[:1] == ['(']:
            toks = toks[toks.index(')') + 1:] if ')' in toks else toks
    if side == '-':
        toks = [renames.get(t, t) for t in toks]
    return tuple(toks)


def rule_format_crate_diff(files, out, ctx):
    """Rule 3 over the whole diff, since a constant moved between crates is one change in two files.

    A crate whose every changed `.rs` file has the same code at base and head (`code_unchanged`:
    comments dropped, renames the diff performs read under their new names) moved no byte, so none
    of its files fires, whatever its comments say. Otherwise each format-crate file fires on its
    first unpaired line: a VERSION/MAGIC constant line, or a layout statement in a doc or block
    comment outside test code. A removed and an added line pair off when they hold the same
    constant (same tokens, visibility aside) or the same layout signature, in the same file or in
    any other file of the diff (a constant or its documentation moved to another crate).
    """
    renames = ctx.renames
    quiet_crates = set()
    by_crate = {}
    for fd in files:
        parts = fd['path'].split('/')
        if len(parts) > 2 and parts[0] == 'crates' and fd['path'].endswith('.rs'):
            by_crate.setdefault(parts[1], []).append(fd)
    for crate, fds in by_crate.items():
        if all(code_unchanged(fd, renames) for fd in fds):
            quiet_crates.add(crate)
    # Pools of removed lines from every Rust file in the diff, for the cross-file pairing.
    removed_consts, removed_layout = [], []
    for fd in files:
        if not fd['path'].endswith('.rs'):
            continue
        for prefix, line in changed_lines(fd):
            if prefix == '-' and not is_comment_only(line) and VERSION_OR_MAGIC_ANY_RE.match(line):
                removed_consts.append([_const_key(line, renames, '-'), fd['path'], False,
                                       bool(VERSION_OR_MAGIC_CONST_RE.match(line)), line])
        for prefix, line in changed_lines_outside_tests(fd):
            if (prefix == '-' and is_comment_only(line) and is_doc_or_block_comment(line)
                    and LAYOUT_COMMENT_RE.search(line)):
                removed_layout.append([layout_signature(_apply_renames_to_words(line, renames)),
                                       fd['path'], False, line])

    def take(pool, key, path):
        # The same file first, then any other: a pair in place is the likelier reading.
        for want_same in (True, False):
            for entry in pool:
                if not entry[2] and entry[0] == key and (entry[1] == path) == want_same:
                    entry[2] = True
                    return True
        return False

    for fd in files:
        crate = _format_crate_of(fd['path'])
        if crate is None or crate in quiet_crates or not is_format_crate_source(head_lines(fd)):
            continue
        fired = None
        for prefix, line in changed_lines(fd):
            if prefix == '+' and not is_comment_only(line) and VERSION_OR_MAGIC_CONST_RE.match(line):
                if not take(removed_consts, _const_key(line, renames, '+'), fd['path']):
                    fired = fired or line
        for prefix, line in changed_lines_outside_tests(fd):
            if (prefix == '+' and is_comment_only(line) and is_doc_or_block_comment(line)
                    and LAYOUT_COMMENT_RE.search(line)):
                if not take(removed_layout, layout_signature(line), fd['path']):
                    fired = fired or line
        if fired:
            out.append(('format-crate', fd['path'], fired.strip()))
            continue
        # A removed constant or layout line of this file that nothing in the diff took back.
        for entry in removed_consts:
            if entry[1] == fd['path'] and not entry[2] and entry[3]:
                fired = fired or entry[4]
        for entry in removed_layout:
            if entry[1] == fd['path'] and not entry[2]:
                fired = fired or entry[3]
        if fired:
            out.append(('format-crate', fd['path'], fired.strip()))


def layout_signature(line):
    return (tuple(re.findall(r'\d+', line)),
            tuple(m.group(0).strip().lower() for m in LAYOUT_COMMENT_RE.finditer(line)))


# ---- rule 4: spawnproto ---------------------------------------------------------------------------

SPAWNPROTO_CONST_RE = re.compile(r'^\s*pub const \w+\s*:')


def rule_spawnproto(fd, out, ctx):
    if not fd['path'].endswith('spawnproto.rs'):
        return
    if code_unchanged(fd, ctx.renames):
        return
    left = unpaired_const_changes(fd, SPAWNPROTO_CONST_RE, ctx.renames)
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


def rule_decisions(fd, out, _ctx=None):
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


def rule_syscall_error(fd, out, ctx):
    path = fd['path']
    if not (path == 'kernel/src/syscall.rs' or path.startswith('kernel/src/syscall/')):
        return
    if code_unchanged(fd, ctx.renames):
        return  # the same code once renames read under their new names (#2779, #2809)
    added, removed = [], []
    for prefix, line in changed_lines(fd):
        if is_comment_only(line) or not SYSCALL_ERROR_RE.search(line.split('//')[0]):
            continue
        toks = line_tokens(line)
        if prefix == '-':
            toks = [ctx.renames.get(t, t) for t in toks]
        (added if prefix == '+' else removed).append((tuple(toks), line.strip()))
    for toks, line in added:
        hit = next((i for i, (rt, _l) in enumerate(removed) if rt == toks), None)
        if hit is not None:
            removed.pop(hit)  # moved, or renamed around the same error
            continue
        out.append(('syscall-error', path, line))
        return
    if removed:
        out.append(('syscall-error', path, removed[0][1]))


RULES = (rule_abi_surface, rule_spawnproto, rule_decisions, rule_syscall_error)


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

# Why each finding might be a one-way door, and what answer releases the hold. A false positive
# costs calef a read and a missed one-way door costs more, so each finding says which it might be:
# the answer is then a sentence, not a reading of the diff. The released-by line names the answers
# the maintainer records with script/record-ruling; nothing here releases a hold itself.
WHY_ONE_WAY = {
    'abi-surface': "every program is compiled against these numbers, and a program built before "
                   "the change keeps the old one",
    'dependency': "code nife ships or builds with would come from outside the organization, "
                  "and §46 rules each third-party crate",
    'format-crate': "a program built from another commit may read these bytes: an installed "
                    "package, a stored file, a page a previous kernel wrote, a live swap",
    'spawnproto': "the spawner and every program it starts agree on these values",
    'decisions': "a decision is the record every lane and reader acts on",
    'syscall-error': "programs branch on the errors a method returns, so a new or vanished error "
                     "changes what callers must handle",
}
RELEASED_BY = {
    'abi-surface': "\"nothing moved\" (a rename or move), or approval of the new or changed value",
    'dependency': "approval of the crate at that version (§46), or \"already approved\" naming "
                  "the ruling that covers it",
    'format-crate': "\"no bytes moved\"; or approval of the new layout, saying either "
                    "\"provisional, in-image\" (every reader and writer ships in one image from one "
                    "commit and nothing persists it) or that older readers must keep working",
    'spawnproto': "\"nothing moved\", or approval of the new or changed value",
    'decisions': "approval of the edit, or a send-back naming what to change",
    'syscall-error': "\"same errors\" (a refactor or rename), or approval of the new error "
                     "behavior",
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
                     f"  ({rule}, `{path}`, matched `{detail.replace('`', '')}`)\n"
                     f"  Why it might be one-way: {WHY_ONE_WAY[rule]}.\n"
                     f"  Released by: {RELEASED_BY[rule]}.")
    return '\n'.join(items)


def merge_bases(main, head):
    """Every merge base of `main` and `head`. More than one when the branch merged another branch
    that main also merged (a criss-cross), and then `git merge-base` alone picks one at random."""
    r = subprocess.run(['git', 'merge-base', '--all', main, head], capture_output=True, text=True)
    return r.stdout.split() if r.returncode == 0 else []


def git_blob(rev, path):
    """The blob id of `path` at `rev`, or None when the path is absent there."""
    r = subprocess.run(['git', 'rev-parse', '--verify', '--quiet', f'{rev}:{path}'],
                       capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else None


def drop_ruled(findings, head, base, ruled, blob):
    """`findings` less those an open pull request's ruling already covers, with what covered each.

    `ruled` is [(number, head, [its merge bases with main])] for open pull requests that carry
    `architect-ruled` and not `held-by-lane`. A finding in `path` is covered when, for one of them,
    the file at this head is the file at that head and the file at this base is the file at one of
    its bases: then this diff of the file is that diff of the file, byte for byte, and the ruling
    read it. The test is the whole file, so a branch that changes the ruled file any further still
    fires on it. Measured 2026-11-11 (UTC): #3450 merged #3434's branch and was held for #3434's
    §130 (how the kernel-resident CMOS RTC reaches the userspace clock service) edit, which #3434's
    ruling covers; both blobs match. `blob(rev, path)` is `git_blob`, injected so the selftest needs
    no repository.
    """
    kept, covered = [], []
    for rule, path, detail in findings:
        by = None
        for number, r_head, r_bases in ruled:
            if (blob(head, path) is not None and blob(head, path) == blob(r_head, path)
                    and any(blob(base, path) == blob(rb, path) for rb in r_bases)):
                by = number
                break
        if by is None:
            kept.append((rule, path, detail))
        else:
            covered.append((rule, path, by))
    return kept, covered


def keep_common(runs):
    """The findings of the first run whose (rule, file) every run has."""
    keep = set.intersection(*({(r, p) for r, p, _d in run} for run in runs))
    return [f for f in runs[0] if (f[0], f[1]) in keep]


def evaluate_against(main, head, ruled_heads=()):
    """The findings of `head` against `main`, read from every merge base; one kept only when every
    base yields it (same rule, same file), and when no open ruled pull request covers it.

    A change the pull request makes shows in the diff from each base. A change main made, which a
    branch carries because it merged a branch main also merged, shows only from a base that
    predates it. Measured 2026-11-11 (UTC): the merge drain held #2838, #3167, #3171, #3173, #3224
    and #3416 on decision files only main had changed, each a head with two merge bases, and with
    the intersection none of the six fires. `ruled_heads` is [(number, head)] for the open pull
    requests `drop_ruled` reads. Returns (findings, covered), or None when there is no merge base.
    """
    bases = merge_bases(main, head)
    if not bases:
        return None
    ruled = [(n, h, merge_bases(main, h)) for n, h in ruled_heads]
    runs, covered = [], []
    for base in bases:
        diff = subprocess.run(['git', 'diff', '--unified=1000000', base, head], check=True,
                              capture_output=True).stdout.decode('utf-8', 'replace')
        kept, cov = drop_ruled(evaluate(diff, known_deps_from_git(base)), head, base, ruled,
                               git_blob)
        runs.append(kept)
        covered.extend(cov)
    return keep_common(runs), covered


def evaluate(diff_text, known=None):
    """The diff text -> [(rule, path, detail), ...], one entry per file per rule that fired."""
    known = known if known is not None else KnownDeps()
    out = []
    files = parse_diff(diff_text)
    ctx = Context(files)
    rule_format_crate_diff(files, out, ctx)
    for fd in files:
        for rule in RULES:
            rule(fd, out, ctx)
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

    ("dependency: a nifeos git dependency at a full commit stays quiet", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+e1000e = { version = "0.1.0", git = "https://github.com/nifeos/e1000e", rev = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
""", None),

    ("dependency: a nifeos git dependency with a .git suffix at a full commit stays quiet", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+e1000e = { git = "https://github.com/nifeos/e1000e.git", rev = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
""", None),

    ("dependency: a nifeos git dependency on a branch fires", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+e1000e = { git = "https://github.com/nifeos/e1000e", branch = "main" }
""", 'dependency'),

    ("dependency: a nifeos git dependency on a tag fires", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+e1000e = { git = "https://github.com/nifeos/e1000e", tag = "v0.1.0" }
""", 'dependency'),

    ("dependency: a nifeos git dependency with no rev fires", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+e1000e = { version = "0.1.0", git = "https://github.com/nifeos/e1000e" }
""", 'dependency'),

    ("dependency: a nifeos git dependency with a short rev fires", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+e1000e = { git = "https://github.com/nifeos/e1000e", rev = "8ac75a6" }
""", 'dependency'),

    ("dependency: a nifeos git dependency with a rev and a branch fires", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+e1000e = { git = "https://github.com/nifeos/e1000e", branch = "main", rev = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
""", 'dependency'),

    ("dependency: a git dependency on another organization at a full commit fires", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+thing = { git = "https://github.com/someone/thing", rev = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
""", 'dependency'),

    ("dependency: a look-alike organization at a full commit fires", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+thing = { git = "https://github.com/nifeos-fork/thing", rev = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
""", 'dependency'),

    ("dependency: a crates.io dependency named like a first-party crate still fires", """\
diff --git a/kernel/Cargo.toml b/kernel/Cargo.toml
--- a/kernel/Cargo.toml
+++ b/kernel/Cargo.toml
@@ -1,2 +1,3 @@
 [dependencies]
+e1000e = "0.1"
""", 'dependency'),

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

    ("format-crate: a layout doc-comment edit (a range) beside a code change fires, const untouched",
     """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,4 +1,4 @@
 //! # The layout
-//! blocks 0..DIR_BLOCKS   the superblock and directory
+//! blocks 0..DIR_BLOCKS+1 the superblock and directory
 pub const MAGIC: [u8; 8] = *b"CRKR0002";
-fn directory_end() -> u64 { DIR_BLOCKS }
+fn directory_end() -> u64 { DIR_BLOCKS + 1 }
""", 'format-crate'),

    ("format-crate: the same layout comment edit with no code moved stays quiet (#2612's class)",
     """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,4 +1,4 @@
 //! # The layout
-//! blocks 0..DIR_BLOCKS   the superblock and directory
+//! blocks 0..DIR_BLOCKS+1 the superblock and directory
 pub const MAGIC: [u8; 8] = *b"CRKR0002";
 fn directory_end() -> u64 { DIR_BLOCKS }
""", None),

    ("format-crate: #1402's shape, a layout table row changed beside the code that reads it, fires", """\
diff --git a/crates/capability_manifest_note/src/lib.rs b/crates/capability_manifest_note/src/lib.rs
--- a/crates/capability_manifest_note/src/lib.rs
+++ b/crates/capability_manifest_note/src/lib.rs
@@ -1,3 +1,3 @@
 //! | version | field | meaning |
-//! | 4 | 1 | `arg` | 0 forbidden, 1 required, 2 words |
+//! | 4 | 1 | `arg` | 0 forbidden, 1 required; hears words: 2 read-only, 3 read-write |
-pub fn parse(arg: u8) -> bool { arg <= 2 }
+pub fn parse(arg: u8) -> bool { arg <= 3 }
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
-pub fn parse() {}
+pub fn parse() { machine() }
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

    ("format-crate: a doc layout line whose number moved with its constant fires", """\
diff --git a/crates/nifefs/src/lib.rs b/crates/nifefs/src/lib.rs
--- a/crates/nifefs/src/lib.rs
+++ b/crates/nifefs/src/lib.rs
@@ -1,3 +1,3 @@
-/// The name sits at offset 12.
+/// The name sits at offset 16.
 pub const MAGIC: [u8; 8] = *b"CRKR0002";
-const NAME_OFFSET: usize = 12;
+const NAME_OFFSET: usize = 16;
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

    ("format-crate: #2315's shape, a constant renamed with its table row, stays quiet", """\
diff --git a/crates/capability_manifest_note/src/lib.rs b/crates/capability_manifest_note/src/lib.rs
--- a/crates/capability_manifest_note/src/lib.rs
+++ b/crates/capability_manifest_note/src/lib.rs
@@ -1,5 +1,5 @@
 pub const VERSION: u32 = 1;
-//! | 36 | 1 | option count | at most `MAX_DECLARED_SHORT_OPTIONS` (16) |
+//! | 36 | 1 | option count | at most `MAXIMUM_DECLARED_SHORT_OPTIONS` (16) |
-pub const MAX_DECLARED_SHORT_OPTIONS: usize = 16;
+pub const MAXIMUM_DECLARED_SHORT_OPTIONS: usize = 16;
-fn count_ok(n: usize) -> bool { n <= MAX_DECLARED_SHORT_OPTIONS }
+fn count_ok(n: usize) -> bool { n <= MAXIMUM_DECLARED_SHORT_OPTIONS }
""", None),

    ("format-crate: the same rename with the bound also moved fires", """\
diff --git a/crates/capability_manifest_note/src/lib.rs b/crates/capability_manifest_note/src/lib.rs
--- a/crates/capability_manifest_note/src/lib.rs
+++ b/crates/capability_manifest_note/src/lib.rs
@@ -1,5 +1,5 @@
 pub const VERSION: u32 = 1;
-//! | 36 | 1 | option count | at most `MAX_DECLARED_SHORT_OPTIONS` (16) |
+//! | 36 | 1 | option count | at most `MAXIMUM_DECLARED_SHORT_OPTIONS` (32) |
-pub const MAX_DECLARED_SHORT_OPTIONS: usize = 16;
+pub const MAXIMUM_DECLARED_SHORT_OPTIONS: usize = 32;
-fn count_ok(n: usize) -> bool { n <= MAX_DECLARED_SHORT_OPTIONS }
+fn count_ok(n: usize) -> bool { n <= MAXIMUM_DECLARED_SHORT_OPTIONS }
""", 'format-crate'),

    ("format-crate: #3094's shape, a register named VERSION renamed at the same value, stays quiet",
     """\
diff --git a/crates/designware_ethernet/src/registers.rs b/crates/designware_ethernet/src/registers.rs
--- a/crates/designware_ethernet/src/registers.rs
+++ b/crates/designware_ethernet/src/registers.rs
@@ -1,3 +1,3 @@
-pub const MAC_VERSION: u32 = 0x0110;
+pub const CORE_VERSION: u32 = 0x0110;
-fn read() -> u32 { reg(MAC_VERSION) }
+fn read() -> u32 { reg(CORE_VERSION) }
""", None),

    ("format-crate: #2736's shape, a magic moved to another crate unchanged, stays quiet", """\
diff --git a/crates/non_volatile_memory_express/src/lib.rs b/crates/non_volatile_memory_express/src/lib.rs
--- a/crates/non_volatile_memory_express/src/lib.rs
+++ b/crates/non_volatile_memory_express/src/lib.rs
@@ -1,3 +1,2 @@
 pub const VERSION: u32 = 1;
-pub(crate) const ESCAPE_MAGIC: u64 = 0xE5CA_9E00_4E56_4D45;
 pub use non_volatile_memory_express_contract::ESCAPE_MAGIC;
diff --git a/crates/non_volatile_memory_express_contract/src/lib.rs b/crates/non_volatile_memory_express_contract/src/lib.rs
--- /dev/null
+++ b/crates/non_volatile_memory_express_contract/src/lib.rs
@@ -0,0 +1,2 @@
+//! What the kernel and the driver agree on.
+pub const ESCAPE_MAGIC: u64 = 0xE5CA_9E00_4E56_4D45;
""", None),

    ("format-crate: the same move with the magic changed on the way fires", """\
diff --git a/crates/non_volatile_memory_express/src/lib.rs b/crates/non_volatile_memory_express/src/lib.rs
--- a/crates/non_volatile_memory_express/src/lib.rs
+++ b/crates/non_volatile_memory_express/src/lib.rs
@@ -1,3 +1,2 @@
 pub const VERSION: u32 = 1;
-pub(crate) const ESCAPE_MAGIC: u64 = 0xE5CA_9E00_4E56_4D45;
 pub use non_volatile_memory_express_contract::ESCAPE_MAGIC;
diff --git a/crates/non_volatile_memory_express_contract/src/lib.rs b/crates/non_volatile_memory_express_contract/src/lib.rs
--- /dev/null
+++ b/crates/non_volatile_memory_express_contract/src/lib.rs
@@ -0,0 +1,2 @@
+//! What the kernel and the driver agree on.
+pub const ESCAPE_MAGIC: u64 = 0xE5CA_9E00_4E56_4D46;
""", 'format-crate'),

    ("format-crate: #3425's shape, a new in-image page with a new magic, still fires", """\
diff --git a/crates/boot_ladder/src/kernel_half.rs b/crates/boot_ladder/src/kernel_half.rs
--- /dev/null
+++ b/crates/boot_ladder/src/kernel_half.rs
@@ -0,0 +1,2 @@
+//! The page the kernel writes for the user half of the boot self-test.
+pub const MAGIC: u64 = u64::from_le_bytes(*b"NIFESTK1");
""", 'format-crate'),

    ("format-crate: #3427's shape, a doc rename beside a newly documented field, still fires", """\
diff --git a/crates/driver_process_contract/src/non_volatile_memory_express.rs b/crates/driver_process_contract/src/non_volatile_memory_express.rs
--- a/crates/driver_process_contract/src/non_volatile_memory_express.rs
+++ b/crates/driver_process_contract/src/non_volatile_memory_express.rs
@@ -1,4 +1,5 @@
 pub const ESCAPE_MAGIC: u64 = 0xE5CA_9E00_4E56_4D45;
-/// outside 512..=4096 bytes. `lba_shift` is the format's
+/// outside 512..=4096 bytes. `logical_block_size_shift` is the format's
+/// Logical blocks per filesystem block, `1..=8`.
+pub const RETIRE: u64 = 7;
 pub fn handoff() {}
""", 'format-crate'),

    ("spawnproto: #2792's shape, a value naming a constant the diff renames, stays quiet", """\
diff --git a/crates/grant_plan/src/lib.rs b/crates/grant_plan/src/lib.rs
--- a/crates/grant_plan/src/lib.rs
+++ b/crates/grant_plan/src/lib.rs
@@ -1,1 +1,1 @@
-pub const IMAGE_MAX_PAGES: u64 = 64;
+pub const IMAGE_MAXIMUM_PAGES: u64 = 64;
diff --git a/crates/grant_plan/src/spawnproto.rs b/crates/grant_plan/src/spawnproto.rs
--- a/crates/grant_plan/src/spawnproto.rs
+++ b/crates/grant_plan/src/spawnproto.rs
@@ -1,1 +1,1 @@
-pub const SHELL_BUDGET_PAGES: u64 = 128 + IMAGE_MAX_PAGES;
+pub const SHELL_BUDGET_PAGES: u64 = 128 + IMAGE_MAXIMUM_PAGES;
""", None),

    ("spawnproto: #2836's shape, a value that gains a term, fires", """\
diff --git a/crates/grant_plan/src/spawnproto.rs b/crates/grant_plan/src/spawnproto.rs
--- a/crates/grant_plan/src/spawnproto.rs
+++ b/crates/grant_plan/src/spawnproto.rs
@@ -1,1 +1,1 @@
-pub const SHELL_BUDGET_PAGES: u64 = 128 + IMAGE_MAXIMUM_PAGES;
+pub const SHELL_BUDGET_PAGES: u64 = 128 + IMAGE_MAXIMUM_PAGES + crate::JOB_POOL_PAGES;
""", 'spawnproto'),

    ("spawnproto: a value re-pointed to another existing constant fires (no declaration renamed)",
     """\
diff --git a/crates/grant_plan/src/spawnproto.rs b/crates/grant_plan/src/spawnproto.rs
--- a/crates/grant_plan/src/spawnproto.rs
+++ b/crates/grant_plan/src/spawnproto.rs
@@ -1,1 +1,1 @@
-pub const SHELL_BUDGET_PAGES: u64 = 128 + IMAGE_MAXIMUM_PAGES;
+pub const SHELL_BUDGET_PAGES: u64 = 128 + JOB_POOL_PAGES;
""", 'spawnproto'),

    ("syscall-error: #2779's shape, a function renamed around the same error, stays quiet", """\
diff --git a/kernel/src/sched.rs b/kernel/src/sched.rs
--- a/kernel/src/sched.rs
+++ b/kernel/src/sched.rs
@@ -1,1 +1,1 @@
-pub fn irq_route(intid: u32) -> Option<u64> { None }
+pub fn interrupt_route(intid: u32) -> Option<u64> { None }
diff --git a/kernel/src/syscall/object_methods.rs b/kernel/src/syscall/object_methods.rs
--- a/kernel/src/syscall/object_methods.rs
+++ b/kernel/src/syscall/object_methods.rs
@@ -1,3 +1,3 @@
 fn wait(intid: u32) -> Result<i64, Error> {
-    let ep = sched::irq_route(intid).ok_or(Error::WrongObject)?;
+    let ep = sched::interrupt_route(intid).ok_or(Error::WrongObject)?;
     Ok(0)
""", None),

    ("syscall-error: one error swapped for another fires (no declaration renamed)", """\
diff --git a/kernel/src/syscall/object_methods.rs b/kernel/src/syscall/object_methods.rs
--- a/kernel/src/syscall/object_methods.rs
+++ b/kernel/src/syscall/object_methods.rs
@@ -1,3 +1,3 @@
 fn wait(intid: u32) -> Result<i64, Error> {
-    let ep = sched::interrupt_route(intid).ok_or(Error::WrongObject)?;
+    let ep = sched::interrupt_route(intid).ok_or(Error::NotPermitted)?;
     Ok(0)
""", 'syscall-error'),

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
    for rule in [r.__name__[len('rule_'):].replace('_', '-') for r in RULES] + [
            'dependency', 'format-crate']:
        if rule not in asked or rule not in QUESTIONS or rule not in WHY_ONE_WAY \
                or rule not in RELEASED_BY:
            bad.append(f"rule {rule} lacks a question, a why, a release, or an ask fixture")
    for line, want in ASK_FIXTURES:
        got = ask(line)
        ok = want in got
        print(f"  {'ok  ' if ok else 'FAIL'}  ask: {line.split(':')[0]} states its question")
        if not ok:
            bad.append(f"ask {line!r}: wanted {want!r} in {got!r}")
    # keep_common: #3416's criss-cross, a decision file only main changed shows from one base only.
    runs = [[('decisions', 'design/decisions/0004-kernel-shape.md', 'x'),
             ('format-crate', 'crates/a/src/lib.rs', 'y')],
            [('format-crate', 'crates/a/src/lib.rs', 'y')]]
    got = keep_common(runs)
    ok = got == [('format-crate', 'crates/a/src/lib.rs', 'y')]
    print(f"  {'ok  ' if ok else 'FAIL'}  against: a finding one merge base lacks is main's, not the PR's")
    if not ok:
        bad.append(f"keep_common: {got!r}")
    # drop_ruled: #3450 carries #3434's ruled §130 edit byte for byte, and an edit of its own to another decision.
    blobs = {('H', 'd/130.md'): 'new130', ('R', 'd/130.md'): 'new130', ('B', 'd/130.md'): 'old130',
             ('RB', 'd/130.md'): 'old130', ('H', 'd/4.md'): 'new4', ('R', 'd/4.md'): 'old4',
             ('B', 'd/4.md'): 'old4', ('RB', 'd/4.md'): 'old4',
             ('H2', 'd/130.md'): 'newer130'}
    blob = lambda rev, path: blobs.get((rev, path))
    found = [('decisions', 'd/130.md', 'a'), ('decisions', 'd/4.md', 'b')]
    kept, covered = drop_ruled(found, 'H', 'B', [('3434', 'R', ['RB'])], blob)
    ok = kept == [('decisions', 'd/4.md', 'b')] and covered == [('decisions', 'd/130.md', '3434')]
    print(f"  {'ok  ' if ok else 'FAIL'}  ruled: #3450's carried §130 edit is covered by #3434's ruling")
    if not ok:
        bad.append(f"drop_ruled #3450: {kept!r} {covered!r}")
    kept, _c = drop_ruled(found[:1], 'H2', 'B', [('3434', 'R', ['RB'])], blob)
    ok = kept == found[:1]
    print(f"  {'ok  ' if ok else 'FAIL'}  ruled: the ruled file changed any further still fires")
    if not ok:
        bad.append(f"drop_ruled changed further: {kept!r}")
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
    "  --against MAIN HEAD [--ruled N:REV]...   diff HEAD from every merge base with MAIN itself\n"
    "               (no stdin), print what every base yields less what the ruled pull request N\n"
    "               at REV already carries; exits 4 when MAIN and HEAD share no merge base\n"
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
    if len(argv) >= 3 and argv[0] == '--against':
        ruled, rest = [], argv[3:]
        while rest[:1] == ['--ruled'] and len(rest) >= 2 and ':' in rest[1]:
            number, _, rev = rest[1].partition(':')
            ruled.append((number, rev))
            rest = rest[2:]
        if rest:
            print(USAGE, file=sys.stderr)
            return 2
        result = evaluate_against(argv[1], argv[2], ruled)
        if result is None:
            print(f"architect-label-rules: {argv[1]} and {argv[2]} share no merge base",
                  file=sys.stderr)
            return 4
        matches, covered = result
        for rule, path, by in covered:
            print(f"architect-label-rules: {rule} on {path} is covered by #{by}'s ruling",
                  file=sys.stderr)
        for rule, path, detail in matches:
            print(f"{rule}: {path}: {detail}")
        return 0 if matches else 1
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
