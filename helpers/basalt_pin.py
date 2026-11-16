#!/usr/bin/env python3
"""Which basalt Release `xtask/basalt-pin` should name, and the file that says so.

    python3 helpers/basalt_pin.py resolve [--repository nifeos/basalt] [--current FILE]
    python3 helpers/basalt_pin.py --selftest

Name: provisional, minted 2026-11-15 (UTC) by milestone/1116-basalt-pin-follows-release. A python
helper under `helpers/`, outside `script/names`' scope, so this paragraph is its provenance.

# WHY

`xtask/basalt-pin` names the basalt commit whose recipes and build script the gate runs, and the
Release whose published programs every other gate fetches. Nothing moved it, so it fell a month
behind (milestone 1115 (the gate builds basalt's delivered programs from the candidate tree)). `.github/workflows/pin-basalt.yml` moves it daily, as a pull request, and
this file decides where to. The decision is pure, so it is tested without a network; `resolve` is
the thin layer that reads basalt's Releases through `gh api` and hands them to it.

# THE RULE

The target is the newest Release of basalt, by publication time, that is all of:

- published: not a draft and not a pre-release;
- named `rolling-<date>-<short commit>`, where the short commit is the front of the commit the
  Release was cut at;
- complete: for every package basalt's recipes name that nife needs (REQUIRED_PACKAGES), the
  Release holds a `.nifepkg` for each architecture and the source archive, each fully uploaded;
- on basalt's `main`: the commit is main's head or one of its ancestors (a compare of
  `main...<commit>` reports `identical` or `behind`), so the pin never points at an unmerged branch.

A newer Release that fails a test is skipped, and the next one down is tried. The pin never moves
backward: when the Release it already names is the first one that passes, or is newer than it,
nothing changes.

# THE FILE

Two lines, `commit <40 hex digits>` and `release <tag>`, as `xtask/src/basalt.rs` reads them.

# BUGS

- `REQUIRED_PACKAGES` and `ARCHITECTURES` repeat what `xtask/src/basalt.rs` builds. A package added
  there and not here is not required of a Release, and the first red pull request says so.
"""
import json
import re
import subprocess
import sys

REQUIRED_PACKAGES = ("ripgrep", "ioping", "hackbench", "sqlite", "demos")
ARCHITECTURES = ("aarch64", "riscv64", "x86_64")
TAG = re.compile(r"^rolling-\d{4}-\d{2}-\d{2}-([0-9a-f]{7,40})$")
HEX40 = re.compile(r"^[0-9a-f]{40}$")
# What a compare of `main...<commit>` says when the commit is on main.
ON_MAIN = ("identical", "behind")


def missing_assets(names, packages=REQUIRED_PACKAGES):
    """The asset names a Release lacks for `packages`, sorted. Empty means complete."""
    missing = []
    for package in packages:
        pattern = re.compile(r"^" + re.escape(package) + r"-([0-9][^-]*)-[a-z0-9_]+\.(?:nifepkg|tar\.gz)$")
        versions = sorted({m.group(1) for m in map(pattern.match, names) if m})
        if not versions:
            missing.append(package + "-*")
            continue
        for version in versions:
            wanted = [f"{package}-{version}-{arch}.nifepkg" for arch in ARCHITECTURES]
            wanted.append(f"{package}-{version}-source.tar.gz")
            missing += [w for w in wanted if w not in names]
    return sorted(missing)


def eligible(release, commit_of, is_on_main):
    """None when `release` (one REST release object) may be pinned, else the reason it may not."""
    tag = release.get("tag_name", "")
    m = TAG.match(tag)
    if not m:
        return "tag is not rolling-<date>-<commit>"
    if release.get("draft"):
        return "draft"
    if release.get("prerelease"):
        return "pre-release"
    if not release.get("published_at"):
        return "not published"
    names = [a["name"] for a in release.get("assets", []) if a.get("state", "uploaded") == "uploaded"]
    gap = missing_assets(names)
    if gap:
        return "assets missing: " + ", ".join(gap)
    commit = release.get("target_commitish", "")
    if not HEX40.match(commit):
        commit = commit_of(m.group(1))
    if not commit or not HEX40.match(commit):
        return "commit of the Release cannot be resolved"
    if not commit.startswith(m.group(1)):
        return "tag names a commit other than the one the Release is cut at"
    if not is_on_main(commit):
        return "commit is not on main"
    return None


def choose(releases, current_tag, commit_of, is_on_main):
    """(commit, tag, log) for the Release to pin, or (None, None, log) when the pin stays.

    `releases` is the REST listing; its order is not trusted, publication time is.
    """
    ordered = sorted(releases, key=lambda r: r.get("published_at") or "", reverse=True)
    log = []
    for release in ordered:
        tag = release.get("tag_name", "")
        if tag == current_tag:
            log.append(f"{tag}: the pin already names it, and nothing newer qualifies")
            return None, None, log
        why = eligible(release, commit_of, is_on_main)
        if why:
            log.append(f"{tag}: skipped, {why}")
            continue
        commit = release.get("target_commitish", "")
        if not HEX40.match(commit):
            commit = commit_of(TAG.match(tag).group(1))
        log.append(f"{tag}: chosen")
        return commit, tag, log
    log.append("no Release qualifies")
    return None, None, log


def render(commit, tag):
    return f"commit {commit}\nrelease {tag}\n"


def parse(text):
    """{'commit': ..., 'release': ...} from the pin file's text; ValueError when it is not two lines."""
    fields = {}
    for line in text.splitlines():
        if not line.strip():
            continue
        key, _, value = line.partition(" ")
        if key not in ("commit", "release") or not value.strip() or key in fields:
            raise ValueError(f"unexpected line in the pin file: {line!r}")
        fields[key] = value.strip()
    if set(fields) != {"commit", "release"}:
        raise ValueError("the pin file needs a commit line and a release line")
    return fields


