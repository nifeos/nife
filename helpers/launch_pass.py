"""The outsider-pass launcher (`script/launch-pass`): one reviewer, pinned, with a record of what ran.

Milestone 920 (an outsider pass is launched pinned and floor-checked). Pass 8 was started with a
milestone number as its prompt and read the lane's instructions as its own; pass 9 was configured
for one model and served by another, which nobody saw until it had finished. This makes both
mistakes impossible to make quietly.

    script/launch-pass --reviewer glm --commit SHA      # GLM 5.3 through opencode
    script/launch-pass --reviewer fable --commit SHA    # Claude Fable through Claude Code
    script/launch-pass ... --dry-run                    # the clone and the prompt, no session
    script/launch-pass --complete RECORD --short-rows FILE   # the one completion round
    script/launch-pass --selftest

Name: provisional, minted by milestone 920's lane on 2026-10-14 (UTC). A shared python module
under `helpers/`, which `script/names` puts out of its own scope, so its provenance is this
paragraph; the entry point's `Name:` block is in `script/launch-pass`.

**Where it runs** (calef, 2026-10-14 UTC, asked "What if we launched a container on patagonia for
the test?": "Yes"). Both harnesses run in a container inside podman's Linux VM on patagonia: the
image `script/runner-container` builds, with the two harnesses added at the host's own versions
(`.github/runner-container/Containerfile.outsider-pass`). The container sees the standalone clone
(read-write) and a results directory, and nothing else of the host: no home directory, SSH key,
`gh` token or MCP configuration. The model's API key is its one secret, passed by name from the
launcher's environment so its value is never on a command line; use a key made for the pass, which
its owner revokes afterward. Outbound network is allowed, for the model's API. The harness's
session state lives in a podman volume named for the pass, kept until the lane has collected, so
`--complete` resumes the same session in a new container run. Before anything starts, the podman
machine must be up with at least 6 CPUs and 8 GiB; the launcher refuses with the exact
`podman machine set` command otherwise, and never resizes or restarts the machine itself.

**What one launch does, in order.** A standalone clone with no remote and one branch, `pass`, at
the pinned commit. The prompt: the Mac's runner text from `briefs/confinement-outsider-pass-launch.md`
(its `### GLM 5.3 on patagonia` heredoc, word for word) and the reviewer's brief from
`briefs/confinement-outsider-pass.md`, placeholders filled as that launch brief's `sed` fills them
with the container's paths, and refused if it names a milestone or a roadmap block. Then the
volume is provisioned (`script/ci-qemu` and `script/bootstrap`, in the clone, no model), and a
setup turn starts a new session with the model pinned on the harness's command line and no tools:
the launcher reads back the model the session reports and stops there, before the brief is ever
sent, if it differs from the pin. Only then does the same session get the prompt. The launch record
(`launch.json`, beside the clone and outside the container) holds the harness and its version, the
image and its digest, the volume, the model pinned and reported, the session id, the commit, the
prompt's SHA-256 and the times.

**The completion round** resumes that session once with the checker's short-row lines as the whole
message, and writes to the record before it sends, so a second `--complete` is refused even after
a crash.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import uuid

REVIEWERS = {
    'glm': {'harness': 'opencode', 'model': 'zai-coding-plan/glm-5.3', 'key': 'ZHIPU_API_KEY',
            'trailer': 'GLM 5.3 <noreply@z.ai>'},
    'fable': {'harness': 'claude', 'model': 'claude-fable-5-1', 'key': 'ANTHROPIC_API_KEY',
              'trailer': 'Claude Fable 5.1 <noreply@anthropic.com>'},
}
# The container (calef's ruling above). Paths inside it are fixed, so the prompt names them.
BASE_IMAGE = 'nife-runner-container'
PASS_IMAGE = 'nife-outsider-pass'
CONTAINERFILE = '.github/runner-container/Containerfile.outsider-pass'
INSIDE = '/pass'
MIN_CPUS, MIN_MEMORY_MIB = 6, 8192
PROBE = ('This is a setup check before your task arrives. Reply with the single word READY. '
         'Run no tool and read no file.')
# What a prompt may never be: the lane's own instructions. A reviewer handed a milestone number or a
# roadmap block follows the lane's brief instead of its own (pass 8).
REFUSED = (
    (re.compile(r'(?i)\bmilestones?\s+\d+'), 'names a milestone number'),
    (re.compile(r'design/roadmap/'), 'names a roadmap block'),
    (re.compile(r'(?m)^(milestone_dependencies|decision_dependencies|needs_person):'),
     'holds a roadmap block\'s frontmatter'),
    (re.compile(r'^\s*\d+\s*$'), 'is a bare milestone number'),
)
# The runner text is the launch brief's, unchanged (calef, 2026-10-14 UTC): the Mac's section,
# which both reviewers get, since both run on patagonia, in a container there.
RUNNER_HEADING = '### GLM 5.3 on patagonia'
RUNNER_TRAILER = 'GLM 5.3 <noreply@z.ai>'
SHORT_ROW = re.compile(r'^row \d+: (?!booted|host \(exempt\)).+$')


def check_prompt(text):
    """Refuse a prompt that is not the reviewer's brief and the runner section."""
    for pattern, why in REFUSED:
        if pattern.search(text):
            raise SystemExit('launch-pass: refused: the prompt %s; it must be the reviewer\'s brief '
                             'and the runner section, nothing else' % why)
    for heading in ('## Before you start', '### Your job', '### Output'):
        if heading not in text:
            raise SystemExit('launch-pass: refused: the prompt has no `%s`' % heading)


