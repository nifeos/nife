#!/usr/bin/env python3
"""Does this change add or change a Kani harness, so a pull request proves before it queues?

    helpers/harness_change.py decide         # verify.yml's draft gate: writes harness=true|false
    helpers/harness_change.py since <base>   # the same answer for <base>..HEAD, with its reasons
    helpers/harness_change.py --selftest     # script/lint: fixtures in a throwaway repository

Name: provisional, minted by milestone 1118 (the prove shards run on a pull request that changes a
harness) on 2026-11-16 UTC. A helper under `helpers/`, outside `script/names`' scope, so its
provenance is this paragraph.

# THE RULE

The Kani proofs run on a pull request whose diff adds or changes a harness (ruled 2026-11-16 UTC;
notes/merge-queue.md has the rule beside the queue-only set). The answer is `harness=true` when
any of these holds between the pull request's merge base and its head:

- a `#[kani::proof]` or `#[kani::proof_for_contract]` function is added, removed or edited. A
  harness is its attributes and the whole function, compared as Rust tokens with the comments
  removed (helpers/rust_source.py `code_tokens`), so a comment or a blank line inside a harness
  does not count and a changed `#[kani::unwind]` or `#[kani::stub]` beside it does;
- a falsification patch changes: any file under a `falsifications/` directory, which §134 (a harness
  carries its falsification) puts at `<package>/falsifications/<module.path>.<harness>.patch`;
- `script/verify` changes. Its tables say which crates a shard proves, how long each takes, and
  which harnesses prove alone; the rest of the file is how a shard runs them. Telling one from the
  other would mean parsing shell, and either can change what a shard holds in memory.

**It fails toward running.** A base that does not resolve, a git command that fails, a file that
does not lex, or a proof attribute with no function after it answers true. A merge group or a push
answers true without looking, as `helpers/full_ci.py` does; the workflow tests the event name as
well, so those run the proofs whatever this says.

# WHY

A harness that outgrew the hosted runner's memory ran first in the merge queue, where its failure
evicted the entries behind it for about two days (#3567). On its pull request it would have failed
in about a minute. The QEMU suites, fuzzing, the mutation census, the bench and the CPU matrix stay
in the queue; only verify.yml's jobs take this answer.

# BUGS

A change to a function a harness calls, and not to the harness, does not count, nor does a new
dependency the harness's crate takes. Either can grow a proof's formula. The rule follows the
ruling's words; the merge group still proves every such change before it merges.
"""

import collections
import os
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rust_source  # noqa: E402

PROVER = 'script/verify'
PROOF_ATTRIBUTES = ('proof', 'proof_for_contract')


class Unsure(Exception):
    """The helper cannot tell, so the caller runs the proofs."""


# --- the harnesses in one file ---------------------------------------------------------------------

def _norm(tok):
    """A punctuation token without its joint marker (`code_tokens` appends `+` to one)."""
    if len(tok) == 2 and tok[1] == '+' and not rust_source._word(tok[0]) and tok[0] not in '"\'':
        return tok[0]
    return tok


def _close(norm, i, open_, shut):
    """Index of the token closing the bracket at `norm[i]`."""
    depth = 0
    for j in range(i, len(norm)):
        if norm[j] == open_:
            depth += 1
        elif norm[j] == shut:
            depth -= 1
            if depth == 0:
                return j
    raise Unsure(f'an unbalanced {open_}')


def _is_proof(attribute):
    """Does an attribute's body (the tokens inside `#[...]`) name a Kani proof?"""
    for k in range(len(attribute) - 3):
        if attribute[k:k + 3] == ['kani', ':', ':'] and attribute[k + 3] in PROOF_ATTRIBUTES:
            return True
    return False


