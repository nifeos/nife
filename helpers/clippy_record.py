"""Run one `cargo clippy` section and keep its diagnostics, so the lint ratchet need not run clippy again.

Built by milestone 980 (script/lint stops paying per-item costs), part 6.

    python3 helpers/clippy_record.py NAME cargo clippy ARGS -- RUSTC-FLAGS     # what script/lint runs
    python3 helpers/clippy_record.py --selftest                                # the recorder, against a stub cargo

Name: provisional, minted by milestone 980's part 6 lane on 2026-10-31 (UTC). A shared module under
`helpers/`, which `script/names` puts out of its own scope, so its provenance is this paragraph.

**What it does.** It runs the command it is given with `--message-format=json` and the lint
ratchet's flags (`python3 helpers/lint_ratchet.py --flags`) added after the section's own, writes
clippy's JSON to `target/lint-clippy/NAME.json`, prints each diagnostic's rendered text to stderr as
the section always printed it, and exits with cargo's status. `NAME` is a path under that
directory without its extension (`host`, `fuzz/clippy`); a nested workspace's file sits at its own
path, which is how `helpers/lint_ratchet.py` puts its workspace-relative paths back.

**Beside the JSON, a sidecar** (`NAME.meta`, JSON): the cargo arguments, the rustc flags as run
(the ratchet's included), when the run started, cargo's exit status, and the JSON's SHA-256. The
ratchet refuses a file whose sidecar is missing, whose hash does not match, whose flags are not the
ratchet's, or which started before the newest input of the tree. The sidecar is written last, so a
run that died leaves a JSON no ratchet will trust. Both files are deleted before the command
starts, so a stale pair never survives a failed launch.

**The section's verdict does not change.** The ratchet's lint is passed as `--force-warn`, which a
`-D warnings` does not turn into an error (checked on the pinned toolchain), so the section fails on
exactly what it failed on before. Its findings are the ratchet's and not the section's, so they are
left out of the printed output and counted in one line. A source `#[allow(clippy::too_many_lines)]`
cannot hide a function from the ratchet either, which `-W` would have allowed.

`NIFE_LINT_CLIPPY_DIR` overrides the directory (the selftest uses it).
"""

import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import threading
import time

DEFAULT_DIR = 'target/lint-clippy'
# cargo's own summary of a crate's warnings; under `-D warnings` the only warnings left are the
# ratchet's force-warned lints, so the line would count findings the section does not print.
SUMMARY = re.compile(r'^warning: `[^`]+` \(.*\) generated \d+ warnings?\b')


def saved_dir():
    return os.environ.get('NIFE_LINT_CLIPPY_DIR') or DEFAULT_DIR


def ratchet_flags(root):
    out = subprocess.run([sys.executable, os.path.join(root, 'helpers', 'lint_ratchet.py'),
                          '--flags'], capture_output=True, text=True, check=True).stdout
    return out.split()


def lint_names(flags):
    """The lints the flags force-warn, as clippy's rendered text names them."""
    return {flags[i + 1].split('::')[-1] for i, f in enumerate(flags[:-1]) if f == '--force-warn'}


def split_command(command):
    """(cargo arguments, rustc flags) around the first `--`."""
    if '--' in command:
        i = command.index('--')
        return command[:i], command[i + 1:]
    return command, []


def pump_stderr(pipe, sink):
    for raw in iter(pipe.readline, b''):
        line = raw.decode('utf-8', 'replace')
        if not SUMMARY.match(line):
            sink.write(line)
            sink.flush()
    pipe.close()