def section(text, heading):
    """The body under one `## ` heading, up to the next heading of the same level."""
    found = re.search(r'(?ms)^%s\n(.*?)(?=^## |\Z)' % re.escape(heading), text)
    if not found:
        raise SystemExit('launch-pass: no `%s` section' % heading)
    return found.group(1).strip() + '\n'


def runner_section(launch_text, scratch, commit, trailer):
    """The Mac's runner section, exactly as the launch brief's heredoc writes it, values filled.

    The text is the `cat <<EOF` body under RUNNER_HEADING, read as the shell would expand it: the
    escaped backticks unescaped and `$scratch` and `$sha` filled. Not a word of it is the launcher's.
    Its commit trailer names GLM 5.3, the reviewer that section was written for, and is swapped for
    the reviewer's own, so a Fable session on the same Mac signs its commits as itself.
    """
    start = launch_text.find('\n%s\n' % RUNNER_HEADING)
    fence = re.compile(r'(?ms)^```sh\n(.*?)^```$').search(launch_text, max(start, 0))
    body = fence and re.search(r'(?ms)^    cat <<EOF\n(.*?)^EOF$', fence.group(1))
    if start < 0 or not body:
        raise SystemExit('launch-pass: no `cat <<EOF` runner text under `%s`' % RUNNER_HEADING)
    text = body.group(1).replace('\\`', '`').replace('$scratch', scratch).replace('$sha', commit)
    if '$' in text or '\\' in text or text.count(RUNNER_TRAILER) != 1:
        raise SystemExit('launch-pass: the runner text under `%s` has changed shape; read it and '
                         'update the launcher' % RUNNER_HEADING)
    return text.replace(RUNNER_TRAILER, trailer).rstrip() + '\n'


def assemble(tree, inside, commit, reviewer):
    """The one prompt: the runner section, then the reviewer's brief, placeholders filled.

    The fills are the launch brief's own `sed` line for the Mac, plus `<logs>` and `<commit>`,
    which the brief's Mechanics added for the floor checker. `inside` is the container's view of
    the pass's directory: the clone at `nife/`, and the results file and logs under `results/`,
    the one other mount.
    """
    with open(os.path.join(tree, 'briefs/confinement-outsider-pass.md')) as brief, \
            open(os.path.join(tree, 'briefs/confinement-outsider-pass-launch.md')) as launch:
        reviewer_brief = section(brief.read(), "## The reviewer's brief")
        runner = runner_section(launch.read(), inside, commit, reviewer['trailer'])
    checkout, results = inside + '/nife', inside + '/results'
    fill = {'<path>': checkout + '/notes/confinement-claims.md, under "## The claims"',
            '<worktree>': checkout, '<results path>': results + '/results.md',
            '<logs>': results + '/logs', '<commit>': commit}
    for placeholder, value in fill.items():
        reviewer_brief = reviewer_brief.replace(placeholder, value)
    return runner + '\n' + reviewer_brief




def git(*args, cwd=None):
    done = subprocess.run(['git', *args], cwd=cwd, capture_output=True, text=True)
    if done.returncode != 0:
        raise SystemExit('launch-pass: git %s: %s' % (' '.join(args), done.stderr.strip()))
    return done.stdout.strip()


def make_clone(tree, scratch, commit):
    """A standalone clone with no remote and one branch, `pass`, at the pinned commit."""
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise SystemExit('launch-pass: --commit wants a full 40-character SHA, not %r' % commit)
    source = git('rev-parse', '--path-format=absolute', '--git-common-dir', cwd=tree)
    git('cat-file', '-e', commit + '^{commit}', cwd=tree)
    clone = os.path.join(scratch, 'nife')
    git('clone', '--quiet', '--no-hardlinks', source, clone)
    git('remote', 'remove', 'origin', cwd=clone)
    git('switch', '--quiet', '-c', 'pass', commit, cwd=clone)
    for branch in git('for-each-ref', '--format=%(refname:short)', 'refs/heads', cwd=clone).split():
        if branch != 'pass':
            git('branch', '--quiet', '-D', branch, cwd=clone)
    os.makedirs(os.path.join(scratch, 'results', 'logs'), exist_ok=True)
    return clone