def harnesses(text):
    """Every harness in `text`: a Counter of (name, tokens), its attributes and its function."""
    try:
        toks = rust_source.code_tokens(text)
    except rust_source.LexError as error:
        raise Unsure(f'does not lex: {error}') from error
    norm = [_norm(t) for t in toks]
    found = collections.Counter()
    i = 0
    while i < len(norm) - 1:
        if not (norm[i] == '#' and norm[i + 1] == '['):
            i += 1
            continue
        shut = _close(norm, i + 1, '[', ']')
        if not _is_proof(norm[i + 2:shut]):
            i = shut + 1
            continue
        # Back over the attributes above it, to the first of its group.
        start = i
        while start >= 2 and norm[start - 1] == ']':
            depth, k = 0, start - 1
            while k >= 0:
                if norm[k] == ']':
                    depth += 1
                elif norm[k] == '[':
                    depth -= 1
                    if depth == 0:
                        break
                k -= 1
            if k >= 1 and norm[k - 1] == '#':
                start = k - 1
            else:
                break
        # Forward to `fn`, past the attributes and qualifiers below it.
        k = shut + 1
        while k < len(norm) and norm[k] != 'fn':
            if norm[k] in ('{', ';', '}'):
                raise Unsure('a proof attribute with no function after it')
            k += 1
        if k + 1 >= len(norm):
            raise Unsure('a proof attribute with no function after it')
        name = norm[k + 1]
        body = k + 2
        while body < len(norm) and norm[body] != '{':
            if norm[body] == ';':
                raise Unsure(f'harness {name} has no body')
            body += 1
        if body >= len(norm):
            raise Unsure(f'harness {name} has no body')
        end = _close(norm, body, '{', '}')
        found[(name, tuple(toks[start:end + 1]))] += 1
        i = end + 1
    return found


# --- the change between two revisions --------------------------------------------------------------

def _git(*args, cwd=None):
    env = {k: v for k, v in os.environ.items() if k not in ('GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE')}
    try:
        return subprocess.run(['git', *args], cwd=cwd, env=env, capture_output=True,
                              text=True, timeout=120, check=True).stdout
    except (OSError, subprocess.SubprocessError) as error:
        raise Unsure(f'git {args[0]} failed: {error}') from error


def _blob(rev, path, cwd):
    """The file at `rev`, or '' when `rev` does not have it."""
    listed = _git('ls-tree', '--name-only', rev, '--', path, cwd=cwd)
    if not listed.strip():
        return ''
    return _git('show', f'{rev}:{path}', cwd=cwd)


def reasons(base, cwd=None):
    """Why `base..HEAD` takes the proofs, one line each; empty when it does not."""
    _git('rev-parse', '--verify', '--quiet', f'{base}^{{commit}}', cwd=cwd)
    paths = [p for p in _git('diff', '--no-renames', '--name-only', base, 'HEAD', cwd=cwd).split('\n') if p]
    out = []
    for path in paths:
        if path == PROVER:
            out.append(f'{path}: the prover and its shard tables changed')
        elif 'falsifications' in path.split('/')[:-1]:
            out.append(f'{path}: a falsification patch changed')
        elif path.endswith('.rs'):
            old, new = _blob(base, path, cwd), _blob('HEAD', path, cwd)
            if 'kani' not in old and 'kani' not in new:
                continue
            try:
                before, after = harnesses(old), harnesses(new)
            except Unsure as error:
                out.append(f'{path}: cannot tell ({error}), so it counts')
                continue
            if before != after:
                names = sorted({name for name, _ in (before - after) + (after - before)})
                out.append(f'{path}: harness added, changed or removed: {", ".join(names)}')
    return out


def _base_from_env():
    """The base verify.yml's other steps diff against, resolved the same way."""
    here = os.path.dirname(os.path.abspath(__file__))
    try:
        base = subprocess.run(
            [os.path.join(here, 'scope-merge-base.sh'),
             os.environ.get('BASE_REF', ''), os.environ.get('BASE_SHA', '')],
            capture_output=True, text=True, timeout=120).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        base = ''
    if not base:
        try:
            base = _git('merge-base', 'HEAD', 'origin/main').strip()
        except Unsure:
            base = ''
    if not base:
        raise Unsure('no base resolved')
    return base


