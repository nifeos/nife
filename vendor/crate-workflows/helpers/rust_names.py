#!/usr/bin/env python3
"""The names a Rust crate defines, read by a lexer: what the naming checks judge.

    python3 rust_names.py FILE...      # every definition, path:line: kind name
    python3 rust_names.py --selftest

helpers/spelled_out.py, unit_symbols.py, service_model.py, contract_names.py, question_shape.py and
british_spelling.py import this. It needs only the files it is given and helpers/naming_words.toml
beside it, so it runs in any repository.

**What a definition is.** A name the crate itself chooses: an item (`fn`, `const`, `static`,
`struct`, `enum`, `union`, `trait`, `type`, an inline `mod`, `macro_rules!`), a struct or variant
field, an enum variant, a function parameter, a `let`, `if let`, `while let` or `for` binding, and
the crate's package name and file names, which the checks add. A use of a name defined elsewhere
(`Duration::as_nanos`, `u32::MAX`, a dependency's type) is not one: the crate did not choose it,
so the checks never see the standard library's spellings and need no list of them.

**Owned by someone else, so skipped:** the names in an `impl Trait for Type` block (the trait's,
though its parameters are the crate's own and are read), everything in an `extern` block, and
everything inside a macro body or a macro invocation.

**A name kept from its source.** A name that mirrors a datasheet, a specification or the standard
library keeps its owner's spelling and states the source where the reader meets it, in two parts on
the item itself:

    /// **Controller Capabilities** (CAP, NVM Express Base Specification 2.0, section 3.1.1).
    #[doc(alias = "controller capabilities")]
    pub const CAP: u32 = 0x00;

The doc comment's first paragraph names the identifier and its source as `(<name>, <source>)`, and
`#[doc(alias)]` carries the plain-English expansion, so a search for either lands on the item. A
name with the alias and no stated source fails, and so does a stated source with no alias: an
exemption is checkable or it is not one. A parameter or a local binding has no doc comment, so it
cannot be kept and takes the spelled-out word. A package or a `[[bin]]` has no item to carry an
attribute, so it keeps its name by opening the crate root's `//!` docs with `(<name>, <source>)`;
no alias is needed. The rule follows the naming rules page of the
operating system project this template serves (design/naming.md there, "A kept foreign name
carries its expansion").

BUGS:
- A lexer, not a parser. Closure parameters, `match` arm bindings and a pattern's field shorthand
  are not read; a macro that builds a name (`paste!`) is not followed.
- The source is checked for presence, never for truth.
- The file's name is provisional.
"""

import re
import sys
from pathlib import Path