def gh_json(*args):
    out = subprocess.run(["gh", "api", *args], check=True, capture_output=True, text=True).stdout
    return json.loads(out)


def resolve(repository, current_path):
    with open(current_path) as f:
        current = parse(f.read())

    def commit_of(short):
        try:
            return gh_json(f"repos/{repository}/commits/{short}")["sha"]
        except subprocess.CalledProcessError:
            return None

    def is_on_main(commit):
        try:
            return gh_json(f"repos/{repository}/compare/main...{commit}")["status"] in ON_MAIN
        except subprocess.CalledProcessError:
            return False

    releases = gh_json(f"repos/{repository}/releases?per_page=30")
    commit, tag, log = choose(releases, current["release"], commit_of, is_on_main)
    for line in log:
        print(line, file=sys.stderr)
    if commit is None or (commit == current["commit"] and tag == current["release"]):
        return 0
    sys.stdout.write(render(commit, tag))
    return 0


def full_assets(package, version):
    names = [f"{package}-{version}-{a}.nifepkg" for a in ARCHITECTURES]
    return names + [f"{package}-{version}-source.tar.gz"]


def fake_release(tag, published, commit=None, drop=(), draft=False, prerelease=False, state="uploaded"):
    names = []
    for package in REQUIRED_PACKAGES:
        names += full_assets(package, "1.0")
    return {
        "tag_name": tag,
        "target_commitish": commit if commit is not None else "main",
        "draft": draft,
        "prerelease": prerelease,
        "published_at": published,
        "assets": [{"name": n, "state": state} for n in names if n not in drop],
    }


def selftest():
    a, b, c = "a" * 40, "b" * 40, "c" * 40
    full = {a: "a" * 7, b: "b" * 7, c: "c" * 7}
    main = {a, b}

    def tag(sha, day):
        return f"rolling-2026-11-{day}-{sha[:7]}"

    def pick(releases, current="rolling-2026-11-01-0000000"):
        commit_of = lambda short: next((s for s in full if s.startswith(short)), None)
        return choose(releases, current, commit_of, lambda s: s in main)[:2]

    def rel(sha, day, **kw):
        return fake_release(tag(sha, day), f"2026-11-{day}T10:00:00Z", commit=sha, **kw)

    # The newest qualifying Release wins, whatever order the listing is in.
    assert pick([rel(a, "10"), rel(b, "12")]) == (b, tag(b, "12"))
    assert pick([rel(b, "12"), rel(a, "10")]) == (b, tag(b, "12"))
    # A commit that is not on main, a draft, a pre-release, an incomplete or half-uploaded Release
    # are each skipped and the next one down is taken.
    skipped = [
        rel(c, "14"),
        rel(b, "13", draft=True),
        rel(b, "13", prerelease=True),
        rel(b, "13", drop=("ioping-1.0-riscv64.nifepkg",)),
        rel(b, "13", drop=("demos-1.0-source.tar.gz",)),
        rel(b, "13", state="starter"),
    ]
    for bad in skipped:
        assert pick([bad, rel(a, "10")]) == (a, tag(a, "10")), bad["tag_name"]
    # Main's own head is on main; so is an ancestor of it (`behind`), by the injected test.
    assert pick([rel(b, "12")]) == (b, tag(b, "12"))
    # A tag that is not a rolling tag, or names another commit than the one cut, is refused.
    odd = fake_release("v1.0", "2026-11-15T10:00:00Z", commit=a)
    assert pick([odd, rel(a, "10")]) == (a, tag(a, "10"))
    liar = fake_release(f"rolling-2026-11-15-{b[:7]}", "2026-11-15T10:00:00Z", commit=a)
    assert pick([liar, rel(a, "10")]) == (a, tag(a, "10"))
    # A branch name in `target_commitish` is resolved through the short commit in the tag.
    assert pick([fake_release(tag(a, "10"), "2026-11-10T10:00:00Z")]) == (a, tag(a, "10"))
    # The pin stays when it already names the newest qualifying Release, and never moves back to an
    # older one when the newer ones fail.
    assert pick([rel(a, "10"), rel(c, "14")], current=tag(a, "10")) == (None, None)
    assert pick([rel(b, "12"), rel(a, "10")], current=tag(b, "12")) == (None, None)
    assert pick([rel(c, "14"), rel(a, "10")], current=tag(a, "10")) == (None, None)
    # Nothing qualifies: the pin stays.
    assert pick([rel(c, "14")]) == (None, None)
    assert pick([]) == (None, None)
    # missing_assets names what is absent.
    assert missing_assets([]) == sorted(p + "-*" for p in REQUIRED_PACKAGES)
    # The file round-trips, and a malformed one is refused.
    assert parse(render(a, tag(a, "10"))) == {"commit": a, "release": tag(a, "10")}
    for bad in ("commit x\n", "commit x\nrelease y\ncommit z\n", "commit x\nrelease\n", "tag x\nrelease y\n"):
        try:
            parse(bad)
        except ValueError:
            continue
        raise AssertionError(bad)
    print("basalt_pin selftest ok")
    return 0


def main(argv):
    if argv == ["--selftest"]:
        return selftest()
    if argv and argv[0] == "resolve":
        repository, current = "nifeos/basalt", "xtask/basalt-pin"
        rest = argv[1:]
        while rest:
            flag = rest.pop(0)
            if flag == "--repository" and rest:
                repository = rest.pop(0)
            elif flag == "--current" and rest:
                current = rest.pop(0)
            else:
                break
        else:
            return resolve(repository, current)
    print(__doc__.split("# WHY")[0], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