def decide(event, base_lookup, reasons_lookup):
    """(harness, lines) for one run. `base_lookup` and `reasons_lookup` raise Unsure to fail."""
    if event not in ('pull_request', 'workflow_dispatch'):
        return True, [f'{event or "an unnamed event"} proves without asking']
    try:
        base = base_lookup()
        found = reasons_lookup(base)
    except Unsure as error:
        return True, [f'cannot tell ({error}), so the proofs run']
    except Exception as error:  # noqa: BLE001 -- any surprise fails toward running
        return True, [f'cannot tell ({type(error).__name__}: {error}), so the proofs run']
    if found:
        return True, [f'against {base}:'] + found
    return False, [f'against {base}: no harness, falsification patch or prover table changed']


def main_decide():
    harness, lines = decide(os.environ.get('EVENT_NAME', ''), _base_from_env, reasons)
    for line in lines:
        print(f'==> {line}')
    word = 'true' if harness else 'false'
    print(f"==> the Kani proofs: {'run on this pull request' if harness else 'skipped; the merge queue runs them'} (harness={word})")
    path = os.environ.get('GITHUB_OUTPUT')
    if path:
        with open(path, 'a', encoding='utf-8') as out:
            out.write(f'harness={word}\n')
    return 0


def main_since(base):
    harness, lines = decide('pull_request', lambda: base, reasons)
    for line in lines:
        print(line)
    print(f'harness={"true" if harness else "false"}')
    return 0


# --- selftest --------------------------------------------------------------------------------------

HARNESS = '''\
pub fn add(a: u8, b: u8) -> u8 { a.wrapping_add(b) }

#[cfg(kani)]
mod verification {
    use super::*;

    // Falsification: unfalsified
    #[kani::proof]
    #[kani::unwind(4)]
    fn add_is_commutative() {
        let (a, b): (u8, u8) = (kani::any(), kani::any());
        assert_eq!(add(a, b), add(b, a));
    }

    fn helper() -> u8 { 1 }
}
'''