def run_command(argv, log_path, cwd):
    """Run one command, append everything it prints to the log, and return its status and stdout."""
    with open(log_path, 'a') as log:
        done = subprocess.run(argv, cwd=cwd, stdout=subprocess.PIPE, stderr=log, text=True)
        log.write(done.stdout)
    return done.returncode, done.stdout


def events(stdout):
    """The JSON objects in a harness's event stream, one per line; anything else is skipped."""
    found = []
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if isinstance(event, dict):
            found.append(event)
    return found


# ----------------------------------------------------------------------------------------------
# The container: the machine's floor, the image, and one pass's run.
# ----------------------------------------------------------------------------------------------

# Run in the clone before the setup turn, with no model: the pinned QEMU and the toolchain, built
# into the pass's volume (the image's HOME, RUSTUP_HOME and CARGO_HOME are all under /cache).
PROVISION = 'mkdir -p "$HOME" && script/ci-qemu && NIFE_SKIP_LINT_TOOLS=1 script/bootstrap'


def podman():
    return os.environ.get('LAUNCH_PASS_PODMAN', 'podman')


def podman_out(*args):
    done = subprocess.run([podman(), *args], capture_output=True, text=True)
    return done.returncode, done.stdout.strip()


def check_machine():
    """Refuse unless the podman machine answers and has the CPUs and memory a pass needs.

    It never resizes or restarts the machine: other work may be running in it, so that is a
    person's call, made when a pass is launched.
    """
    status, out = podman_out('machine', 'inspect')
    machines = json.loads(out) if status == 0 and out.startswith('[') else []
    if not machines:
        raise SystemExit('launch-pass: refused: `podman machine inspect` found no machine')
    machine = machines[0]
    name = machine.get('Name', 'podman-machine-default')
    cpus = machine.get('Resources', {}).get('CPUs', 0)
    memory = machine.get('Resources', {}).get('Memory', 0)
    if cpus < MIN_CPUS or memory < MIN_MEMORY_MIB:
        raise SystemExit(
            'launch-pass: refused: the podman machine %s has %d CPUs and %d MiB, and a pass needs at '
            'least %d and %d. Resizing restarts the machine, so check what else is running in it '
            'first, then:\n    podman machine stop %s && podman machine set --cpus %d --memory %d %s '
            '&& podman machine start %s'
            % (name, cpus, memory, MIN_CPUS, MIN_MEMORY_MIB, name, max(cpus, MIN_CPUS),
               max(memory, MIN_MEMORY_MIB), name, name))
    if machine.get('State') != 'running' or podman_out('info')[0] != 0:
        raise SystemExit('launch-pass: refused: the podman machine %s is not answering; start it '
                         'with `podman machine start %s`' % (name, name))


def host_version(binary):
    """The harness's version on the host. The image installs the same, so both run one release."""
    done = subprocess.run([binary, '--version'], capture_output=True, text=True)
    words = done.stdout.split()
    if done.returncode != 0 or not words:
        raise SystemExit('launch-pass: refused: `%s --version` gave no version to pin the image to'
                         % binary)
    return words[0]


def build_image(tree):
    """The outsider-pass image, on `script/runner-container`'s; returns the versions and digest."""
    if podman_out('image', 'exists', BASE_IMAGE)[0] != 0:
        raise SystemExit('launch-pass: refused: there is no %s image; build it with '
                         '`script/runner-container --build-only`' % BASE_IMAGE)
    versions = {'CLAUDE_CODE_VERSION': host_version(Claude().host),
                'OPENCODE_VERSION': host_version(Opencode().host)}
    argv = [podman(), 'build', '-t', PASS_IMAGE, '-f', os.path.join(tree, CONTAINERFILE)]
    for name, version in sorted(versions.items()):
        argv += ['--build-arg', '%s=%s' % (name, version)]
    if subprocess.run(argv + [os.path.join(tree, '.github/runner-container')]).returncode != 0:
        raise SystemExit('launch-pass: `podman build` of %s failed' % PASS_IMAGE)
    return versions, podman_out('image', 'inspect', '--format', '{{.Id}}', PASS_IMAGE)[1]


