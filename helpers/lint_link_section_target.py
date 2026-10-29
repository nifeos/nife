#!/usr/bin/env python3
"""lint: every custom-section `link_section` under `kernel/src` is gated to `target_os = "none"`.

The `script/lint` section "==> the custom-section link_section names are gated to the bare-metal
target" runs this helper. It walks `kernel/src` for `.rs` files, reads each `#[...]` attribute
whole (so a multi-line `cfg_attr` is one unit), and fails any attribute that names a custom
section in `link_section` without also carrying `target_os = "none"`.

Why the gate: a custom section name is meaningful only to the bare-metal build. The linker
scripts (`kernel/link-*.ld`) place these sections, and nothing else reads the name. On a host
compile the name is a placeholder, and rustc validates it against the object format when it
codegens the item. A Mach-O host therefore refuses an ELF name outright (`invalid Mach-O section
specifier`), which is how a `script/verify` run died at the kernel compile on a macOS host. The
`target_os = "none"` gate keeps the exact ELF name for the linker scripts and drops the attribute
on a host compile, where the placement does not matter. `notes/kernel-proofs.md` stub-list item 5
and `notes/linker-scripts.md` carry the design.

Scope: `kernel/src` alone. The kernel is the crate compiled both for bare metal and for a host
the prover drives, so it is where an ungated custom section breaks a host build. The
`carry!`-family macros in `crates/capability_manifest_note` name `.note.nife.manifest` in their
bodies, but the crate compiles for the host with the macros unexpanded, and the tools that expand
them target bare metal; those are out of this gate's reach and its scope, not blind spots it
chose.

The standard sections in `STANDARD_SECTIONS` need no gate; a `link_section` that names one is not
judged. An empty judged set fails rather than passing, because a gate that selects the set it
judges can pass by checking nothing (milestone 401 (a gate that selects the set it judges)).

BUGS:
- It reads attributes syntactically, not through a Rust parser. A custom section named in a
  `link_section` that this attribute scanner cannot see (inside a macro expansion read from a
  different file, say) is not judged. The kernel's custom sections sit on items in `kernel/src`,
  which is what the scanner reads.
- The gate accepts any attribute carrying the text `target_os = "none"`, so a narrower gate such
  as `all(target_os = "none", target_arch = "aarch64")` passes. That is intended: the host is
  still excluded, which is the whole point.

Name: provisional, minted 2026-10-29 (UTC) for debt-paydown #2885 (gate the ELF-only
`link_section` names so the kernel compiles on a macOS host). calef has not ratified it.
"""

import pathlib
import re
import sys

# Section names a host object format understands, so a `link_section` that names one needs no
# bare-metal gate. `kernel/src` uses custom sections only today; the allowlist keeps a future
# standard use from reading as a violation.
STANDARD_SECTIONS = {".text", ".data", ".bss", ".rodata"}

LINK_SECTION_RE = re.compile(r'''link_section\s*=\s*"([^"]*)"''')
GATE = 'target_os = "none"'


def _skip_string(text, i):
    """Index just past the string literal that starts at the opening quote `i`."""
    n = len(text)
    j = i + 1
    while j < n:
        if text[j] == "\\":
            j += 2
            continue
        if text[j] == '"':
            return j + 1
        j += 1
    return n


def _skip_block_comment(text, i):
    """Index just past the block comment that starts at `/*` at `i`; Rust nests them."""
    n = len(text)
    depth = 0
    j = i
    while j < n:
        if text.startswith("/*", j):
            depth += 1
            j += 2
            continue
        if text.startswith("*/", j):
            depth -= 1
            j += 2
            if depth == 0:
                return j
            continue
        j += 1
    return n


def iter_attributes(text):
    """Yield `(offset, attribute_text)` for every `#[...]` outside comments and strings.

    The attribute text is balanced, so a multi-line `cfg_attr(...)` comes back whole. Strings
    inside the attribute are skipped when balancing, so a bracket in a section name cannot end
    it early.
    """
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            nl = text.find("\n", i)
            i = n if nl < 0 else nl + 1
        elif c == "/" and i + 1 < n and text[i + 1] == "*":
            i = _skip_block_comment(text, i)
        elif c == '"':
            i = _skip_string(text, i)
        elif c == "#" and i + 1 < n and text[i + 1] == "[":
            start = i
            j = i + 1
            depth = 0
            while j < n:
                d = text[j]
                if d == '"':
                    j = _skip_string(text, j)
                    continue
                if d == "[":
                    depth += 1
                elif d == "]":
                    depth -= 1
                    if depth == 0:
                        j += 1
                        break
                j += 1
            yield start, text[start:j]
            i = j
        else:
            i += 1


def violations(root):
    """Sorted `(relpath, lineno, section)` for each custom section that lacks the gate."""
    src = root / "kernel" / "src"
    if not src.is_dir():
        return None  # the selector is broken; caller fails loudly
    found = []
    judged = 0
    for path in sorted(src.rglob("*.rs")):
        text = path.read_text(encoding="utf8")
        rel = str(path.relative_to(root))
        for start, attr in iter_attributes(text):
            match = LINK_SECTION_RE.search(attr)
            if match is None:
                continue
            section = match.group(1)
            if section in STANDARD_SECTIONS:
                continue
            judged += 1
            if GATE not in attr:
                lineno = text.count("\n", 0, start) + 1
                found.append((rel, lineno, section))
    return judged, found


