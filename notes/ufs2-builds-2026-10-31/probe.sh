#!/bin/sh
# Build probes for notes/ufs2-builds-2026-10-31.md. Name provisional.
#
#   notes/ufs2-builds-2026-10-31/probe.sh bare [shipped|patched] [probe...]  # no_std, the three -none targets
#   notes/ufs2-builds-2026-10-31/probe.sh nife [shipped|patched] [probe...]  # std, targets/*-unknown-nife.json
#   notes/ufs2-builds-2026-10-31/probe.sh host [shipped|patched] [probe...]  # the host, as a control
#   notes/ufs2-builds-2026-10-31/probe.sh check                              # the host round trip
#
# `bare` builds a freestanding binary (probes/*-bare.rs, with probes/bare.rs's panic handler and
# talc allocator appended) for aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf and
# x86_64-unknown-none with the toolchain rust-toolchain.toml pins, whose prebuilt core and alloc
# those targets carry. It answers whether a crate builds with no std at all, as redoxfs_server's
# engine does.
#
# `nife` is notes/filesystem-crates-2026-10-07/probe.sh's recipe unchanged: the probe is generated
# OUTSIDE this repository (so its rust-toolchain.toml cannot beat RUSTUP_TOOLCHAIN), built with this
# worktree's patched farm at target/nife-farm, -Zbuild-std, release, panic = "abort". It answers
# whether a crate builds as a std program here, as jig is. Run `cargo xtask std-src` first.
#
# `shipped` is the crate as crates.io publishes it, pinned with `=`. `patched` copies the crate out
# of cargo's registry, applies patches/<crate>-<mode>.patch and builds against the copy through
# [patch.crates-io], so the patch size in the note is the diff a reader can apply.
#
# `check` fetches rufs's own FreeBSD-made test image at a pinned commit, checks its hash, and runs
# probes/rufs-host.rs twice: against rufs as shipped, with std, and against the no_std port with
# std off. It prints both outputs' hashes and their diff, which is empty when the port is faithful.
#
# BUGS: `bare` and `nife` build and link only; nothing here runs a probe under nife, and every probe image is zeroes.
# Needs the network for the first fetch.
# shellcheck disable=SC2012,SC2016 # registry paths are plain; the backticks are literal
set -eu
HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH='' cd -- "$HERE/../.." && pwd)
WORK="${WORK:-${TMPDIR:-/tmp}/nife-ufs2-probes}"
MODE=${1:?usage: probe.sh bare|nife|host shipped|patched [probe...] | probe.sh check}
TOOLCHAIN=$(sed -n 's/^channel = "\(.*\)"/\1/p' "$ROOT/rust-toolchain.toml")
mkdir -p "$WORK"

all='ufs-core rufs'

crate_of() {
    case "$1" in
    ufs-core) echo 'ufs-core 0.1.5' ;;
    rufs) echo 'rufs 0.6.0' ;;
    *) return 1 ;;
    esac
}

dep_line() {
    # ufs-core's library is named `ufs`, so the dependency keeps that name in both variants.
    case "$1" in
    ufs-core) echo 'ufs = { package = "ufs-core", version = "=0.1.5" }' ;;
    rufs) echo 'rufs = { version = "=0.6.0", default-features = false }' ;;
    esac
}

registry_copy() {
    # $1 crate name, $2 version. Fetches through a throwaway probe when the registry lacks it.
    src=$(ls -d "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/"$1-$2" 2>/dev/null | head -1 || true)
    if [ -z "$src" ]; then
        f="$WORK/.fetch-$1"
        rm -rf "$f" && mkdir -p "$f/src" && : >"$f/src/lib.rs"
        printf '[package]\nname = "fetch"\nversion = "0.1.0"\nedition = "2021"\n[workspace]\n[dependencies]\n%s = "=%s"\n' "$1" "$2" >"$f/Cargo.toml"
        (cd "$f" && cargo +"$TOOLCHAIN" fetch -q)
        src=$(ls -d "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/"$1-$2" | head -1)
    fi
    echo "$src"
}

probe_src() {
    case "$MODE-$VARIANT-$1" in
    bare-patched-rufs) echo rufs-bare-port.rs ;;
    bare-*) echo "$1-bare.rs" ;;
    *) echo "$1.rs" ;;
    esac
}

patch_of() {
    # The mode's own patch, else the bare one: a crate that builds no_std builds as std too.
    for m in "$MODE" nife bare; do
        [ "$MODE" = bare ] && [ "$m" = nife ] && continue
        [ -f "$HERE/patches/$1-$m.patch" ] && { echo "$HERE/patches/$1-$m.patch"; return; }
    done
    echo "probe.sh: no patch for $1" >&2
    return 1
}