class Box:
    """One pass's container runs: the image, the pass's own volume, the clone and the results.

    The image is named by its digest, so every run of one pass, the completion round included, is
    the same harness even if a later launch rebuilds the tag.
    """

    def __init__(self, scratch, key, image, volume=None):
        self.scratch, self.key, self.image = scratch, key, image
        self.volume = volume or 'nife-pass-' + os.path.basename(os.path.normpath(scratch))

    def argv(self, inner):
        # The whole of what the container is given. `-e NAME` with no value copies the launcher's
        # own variable, so the key's value is never on a command line. The volume holds the
        # harness's session state and the built QEMU; `:z` because the machine runs SELinux
        # (script/runner-container says how that shows). The default network: the model's API.
        return [podman(), 'run', '--rm',
                '-v', '%s:%s/nife' % (os.path.join(self.scratch, 'nife'), INSIDE),
                '-v', '%s:%s/results' % (os.path.join(self.scratch, 'results'), INSIDE),
                '-v', '%s:/cache:z' % self.volume,
                '-e', self.key, '-e', 'IS_SANDBOX=1', '-e', 'DISABLE_AUTOUPDATER=1',
                '-w', INSIDE + '/nife', self.image, *inner]

    def run(self, inner, log):
        return run_command(self.argv(inner), log, self.scratch)


class Claude:
    """Claude Code headless: `init` names the session and model; each assistant message, the model."""
    name = 'claude'
    version_arg = 'CLAUDE_CODE_VERSION'
    # The container is the fence (calef, 2026-10-14 UTC). Inside it the reviewer still gets the
    # shell and the files and nothing else: no web tool, no MCP server, no settings but the tree's.
    TOOLS = 'Bash,Read,Edit,Write,Glob,Grep'
    FENCE = ('--strict-mcp-config', '--setting-sources', 'project')

    def __init__(self):
        self.host = os.environ.get('LAUNCH_PASS_CLAUDE', 'claude')

    @staticmethod
    def models(stdout):
        found = set()
        for event in events(stdout):
            if event.get('type') == 'system' and event.get('subtype') == 'init':
                found.add(event.get('model'))
            if event.get('type') == 'assistant':
                found.add(event.get('message', {}).get('model'))
        return {m for m in found if m}

    def probe(self, model, box, log):
        session = str(uuid.uuid4())
        inner = ['claude', '-p', '--model', model, '--session-id', session, '--tools', '',
                 *self.FENCE, '--output-format', 'stream-json', '--verbose', PROBE]
        _, out = box.run(inner, log)
        return session, self.models(out)

    def send(self, session, model, message, box, log):
        inner = ['claude', '-p', '--resume', session, '--model', model, '--tools', self.TOOLS,
                 *self.FENCE, '--permission-mode', 'bypassPermissions', '--output-format',
                 'stream-json', '--verbose', message]
        status, out = box.run(inner, log)
        return status, self.models(out)


class Opencode:
    """opencode headless: the event stream names the session; its exported record, the model."""
    name = 'opencode'
    version_arg = 'OPENCODE_VERSION'

    def __init__(self):
        self.host = os.environ.get('LAUNCH_PASS_OPENCODE', 'opencode')

    @staticmethod
    def models(session, box, log):
        _, out = box.run(['opencode', 'export', session], log)
        start = out.find('{')
        record = json.loads(out[start:]) if start >= 0 else {}
        return {'%s/%s' % (m['info'].get('providerID'), m['info'].get('modelID'))
                for m in record.get('messages', []) if m.get('info', {}).get('role') == 'assistant'}

    def probe(self, model, box, log):
        inner = ['opencode', 'run', '-m', model, '--dir', INSIDE + '/nife', '--format', 'json', PROBE]
        _, out = box.run(inner, log)
        sessions = {e.get('sessionID') or e.get('part', {}).get('sessionID') for e in events(out)}
        sessions.discard(None)
        if len(sessions) != 1:
            raise SystemExit('launch-pass: opencode named %d sessions in its setup turn' % len(sessions))
        session = sessions.pop()
        return session, self.models(session, box, log)

    def send(self, session, model, message, box, log):
        inner = ['opencode', 'run', '--session', session, '-m', model, '--auto', '--dir',
                 INSIDE + '/nife', '--format', 'json', message]
        status, _ = box.run(inner, log)
        return status, self.models(session, box, log)


HARNESSES = {'claude': Claude, 'opencode': Opencode}


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


def write_record(path, record):
    with open(path, 'w') as out:
        json.dump(record, out, indent=2, sort_keys=True)
        out.write('\n')


def require_key(name):
    if not os.environ.get(name):
        raise SystemExit('launch-pass: refused: %s is not set. Export a key made for this pass, which '
                         'its owner revokes once the pass is collected; it is the container\'s one '
                         'secret' % name)


