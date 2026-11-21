#!/bin/sh
# Same-machine Linux run of the search milestone 121 (`ripgrep` on nife) measures: unmodified
# `ripgrep` 14.1.1 told `fixture::walk::RG_SEARCH` over the priced tree, which nife's
# `script/bench --real --release --smp` times as `rg_search`.
#
# It boots the SAME Alpine kernel run_linux_walk.sh uses, on the SAME `virt,accel=hvf` machine with
# the SAME `-cpu host`, `-m 256M` and `-smp 4` as nife's bench boot. PID 1 is `walk_pricing`'s
# `rg_host` example, built static for musl: it stages the priced tree, runs `/rg` over it with the
# words nife's `rg` hears (split by `grant_plan::each_word`, the function that builds nife's argv),
# and prints ripgrep's own `--stats` figure per run. `/rg` is built from the same crates.io source
# `helpers/build-ripgrep.sh` builds nife's from, with the same `-Copt-level=s`.
#
# What is matched and what is not, which notes/ripgrep-on-nife.md repeats beside the numbers:
# - the figure is ripgrep's own clock on both sides, from after argument parsing to after the last
#   file, so process creation and image loading are out of it on both;
# - the tree is on the initramfs, which is tmpfs, against nife's RedoxFS image on virtio-blk. Every
#   timed run is warm on both sides, so neither reads its device; run_linux_walk.sh says the same;
# - stdout is a pipe the init drains on Linux and a byte sink the kernel drains on nife.
#
# Needs: rustup target add aarch64-unknown-linux-musl; qemu-system-aarch64; network once, for the
# kernel and the ripgrep crate.
#
# Run: sh bench/host/run_linux_rg.sh
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
WORK=${WORK:-$ROOT/target/walkbench}
VERSION=14.1.1
mkdir -p "$WORK"

KERNEL="$WORK/vmlinuz-virt"
if [ ! -f "$KERNEL" ]; then
    echo "fetching an aarch64 Linux kernel..."
    curl -sSL -o "$KERNEL" \
        "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/aarch64/netboot/vmlinuz-virt"
fi

# The source helpers/build-ripgrep.sh unpacks, outside this repository: under it, cargo would take
# ripgrep for a member of this workspace and refuse to build it.
BUILD="${TMPDIR:-/tmp}/nife-ripgrep"
SRC="$BUILD/ripgrep-$VERSION"
mkdir -p "$BUILD"
if [ ! -f "$SRC/Cargo.toml" ]; then
    curl -sSL --max-time 120 -o "$BUILD/ripgrep-$VERSION.crate" \
        "https://static.crates.io/crates/ripgrep/ripgrep-$VERSION.crate"
    tar xzf "$BUILD/ripgrep-$VERSION.crate" -C "$BUILD"
fi

# rust-lld and the self-contained musl objects, as run_linux_fs.sh links: macOS's `cc` cannot link
# a Linux binary.
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld
MUSL_FLAGS="-C link-self-contained=yes -C target-feature=+crt-static"

CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="$MUSL_FLAGS" \
    cargo build -q --release --manifest-path "$ROOT/Cargo.toml" -p walk_pricing --example rg_host \
    --target aarch64-unknown-linux-musl

# **One change to ripgrep, on this side only.** Upstream's musl build links jemalloc, and
# `jemalloc-sys` needs a C cross-compiler for aarch64 musl, which this machine does not have. A
# copy of the source drops that dependency and the two lines that install it, so Linux's `rg` uses
# musl's own malloc, as nife's uses std's heap. Upstream chose jemalloc because musl's allocator is
# slow for ripgrep, so this leaves Linux's figure pessimistic rather than flattering; the note says so.
LINUX_SRC="$BUILD/ripgrep-$VERSION-linux-no-jemalloc"
if [ ! -f "$LINUX_SRC/Cargo.toml" ]; then
    rm -rf "$LINUX_SRC" && cp -R "$SRC" "$LINUX_SRC" && rm -rf "$LINUX_SRC/target"
    # perl rather than sed: macOS's sed has no `,+N` address.
    perl -0pi -e 's/\[target[^\n]*jemallocator\]\nversion[^\n]*\n//' "$LINUX_SRC/Cargo.toml"
    perl -0pi -e 's/#\[cfg\(all\(target_env = "musl"[^\n]*\n#\[global_allocator\]\n[^\n]*\n//' \
        "$LINUX_SRC/crates/core/main.rs"
    ! grep -q jemallocator "$LINUX_SRC/Cargo.toml" "$LINUX_SRC/crates/core/main.rs"
fi

# ripgrep's own release profile, with the size optimization and stripping nife's build uses.
( cd "$LINUX_SRC" && RUSTUP_TOOLCHAIN="$(sed -n 's/^channel = "\(.*\)"/\1/p' "$ROOT/rust-toolchain.toml")" \
    CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="$MUSL_FLAGS -Cstrip=debuginfo -Copt-level=s" \
    cargo build -q --release --target aarch64-unknown-linux-musl )

rm -rf "$WORK/rgroot" && mkdir "$WORK/rgroot"
cp "$ROOT/target/aarch64-unknown-linux-musl/release/examples/rg_host" "$WORK/rgroot/init"
cp "$LINUX_SRC/target/aarch64-unknown-linux-musl/release/rg" "$WORK/rgroot/rg"
( cd "$WORK/rgroot" && find . | cpio -o -H newc 2>/dev/null ) > "$WORK/rg-initramfs.cpio"

# `helpers/qemu-bounded.sh`, never `timeout(1)` (macOS has none) or `perl -e alarm` (QEMU swallows
# SIGALRM). CLAUDE.md has the rule. `init` is given `/rg`, five timed runs (nife's
# `RG_SEARCH_RUNS`) and `/walk`.
"$ROOT/helpers/qemu-bounded.sh" 120 \
    qemu-system-aarch64 -M virt,accel=hvf,gic-version=3 -cpu host -m 256M -smp 4 -no-reboot \
    -kernel "$KERNEL" -initrd "$WORK/rg-initramfs.cpio" \
    -append "console=ttyAMA0 rdinit=/init panic=-1 quiet loglevel=0 -- /rg 5 /walk" \
    -display none -serial stdio 2>&1 | grep '^rg'
