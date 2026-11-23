#!/usr/bin/env python3
# Prove that splitting AGENTS.md into always-loaded rules and on-demand skills deleted nothing.
#
#     helpers/agents-md-split-check.py [<base-rev>]
#
# <base-rev> is the commit whose AGENTS.md is the "before". The default is cde620b1d, the tip of
# lane/agents-md-stale-refs that lane/agents-md-skills branched from on 2026-10-06 (UTC).
#
# Why this exists. calef approved the split on 2026-10-06 (UTC) on one condition: nothing is
# deleted, every sentence moves to exactly one place. A reviewer cannot check that by reading two
# diffs of several thousand words each, and "I moved it all" is rung zero. So this reads every
# sentence of the old file and looks for it, whitespace-, heading- and link-target-insensitively,
# in the union of the new files: AGENTS.md, every notes/skills/*/SKILL.md, and kernel/CLAUDE.md
# if one exists. A sentence found nowhere must be named in CONDENSED below, which is the record of
# what was cut on purpose; one that is missing and unnamed fails, and so does a CONDENSED entry
# that turns out to be present (the list would be lying). For each condensed sentence it also says
# whether design/tenets/ still holds it, since the cut was argued on the tenets holding the reasons.
#
# It checks coverage, not "exactly one place": a sentence present twice passes. The split was built
# from line ranges (each range used once), which is what makes "exactly one" true; this script
# reports duplicates so a reader can see that too.
#
# Name: provisional, minted 2026-10-06 by lane/agents-md-skills; calef has not ruled on it.
#
# BUGS. Sentence splitting is a regex on ". " and friends, so an abbreviation can split a sentence
# in two; both halves are then looked up separately, which can only make the check stricter. It is
# a one-off proof for one change: nothing runs it, and after a later edit to AGENTS.md it will
# report that edit as missing text, correctly but uselessly.

import glob
import os
import re
import subprocess
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import record_root  # noqa: E402

DEFAULT_BASE = 'cde620b1ddd55b47561f7e35c66ef6f7cb538ed6'

# Sentences of the old AGENTS.md cut on purpose, normalized the way `norm` does (a prefix of the
# sentence is enough). They are the retired word-cap exception's marker and pointer.
# Principle 2 was going to be cut to its rule sentences and was not: design/tenets/three-principles.md
# says its argument and caveats live in AGENTS.md itself, so the tenet does not hold them.
CONDENSED = [
    # The word-cap exception: its marker, and the sentence pointing at it. calef retired the
    # exception on 2026-10-06 (UTC), "Retire the word-cap exception", once the split took the file
    # under the cap, so both went rather than moved.
    '<!-- prose-budget: exception.',
    'Ratified by calef on 2026-09-24 (UTC) at 5,873 words',
    'Reason: this file is nothing but rules',
    'It is a ratified exception to the 3,000-word cap',
]


def norm(text):
    text = re.sub(r'\]\([^)]*\)', ']', text)          # link targets move with the file
    text = re.sub(r'^\s{0,3}#{1,6}\s+', '', text, flags=re.M)   # heading levels change
    text = re.sub(r'^\s*[-*]\s+', '', text, flags=re.M)         # a bullet may become a paragraph
    text = text.replace('*' * 2, '')    # bold is emphasis here, and it hides a sentence's end
    return re.sub(r'\s+', ' ', text).strip()


def sentences(text):
    out = []
    for block in re.split(r'\n\s*\n', text):
        for s in re.split(r'(?<=[.!?:])\s+(?=[A-Z`*(\[<"])', norm(block)):
            if len(s) > 3:
                out.append(s)
    return out


def main():
    base = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_BASE
    root = subprocess.run(['git', 'rev-parse', '--show-toplevel'], capture_output=True, text=True,
                          check=True).stdout.strip()
    old = subprocess.run(['git', '-C', root, 'show', f'{base}:AGENTS.md'], capture_output=True,
                         text=True, check=True).stdout
    new_files = ['AGENTS.md'] + sorted(
        os.path.relpath(p, root) for p in glob.glob(os.path.join(root, 'notes/skills/*/SKILL.md')))
    if os.path.exists(os.path.join(root, 'kernel/CLAUDE.md')):
        new_files.append('kernel/CLAUDE.md')
    texts = {f: norm(open(os.path.join(root, f)).read()) for f in new_files}
    union = ' '.join(texts.values())
    tenets = {p: norm(open(p).read())
              for p in sorted(glob.glob(os.path.join(record_root.path('design', 'tenets', top=root), '*.md')))}

    old_sentences = sentences(old)
    missing, condensed, dup = [], [], []
    used = set()
    for s in old_sentences:
        homes = [f for f, t in texts.items() if s in t]
        if homes:
            if len(s.split()) >= 5 and (len(homes) > 1 or union.count(s) > 1):
                dup.append((s, homes))
            continue
        hit = next((c for c in CONDENSED if s.startswith(c)), None)
        if hit is None:
            missing.append(s)
        else:
            used.add(hit)
            in_tenets = [os.path.relpath(p, root) for p, t in tenets.items() if s in t]
            condensed.append((s, in_tenets))
    stale = [c for c in CONDENSED if c not in used]

    print(f'before: {base[:9]}:AGENTS.md, {len(old.split())} words, {len(old_sentences)} sentences')
    for f in new_files:
        print(f'after:  {f}, {len(open(os.path.join(root, f)).read().split())} words')
    print(f'covered: {len(old_sentences) - len(missing) - len(condensed)} of {len(old_sentences)}')
    print(f'condensed on purpose: {len(condensed)}')
    for s, where in condensed:
        tag = 'also in ' + ', '.join(where) if where else 'in no tenet verbatim'
        print(f'  - [{tag}] {s[:110]}')
    if dup:
        print(f'present more than once: {len(dup)}')
        for s, homes in dup:
            print(f'  - [{", ".join(homes)}] {s[:110]}')
    if missing:
        print(f'MISSING, not condensed on purpose: {len(missing)}')
        for s in missing:
            print(f'  - {s[:140]}')
    if stale:
        print(f'CONDENSED entries that match nothing missing: {len(stale)}')
        for c in stale:
            print(f'  - {c}')
    if missing or stale:
        sys.exit(1)
    print('ok: every sentence of the old AGENTS.md is in the new files or listed as condensed')


if __name__ == '__main__':
    main()