def launch(args):
    reviewer = dict(REVIEWERS[args.reviewer])
    reviewer['model'] = args.model or reviewer['model']
    given = None
    if args.prompt:
        with open(args.prompt) as source:
            given = source.read()
        check_prompt(given)
    if not args.dry_run:
        require_key(reviewer['key'])
        check_machine()
        versions, digest = build_image(args.tree)
    scratch = os.path.abspath(args.scratch or tempfile.mkdtemp(prefix='nife-review-'))
    clone = make_clone(args.tree, scratch, args.commit)
    prompt = given or assemble(args.tree, INSIDE, args.commit, reviewer)
    check_prompt(prompt)
    with open(os.path.join(scratch, 'prompt.md'), 'w') as out:
        out.write(prompt)
    print('launch-pass: clone %s at %s; prompt %s' % (clone, args.commit, os.path.join(scratch, 'prompt.md')))
    if args.dry_run:
        return 0
    harness = HARNESSES[reviewer['harness']]()
    box = Box(scratch, reviewer['key'], digest)
    if podman_out('volume', 'create', box.volume)[0] != 0:
        raise SystemExit('launch-pass: refused: could not create the volume %s; is it a past '
                         'pass\'s, not yet collected?' % box.volume)
    record = {'harness': harness.name, 'harness_version': versions[harness.version_arg],
              'image': PASS_IMAGE, 'image_digest': digest, 'volume': box.volume,
              'key_variable': reviewer['key'], 'model_pinned': reviewer['model'],
              'commit': args.commit, 'clone': clone, 'started': now(),
              'prompt_sha256': hashlib.sha256(prompt.encode()).hexdigest()}
    return attack(record, harness, box, prompt, scratch)


def attack(record, harness, box, prompt, scratch):
    """Provision the volume, read the model back in a setup turn, and only then send the brief."""
    record_path = os.path.join(scratch, 'launch.json')
    if box.run(['sh', '-c', PROVISION], os.path.join(scratch, 'provision.log'))[0] != 0:
        record['status'] = 'stopped: provisioning the volume failed; see provision.log'
        write_record(record_path, record)
        print('launch-pass: %s. Record: %s' % (record['status'], record_path))
        return 1
    model = record['model_pinned']
    session, reported = harness.probe(model, box, os.path.join(scratch, 'setup.log'))
    record.update(session_id=session, model_reported=sorted(reported))
    if reported != {model}:
        record['status'] = 'stopped: the session reported %s, not the pinned model' % sorted(reported)
        write_record(record_path, record)
        print('launch-pass: %s; the brief was not sent. Record: %s' % (record['status'], record_path))
        return 1
    record['status'] = 'attacking'
    write_record(record_path, record)
    print('launch-pass: session %s on %s; sending the brief. Record: %s' % (session, model, record_path))
    status, seen = harness.send(session, model, prompt, box, os.path.join(scratch, 'attack.log'))
    record.update(status='reported' if status == 0 else 'harness exited %d' % status,
                  models_seen=sorted(seen), finished=now())
    write_record(record_path, record)
    print('launch-pass: %s. Keep the volume %s until the pass is collected; then '
          '`podman volume rm %s`' % (record['status'], box.volume, box.volume))
    return status


def complete(args):
    """Resume the recorded session once, with the checker's short rows as the whole message."""
    with open(args.complete) as source:
        record = json.load(source)
    if 'completion' in record:
        raise SystemExit('launch-pass: refused: this session already had its completion round (%s)'
                         % record['completion']['started'])
    if record.get('status') != 'reported':
        raise SystemExit('launch-pass: refused: the record says %r, not a finished pass' % record.get('status'))
    with (sys.stdin if args.short_rows == '-' else open(args.short_rows)) as source:
        lines = [line.rstrip() for line in source if line.strip()]
    if not lines or not all(SHORT_ROW.match(line) for line in lines):
        raise SystemExit('launch-pass: refused: --short-rows must be `script/booted-rows --short-only` '
                         'output, short rows only, one `row N: reason` per line')
    require_key(record['key_variable'])
    check_machine()
    if podman_out('image', 'exists', record['image_digest'])[0] != 0:
        raise SystemExit('launch-pass: refused: the image the session started in (%s) is gone; the '
                         'round must run the same harness' % record['image_digest'])
    message = '\n'.join(lines) + '\n'
    record['completion'] = {'started': now(), 'message_sha256': hashlib.sha256(message.encode()).hexdigest()}
    write_record(args.complete, record)
    scratch = os.path.dirname(os.path.abspath(args.complete))
    box = Box(scratch, record['key_variable'], record['image_digest'], record['volume'])
    status, seen = HARNESSES[record['harness']]().send(
        record['session_id'], record['model_pinned'], message, box, os.path.join(scratch, 'completion.log'))
    record['completion'].update(finished=now(), status=status, models_seen=sorted(seen))
    write_record(args.complete, record)
    return status