IDENT_START = re.compile(r'[A-Za-z_]')
IDENT = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')
RAW_STRING = re.compile(r'(?:b|c)?r(#*)"')
CHAR_LITERAL = re.compile(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^\\'\n])'")
NUMBER = re.compile(r'[0-9][0-9A-Za-z_]*')
COMPONENT = re.compile(r'[0-9]+|[A-Z]+(?![a-z])|[A-Z]?[a-z]+|[A-Z]+')
KEYWORDS = {'mut', 'ref', 'box', 'in', 'self', 'Self', 'super', 'crate', 'true', 'false', 'as',
            'if', 'else', 'match', 'move', 'return', 'loop', 'while', 'for', 'let', 'const'}


class Token:
    __slots__ = ('kind', 'text', 'line')

    def __init__(self, kind, text, line):
        self.kind, self.text, self.line = kind, text, line

    def __repr__(self):
        return f'{self.kind}:{self.text!r}@{self.line}'


def _block_comment(text, i, n):
    """The end of the (nested) block comment starting at `i`."""
    depth, i = 1, i + 2
    while i < n and depth:
        if text.startswith('/*', i):
            depth, i = depth + 1, i + 2
        elif text.startswith('*/', i):
            depth, i = depth - 1, i + 2
        else:
            i += 1
    return i


def tokens(text):
    """Rust source as tokens: `ident`, `lifetime`, `str`, `char`, `num`, `punct`, `doc` (an outer doc
    comment, its text) and `inner_doc`. Plain comments are dropped."""
    out = []
    i, n, line = 0, len(text), 1
    while i < n:
        c = text[i]
        if c == '\n':
            line += 1
            i += 1
        elif c.isspace():
            i += 1
        elif text.startswith('//', i):
            j = text.find('\n', i)
            j = n if j < 0 else j
            body = text[i:j]
            if body.startswith('///') and not body.startswith('////'):
                out.append(Token('doc', body[3:].removeprefix(' '), line))
            elif body.startswith('//!'):
                out.append(Token('inner_doc', body[3:], line))
            i = j
        elif text.startswith('/*', i):
            j = _block_comment(text, i, n)
            body = text[i:j]
            if body.startswith('/**') and not body.startswith(('/***', '/**/')):
                for k, part in enumerate(body[3:-2].split('\n')):
                    out.append(Token('doc', part.strip().removeprefix('*').strip(), line + k))
            line += body.count('\n')
            i = j
        elif c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == '\\' else 1
            out.append(Token('str', text[i + 1:j].replace('\\"', '"'), line))
            line += text.count('\n', i, j)
            i = j + 1
        elif c == "'":
            m = CHAR_LITERAL.match(text, i)
            if m:
                out.append(Token('char', m.group(0), line))
                i = m.end()
            else:
                m = IDENT.match(text, i + 1)
                if m:
                    out.append(Token('lifetime', m.group(0), line))
                    i = m.end()
                else:
                    i += 1
        elif c.isdigit():
            end = NUMBER.match(text, i).end()
            out.append(Token('num', text[i:end], line))
            i = end
        elif IDENT_START.match(c):
            raw = RAW_STRING.match(text, i)
            if raw:
                close = '"' + raw.group(1)
                j = text.find(close, raw.end())
                j = n if j < 0 else j
                out.append(Token('str', text[raw.end():j], line))
                line += text.count('\n', i, j)
                i = j + len(close)
                continue
            if c in 'bc' and i + 1 < n and text[i + 1] in '"\'':
                i += 1  # a byte or C string, or a byte char: the quote branch reads it
                continue
            m = IDENT.match(text, i)
            word = m.group(0)
            if word == 'r' and text.startswith('#', m.end()) and m.end() + 1 < n \
                    and IDENT_START.match(text[m.end() + 1]):
                m = IDENT.match(text, m.end() + 1)  # a raw identifier, r#type
                word = m.group(0)
            out.append(Token('ident', word, line))
            i = m.end()
        elif text.startswith('::', i):
            out.append(Token('punct', '::', line))
            i += 2
        else:
            out.append(Token('punct', c, line))
            i += 1
    return out


class Definition:
    """One name the crate chose: `name`, `kind`, `line`, the doc comment lines above it (`docs`)
    and the `#[doc(alias)]` strings it carries (`aliases`). A definition inside inline modules also
    carries their names (`module`, outermost first); a `const` or `static` carries its type's first
    word (`type_name`) and the tokens of its initializer (`value`, as text)."""

    def __init__(self, name, kind, line, docs=(), aliases=(), returns_bool=False, takes_mut=False):
        self.name, self.kind, self.line = name, kind, line
        self.docs, self.aliases = list(docs), list(aliases)
        # For a `fn`: whether it returns `bool`, and whether it takes any `&mut`.
        self.returns_bool, self.takes_mut = returns_bool, takes_mut
        self.module, self.type_name, self.value = (), '', ()

    def __repr__(self):
        return f'{self.kind} {self.name}@{self.line}'


OPEN, CLOSE = '([{', ')]}'


def _match(toks, i):
    """The index just past the bracket group that opens at `i`."""
    depth = 0
    for j in range(i, len(toks)):
        t = toks[j]
        if t.kind == 'punct' and t.text in OPEN:
            depth += 1
        elif t.kind == 'punct' and t.text in CLOSE:
            depth -= 1
            if depth == 0:
                return j + 1
    return len(toks)


def _is(t, kind, text=None):
    return t is not None and t.kind == kind and (text is None or t.text == text)


def _attribute(toks, i, docs, aliases):
    """Read the `#[...]` at `i` into `docs` and `aliases`; the index past it."""
    end = _match(toks, i + 1)
    body = toks[i + 2:end - 1]
    if body and _is(body[0], 'ident', 'doc'):
        if len(body) > 2 and _is(body[1], 'punct', '=') and body[2].kind == 'str':
            docs.extend(body[2].text.split('\n'))
        for k, t in enumerate(body):
            if _is(t, 'ident', 'alias'):
                rest = body[k + 1:]
                if rest and _is(rest[0], 'punct', '=') and len(rest) > 1 and rest[1].kind == 'str':
                    aliases.append(rest[1].text)
                elif rest and _is(rest[0], 'punct', '('):
                    aliases.extend(x.text for x in rest[:_match(rest, 0)] if x.kind == 'str')
    return end


class _Reader:
    def __init__(self, toks):
        self.toks = toks
        self.out = []
        self.modules = []

    def at(self, i):
        return self.toks[i] if 0 <= i < len(self.toks) else None

    def add(self, t, kind, docs=(), aliases=()):
        if t is not None and t.kind == 'ident' and t.text != '_' and t.text not in KEYWORDS:
            self.out.append(Definition(t.text, kind, t.line, docs, aliases))
            self.out[-1].module = tuple(self.modules)
            return self.out[-1]
        return None

    def signature(self, definition, start, end, header_end):
        """Record on `definition` whether the signature in toks[start:header_end] returns `bool`
        and whether the parameters in toks[start:end] take a `&mut`."""
        if definition is None:
            return
        for k in range(start, end - 1):
            if _is(self.toks[k], 'punct', '&'):
                for t in self.toks[k + 1:k + 4]:
                    if _is(t, 'ident', 'mut'):
                        definition.takes_mut = True
                    if t.kind != 'lifetime' and not _is(t, 'ident', 'mut'):
                        break
        for k in range(end, header_end - 2):
            if (_is(self.toks[k], 'punct', '-') and _is(self.toks[k + 1], 'punct', '>')
                    and _is(self.toks[k + 2], 'ident', 'bool')):
                after = self.at(k + 3)
                definition.returns_bool = not (_is(after, 'punct', '::') or _is(after, 'punct', '<'))

    def preamble(self, i, end):
        """Skip docs, attributes and visibility from `i`; (index, docs, aliases)."""
        docs, aliases = [], []
        while i < end:
            t = self.toks[i]
            if t.kind == 'doc':
                docs.append(t.text)
                i += 1
            elif _is(t, 'punct', '#') and _is(self.at(i + 1), 'punct', '['):
                i = _attribute(self.toks, i, docs, aliases)
            elif _is(t, 'ident', 'pub'):
                i += 1
                if _is(self.at(i), 'punct', '('):
                    i = _match(self.toks, i)
            else:
                break
        return i, docs, aliases

    def segments(self, start, end):
        """The comma-separated parts of the bracket group's inside, at its own depth."""
        parts, depth, s = [], 0, start
        for j in range(start, end):
            t = self.toks[j]
            if t.kind == 'punct' and t.text in OPEN:
                depth += 1
            elif t.kind == 'punct' and t.text in CLOSE:
                depth -= 1
            elif depth == 0 and _is(t, 'punct', ','):
                parts.append((s, j))
                s = j + 1
        parts.append((s, end))
        return parts

    def fields(self, i):
        """The named fields of the `{` group at `i`; the index past it."""
        end = _match(self.toks, i)
        for s, e in self.segments(i + 1, end - 1):
            j, docs, aliases = self.preamble(s, e)
            if j + 1 < e and _is(self.at(j + 1), 'punct', ':'):
                self.add(self.at(j), 'field', docs, aliases)
        return end

    def variants(self, i):
        end = _match(self.toks, i)
        for s, e in self.segments(i + 1, end - 1):
            j, docs, aliases = self.preamble(s, e)
            if j < e:
                self.add(self.at(j), 'variant', docs, aliases)
                if _is(self.at(j + 1), 'punct', '{'):
                    self.fields(j + 1)
        return end

    def parameters(self, i):
        """The parameters of the `(` group at `i`; the index past it."""
        end = _match(self.toks, i)
        for s, e in self.segments(i + 1, end - 1):
            j, _, _ = self.preamble(s, e)
            if _is(self.at(j), 'ident', 'mut'):
                j += 1
            if j + 1 < e and _is(self.at(j + 1), 'punct', ':'):
                self.add(self.at(j), 'parameter')
        return end

    def binding(self, i, stops):
        """The names a pattern binds, from `i` to the first of `stops` at depth 0."""
        depth, j = 0, i
        while j < len(self.toks):
            t = self.toks[j]
            if t.kind == 'punct' and t.text in OPEN:
                depth += 1
            elif t.kind == 'punct' and t.text in CLOSE:
                depth -= 1
                if depth < 0:
                    return j
            elif depth == 0 and (t.text in stops) and t.kind in ('punct', 'ident'):
                return j
            elif t.kind == 'ident' and (t.text[0].islower() or t.text[0] == '_'):
                after, before = self.at(j + 1), self.at(j - 1)
                if not (_is(after, 'punct') and after.text in ('(', '{', '::', '!', ':')
                        or _is(before, 'punct', '::') or _is(before, 'punct', '.')):
                    self.add(t, 'binding')
            j += 1
        return j

    def skip_to_brace(self, i):
        """From `i`, the index of the next `{` or `;` at depth 0 (a header's end)."""
        depth = 0
        while i < len(self.toks):
            t = self.toks[i]
            if t.kind == 'punct' and t.text in '([<':
                depth += 1
            elif t.kind == 'punct' and t.text in ')]>':
                depth = max(depth - 1, 0)
            elif depth == 0 and t.kind == 'punct' and t.text in '{;':
                return i
            i += 1
        return i

    def block(self, i, end, context):
        """Read definitions in toks[i:end] whose enclosing block is `context`."""
        docs, aliases = [], []
        while i < end:
            t = self.toks[i]
            nxt = self.at(i + 1)
            if t.kind == 'doc':
                docs.append(t.text)
                i += 1
                continue
            if _is(t, 'punct', '#') and _is(nxt, 'punct', '['):
                i = _attribute(self.toks, i, docs, aliases)
                continue
            if _is(t, 'punct', '#') and _is(nxt, 'punct', '!'):
                i = _match(self.toks, i + 2)
                continue
            if t.kind == 'ident' and _is(nxt, 'punct', '!'):
                # A macro invocation or macro_rules!: its body is not read.
                k = i + 2
                if t.text == 'macro_rules':
                    self.add(self.at(k), 'macro', docs, aliases)
                    k += 1
                if _is(self.at(k), 'punct') and self.at(k).text in OPEN:
                    i = _match(self.toks, k)
                else:
                    i = k
                docs, aliases = [], []
                continue
            if t.kind != 'ident':
                if _is(t, 'punct', '{'):
                    i = self.block_group(i, 'block')
                else:
                    i += 1
                if t.text in ';{}':
                    docs, aliases = [], []
                continue
            word = t.text
            owned = context != 'trait_impl'
            if word == 'fn':
                made = self.add(nxt, 'fn', docs, aliases) if owned else None
                j = i + 2
                if _is(self.at(j), 'punct', '<'):
                    j = self.angle(j)
                params_start = j
                if _is(self.at(j), 'punct', '('):
                    j = self.parameters(j)
                params_end = j
                j = self.skip_to_brace(j)
                self.signature(made, params_start, params_end, j)
                i = self.block_group(j, 'fn') if _is(self.at(j), 'punct', '{') else j + 1
                docs, aliases = [], []
            elif word in ('const', 'static') and nxt is not None and nxt.kind == 'ident' \
                    and nxt.text not in ('fn', 'unsafe', 'extern', 'async'):
                name = self.at(i + 2) if nxt.text == 'mut' else nxt
                if owned:
                    made = self.add(name, 'const' if word == 'const' else 'static', docs, aliases)
                    self.initializer(made, i)
                docs, aliases = [], []
                i += 2
            elif word in ('struct', 'union', 'enum') and _is(nxt, 'ident'):
                self.add(nxt, word, docs, aliases)
                j = self.skip_to_brace(i + 2)
                if _is(self.at(j), 'punct', '{'):
                    i = self.variants(j) if word == 'enum' else self.fields(j)
                else:
                    i = j + 1
                docs, aliases = [], []
            elif word == 'trait' and _is(nxt, 'ident'):
                self.add(nxt, 'trait', docs, aliases)
                j = self.skip_to_brace(i + 2)
                i = self.block_group(j, 'trait') if _is(self.at(j), 'punct', '{') else j + 1
                docs, aliases = [], []
            elif word == 'type' and _is(nxt, 'ident'):
                if owned:
                    self.add(nxt, 'type', docs, aliases)
                docs, aliases = [], []
                i += 2
            elif word == 'mod' and _is(nxt, 'ident'):
                j = i + 2
                if _is(self.at(j), 'punct', '{'):
                    self.add(nxt, 'module', docs, aliases)
                    self.modules.append(nxt.text)
                    i = self.block_group(j, 'module')
                    self.modules.pop()
                else:
                    i = j  # `mod x;` is its file, which the checks read by name
                docs, aliases = [], []
            elif word == 'impl':
                j = self.skip_to_brace(i + 1)
                header = self.toks[i + 1:j]
                trait = any(_is(h, 'ident', 'for') and not _is(header[k + 1] if k + 1 < len(header)
                                                              else None, 'punct', '<')
                            for k, h in enumerate(header))
                kind = 'trait_impl' if trait else 'impl'
                i = self.block_group(j, kind) if _is(self.at(j), 'punct', '{') else j + 1
                docs, aliases = [], []
            elif word == 'extern' and (_is(nxt, 'punct', '{') or
                                       (_is(nxt, 'str') and _is(self.at(i + 2), 'punct', '{'))):
                j = i + 1 if _is(nxt, 'punct', '{') else i + 2
                i = _match(self.toks, j)
                docs, aliases = [], []
            elif word == 'let':
                i = self.binding(i + 1, ('=', ';', ':', 'else'))
            elif word == 'for' and context in ('fn', 'block') and not _is(nxt, 'punct', '<'):
                i = self.binding(i + 1, ('in',))
            else:
                i += 1
        return end

    def initializer(self, made, i):
        """Record on the const or static at `i` its type's first word and its initializer."""
        if made is None:
            return
        depth, colon, equals = 0, None, None
        for j in range(i + 1, len(self.toks)):
            t = self.toks[j]
            if t.kind == 'punct' and t.text in OPEN:
                depth += 1
            elif t.kind == 'punct' and t.text in CLOSE:
                depth -= 1
            elif depth == 0 and _is(t, 'punct', ':') and colon is None and equals is None:
                colon = j
            elif depth == 0 and _is(t, 'punct', '=') and equals is None:
                equals = j
            elif depth == 0 and _is(t, 'punct', ';'):
                if equals is not None:
                    made.value = tuple(x.text for x in self.toks[equals + 1:j])
                break
        if colon is not None and _is(self.at(colon + 1), 'ident'):
            made.type_name = self.at(colon + 1).text

    def angle(self, i):
        depth = 0
        while i < len(self.toks):
            t = self.toks[i]
            if _is(t, 'punct', '<'):
                depth += 1
            elif _is(t, 'punct', '>'):
                depth -= 1
                if depth == 0:
                    return i + 1
            i += 1
        return i

    def block_group(self, i, context):
        """Read the `{` group at `i` as a block of kind `context`; the index past it."""
        end = _match(self.toks, i)
        self.block(i + 1, end - 1, context)
        return end


def definitions(text):
    """Every name the source in `text` defines, in source order."""
    reader = _Reader(tokens(text))
    reader.block(0, len(reader.toks), 'module')
    return sorted(reader.out, key=lambda d: d.line)


def components(identifier):
    """`RX_RING` -> rx, ring; `RxRing` -> rx, ring; `within_30_s` -> within, 30, s."""
    out = []
    for chunk in identifier.split('_'):
        out += [c.lower() for c in COMPONENT.findall(chunk)]
    return out


SOURCE = '\\(\\s*`?{name}`?\\s*,\\s*([^)]*[A-Za-z][^)]*)\\)'


def kept(definition):
    """(kept, why) for a name a check refuses: whether it states its source in the kept form, and
    what it lacks when it claims the form and falls short."""
    if definition.kind in ('parameter', 'binding', 'file'):
        return False, ''
    paragraph = '\n'.join(definition.docs).split('\n\n')[0].replace('\n', ' ')
    source = re.search(SOURCE.format(name=re.escape(definition.name)), paragraph)
    alias = [a for a in definition.aliases if a.strip() and a != definition.name]
    if definition.kind in ('package', 'program'):
        # A package has no item to carry an attribute: its crate root's docs state the source.
        return bool(source), ''
    if source and alias:
        return True, ''
    if alias:
        return False, (f'it carries #[doc(alias)] but its doc comment states no source; open the doc '
                       f'comment with `({definition.name}, <source>)`')
    if source:
        return False, ('its doc comment states a source but it carries no #[doc(alias = '
                       '"<plain-English name>")]')
    return False, ''


def file_definitions(path, text):
    """The definitions in one file, plus its file name when the stem names a module or a target."""
    found = definitions(text)
    stem = Path(path).stem
    if stem not in ('lib', 'main', 'mod', 'build'):
        found.insert(0, Definition(stem, 'file', 1))
    elif stem == 'mod':
        found.insert(0, Definition(Path(path).parent.name, 'file', 1))
    return found


# --- the command line both checks share ---------------------------------------------------------

WORDS = Path(__file__).with_name('naming_words.toml')

# A new crate repository starts with no name that breaks a rule, so there is no baseline file and
# the ceiling is 0: every finding fails. A name that mirrors its source is kept in the form above,
# at the item, never in a list here.
CEILING = 0


def load_words(path=WORDS):
    import tomllib
    with open(path, 'rb') as f:
        return tomllib.load(f)


def inner_docs(path):
    """The `//!` lines at the head of a Rust file, as doc lines; empty when the file is missing."""
    try:
        text = Path(path).read_text(encoding='utf-8', errors='replace')
    except OSError:
        return []
    return [t.text.removeprefix(' ') for t in tokens(text) if t.kind == 'inner_doc']


def manifest_definitions(manifest):
    """The package name and each `[[bin]]` name in a Cargo.toml, as definitions at their lines.

    A package or a program cannot carry an attribute, so its doc lines are the crate root's
    `//!` lines (`src/lib.rs` or `src/main.rs` for the package, the bin's own file for a bin), and
    `kept` reads the stated source there."""
    import tomllib
    text = Path(manifest).read_text(encoding='utf-8')
    data = tomllib.loads(text)
    here = Path(manifest).parent
    rows = text.splitlines()

    def line_of(name):
        return next((k for k, row in enumerate(rows, 1)
                     if re.match(r'\s*name\s*=\s*["\']' + re.escape(name), row)), 1)
    out = []
    name = data.get('package', {}).get('name')
    if name:
        root = data.get('lib', {}).get('path') or 'src/lib.rs'
        docs = inner_docs(here / root) or inner_docs(here / 'src/main.rs')
        out.append(Definition(name, 'package', line_of(name), docs))
    for table in data.get('bin', []):
        bin_name = table.get('name')
        if bin_name:
            path = table.get('path') or f'src/bin/{bin_name}.rs'
            docs = inner_docs(here / path)
            if not docs and bin_name == name:
                docs = inner_docs(here / 'src/main.rs')
            out.append(Definition(bin_name, 'program', line_of(bin_name), docs))
    return out


def package_definition(manifest):
    """The package name in a Cargo.toml, as a definition at its line, or None."""
    return next((d for d in manifest_definitions(manifest) if d.kind == 'package'), None)


def spelled_phrase(word, instead):
    return f'`{word}` is spelled out: write `{instead}`'


def judge(path, found, hits_of, phrase=spelled_phrase):
    """Failure lines for the definitions in `found` that `hits_of` refuses and that are not kept.

    `phrase(word, instead)` words one hit; the default is the spelled-out message."""
    out = []
    for d in found:
        hits = hits_of(d)
        if not hits:
            continue
        is_kept, why = kept(d)
        if is_kept:
            continue
        said = '; '.join(phrase(word, instead) for word, instead in hits)
        if why:
            tail = f' It claims to keep its source spelling, but {why}.'
        elif d.kind in ('parameter', 'binding', 'file'):
            tail = ''
        elif d.kind in ('package', 'program'):
            tail = (f' If it mirrors a specification or a standard, keep it and say so in the crate'
                    f' root docs: open them with `({d.name}, <source>)`.')
        else:
            tail = (f' If it mirrors a datasheet, a specification or the standard library, keep it'
                    f' and say so at the item: open its doc comment with `({d.name}, <source>)` and'
                    f' add #[doc(alias = "<plain-English name>")].')
        out.append(f'{path}:{d.line}: {d.kind} {d.name}: {said}.{tail}')
    return out


def run(argv, label, rule, hits_of_words, own_selftest, phrase=spelled_phrase):
    """The command line of a naming check: `label` names it, `rule` says what it holds, and
    `hits_of_words(words)` returns the function that lists a definition's refused words, and
    `phrase(word, instead)` words each one."""
    import argparse
    parser = argparse.ArgumentParser(prog=f'{label}.py')
    parser.add_argument('files', nargs='*')
    parser.add_argument('--manifest', help="the crate's Cargo.toml, whose package name is judged too")
    parser.add_argument('--words', default=str(WORDS), help='the word list (default: beside this file)')
    parser.add_argument('--report', action='store_true', help='print the findings and exit 0')
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args(argv)
    if args.selftest:
        return own_selftest()
    hits_of = hits_of_words(load_words(args.words))
    bad = []
    if args.manifest:
        bad += judge(args.manifest, manifest_definitions(args.manifest), hits_of, phrase)
    judged = 0
    for path in args.files:
        if not path.endswith('.rs'):
            continue
        found = file_definitions(path, Path(path).read_text(encoding='utf-8', errors='replace'))
        judged += len(found)
        bad += judge(path, found, hits_of, phrase)
    for line in bad:
        print(line, file=sys.stdout if args.report else sys.stderr)
    if args.report or len(bad) <= CEILING:
        print(f'{label}: {judged} names judged, {len(bad)} refused (ceiling {CEILING})')
        return 0
    print(f'\n{label}: {len(bad)} names break the rule that {rule}, over the ceiling of {CEILING}.',
          file=sys.stderr)
    return 1


def selftest():
    src = '''
//! inner doc mentions rx_ring
/// A doc comment mentioning tx_queue.
#[doc(alias = "rx alias")]
pub struct Ring<T: Copy> {
    /// head
    pub head: u32,
    pub(crate) tail: [u8; 4],
    cb: fn(arg_inside: u8) -> u8,
}
enum Mode { Idle, Busy { depth: u8 }, Code(u8) = 3 }
impl Ring<u8> {
    pub const fn count(&self, limit: usize) -> usize {
        let (a, mut b) = (1, 2);
        let Some(c) = x else { return 0 };
        for idx in 0..limit { let _ = idx; }
        let s = "let rx = 1"; // let tx = 2
        a + b + c
    }
}
impl core::fmt::Display for Ring<u8> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { Ok(()) }
}
extern "C" { fn foreign_rx(len: usize); }
macro_rules! make { ($x:ident) => { fn $x() {} } }
make!(generated_rx);
const MAXIMUM: u8 = 1;
static mut COUNTER: u32 = 0;
trait Speak { fn speak(&self, times: u8); type Out; }
mod inner { pub fn helper() {} }
mod outer;
'''
    got = [(d.kind, d.name) for d in definitions(src)]
    want = [('struct', 'Ring'), ('field', 'head'), ('field', 'tail'), ('field', 'cb'),
            ('enum', 'Mode'), ('variant', 'Idle'), ('variant', 'Busy'), ('field', 'depth'),
            ('variant', 'Code'), ('fn', 'count'), ('parameter', 'limit'), ('binding', 'a'),
            ('binding', 'b'), ('binding', 'c'), ('binding', 'idx'), ('binding', 's'),
            ('parameter', 'f'), ('macro', 'make'), ('const', 'MAXIMUM'), ('static', 'COUNTER'),
            ('trait', 'Speak'), ('fn', 'speak'), ('parameter', 'times'), ('type', 'Out'),
            ('module', 'inner'), ('fn', 'helper')]
    failed = 0
    if sorted(got) != sorted(want):
        failed += 1
        print(f'rust_names selftest: definitions\n  got  {sorted(got)}\n  want {sorted(want)}',
              file=sys.stderr)
    ring = definitions(src)[0]
    if ring.aliases != ['rx alias'] or ring.docs != ['A doc comment mentioning tx_queue.']:
        failed += 1
        print(f'rust_names selftest: docs {ring.docs} aliases {ring.aliases}', file=sys.stderr)
    for name, want_parts in (('RX_RING', ['rx', 'ring']), ('RxRing', ['rx', 'ring']),
                             ('within_30_s', ['within', '30', 's']), ('RXDesc', ['rx', 'desc']),
                             ('CLOCK_MHZ', ['clock', 'mhz'])):
        if components(name) != want_parts:
            failed += 1
            print(f'rust_names selftest: components({name}) = {components(name)}', file=sys.stderr)
    good = Definition('CAP', 'const', 1, ['**Controller Capabilities** (CAP, NVM Express Base '
                                          'Specification 2.0, section 3.1.1).'], ['controller capabilities'])
    no_source = Definition('CAP', 'const', 1, ['Controller capabilities.'], ['controller capabilities'])
    no_alias = Definition('CAP', 'const', 1, ['(CAP, NVM Express Base Specification 2.0).'])
    local = Definition('rx', 'binding', 1)
    checks = [(kept(good), (True, '')), (kept(no_source)[0], False), (kept(no_alias)[0], False),
              ('states no source' in kept(no_source)[1], True),
              ('no #[doc(alias' in kept(no_alias)[1], True), (kept(local), (False, ''))]
    for k, (have, want_v) in enumerate(checks):
        if have != want_v:
            failed += 1
            print(f'rust_names selftest: kept case {k}: {have!r}, wanted {want_v!r}', file=sys.stderr)
    sig = {d.name: (d.returns_bool, d.takes_mut) for d in definitions(
        "fn a(&self) -> bool {} fn b(&mut self) -> bool {} fn c<'x>(&'x mut self) -> bool {}"
        " fn d() -> Option<bool> {} fn e(x: &u8) -> u8 {}") if d.kind == 'fn'}
    if sig != {'a': (True, False), 'b': (True, True), 'c': (True, True), 'd': (False, False),
               'e': (False, False)}:
        failed += 1
        print(f'rust_names selftest: signatures {sig}', file=sys.stderr)
    root = Definition('uuid', 'package', 1, ['The identifier (uuid, RFC 9562).'])
    if kept(root) != (True, '') or kept(Definition('uuid', 'package', 1))[0]:
        failed += 1
        print('rust_names selftest: a package keeps a name through its crate root docs',
              file=sys.stderr)
    lines = [d.line for d in definitions('/* a\nb */\nlet s = "x\ny";\nlet rx = 1;')]
    if lines != [3, 5]:
        failed += 1
        print(f'rust_names selftest: lines {lines}, wanted [3, 5]', file=sys.stderr)
    if failed:
        return 1
    print('rust_names selftest: ok')
    return 0


def main(argv):
    if argv == ['--selftest']:
        return selftest()
    if not argv or argv[0].startswith('-'):
        print(__doc__.split('\n\n')[1], file=sys.stderr)
        return 2
    for path in argv:
        for d in file_definitions(path, Path(path).read_text(encoding='utf-8', errors='replace')):
            print(f'{path}:{d.line}: {d.kind} {d.name}')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
