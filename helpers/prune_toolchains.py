#!/usr/bin/env python3
"""List, and on request uninstall, dated nightly toolchains nothing on this machine pins.

    python3 helpers/prune_toolchains.py            # dry run: say what would go, and how big it is
    python3 helpers/prune_toolchains.py --remove   # uninstall exactly those

PROVISIONAL NAME, minted 2026-10-06 (UTC) by lane/orphan-work; calef has not ruled on it.
Underscores and `.py` because that is how the other Python helpers here are spelled (nanny.py,
runwatch.py).

Why this exists. Every toolchain bump (script/toolchain-bump, and the CI workflow that opens its
pull request) moves `rust-toolchain.toml` to a new `nightly-YYYY-MM-DD`, and rustup installs it the
first time any worktree builds. Nothing ever uninstalls the old one. On 2026-10-06 the maintainer
removed 15 of them by hand and `~/.rustup` went from 44 GB to 14 GB. calef approved making that a
step: stale nightlies go once nothing pins them.

What counts as pinned, and so is never touched:

  - the `channel` in every worktree's `rust-toolchain.toml`, the main checkout included, read from
    `git worktree list` so a lane cut yesterday on an older pin keeps it;
  - Kani's own toolchain. notes/kernel-proofs.md records that Kani bundles its own rustc
    (`kani-0.67.0` pins `nightly-2025-11-21`), and an installed Kani writes that pin to
    `~/.kani/kani-<version>/rust-toolchain-version`. The patched riscv64 Kani
    (notes/kernel-proofs/riscv64-with-a-patched-kani.md) builds from a source tree under
    `~/.cache/nife-kani-riscv64/kani-<version>/src`, whose `rust-toolchain.toml` names its pin. Both
    are read, never hard-coded, so a Kani upgrade moves the pin with it
    (helpers/kani-install-selftest.sh holds the prune to keeping it);
  - anything that is not a dated `nightly-*`: `stable`, a numbered release such as `1.98.1`, and
    the `nife-dev-*` links, which are each worktree's std farm (notes/std.md) and not installs.

It runs on this Mac, at prune time (briefs/merge-and-cleanup.md). It cannot live in the CI
toolchain-bump workflow: that runs on a GitHub runner, whose `~/.rustup` is thrown away after every
job, and it never sees this machine's.

BUGS
  - A worktree is the only pin it knows. A branch that is not checked out, or a `cargo +nightly-...`
    typed by hand, pins nothing here; removing its toolchain costs one re-download the next time it
    builds, which is the whole downside.
  - If `~/.kani` exists and no `rust-toolchain-version` file can be read in it, `--remove` refuses
    rather than guess, because a removed Kani toolchain breaks `cargo kani` until `cargo kani setup`
    runs again.
"""

import glob
import os
import re
import subprocess
import sys

DATED = re.compile(r"^nightly-\d{4}-\d{2}-\d{2}(-.+)?$")
CHANNEL = re.compile(r'^\s*channel\s*=\s*"([^"]+)"', re.M)


def run(*args, **kw):
    return subprocess.run(args, capture_output=True, text=True, **kw)


def channel_of(path):
    try:
        m = CHANNEL.search(open(path).read())
    except OSError:
        return None
    return m.group(1) if m else None


def worktree_pins(repo):
    pins = {}
    out = run("git", "-C", repo, "worktree", "list", "--porcelain").stdout
    for line in out.splitlines():
        if line.startswith("worktree "):
            wt = line[len("worktree "):]
            ch = channel_of(os.path.join(wt, "rust-toolchain.toml"))
            if ch:
                pins.setdefault(ch, []).append(wt)
    return pins


def kani_pins(home):
    pins, unreadable = {}, False
    kani = os.path.join(home, ".kani")
    for d in glob.glob(os.path.join(kani, "kani-*")):
        try:
            pins.setdefault(open(os.path.join(d, "rust-toolchain-version")).read().strip(), []).append(d)
        except OSError:
            unreadable = True
    for f in glob.glob(os.path.join(home, ".cache/nife-kani-riscv64/kani-*/src/rust-toolchain.toml")):
        ch = channel_of(f)
        if ch:
            pins.setdefault(ch, []).append(f)
    if os.path.isdir(kani) and not pins:
        unreadable = True
    return pins, unreadable


def pinned_by(name, pins):
    """A toolchain `nightly-2026-10-06-aarch64-apple-darwin` is pinned by channel `nightly-2026-10-06`."""
    return [who for ch, holders in pins.items() if name == ch or name.startswith(ch + "-") for who in holders]


def size(path):
    out = run("du", "-sh", path).stdout.split()
    return out[0] if out else "?"


def main(argv):
    remove = "--remove" in argv
    if any(a not in ("--remove",) for a in argv):
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    repo = run("git", "rev-parse", "--path-format=absolute", "--git-common-dir").stdout.strip()
    repo = os.path.dirname(repo) if repo else os.getcwd()
    home = os.path.expanduser("~")
    rustup_home = os.environ.get("RUSTUP_HOME", os.path.join(home, ".rustup"))

    pins = worktree_pins(repo)
    kpins, kani_unreadable = kani_pins(home)
    for ch, holders in kpins.items():
        pins.setdefault(ch, []).extend(holders)

    installed = [l.split()[0] for l in run("rustup", "toolchain", "list").stdout.splitlines() if l.strip()]
    stale = []
    for name in installed:
        if not DATED.match(name):
            continue
        who = pinned_by(name, pins)
        if who:
            print(f"prune-toolchains: KEEP {name}, pinned by {who[0]}" + (f" and {len(who) - 1} more" if len(who) > 1 else ""))
        else:
            stale.append(name)

    if kani_unreadable:
        print("prune-toolchains: ~/.kani exists but no Kani pin could be read from it", file=sys.stderr)
        if remove:
            print("prune-toolchains: refusing --remove; a removed Kani toolchain breaks cargo kani", file=sys.stderr)
            return 1

    for name in stale:
        where = os.path.join(rustup_home, "toolchains", name)
        if remove:
            r = run("rustup", "toolchain", "uninstall", name)
            print(f"prune-toolchains: REMOVED {name}" if r.returncode == 0 else f"prune-toolchains: FAILED {name}: {r.stderr.strip()}")
        else:
            print(f"prune-toolchains: WOULD REMOVE {name} ({size(where)}); nothing pins it")
    if not stale:
        print("prune-toolchains: nothing to remove")
    elif not remove:
        print("prune-toolchains: dry run; pass --remove to uninstall these")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