# ----------------------------------------------------------------------------------------------
# The selftest: a fake podman and a fake harness on disk, a one-commit repository holding the real
# briefs, and no container, model or network at all.
# ----------------------------------------------------------------------------------------------

FAKE_HARNESS = r'''#!/usr/bin/env python3
import json, os, sys
argv = sys.argv[1:]
with open(os.environ['FAKE_CALLS'], 'a') as calls:
    calls.write(json.dumps(argv) + '\n')
model = os.environ['FAKE_SERVED']
if argv[:1] == ['--version']:
    print('1.0 (fake)')
elif argv[:1] == ['export']:
    print('Exporting session'); print(json.dumps({'messages': [{'info': {'role': 'assistant',
        'providerID': model.split('/')[0], 'modelID': model.split('/')[-1]}}]}))
elif argv[:1] == ['run']:
    print(json.dumps({'type': 'text', 'sessionID': 'ses_fake'}))
else:
    print(json.dumps({'type': 'system', 'subtype': 'init', 'model': model, 'session_id': 's'}))
    print(json.dumps({'type': 'assistant', 'message': {'model': model}}))
'''
# Answers what the launcher asks podman, and runs a harness command in place of the container.
FAKE_PODMAN = r'''#!/usr/bin/env python3
import json, os, sys
argv = sys.argv[1:]
with open(os.environ['FAKE_PODMAN_CALLS'], 'a') as calls:
    calls.write(json.dumps(argv) + '\n')
if argv[:2] == ['machine', 'inspect']:
    print(json.dumps([{'Name': 'fake-machine', 'State': 'running', 'Resources': {
        'CPUs': int(os.environ['FAKE_CPUS']), 'Memory': int(os.environ['FAKE_MEMORY'])}}]))
elif argv[:2] == ['image', 'inspect']:
    print('sha256:fakeimage')
elif argv[:1] == ['run']:
    inner = argv[argv.index('-w') + 3:]
    if inner[0] in ('claude', 'opencode'):
        os.execv(os.environ['FAKE_HARNESS'], [os.environ['FAKE_HARNESS'], *inner[1:]])
'''
BRIEFS = ('confinement-outsider-pass.md', 'confinement-outsider-pass-launch.md')
SECRET = 'sk-pass-only-fake-key'
# What a pass's `podman run` may never carry: another mount, the host's environment, a widened
# privilege, or a network setting other than the default's outbound access.
FORBIDDEN_FLAGS = {'--mount', '--volume', '--env-host', '--env-file', '--privileged', '--network',
                   '--net', '--cap-add', '--security-opt', '--userns', '--pid', '--ipc'}


def write_executable(path, text):
    with open(path, 'w') as out:
        out.write(text)
    os.chmod(path, 0o755)


def selftest_tree(tmp, real_tree):
    """A one-commit repository holding the real briefs, and the fakes for podman and both harnesses."""
    tree = os.path.join(tmp, 'tree')
    os.makedirs(os.path.join(tree, 'briefs'))
    for name in BRIEFS:
        with open(os.path.join(real_tree, 'briefs', name)) as source, \
                open(os.path.join(tree, 'briefs', name), 'w') as out:
            out.write(source.read())
    git('init', '-q', cwd=tree)
    git('add', '-A', cwd=tree)
    git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'pinned', cwd=tree)
    fake, podman_fake = os.path.join(tmp, 'fake-harness'), os.path.join(tmp, 'fake-podman')
    write_executable(fake, FAKE_HARNESS)
    write_executable(podman_fake, FAKE_PODMAN)
    os.environ.update(LAUNCH_PASS_CLAUDE=fake, LAUNCH_PASS_OPENCODE=fake, LAUNCH_PASS_PODMAN=podman_fake,
                      FAKE_HARNESS=fake, FAKE_CALLS=os.path.join(tmp, 'calls'),
                      FAKE_PODMAN_CALLS=os.path.join(tmp, 'podman-calls'), FAKE_CPUS='6',
                      FAKE_MEMORY='8192', ANTHROPIC_API_KEY=SECRET, ZHIPU_API_KEY=SECRET)
    return tree, git('rev-parse', 'HEAD', cwd=tree)


def read_calls(variable):
    with open(os.environ[variable]) as source:
        return [json.loads(line) for line in source]


def reset_calls():
    for variable in ('FAKE_CALLS', 'FAKE_PODMAN_CALLS'):
        open(os.environ[variable], 'w').close()