def selftest():
    cases = []

    def check(name, ok):
        cases.append((name, ok))

    # The extraction itself.
    one = harnesses(HARNESS)
    check('one harness is found, with its name', [n for n, _ in one] == ['add_is_commutative'])
    check('a harness with its attribute group starts at the first attribute',
          next(iter(one))[1][0].startswith('#'))
    check('a comment inside a harness is not a change',
          harnesses(HARNESS.replace('let (a, b)', '// a comment\n        let (a, b)')) == one)
    check('a changed unwind attribute is a change',
          harnesses(HARNESS.replace('unwind(4)', 'unwind(8)')) != one)
    check('an attribute above proof is part of the harness',
          harnesses(HARNESS.replace('    #[kani::proof]', '    #[kani::stub(a, b)]\n    #[kani::proof]')) != one)
    check('a brace in a string does not end the harness',
          len(harnesses(HARNESS.replace('add(b, a));', 'add(b, a), "}}");'))) == 1)
    check('proof_for_contract is a harness',
          len(harnesses(HARNESS.replace('kani::proof]', 'kani::proof_for_contract(add)]'))) == 1)
    try:
        harnesses('#[kani::proof]\n')
        check('a proof attribute with nothing after it is unsure', False)
    except Unsure:
        check('a proof attribute with nothing after it is unsure', True)
    try:
        harnesses('#[kani::proof]\nfn f() { "unterminated }\n')
        check('a file that does not lex is unsure', False)
    except Unsure:
        check('a file that does not lex is unsure', True)

    # The decision, over a throwaway repository.
    with tempfile.TemporaryDirectory(prefix='harness-change-') as repo:
        def write(path, text):
            full = os.path.join(repo, path)
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, 'w', encoding='utf-8') as out:
                out.write(text)

        def commit():
            _git('add', '-A', cwd=repo)
            _git('-c', 'user.name=selftest', '-c', 'user.email=selftest@invalid',
                 'commit', '-q', '--allow-empty', '-m', 'fixture', cwd=repo)
            return _git('rev-parse', 'HEAD', cwd=repo).strip()

        def verdict(edit):
            base = _git('rev-parse', 'HEAD', cwd=repo).strip()
            edit()
            commit()
            harness, _ = decide('pull_request', lambda: base, lambda b: reasons(b, cwd=repo))
            return harness

        _git('init', '-q', '-b', 'main', cwd=repo)
        write('crates/add/src/lib.rs', HARNESS)
        write('crates/add/falsifications/verification.add_is_commutative.patch', 'patch\n')
        write('script/verify', 'CRATES="add\t1"\n')
        write('crates/other/src/lib.rs', 'pub fn f() {}\n')
        write('notes/a.md', 'prose\n')
        commit()

        check('a prose change takes no proofs',
              verdict(lambda: write('notes/a.md', 'more prose\n')) is False)
        check('a change to a crate with no harness takes no proofs',
              verdict(lambda: write('crates/other/src/lib.rs', 'pub fn f() { let _ = 1; }\n')) is False)
        check('a change beside a harness, not in it, takes no proofs',
              verdict(lambda: write('crates/add/src/lib.rs',
                                    HARNESS.replace('fn helper() -> u8 { 1 }', 'fn helper() -> u8 { 2 }'))) is False)
        check('a comment edit inside a harness takes no proofs',
              verdict(lambda: write('crates/add/src/lib.rs',
                                    HARNESS.replace('fn helper() -> u8 { 1 }', 'fn helper() -> u8 { 2 }')
                                    .replace('let (a, b)', '// why\n        let (a, b)'))) is False)
        check('an edited harness takes the proofs',
              verdict(lambda: write('crates/add/src/lib.rs', HARNESS.replace('add(b, a)', 'add(a, b)'))) is True)
        check('an added harness takes the proofs',
              verdict(lambda: write('crates/add/src/lib.rs', HARNESS.replace(
                  '    fn helper()', '    #[kani::proof]\n    fn second() { assert!(true); }\n\n    fn helper()'))) is True)
        check('a removed harness takes the proofs',
              verdict(lambda: write('crates/add/src/lib.rs', 'pub fn add(a: u8, b: u8) -> u8 { a }\n')) is True)
        check('a harness in a new file takes the proofs',
              verdict(lambda: write('crates/other/src/proofs.rs', HARNESS)) is True)
        check('a changed falsification patch takes the proofs',
              verdict(lambda: write('crates/add/falsifications/verification.add_is_commutative.patch',
                                    'patch two\n')) is True)
        check('a changed prover table takes the proofs',
              verdict(lambda: write('script/verify', 'CRATES="add\t2"\n')) is True)
        check('an unlexable file that names kani takes the proofs',
              verdict(lambda: write('crates/other/src/broken.rs', '// kani\nfn f() { "open\n')) is True)
        check('a base that is not a commit takes the proofs',
              decide('pull_request', lambda: 'f' * 40, lambda b: reasons(b, cwd=repo))[0] is True)
    check('no base takes the proofs',
          decide('pull_request', lambda: (_ for _ in ()).throw(Unsure('none')), reasons)[0] is True)
    check('a surprise in the lookup takes the proofs',
          decide('pull_request', lambda: 'x', lambda b: 1 / 0)[0] is True)
    check('a merge group proves without asking', decide('merge_group', None, None)[0] is True)
    check('a push proves without asking', decide('push', None, None)[0] is True)

    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f'harness_change selftest FAILED: {name}', file=sys.stderr)
    print(f'harness_change selftest: {len(cases) - len(failed)}/{len(cases)} passed')
    return 1 if failed else 0


if __name__ == '__main__':
    args = sys.argv[1:]
    if args == ['--selftest']:
        sys.exit(selftest())
    if args == ['decide']:
        sys.exit(main_decide())
    if len(args) == 2 and args[0] == 'since':
        sys.exit(main_since(args[1]))
    print(__doc__.split('\n\n')[1], file=sys.stderr)
    sys.exit(2)
