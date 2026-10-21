#!/usr/bin/env python3
"""helpers/lint_arch_parity.py: every `arch::` path portable kernel code names exists in every port.

CLAUDE.md rule 5 (architectural parity is a gate). `kernel/src/arch/mod.rs` re-exports one port
flat (`pub use aarch64::*;` and its two twins), so the interface the rest of the kernel calls is
an implicit set of shared names, and nothing said the three ports provide the same set. This
check reads that set from the callers and holds each port to it.

What it does, in order:

1. Walks every `.rs` file reached from the module trees of kernel/src (outside kernel/src/arch/)
   and system_tests/src, and collects each `arch::<path>` it names: written out, through a grouped
   `use crate::arch::{...}`, or through a module such a `use` brought in and then named as
   `timer::now`. Comments are stripped first.
2. Works out which architectures can compile each use, from the `#[cfg(...)]` attributes around
   it and on the `mod` declarations that lead to its file. Only `target_arch` is decided; every
   other predicate (a feature, `test`) is treated as possibly true, so a use behind
   `cfg(feature = "bench")` is still required on every port that could turn `bench` on.
3. Resolves the path in each of those ports: arch/mod.rs's own items (with their cfg), then the
   port's mod.rs, following `mod`, `pub use` and glob re-exports. A segment that lands on a
   non-module item (a type, a function) ends the walk, so `HaltReason::measurement_boot` checks
   `HaltReason` and not its methods.
4. Fails on any (path, port) it cannot resolve that ALLOWED below does not name.

What the compiler already catches, and why this is still worth having: script/lint's clippy passes
compile the kernel for all three targets, so an unconditional call to a missing item fails there.
What they do not compile is every feature on every port (seven of the nine boot-mode features are
not built for x86_64; see the x86_64 clippy note in script/lint). A call inside one of those
reaches a port's missing item only when someone first builds that configuration. This check sees
it at once, without cargo, and runs in the pre-push hook.

BUGS: it reads text, not the compiler's name resolution, and it errs both ways.

- It can over-require (a false failure, fixed by an ALLOWED entry): a `#[cfg]` it cannot parse
  counts as "any architecture", and an attribute's reach is approximated (to the item's brace, or
  a `;`, or a `,` in a list), so an odd shape can leave a gated use looking ungated.
- It can under-require (a missed gap; the clippy passes remain the backstop for what they build):
  a port's `pub use` from `crate::`, `super::` or an external crate is trusted without being
  followed; an inline `mod x { ... }` is accepted whole; a top-level macro call whose first
  argument is a name (`linker_symbol!(text_end, ...)`) is taken to define that name; and a port
  item gated on a feature counts as present, since only `target_arch` is decided.
- Names a portable file reaches by a type alias, a macro, or `use crate::arch::*` are not seen.

Name: provisional (lane lint-arch-parity, 2026-10-08).
"""

import os
import re
import sys

PORTS = ("aarch64", "riscv64", "x86_64")
ARCH = "kernel/src/arch"

# (path as named after `arch::`, port) -> why the port legitimately lacks it.
# **Every entry is an exception and a foot gun**: it tells this check to stop asking whether the
# port provides the name. Keep the reason honest enough that a reader can tell when it stops
# being true, and delete the entry when the gap closes (an entry that matches nothing fails).
ALLOWED = {
    # A real gap, not a design. The padding sled of milestone 134 (the register of measures) exists
    # for aarch64 and riscv64 only, so `--features fastpath_pad` does not compile for x86_64
    # (script/lint's x86_64 clippy note). Its scope note, with why and the plan, is in
    # notes/x86-port.md's BUGS.
    ("fastpath_pad_body", "x86_64"): "GAP: no x86_64 sled; scope note in notes/x86-port.md BUGS",
    # A real gap, not a design. The icount boot mode's timer half (a calibration loop, a deadline,
    # the missed-tick count and two bounds) was never written for x86_64, so `--features icount`
    # does not compile there (script/lint's x86_64 clippy note). Its scope note, with why and the
    # plan, is in notes/x86-port.md's BUGS.
    ("timer::ARRIVAL_BOUND", "x86_64"): "GAP: icount timer half not on x86_64; scope note in notes/x86-port.md BUGS",
    ("timer::HANDLER_BOUND", "x86_64"): "GAP: icount timer half not on x86_64; scope note in notes/x86-port.md BUGS",
    ("timer::calibration_loop", "x86_64"): "GAP: icount timer half not on x86_64; scope note in notes/x86-port.md BUGS",
    ("timer::deadline", "x86_64"): "GAP: icount timer half not on x86_64; scope note in notes/x86-port.md BUGS",
    ("timer::missed_ticks", "x86_64"): "GAP: icount timer half not on x86_64; scope note in notes/x86-port.md BUGS",
}