def check_container_runs(scratch, key):
    """Every `podman run` gives the container the clone, the results and the volume, and no more."""
    runs = [argv for argv in read_calls('FAKE_PODMAN_CALLS') if argv[:1] == ['run']]
    assert runs, 'no container ran'
    for argv in runs:
        check_container_argv(argv, scratch, key)
    return runs


def check_container_argv(argv, scratch, key):
    """One `podman run`: the three pinned mounts, the three pinned variables, and nothing of $HOME.

    The scratch mounts are pinned exactly, so a scratch under $HOME (a TMPDIR there) is allowed;
    every other option is searched for the home path, which catches a mount spelled any other way.
    """
    assert argv[:2] == ['run', '--rm'], argv
    mounts = [i + 1 for i, flag in enumerate(argv) if flag == '-v']
    assert [argv[i] for i in mounts] == [
        '%s/nife:/pass/nife' % scratch, '%s/results:/pass/results' % scratch,
        'nife-pass-%s:/cache:z' % os.path.basename(scratch)], [argv[i] for i in mounts]
    variables = [argv[i + 1] for i, flag in enumerate(argv) if flag == '-e']
    assert variables == [key, 'IS_SANDBOX=1', 'DISABLE_AUTOUPDATER=1'], variables
    assert argv[argv.index('-w') + 2] == 'sha256:fakeimage', 'not the digest the record holds'
    end = argv.index('-w') + 2
    options = argv[:end]
    assert not FORBIDDEN_FLAGS.intersection(o.split('=')[0] for o in options), options
    assert SECRET not in ' '.join(argv), 'the key is on the command line'
    home = os.path.expanduser('~')
    others = [o for i, o in enumerate(options) if i not in mounts]
    assert not any(home in o for o in others), 'the home directory is given to the container'


def selftest_home_refused(argv, scratch, key):
    """A run that also mounts $HOME fails the check, however the mount is spelled."""
    home = os.path.expanduser('~')
    for extra in (['-v', '%s:/home' % home], ['-v=%s:/home' % home]):
        try:
            check_container_argv(argv[:2] + extra + argv[2:], scratch, key)
        except AssertionError:
            continue
        raise AssertionError('a run mounting $HOME with %r passed the check' % extra)


def selftest_launch(tree, tmp, reviewer, served, commit):
    """One launch against the fakes; returns its exit, its record, the harness calls and the record path."""
    os.environ['FAKE_SERVED'] = served
    reset_calls()
    scratch = os.path.join(tmp, 'pass-%s-%s' % (reviewer, served.replace('/', '-')))
    args = argparse.Namespace(tree=tree, reviewer=reviewer, model=None, scratch=scratch,
                              commit=commit, dry_run=False, prompt=None)
    status = launch(args)
    with open(os.path.join(scratch, 'launch.json')) as source:
        record = json.load(source)
    check_container_runs(scratch, REVIEWERS[reviewer]['key'])
    calls = [c for c in read_calls('FAKE_CALLS') if c != ['--version']]
    return status, record, calls, os.path.join(scratch, 'launch.json')


def expect_refusal(action, needle):
    try:
        action()
    except SystemExit as refused:
        assert needle in str(refused), refused
        return
    raise AssertionError('expected a refusal naming %r' % needle)


def selftest_real_prompt(real_tree):
    """The prompt each reviewer would get from the briefs in this tree: the runner section and the
    reviewer's brief, every placeholder filled, and no milestone number or roadmap block."""
    commit = '0123456789abcdef0123456789abcdef01234567'
    for name, reviewer in sorted(REVIEWERS.items()):
        prompt = assemble(real_tree, INSIDE, commit, reviewer)
        check_prompt(prompt)
        assert prompt.startswith('## Before you start\n'), (name, prompt[:80])
        assert 'milestone' not in prompt.lower() and 'roadmap' not in prompt.lower(), name
        assert not re.search(r'(?m)^(status|raised|needs_person):', prompt), name
        for placeholder in ('<worktree>', '<path>', '<results path>', '<logs>', '<commit>', '$'):
            assert placeholder not in prompt, (name, placeholder)
        assert 'commit `%s`' % commit in prompt and '`/pass/nife`' in prompt, name
        assert '/pass/results/results.md' in prompt and '/pass/results/logs/' in prompt, name
        assert prompt.count(reviewer['trailer']) == 1, (name, reviewer['trailer'])
        assert name == 'glm' or RUNNER_TRAILER not in prompt, name