if [ "$MODE" = check ]; then
    img="$WORK/img"
    commit=fb4675b704c55f6c1037556698e6a218f49c5836
    want_sha=5ec811d03c028566c5f66ecb7dda09ab31eed1a490bccf5e3d96dd6ddd154da5
    mkdir -p "$img"
    if [ ! -f "$img/ufs-little.img" ]; then
        curl -sSfL -o "$img/ufs-little.img.zst" \
            "https://raw.githubusercontent.com/realchonk/fuse-ufs/$commit/resources/ufs-little.img.zst"
        zstd -q -d -f "$img/ufs-little.img.zst" -o "$img/ufs-little.img"
    fi
    got_sha=$(sha256sum "$img/ufs-little.img" 2>/dev/null || shasum -a 256 "$img/ufs-little.img")
    [ "${got_sha%% *}" = "$want_sha" ] || { echo "probe.sh: image hash ${got_sha%% *}, want $want_sha" >&2; exit 1; }
    MODE=bare; VARIANT=patched
    port=$WORK/.patched/check/rufs
    rm -rf "$port" && mkdir -p "$WORK/.patched/check"
    cp -R "$(registry_copy rufs 0.6.0)" "$port"
    (cd "$port" && patch -s -p1 <"$(patch_of rufs)")
    for v in shipped port; do
        dir="$WORK/check-$v"
        rm -rf "$dir" && mkdir -p "$dir/src"
        {
            printf '[package]\nname = "rufs-host"\nversion = "0.1.0"\nedition = "2024"\nlicense = "MIT OR Apache-2.0"\n[workspace]\n'
            printf '[features]\nport = []\n[dependencies]\n'
            echo 'ufs = { package = "ufs-core", version = "=0.1.5" }'
            if [ "$v" = port ]; then
                printf 'rufs = { path = "%s", default-features = false }\n' "$port"
            else
                echo 'rufs = "=0.6.0"'
            fi
        } >"$dir/Cargo.toml"
        cp "$HERE/probes/rufs-host.rs" "$dir/src/main.rs"
        feat=""
        [ "$v" = port ] && feat="--features port"
        # shellcheck disable=SC2086
        (cd "$dir" && CARGO_TARGET_DIR="$WORK/.target-check-$v" cargo +"$TOOLCHAIN" run -q --release $feat -- "$img/ufs-little.img") >"$WORK/check-$v.out" 2>&1 ||
            echo "probe.sh: $v run failed, see $WORK/check-$v.out" >&2
    done
    for v in shipped port; do
        echo "$v $(grep -c '' "$WORK/check-$v.out") lines $(cksum <"$WORK/check-$v.out")"
    done
    if diff "$WORK/check-shipped.out" "$WORK/check-port.out"; then echo "check: identical"; else echo "check: DIFFER"; fi
    exit 0
fi

VARIANT=${2:?usage: probe.sh bare|nife|host shipped|patched [probe...]}
shift 2

write_probe() {
    dir="$WORK/$MODE-$VARIANT/$1"
    rm -rf "$dir"
    mkdir -p "$dir/src"
    {
        printf '[package]\nname = "probe"\nversion = "0.1.0"\nedition = "2024"\n[workspace]\n[dependencies]\n'
        dep_line "$1"
        if [ "$MODE" = bare ]; then
            # probes/bare.rs's allocator: the tree's vendored talc, and the lock its example uses.
            echo "talc = { path = \"$ROOT/vendor/talc\" }"
            echo 'spinning_top = "0.3"'
        fi
        if [ "$VARIANT" = patched ]; then
            # shellcheck disable=SC2046 # "name version", split on purpose
            set -- $(crate_of "$1")
            patch=$(patch_of "$1")
            copy="$WORK/.patched/$MODE/$1"
            rm -rf "$copy" && mkdir -p "$WORK/.patched/$MODE"
            cp -R "$(registry_copy "$1" "$2")" "$copy"
            (cd "$copy" && patch -s -p1 <"$patch")
            printf '[patch.crates-io]\n%s = { path = "%s" }\n' "$1" "$copy"
        fi
        printf '[profile.release]\npanic = "abort"\n[profile.dev]\npanic = "abort"\n'
    } >"$dir/Cargo.toml"
    cp "$HERE/probes/$(probe_src "$1")" "$dir/src/main.rs"
    [ "$MODE" = bare ] && cat "$HERE/probes/bare.rs" >>"$dir/src/main.rs"
    return 0
}

report() {
    # $1 label, $2 log
    crate=$(grep -m1 -oE 'compile `[^`]+`|failed to run custom build command for `[^`]+`' "$2" || true)
    echo "$1 FAIL $crate | $(grep -m1 -E '^error(\[|:)' "$2" || true)"
}

want="$*"
[ -n "$want" ] || want=$all
for p in $want; do
    write_probe "$p"
    dir="$WORK/$MODE-$VARIANT/$p"
    case "$MODE" in
    host)
        log="$dir/build-host.log"
        if (cd "$dir" && CARGO_TARGET_DIR="$WORK/.target-host" cargo +"$TOOLCHAIN" build --release) >"$log" 2>&1; then
            echo "host $VARIANT $p PASS"
        else
            report "host $VARIANT $p" "$log"
        fi ;;
    bare)
        for t in aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf x86_64-unknown-none; do
            log="$dir/build-$t.log"
            if (cd "$dir" && CARGO_TARGET_DIR="$WORK/.target-bare" cargo +"$TOOLCHAIN" build --release \
                --target "$t" --config 'target.x86_64-unknown-none.rustflags=["-C","relocation-model=static"]') >"$log" 2>&1; then
                echo "$t $VARIANT $p PASS"
            else
                report "$t $VARIANT $p" "$log"
            fi
        done ;;
    nife)
        for a in aarch64 riscv64 x86_64; do
            log="$dir/build-$a.log"
            if (cd "$dir" && RUSTUP_TOOLCHAIN="$ROOT/target/nife-farm" CARGO_TARGET_DIR="$WORK/.target-$a" \
                cargo build --release -Zjson-target-spec \
                -Zbuild-std=core,alloc,std,panic_abort -Zbuild-std-features=compiler-builtins-mem \
                --target "$ROOT/targets/$a-unknown-nife.json") >"$log" 2>&1; then
                echo "$a-unknown-nife $VARIANT $p PASS"
            else
                report "$a-unknown-nife $VARIANT $p" "$log"
            fi
        done ;;
    *) echo "probe.sh: unknown mode $MODE" >&2; exit 2 ;;
    esac
done
