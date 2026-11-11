#!/bin/sh
# Probe the owned runner's job image for what the stage 1 jobs need, and print a FINDING line for
# each thing missing. Exit 1 when any is.
#
#     helpers/owned-image-probe.sh tools        rustup and cargo are on PATH
#     helpers/owned-image-probe.sh toolchain    the toolchain rust-toolchain.toml pins is installed and runs
#     helpers/owned-image-probe.sh apt-proxy    apt is configured to use the proxy, also under sudo
#     helpers/owned-image-probe.sh pip          pip reaches its index and file host through the proxy
#     helpers/owned-image-probe.sh qemu         the QEMU cache landed where script/ci-qemu looks, and
#                                               what script/ci-qemu would decide (build or skip)
#     helpers/owned-image-probe.sh qemu-fetch   the hosts a cold QEMU build fetches from answer
#     helpers/owned-image-probe.sh packages     every apt package script/ci-qemu and script/bootstrap install
#                                               (helpers/apt-packages.txt) is already installed
#     helpers/owned-image-probe.sh kvm          /dev/kvm can create a VM (KVM_GET_API_VERSION is 12, KVM_CREATE_VM returns a descriptor)
#
# Run from the repository root. The owned canary (strict checks) and its dry run call this, so the
# image is judged by one definition; notes/owned-runners.md says which job runs which. The probe
# changes nothing outside its own temporary directory.
#
# Name: provisional, minted by lane/owned-canary-strict-dry-run.

set -u

findings=0
finding() {
    echo "FINDING $*"
    findings=1
}

check_tools() {
    for tool in rustup cargo; do
        if path=$(command -v "$tool" 2>/dev/null); then
            echo "$tool: $path"
        else
            finding "$tool is not on PATH"
        fi
    done
}

check_toolchain() {
    if ! command -v rustup >/dev/null 2>&1; then
        finding "rustup is not on PATH, so the pinned toolchain cannot be checked"
        return
    fi
    channel=$(sed -n 's/^channel *= *"\(.*\)".*/\1/p' rust-toolchain.toml | head -1)
    if test -z "$channel"; then
        finding "rust-toolchain.toml names no channel"
        return
    fi
    if rustup toolchain list 2>/dev/null | grep -q "^$channel"; then
        echo "pinned toolchain: $channel (installed)"
    else
        finding "pinned toolchain $channel is not installed"
    fi
    if out=$(rustc -vV 2>&1); then
        echo "rustc: $(echo "$out" | head -1)"
    else
        finding "rustc -vV fails in the repository: $(echo "$out" | head -1)"
    fi
    if out=$(cargo --version 2>&1); then
        echo "cargo: $out"
    else
        finding "cargo --version fails in the repository: $(echo "$out" | head -1)"
    fi
}

check_apt_proxy() {
    configured=$(apt-config dump 2>/dev/null | grep -iE '^Acquire::https?::Proxy ' | grep -viE '"(DIRECT)?";' || true)
    if test -n "$configured"; then
        echo "apt proxy: $(echo "$configured" | head -1)"
    else
        finding "apt has no proxy configured (no Acquire::http::Proxy in /etc/apt/apt.conf.d)"
    fi
    # Other tools run under sudo need the runner's proxy variables too.
    if test -n "${http_proxy:-}${HTTP_PROXY:-}"; then
        if sudo -n env 2>/dev/null | grep -qi '^http_proxy='; then
            echo "sudo keeps http_proxy"
        else
            finding "sudo drops http_proxy, so a tool run under sudo bypasses the proxy"
        fi
    fi
}

check_pip() {
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    if ! python3 -m venv "$work/venv" >/dev/null 2>&1; then
        finding "python3 -m venv fails; pip cannot be tried (python3-venv missing)"
        return
    fi
    # The two packages QEMU's configure fetches when it builds from source.
    if "$work/venv/bin/pip" download --no-deps --quiet --disable-pip-version-check \
        --dest "$work/dl" "wheel>=0.34.2" "qemu.qmp==0.0.6" 2>"$work/err"; then
        echo "pip: fetched $(cd "$work/dl" && echo *)"
    else
        finding "pip cannot reach its index through the proxy: $(grep -iE 'error|proxy|403' "$work/err" | head -2 | tr '\n' ' ')"
    fi
}