def check(root, out, err):
    result = violations(root)
    if result is None:
        err.write("lint: kernel/src does not exist, so the link_section gate judged nothing. "
                  "The selector or the layout moved; fix the helper.\n")
        return 1
    judged, found = result
    if not found and judged == 0:
        err.write("lint: no custom-section link_section matched under kernel/src, so the gate "
                  "judged nothing (milestone 401 (a gate that selects the set it judges)).\n")
        return 1
    if found:
        err.write("lint: a custom-section `link_section` under kernel/src lacks the "
                  "target_os = \"none\" gate:\n")
        for rel, lineno, section in found:
            err.write(f"  {rel}:{lineno}  {section}\n")
        err.write(
            "\nA custom section name is bare metal: the linker scripts kernel/link-*.ld give these\n"
            "sections their placement, and a host compile reads the name for nothing. rustc\n"
            "validates the name against the object format, so a Mach-O host refuses an ELF name\n"
            "with `invalid Mach-O section specifier`. Gate the attribute so the host drops it and\n"
            "the bare-metal build keeps the exact name:\n\n"
            "    #[cfg_attr(target_os = \"none\", unsafe(link_section = \"...\"))]\n\n"
            "See notes/kernel-proofs.md and notes/linker-scripts.md.\n")
        return 1
    out.write(f'link_section: {judged} custom section(s) under kernel/src, each gated to '
              f'target_os = "none"\n')
    return 0


SELFTEST_CASES = [
    # (name, {relpath: text}, want_rc, want_substrings_in_stdout_or_stderr)
    ("an ungated custom section fails",
     {"kernel/src/a.rs":
      '#[unsafe(no_mangle)]\n'
      '#[unsafe(link_section = ".text.hot.thing")]\n'
      'fn thing() {}\n'},
     1, ["a.rs", ".text.hot.thing", "cfg_attr"]),
    ("a gated custom section passes",
     {"kernel/src/a.rs":
      '#[cfg_attr(target_os = "none", unsafe(link_section = ".text.hot.thing"))]\n'
      'fn thing() {}\n'},
     0, ["1 custom section(s)", "target_os = \"none\""]),
    ("a multi-line gated section passes",
     {"kernel/src/a.rs":
      '#[cfg_attr(\n'
      '    target_os = "none",\n'
      '    unsafe(link_section = ".text.hot.thing")\n'
      ')]\n'
      'fn thing() {}\n'},
     0, ["1 custom section(s)"]),
    ("a narrower target_os gate passes",
     {"kernel/src/a.rs":
      '#[cfg_attr(\n'
      '    all(target_os = "none", target_arch = "aarch64"),\n'
      '    unsafe(link_section = ".text.hot.thing")\n'
      ')]\n'
      'fn thing() {}\n'},
     0, ["1 custom section(s)"]),
    ("a standard section is not judged",
     {"kernel/src/a.rs":
      '#[unsafe(link_section = ".text")]\n'
      'fn plain() {}\n'
      '#[cfg_attr(target_os = "none", unsafe(link_section = ".text.hot.thing"))]\n'
      'fn thing() {}\n'},
     0, ["1 custom section(s)"]),
    ("a commented-out attribute is not judged",
     {"kernel/src/a.rs":
      '// an example that must not trip the gate:\n'
      '// #[unsafe(link_section = ".text.hot.example")]\n'
      '#[cfg_attr(target_os = "none", unsafe(link_section = ".text.hot.thing"))]\n'
      'fn thing() {}\n'},
     0, ["1 custom section(s)"]),
    ("an empty judged set fails",
     {"kernel/src/a.rs": "fn nothing() {}\n"},
     1, ["judged nothing"]),
]


def selftest():
    import io
    import tempfile

    failures = []

    for name, files, want_rc, want_msgs in SELFTEST_CASES:
        with tempfile.TemporaryDirectory(prefix="lint-link-section.") as td:
            root = pathlib.Path(td)
            for rel, content in files.items():
                path = root / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            out, err = io.StringIO(), io.StringIO()
            rc = check(root, out, err)
            blob = out.getvalue() + err.getvalue()
            if rc != want_rc:
                failures.append(f"{name}: exit {rc}, want {want_rc}")
            for msg in want_msgs:
                if msg not in blob:
                    failures.append(f"{name}: output does not name {msg!r}")

    # The selector guard: a tree with no kernel/src at all must fail rather than pass.
    with tempfile.TemporaryDirectory(prefix="lint-link-section.") as td:
        out, err = io.StringIO(), io.StringIO()
        rc = check(pathlib.Path(td), out, err)
        if rc != 1:
            failures.append(f"a missing kernel/src must fail: exit {rc}, want 1")

    if failures:
        for f in failures:
            print(f"selftest: {f}", file=sys.stderr)
        sys.exit(1)
    print("selftest: ungated fails, gated and narrow-gated pass, standard and commented-out "
          "sections are not judged, and the empty-selector guard fails")


def main():
    if "--selftest" in sys.argv:
        selftest()
        return
    sys.exit(check(pathlib.Path.cwd(), sys.stdout, sys.stderr))


if __name__ == "__main__":
    main()