# --- text preparation -------------------------------------------------------------------------

def strip_comments_and_strings(src, strings=True):
    """Blank out comments and (unless `strings` is false) string and char literals, keeping offsets."""
    out = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            out.append(" " * (j - i))
            i = j
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            out.append(re.sub(r"[^\n]", " ", src[i:j]))
            i = j
        elif c == "r" and re.match(r'r#*"', src[i:i + 8]) and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            hashes = re.match(r'r(#*)"', src[i:]).group(1)
            end = src.find('"' + hashes, i + 2 + len(hashes))
            end = n if end < 0 else end + 1 + len(hashes)
            out.append(re.sub(r"[^\n]", " ", src[i:end]) if strings else src[i:end])
            i = end
        elif c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append('"' + (re.sub(r"[^\n]", " ", src[i + 1:j]) if strings else src[i + 1:j]) + '"')
            i = j + 1
        elif c == "'" and re.match(r"'(\\.[^']*|[^\\'])'", src[i:i + 12]):
            m = re.match(r"'(\\.[^']*|[^\\'])'", src[i:i + 12])
            out.append(" " * len(m.group(0)))
            i += len(m.group(0))
        else:
            out.append(c)
            i += 1
    return "".join(out)

# --- cfg evaluation ---------------------------------------------------------------------------

def parse_cfg(text):
    """Parse a cfg predicate into a nested tuple; unknown shapes become ("unknown",)."""
    toks = re.findall(r'[A-Za-z_][A-Za-z0-9_]*|"[^"]*"|[(),=]', text)
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None

    def take():
        pos[0] += 1
        return toks[pos[0] - 1]

    def pred():
        name = take()
        if name in ("all", "any", "not") and peek() == "(":
            take()
            args = []
            while peek() not in (")", None):
                args.append(pred())
                if peek() == ",":
                    take()
            take()
            return (name, args)
        if peek() == "=":
            take()
            value = take().strip('"')
            return ("kv", name, value)
        return ("flag", name)

    try:
        return pred()
    except IndexError:
        return ("unknown",)


def eval_cfg(p, port):
    """Three-valued: True, False, or None (depends on something other than target_arch)."""
    kind = p[0]
    if kind == "kv":
        if p[1] == "target_arch":
            return p[2] == port
        return None
    if kind in ("flag", "unknown"):
        return None
    vals = [eval_cfg(a, port) for a in p[1]]
    if kind == "not":
        return None if vals[0] is None else not vals[0]
    if kind == "all":
        if False in vals:
            return False
        return None if None in vals else True
    if kind == "any":
        if True in vals:
            return True
        return None if None in vals else False
    return None


def ports_of(cfg_text):
    p = parse_cfg(cfg_text)
    return frozenset(port for port in PORTS if eval_cfg(p, port) is not False)

# --- the scanner ------------------------------------------------------------------------------

ATTR = re.compile(r"#(!?)\[")


def match_close(s, i):
    """s[i] is an opener; return the index just past its matching closer."""
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack = [pairs[s[i]]]
    j = i + 1
    while j < len(s) and stack:
        c = s[j]
        if c in pairs:
            stack.append(pairs[c])
        elif c == stack[-1]:
            stack.pop()
        j += 1
    return j


def load(path):
    """(code with comments and strings blanked, code with only comments blanked)."""
    src = open(path).read()
    return strip_comments_and_strings(src), strip_comments_and_strings(src, strings=False)