check_qemu() {
    want=$(tr -d ' \n' <.qemu-version)
    prefix="${QEMU_PREFIX:-$HOME/.cache/nife-qemu}"
    expected=/home/runner/work/nife/nife
    if test "${GITHUB_WORKSPACE:-$expected}" != "$expected"; then
        finding "workspace is ${GITHUB_WORKSPACE}, not $expected; a cache saved on a hosted runner restores outside HOME"
    else
        echo "workspace: ${GITHUB_WORKSPACE:-unset}"
    fi
    if test -x "$prefix/bin/qemu-system-x86_64" && test -x "$prefix/bin/qemu-system-riscv64"; then
        have=$("$prefix/bin/qemu-system-riscv64" --version 2>/dev/null | head -1 | awk '{print $4}')
        if test "$have" = "$want"; then
            echo "qemu decision: skip, $want already built at $prefix (cache hit)"
        else
            echo "qemu decision: build, cache holds '${have:-unrunnable}' and the pin is $want"
        fi
    else
        echo "qemu decision: build, no cached QEMU at $prefix"
        if test "${QEMU_CACHE_HIT:-}" = true; then
            finding "the cache step reported a hit but $prefix holds no QEMU: the entry restored to another directory"
        fi
    fi
}

check_qemu_fetch() {
    want=$(tr -d ' \n' <.qemu-version)
    if sudo -n apt-get update >/dev/null 2>"${TMPDIR:-/tmp}/owned-image-probe-apt.$$"; then
        echo "apt-get update: ok"
    else
        finding "apt-get update fails: $(head -2 "${TMPDIR:-/tmp}/owned-image-probe-apt.$$" | tr '\n' ' ')"
    fi
    rm -f "${TMPDIR:-/tmp}/owned-image-probe-apt.$$"
    url="https://download.qemu.org/qemu-$want.tar.xz"
    code=$(curl -sS -o /dev/null -I -m 30 -w '%{http_code}' "$url" 2>&1)
    if test "$code" = 200; then
        echo "curl -I $url: $code"
    else
        finding "$url answered '$code'"
    fi
    check_pip
}

check_packages() {
    if ! list=$(helpers/apt-packages.sh); then
        finding "helpers/apt-packages.sh cannot list the packages"
        return
    fi
    checked=0
    missing=0
    for package in $list; do
        checked=$((checked + 1))
        status=$(dpkg-query -W -f='${Status}' "$package" 2>/dev/null || true)
        if test "$status" != "install ok installed"; then
            missing=$((missing + 1))
            finding "apt package $package is not installed (dpkg status: '${status:-unknown}')"
        fi
    done
    echo "packages: $checked checked, $missing missing"
}

check_kvm() {
    if out=$(python3 -c 'import os,fcntl;f=os.open("/dev/kvm",os.O_RDWR);print(fcntl.ioctl(f,0xAE00));print(fcntl.ioctl(f,0xAE01,0))' 2>&1); then
        version=$(echo "$out" | sed -n 1p)
        echo "kvm api version: $version, KVM_CREATE_VM descriptor: $(echo "$out" | sed -n 2p)"
        if test "$version" != 12; then
            finding "/dev/kvm reports API version '$version', not 12"
        fi
    else
        finding "/dev/kvm cannot create a VM: $(echo "$out" | tail -1)"
    fi
}

case "${1:-}" in
tools) check_tools ;;
toolchain) check_toolchain ;;
apt-proxy) check_apt_proxy ;;
pip) check_pip ;;
qemu) check_qemu ;;
qemu-fetch) check_qemu_fetch ;;
packages) check_packages ;;
kvm) check_kvm ;;
*)
    echo "usage: helpers/owned-image-probe.sh tools|toolchain|apt-proxy|pip|qemu|qemu-fetch|packages|kvm" >&2
    exit 2
    ;;
esac
exit "$findings"