def record(name, command, flags, directory, out=None, err=None):
    """Run `command` with the flags added; return its exit status."""
    out = out or sys.stdout
    err = err or sys.stderr
    base = os.path.join(directory, name)
    os.makedirs(os.path.dirname(base) or '.', exist_ok=True)
    for ext in ('.json', '.meta'):
        if os.path.exists(base + ext):
            os.remove(base + ext)
    cargo_args, rustc_flags = split_command(command)
    ran = cargo_args + ['--message-format=json', '--'] + rustc_flags + flags
    quiet = lint_names(flags)
    started = time.time_ns()
    proc = subprocess.Popen(ran, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    thread = threading.Thread(target=pump_stderr, args=(proc.stderr, err))
    thread.start()
    digest = hashlib.sha256()
    held = 0
    with open(base + '.json.partial', 'wb') as f:
        for raw in iter(proc.stdout.readline, b''):
            f.write(raw)
            digest.update(raw)
            line = raw.decode('utf-8', 'replace')
            try:
                message = json.loads(line)
            except ValueError:
                out.write(line)
                continue
            if message.get('reason') != 'compiler-message':
                continue
            msg = message['message']
            code = ((msg.get('code') or {}).get('code') or '').split('::')[-1]
            if code in quiet:
                held += 1
            elif msg.get('rendered'):
                err.write(msg['rendered'])
    status = proc.wait()
    thread.join()
    os.replace(base + '.json.partial', base + '.json')
    meta = {'name': name, 'cargo_args': cargo_args[1:],
            'rustc_flags': rustc_flags + flags, 'started_ns': started, 'exit': status,
            'sha256': digest.hexdigest()}
    with open(base + '.meta.partial', 'w') as f:
        json.dump(meta, f, sort_keys=True)
    os.replace(base + '.meta.partial', base + '.meta')
    if held:
        err.write(f'clippy_record: {held} finding(s) of {", ".join(sorted(quiet))} kept in '
                  f'{base}.json for the lint ratchet\n')
    return status


def selftest():
    failed = []

    def check(name, ok):
        if not ok:
            failed.append(name)

    with tempfile.TemporaryDirectory() as d:
        stub = os.path.join(d, 'cargo')
        lint = json.dumps({'reason': 'compiler-message', 'message': {
            'code': {'code': 'clippy::too_many_lines'}, 'rendered': 'RATCHET-FINDING\n',
            'spans': []}})
        other = json.dumps({'reason': 'compiler-message', 'message': {
            'code': {'code': 'clippy::len_zero'}, 'rendered': 'SECTION-FINDING\n', 'spans': []}})
        done = json.dumps({'reason': 'build-finished', 'success': True})
        with open(stub, 'w') as f:
            f.write('#!/bin/sh\necho "$@" > "$STUB_ARGS"\ncat <<EOF\n' + lint + '\n' + other + '\n'
                    + done + '\nnot json\nEOF\n'
                    'echo \'warning: `x` (lib) generated 1 warning\' >&2\n'
                    'echo "kept on stderr" >&2\nexit "${STUB_EXIT:-0}"\n')
        os.chmod(stub, 0o755)
        saved = os.path.join(d, 'saved')
        os.environ['STUB_ARGS'] = os.path.join(d, 'args')
        flags = ['--force-warn', 'clippy::too_many_lines']

        class Sink:
            def __init__(self):
                self.text = ''

            def write(self, s):
                self.text += s

            def flush(self):
                pass

        o, e = Sink(), Sink()
        status = record('nested/clippy', [stub, 'clippy', '-p', 'x', '--', '-D', 'warnings'],
                        flags, saved, o, e)
        base = os.path.join(saved, 'nested/clippy')
        check('the exit status is cargo\'s', status == 0)
        check('the section\'s own finding is printed', 'SECTION-FINDING' in e.text)
        check('the ratchet\'s finding is held back and counted',
              'RATCHET-FINDING' not in e.text and '1 finding(s)' in e.text)
        check('cargo\'s warning summary and other stderr pass as described',
              'generated' not in e.text and 'kept on stderr' in e.text)
        check('a line that is not JSON goes to stdout', o.text == 'not json\n')
        with open(os.environ['STUB_ARGS']) as f:
            args = f.read().split()
        check('the arguments are json, then the section\'s flags, then the ratchet\'s',
              args == ['clippy', '-p', 'x', '--message-format=json', '--', '-D', 'warnings']
              + flags)
        with open(base + '.json', 'rb') as f:
            body = f.read()
        with open(base + '.meta') as f:
            meta = json.load(f)
        check('the sidecar holds the hash, the flags and the status',
              meta['sha256'] == hashlib.sha256(body).hexdigest() and meta['exit'] == 0
              and meta['rustc_flags'] == ['-D', 'warnings'] + flags
              and meta['cargo_args'] == ['clippy', '-p', 'x']
              and meta['started_ns'] > 0)
        check('the JSON keeps every line', body.count(b'\n') == 4)
        os.environ['STUB_EXIT'] = '3'
        check('a failing command\'s status is passed on',
              record('nested/clippy', [stub, 'clippy', '--', '-D', 'warnings'], flags, saved,
                     Sink(), Sink()) == 3)
        del os.environ['STUB_EXIT']
    for name in failed:
        print(f'clippy_record selftest: {name}', file=sys.stderr)
    if not failed:
        print('clippy_record selftest: 9 cases pass')
    return 1 if failed else 0


def main(argv):
    if argv[:1] == ['--selftest']:
        return selftest()
    if len(argv) < 3 or argv[1] != 'cargo':
        print(__doc__.split('\n\n')[2], file=sys.stderr)
        return 2
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    os.chdir(root)
    return record(argv[0], argv[1:], ratchet_flags(root), saved_dir())


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