def arch_sets(s, file_ports, raw):
    """Return a list giving, for each offset in s, the set of ports that can compile it.

    `s` has comments and strings blanked; `raw` only comments, so a cfg's values survive.
    Represented as a list of (start, end, ports) spans; the set at an offset is the intersection
    of file_ports and every span containing it.
    """
    spans = []
    for m in ATTR.finditer(s):
        inner_start = m.end()
        end = match_close(s, m.end() - 1)
        body = raw[inner_start:end - 1].strip()
        if not body.startswith("cfg(") and not body.startswith("cfg ("):
            continue
        ports = ports_of(body[body.index("(") + 1:-1])
        if ports == frozenset(PORTS):
            continue
        if m.group(1) == "!":
            spans.append((0, len(s), ports))
            continue
        # The attribute governs the next item: skip any further attributes, then run to the
        # item's end, which is the close of its first top-level brace, or a `;` or `,` at this
        # nesting depth before any brace opens, or the close of the enclosing group.
        j = end
        while True:
            while j < len(s) and s[j].isspace():
                j += 1
            a = ATTR.match(s, j)
            if not a:
                break
            j = match_close(s, a.end() - 1)
        # A comma ends an item only in a list (a match arm, a struct field, an argument). An item
        # that opens with a keyword runs to its brace or `;`, so `-> Result<A, B>` cannot end it.
        keyword = re.match(r"(?:pub(?:\([^)]*\))?\s+)?(?:fn|struct|enum|union|impl|mod|trait|const|"
                           r"static|type|use|unsafe|extern|async|let|macro_rules)\b", s[j:j + 64])
        k = j
        while k < len(s):
            c = s[k]
            if c in "([":
                k = match_close(s, k)
                continue
            if c == "{":
                k = match_close(s, k)
                break
            if c == ";" or (c == "," and not keyword):
                k += 1
                break
            if c in ")]}":
                break
            k += 1
        spans.append((j, k, ports))
    return spans


def ports_at(spans, file_ports, off):
    ports = set(file_ports)
    for a, b, p in spans:
        if a <= off < b:
            ports &= p
    return ports


def module_file(dir_, name):
    for cand in (os.path.join(dir_, name + ".rs"), os.path.join(dir_, name, "mod.rs")):
        if os.path.exists(cand):
            return cand
    return None


def child_dir(path):
    """Directory where `mod x;` declared in `path` looks for x."""
    base = os.path.basename(path)
    if base in ("mod.rs", "lib.rs", "main.rs"):
        return os.path.dirname(path)
    return os.path.splitext(path)[0]


MOD_DECL = re.compile(r"(?:\bpub(?:\([^)]*\))?\s+)?\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;")
PATH_ATTR = re.compile(r'#\[path\s*=\s*"([^"]+)"\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_]\w*)\s*;')


def walk_kernel():
    """Yield (path, stripped source, ports the file can compile for) for portable kernel files."""
    seen = {}
    # The system-test image is kernel-side code that left the kernel crate (milestone 609 (the
    # system tests leave the kernel crate)) and reaches the same `crate::arch` through
    # `kernel::system_test_access`, so it is held to it too.
    todo = [("kernel/src/lib.rs", frozenset(PORTS)), ("system_tests/src/main.rs", frozenset(PORTS))]
    if os.path.exists("kernel/src/main.rs"):
        todo.append(("kernel/src/main.rs", frozenset(PORTS)))
    while todo:
        path, fports = todo.pop()
        if path in seen:
            seen[path] = seen[path] | fports
            continue
        seen[path] = fports
        s, raw = load(path)
        spans = arch_sets(s, fports, raw)
        paths = {m.end(): m.group(1) for m in PATH_ATTR.finditer(raw)}
        for m in MOD_DECL.finditer(s):
            name = m.group(1)
            if os.path.normpath(os.path.join(child_dir(path), name)) == os.path.normpath(ARCH):
                continue
            target = None
            for end, p in paths.items():
                if abs(end - m.end()) < 3:
                    target = os.path.normpath(os.path.join(os.path.dirname(path), p))
            target = target or module_file(child_dir(path), name)
            if target is None:
                continue
            todo.append((target, frozenset(ports_at(spans, fports, m.start()))))
    # A file reached twice keeps the union of its contexts.
    for path, fports in sorted(seen.items()):
        if path.startswith(ARCH + "/"):
            continue
        yield path, fports


IDENT = r"[A-Za-z_][A-Za-z0-9_]*"
ARCH_PATH = re.compile(r"(?<![A-Za-z0-9_:])(?:crate::|super::)*arch((?:::" + IDENT + r")+)")
ARCH_GROUP = re.compile(r"\buse\s+(?:crate::)?(arch::[^;]*\{[^;]*);")
ARCH_USE_ONE = re.compile(r"\buse\s+(?:crate::)?arch::(" + IDENT + r")(?:\s+as\s+(" + IDENT + r"))?\s*;")


