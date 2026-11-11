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
# security sources to mirrors.edge.kernel.org and update again, then install. When no Ubuntu host
# the sources name resolves at all, it skips the first update, whose retries would spend minutes on
# DNS timeouts. It reads both source formats: deb822 (`/etc/apt/sources.list.d/*.sources`, which
# Ubuntu 26.04 uses) and the classic one-line form (`/etc/apt/sources.list`, `*.list`), and the
# mirror list a `mirror+file:` source names, which is how the hosted image points at its archive
# (`/etc/apt/apt-mirrors.txt`). The rewrite changes only the machine it runs on, and a hosted runner
# is discarded after its job.
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

sources="/etc/apt/sources.list /etc/apt/sources.list.d/*.list /etc/apt/sources.list.d/*.sources"
ubuntu_uri='https?://([a-z0-9-]+\.)*(archive|security)\.ubuntu\.com/ubuntu'

# The files that hold an Ubuntu archive URI: the source files, and each mirror list a
# `mirror+file:` source names.
# shellcheck disable=SC2086 # $sources holds globs on purpose.
ubuntu_files() {
    for file in $sources $(cat $sources 2>/dev/null | grep -Eo 'mirror\+file:[^ ]+' | sed 's/^mirror+file://' | sort -u); do
        [ -f "$file" ] && grep -Eq "$ubuntu_uri" "$file" && echo "$file"
    done
}

# Succeed when at least one Ubuntu host those files name resolves.
ubuntu_resolves() {
    for host in $(ubuntu_files | xargs cat | grep -Eo "$ubuntu_uri" | sed -E 's#^https?://##; s#/.*##' | sort -u); do
        getent hosts "$host" >/dev/null && return 0
    done
    return 1
}

if ! ubuntu_resolves || ! update; then
    echo "apt-install: the runner's Ubuntu archive is unreachable; switching its sources to $mirror" >&2
    for file in $(ubuntu_files); do
        # One expression serves every format: deb822's `URIs:` field, a one-line `deb` entry and a
        # mirror list's line each hold the URI as a single token.
        $as_root sed -Ei "s#$ubuntu_uri/?#$mirror#g" "$file"
        echo "apt-install: rewrote $file" >&2
    done
    update
fi

# shellcheck disable=SC2086 # $retries is two words on purpose.
$as_root apt-get $retries install -y "$@"
