#!/usr/bin/env python3
"""The prose of a Markdown or Rust file, shared by the acronym and banned-figure gates.

    python3 prose_text.py --selftest
"""
import re
import sys

CODE = re.compile(r'`[^`\n]*`')
COMMENT = re.compile(r'<!--.*?-->', re.S)
URL = re.compile(r'<https?://[^>\s]*>|\]\([^)\s]*\)|https?://\S+')
FENCE = re.compile(r'\s*(```|~~~)')


def clean(line):
    """A line of prose with inline code and addresses removed."""
    line = URL.sub(lambda m: ']()' if m.group(0).startswith('](') else '', line)
    return CODE.sub('``', line)


def markdown_lines(text):
    text = COMMENT.sub(lambda m: '\n' * m.group(0).count('\n'), text)
    lines = text.split('\n')
    start = 0
    if lines and lines[0] == '---':
        for i in range(1, len(lines)):
            if lines[i] == '---':
                start = i + 1
                break
    out, fence = [], None
    for n in range(start, len(lines)):
        line = lines[n]
        mark = FENCE.match(line)
        if mark:
            fence = None if fence == mark.group(1) else (fence or mark.group(1))
            continue
        if fence or not line.strip():
            continue
        out.append((n + 1, clean(line), bool(re.match(r'#{1,6} ', line))))
    return out


def rust_lines(text):
    """Doc comments only (`///` and `//!`), with their fenced examples removed."""
    out, fence = [], None
    for n, line in enumerate(text.split('\n'), 1):
        m = re.match(r'\s*//[/!](?!/)\s?(.*)$', line)
        if not m:
            fence = None
            continue
        body = m.group(1)
        mark = FENCE.match(body)
        if mark:
            fence = None if fence == mark.group(1) else (fence or mark.group(1))
            continue
        if fence or not body.strip():
            continue
        out.append((n, clean(body), bool(re.match(r'#{1,6} ', body))))
    return out


def prose_lines(path, text):
    """[(line number, text, is_heading)] for the sentences of one file."""
    if path.endswith('.md'):
        return markdown_lines(text)
    if path.endswith('.rs'):
        return rust_lines(text)
    return []


def selftest():
    cases = [
        # (path, source, wanted [(line, text, is_heading)])
        ('a.md', 'Plain `code` words.\n', [(1, 'Plain `` words.', False)]),
        ('a.md', '# Title EFI\nBody\n', [(1, '# Title EFI', True), (2, 'Body', False)]),
        ('a.md', '```\nEFI\n```\nafter\n', [(4, 'after', False)]),
        ('a.md', '<!-- EFI -->\nafter\n', [(2, 'after', False)]),
        ('a.md', '---\nk: EFI\n---\nafter\n', [(4, 'after', False)]),
        ('a.md', 'See <https://x.example/EFI> and [text](https://x.example/EFI).\n',
         [(1, 'See  and [text]().', False)]),
        ('a.rs', 'let EFI = 1; // EFI\n/// Doc line.\n//! Inner doc.\n', [(2, 'Doc line.', False), (3, 'Inner doc.', False)]),
        ('a.rs', '/// ```\n/// EFI\n/// ```\n/// after\n', [(4, 'after', False)]),
        ('a.txt', 'EFI\n', []),
    ]
    failed = 0
    for path, src, want in cases:
        got = prose_lines(path, src)
        if got != want:
            failed += 1
            print(f'FAIL {path} {src!r} gave {got}, wanted {want}', file=sys.stderr)
    print('prose_text selftest: %s' % ('FAILED' if failed else 'ok'))
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(selftest() if '--selftest' in sys.argv else 2)