def collect_uses():
    """Yield (file, line, path segments, ports) for every arch:: path portable code names."""
    for path, fports in walk_kernel():
        s, raw = load(path)
        spans = arch_sets(s, fports, raw)

        def line_of(off):
            return s.count("\n", 0, off) + 1

        aliases = {}
        for m in ARCH_PATH.finditer(s):
            # `super::arch` only reaches the arch module from the crate root's children; anything
            # else spelled `foo::arch::` (core::arch, a crate's own arch module) is not ours.
            segs = m.group(1).split("::")[1:]
            yield path, line_of(m.start()), segs, ports_at(spans, fports, m.start())
        for m in ARCH_GROUP.finditer(s):
            for src, vis in expand_use(use_tree(m.group(1))):
                segs = src[1:]
                if not segs or vis == "*":
                    continue
                aliases[vis] = segs
                yield path, line_of(m.start()), segs, ports_at(spans, fports, m.start())
        for m in ARCH_USE_ONE.finditer(s):
            aliases[m.group(2) or m.group(1)] = [m.group(1)]
        for alias, base in aliases.items():
            for m in re.finditer(r"(?<![A-Za-z0-9_:])" + re.escape(alias) + r"((?:::" + IDENT + r")+)", s):
                segs = base + m.group(1).split("::")[1:]
                yield path, line_of(m.start()), segs, ports_at(spans, fports, m.start())

# --- resolution inside a port -----------------------------------------------------------------

ITEM = re.compile(
    r"\b(?:fn|struct|enum|union|trait|type|const(?!\s+(?:fn|unsafe|extern)\b)|static(?:\s+mut)?"
    r"|macro_rules!)\s+(" + IDENT + r")"
)
MACRO_ITEM = re.compile(r"(?<![A-Za-z0-9_:])" + IDENT + r"!\s*\(\s*(" + IDENT + r")\s*,")
MOD_ITEM = re.compile(r"\bmod\s+(" + IDENT + r")\s*[;{]")
USE = re.compile(r"\bpub(?:\([^)]*\))?\s+use\s+([^;]+);")

_defs_cache = {}


def defs(path, port):
    """Names a module file defines or re-exports that are live on `port`.

    Returns (names: dict name -> kind, globs: list of module paths re-exported with `*`).
    """
    key = (path, port)
    if key in _defs_cache:
        return _defs_cache[key]
    s, raw = load(path)
    spans = arch_sets(s, frozenset(PORTS), raw)
    names, globs = {}, []

    # Only items at the file's top level are the module's own; a `const` in an `impl` or a `fn`
    # inside a body is not reachable as `module::name`.
    # An `extern "C" { ... }` block declares module-level items, so its braces do not count.
    depth, stack = [0] * (len(s) + 1), []
    for i, c in enumerate(s):
        depth[i] = sum(stack)
        if c == "{":
            stack.append(0 if re.search(r"\bextern\s*(\"[^\"]*\"\s*)?$", raw[max(0, i - 40):i]) else 1)
        elif c == "}" and stack:
            stack.pop()

    def live(off):
        return depth[off] == 0 and port in ports_at(spans, PORTS, off)

    for m in ITEM.finditer(s):
        if live(m.start()):
            names.setdefault(m.group(1), "item")
    # A top-level macro call whose first argument is a name (`linker_symbol!(text_end, ...)`)
    # is taken to define that name. Over-accepts (see BUGS); the ports use it for generated items.
    for m in MACRO_ITEM.finditer(s):
        if live(m.start()):
            names.setdefault(m.group(1), "item")
    for m in MOD_ITEM.finditer(s):
        if live(m.start()):
            names[m.group(1)] = "mod"
    for m in USE.finditer(s):
        if not live(m.start()):
            continue
        for src, vis in expand_use(use_tree(m.group(1))):
            if vis == "*":
                globs.append(src[:-1])
            else:
                names.setdefault(vis, ("use", src))
    _defs_cache[key] = (names, globs)
    return names, globs


