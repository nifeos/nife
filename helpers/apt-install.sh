#!/bin/sh
# Install apt packages on a GitHub-hosted runner, and fall back to a second Ubuntu mirror when the
# runner's own Ubuntu archive cannot be reached.
#
#     helpers/apt-install.sh clang shellcheck device-tree-compiler ruby
#
# Why the fallback exists: a hosted runner's apt sources name azure.archive.ubuntu.com and
# security.ubuntu.com. When those names do not resolve, `apt-get update` fails, or warns and leaves
# the package lists empty, and every job that installs a compiler fails with it although nothing in
# the tree changed. Other hosts can still resolve while Ubuntu's do, so a second mirror is enough.
#
# What it does: update against the runner's own sources, with retries. If that update fails, or
# reports a `Temporary failure resolving` for an ubuntu.com host, rewrite the Ubuntu archive and
# security sources to mirrors.edge.kernel.org and update again, then install. It reads both source
# formats: deb822 (`/etc/apt/sources.list.d/*.sources`, which Ubuntu 26.04 uses) and the classic
# one-line form (`/etc/apt/sources.list`, `*.list`). The rewrite changes only the machine it runs
# on, and a hosted runner is discarded after its job.
#
# Signature verification stays on. The mirror serves Ubuntu's own signed Release files, and apt
# checks them against the archive keys the image already trusts, so a mirror can withhold a package
# but cannot alter one. This script never passes `trusted=yes` or `--allow-unauthenticated`.
#
# The workflows that install with apt on hosted runners call this: ci.yml's lint job,
# toolchain-drift.yml and toolchain-bump.yml.
#
# Name: provisional, minted by lane/ci-apt-mirror-fallback.

set -eu

if [ "$#" -eq 0 ]; then
    echo "usage: helpers/apt-install.sh <package>..." >&2
    exit 2
fi

mirror=http://mirrors.edge.kernel.org/ubuntu/
retries='-o Acquire::Retries=3'

as_root=sudo
[ "$(id -u)" -eq 0 ] && as_root=

log=$(mktemp)
trap 'rm -f "$log"' EXIT

# Run `apt-get update`, showing its output, and succeed only if it exited 0 and resolved every
# ubuntu.com host.
update() {
    # shellcheck disable=SC2086 # $retries is two words on purpose.
    if $as_root apt-get $retries update 2>&1 | tee "$log"; then
        status=0
    else
        status=1
    fi
    # Under `sh`, a pipeline's status is tee's; the log tells an apt failure apart.
    if grep -q '^E:' "$log" || grep -q 'Temporary failure resolving .[^ ]*ubuntu\.com' "$log"; then
        status=1
    fi
    return "$status"
}

if ! update; then
    echo "apt-install: the runner's Ubuntu archive is unreachable; switching its sources to $mirror" >&2
    for file in /etc/apt/sources.list /etc/apt/sources.list.d/*.list /etc/apt/sources.list.d/*.sources; do
        [ -f "$file" ] || continue
        grep -Eq '://([a-z0-9-]+\.)*(archive|security)\.ubuntu\.com/ubuntu' "$file" || continue
        # One expression serves both formats: deb822's `URIs:` field and a one-line `deb` entry both
        # hold the URI as a single token.
        $as_root sed -Ei "s#https?://([a-z0-9-]+\.)*(archive|security)\.ubuntu\.com/ubuntu/?#$mirror#g" "$file"
        echo "apt-install: rewrote $file" >&2
    done
    update
fi

# shellcheck disable=SC2086 # $retries is two words on purpose.
$as_root apt-get $retries install -y "$@"