def selftest_refusals(tree, tmp, commit):
    """A milestone prompt, a missing key or a small machine stops a launch before any clone or run."""
    expect_refusal(lambda: check_prompt('Do milestone ' + '913'), 'milestone number')
    expect_refusal(lambda: check_prompt('913'), 'milestone number')
    expect_refusal(lambda: check_prompt('Read design/roadmap/0913-x.md'), 'roadmap block')
    hand = os.path.join(tmp, 'hand-prompt')
    with open(hand, 'w') as out:
        out.write('Do milestone ' + '913\n')
    args = argparse.Namespace(tree=tree, reviewer='glm', model=None, scratch=os.path.join(tmp, 'm'),
                              commit=commit, dry_run=False, prompt=hand)
    reset_calls()
    expect_refusal(lambda: launch(args), 'milestone number')
    args.prompt = None
    os.environ.update(FAKE_CPUS='4', FAKE_MEMORY='2048')
    expect_refusal(lambda: launch(args), 'podman machine stop fake-machine && podman machine set '
                   '--cpus 6 --memory 8192 fake-machine && podman machine start fake-machine')
    os.environ.update(FAKE_CPUS='6', FAKE_MEMORY='8192')
    del os.environ['ZHIPU_API_KEY']
    expect_refusal(lambda: launch(args), 'ZHIPU_API_KEY is not set')
    os.environ['ZHIPU_API_KEY'] = SECRET
    assert not os.path.exists(args.scratch) and os.path.getsize(os.environ['FAKE_CALLS']) == 0
    podman_calls = read_calls('FAKE_PODMAN_CALLS')
    assert all(c[:1] == ['machine'] for c in podman_calls), 'a refused launch built or ran something'
    args.dry_run, args.scratch = True, os.path.join(tmp, 'dry')
    reset_calls()
    assert launch(args) == 0 and os.path.getsize(os.environ['FAKE_PODMAN_CALLS']) == 0


def selftest():
    real_tree = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    selftest_real_prompt(real_tree)
    with tempfile.TemporaryDirectory() as tmp:
        tree, commit = selftest_tree(tmp, real_tree)
        selftest_refusals(tree, tmp, commit)
        for reviewer, wrong in (('fable', 'claude-opus-4-8'), ('glm', 'zai-coding-plan/glm-4.7')):
            status, record, calls, _ = selftest_launch(tree, tmp, reviewer, wrong, commit)
            assert status == 1 and record['status'].startswith('stopped'), record
            assert not any('### Your job' in ' '.join(c) for c in calls), 'the brief went out'
            assert not any('--resume' in c or '--session' in c for c in calls), calls
        status, record, calls, path = selftest_launch(tree, tmp, 'fable', 'claude-fable-5-1', commit)
        assert status == 0 and record['status'] == 'reported' and record['model_reported'] == ['claude-fable-5-1']
        assert record['image_digest'] == 'sha256:fakeimage' and record['harness_version'] == '1.0'
        assert record['key_variable'] == 'ANTHROPIC_API_KEY' and SECRET not in json.dumps(record)
        assert git('branch', '--format=%(refname:short)', cwd=record['clone']) == 'pass'
        assert git('remote', cwd=record['clone']) == ''
        rows = os.path.join(tmp, 'short')
        with open(rows, 'w') as out:
            out.write('row 10: host only\nrow 22: timed out on every ISA (inconclusive)\n')
        args = argparse.Namespace(complete=path, short_rows=rows)
        reset_calls()
        assert complete(args) == 0
        runs = check_container_runs(os.path.dirname(path), 'ANTHROPIC_API_KEY')
        selftest_home_refused(runs[0], os.path.dirname(path), 'ANTHROPIC_API_KEY')
        assert any('--resume' in c and record['session_id'] in c for c in read_calls('FAKE_CALLS'))
        expect_refusal(lambda: complete(args), 'already had its completion round')
    print('launch-pass: selftest passed')
    return 0


def main(argv):
    parser = argparse.ArgumentParser(prog='script/launch-pass')
    parser.add_argument('--tree', default=os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    parser.add_argument('--reviewer', choices=sorted(REVIEWERS))
    parser.add_argument('--model', help='override the reviewer\'s pinned model id')
    parser.add_argument('--commit')
    parser.add_argument('--scratch', help='a new directory for the clone, results, prompt and record')
    parser.add_argument('--prompt', help='a hand-assembled prompt, checked like the assembled one')
    parser.add_argument('--dry-run', action='store_true', help='the clone and the prompt; no podman')
    parser.add_argument('--complete', metavar='RECORD')
    parser.add_argument('--short-rows', default='-')
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args(argv)
    if args.selftest:
        return selftest()
    if args.complete:
        return complete(args)
    if not (args.reviewer and args.commit):
        parser.error('--reviewer and --commit are required to launch')
    return launch(args)


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