def expand_use(tree):
    """`a::{b, c::d as e, f::*}` -> [([a,b],b), ([a,c,d],e), ([a,f,*],*)]: source path, visible name.

    `tree` has had whitespace removed and every ` as ` turned into `@` (see `use_tree`).
    """
    if "{" not in tree:
        parts = tree.split("::")
        orig, _, vis = parts[-1].partition("@")
        return [(parts[:-1] + [orig], vis or orig)]
    head, rest = tree.split("{", 1)
    rest = rest[: rest.rfind("}")]
    prefix = [p for p in head.split("::") if p]
    out, depth, cur = [], 0, ""
    for c in rest + ",":
        if c == "," and depth == 0:
            if cur and cur != "self":
                out.extend((prefix + src, vis) for src, vis in expand_use(cur))
            elif cur == "self" and prefix:
                out.append((prefix, prefix[-1]))
            cur = ""
            continue
        depth += c == "{"
        depth -= c == "}"
        cur += c
    return out


def use_tree(text):
    return re.sub(r"\s+", "", re.sub(r"\s+as\s+", "@", text))


def resolve(port, segs):
    """True if the path resolves in `port`, starting from arch/mod.rs plus the port's root."""
    roots = [f"{ARCH}/mod.rs", f"{ARCH}/{port}/mod.rs"]
    return any(resolve_in(port, root, segs, 0) for root in roots)


def resolve_in(port, path, segs, depth):
    if depth > 12 or not segs:
        return bool(not segs)
    names, globs = defs(path, port)
    seg = segs[0]
    hit = names.get(seg)
    if hit == "mod":
        target = module_file(child_dir(path), seg)
        if target is None:
            return True  # an inline `mod x { ... }`: unread, so accepted whole (see BUGS)
        return resolve_in(port, target, segs[1:], depth + 1) if len(segs) > 1 else True
    if hit == "item":
        return True  # a type or function; what follows is an associated item
    if isinstance(hit, tuple):
        src = hit[1]
        local = src[1:] if src[0] == "self" else src
        if src[0] in ("crate", "super", "core", "alloc") or not local:
            return True  # re-exported from outside this port's module tree; trust the compiler
        sub = module_file(child_dir(path), local[0])
        if sub is None:
            return True  # a re-export from an external crate (`pub use paging::Format`)
        return resolve_in(port, sub, local[1:] + segs[1:], depth + 1) if len(local) > 1 else True
    for g in globs:
        local = g[1:] if g and g[0] == "self" else g
        if not local:
            continue
        sub = module_file(child_dir(path), local[0])
        if sub is None:
            continue
        rest = local[1:]
        if rest:
            continue  # `pub use a::b::*` with a nested module: rare; fall through to a miss
        if resolve_in(port, sub, segs, depth + 1):
            return True
    return False

# --- the check --------------------------------------------------------------------------------

def main():
    misses = {}
    for path, line, segs, ports in collect_uses():
        if not segs or segs[0] in PORTS:
            continue  # `arch::x86_64::...` is reachable only from inside arch/, never portable
        for port in sorted(ports):
            if not resolve(port, segs):
                misses.setdefault(("::".join(segs), port), []).append(f"{path}:{line}")
    bad = sorted(k for k in misses if k not in ALLOWED)
    stale = sorted(k for k in ALLOWED if k not in misses)
    if bad:
        print("lint: portable kernel code names an arch:: path that a port it compiles for does "
              "not define (CLAUDE.md rule 5, architectural parity):", file=sys.stderr)
        for path, port in bad:
            where = misses[(path, port)]
            more = f" (+{len(where) - 1} more)" if len(where) > 1 else ""
            print(f"  arch::{path} missing from {port}: {where[0]}{more}", file=sys.stderr)
        print("Either add it to the port, gate the caller with #[cfg(target_arch = ...)], or, if "
              "the gap is intended, add it to ALLOWED in helpers/lint_arch_parity.py with a reason.",
              file=sys.stderr)
    if stale:
        print("lint: ALLOWED in helpers/lint_arch_parity.py names gaps that no longer exist; "
              "delete them so the list stays a list of real exceptions:", file=sys.stderr)
        for path, port in stale:
            print(f"  arch::{path} on {port}", file=sys.stderr)
    if bad or stale:
        sys.exit(1)
    count = len({(tuple(segs)) for _, _, segs, _ in collect_uses()})
    print(f"arch parity: {count} distinct arch:: paths from portable code resolve in every port "
          f"that compiles them ({len(ALLOWED)} recorded exceptions)")


if __name__ == "__main__":
    main()
