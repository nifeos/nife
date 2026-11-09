#!/bin/sh
# build.sh: build the jh7110_entropy board-check payload and stage it for script/board-netboot.
#
#   tools/jh7110_entropy_board_check/build.sh                         # the crate commit Cargo.toml pins
#   tools/jh7110_entropy_board_check/build.sh --crate-commit <sha>    # another commit, for one build
#   tools/jh7110_entropy_board_check/build.sh --model                 # the QEMU rehearsal build
#   tools/jh7110_entropy_board_check/build.sh --model --rehearse      # ...and run it under QEMU virt
#
# The board build ends in target/jh7110-entropy-board-check/serve/, which holds the payload under
# the file name the board's network-first boot script asks for (nife-vf2.img) and a placeholder
# beside it under the archive's name, because that script falls back to the card unless both
# transfers succeed. The payload ignores the placeholder. Serve the directory with
# `script/board-netboot --root target/jh7110-entropy-board-check/serve`.
#
# --crate-commit clones nifeos/jh7110_entropy at that commit under target/ and builds against the
# clone through a one-build `[patch]`, so Cargo.toml and Cargo.lock stay as committed. The payload
# prints the commit it was built against on its first record either way.
#
# --rehearse boots the model build under QEMU virt for at most ten minutes and runs
# `results.py summarize` and `results.py check --allow-model` on its log. It needs
# qemu-system-riscv64 on PATH. See notes/jh7110-entropy-board-check.md.
#
# Name: provisional.
set -eu
cd "$(dirname "$0")/../.."
. helpers/qemu-path.sh

TOOL=tools/jh7110_entropy_board_check
OUT=target/jh7110-entropy-board-check
TARGET=riscv64imac-unknown-none-elf
COMMIT=""
MODEL=no
REHEARSE=no
while [ $# -gt 0 ]; do
    case "$1" in
        --crate-commit)
            [ $# -ge 2 ] || { echo "build.sh: --crate-commit needs a commit" >&2; exit 2; }
            COMMIT="$2"; shift 2 ;;
        --model) MODEL=yes; shift ;;
        --rehearse) REHEARSE=yes; shift ;;
        *) echo "build.sh: unknown argument $1 (--crate-commit <sha>, --model, --rehearse)" >&2; exit 2 ;;
    esac
done
if [ "$REHEARSE" = yes ] && [ "$MODEL" = no ]; then
    echo "build.sh: --rehearse runs the model build under QEMU; add --model" >&2
    exit 2
fi
mkdir -p "$OUT"

set --
if [ -n "$COMMIT" ]; then
    CLONE="$PWD/$OUT/crate"
    rm -rf "$CLONE"
    git clone --quiet https://github.com/nifeos/jh7110_entropy "$CLONE"
    git -C "$CLONE" checkout --quiet --detach "$COMMIT"
    RESOLVED="$(git -C "$CLONE" rev-parse HEAD)"
    set -- --config "patch.'https://github.com/nifeos/jh7110_entropy'.jh7110_entropy.path=\"$CLONE\""
else
    RESOLVED="$(sed -n 's/^jh7110_entropy = .*rev = "\([0-9a-f]*\)".*/\1/p' "$TOOL/Cargo.toml")"
fi
[ -n "$RESOLVED" ] || { echo "build.sh: could not tell which crate commit this build uses" >&2; exit 1; }

FEATURES=""
[ "$MODEL" = yes ] && FEATURES="--features model"
# shellcheck disable=SC2086 # FEATURES is empty or one flag and its value
BOARD_CHECK_CRATE_COMMIT="$RESOLVED" cargo build --quiet --release --target "$TARGET" \
    --manifest-path "$TOOL/Cargo.toml" $FEATURES "$@"
ELF="$TOOL/target/$TARGET/release/jh7110_entropy_board_check"

if [ "$MODEL" = yes ]; then
    echo "build.sh: model build of crate commit $RESOLVED at $ELF (not a board payload)"
    if [ "$REHEARSE" = yes ]; then
        LOG="$OUT/rehearsal.log"
        helpers/qemu-bounded.sh 600 qemu-system-riscv64 -machine virt -smp 1 -m 256M \
            -display none -serial stdio -kernel "$ELF" > "$LOG" || true
        python3 -I "$TOOL/results.py" summarize "$LOG" > "$OUT/rehearsal-results.md"
        python3 -I "$TOOL/results.py" check --allow-model "$OUT/rehearsal-results.md"
    fi
    exit 0
fi

SYSROOT="$(rustc --print sysroot)"
HOST="$(rustc -vV | sed -n 's/^host: //p')"
OBJCOPY="$SYSROOT/lib/rustlib/$HOST/bin/llvm-objcopy"
[ -x "$OBJCOPY" ] || { echo "build.sh: no llvm-objcopy at $OBJCOPY (the llvm-tools component)" >&2; exit 1; }
mkdir -p "$OUT/serve"
IMG="$OUT/serve/nife-vf2.img"
"$OBJCOPY" -O binary "$ELF" "$IMG"
# The one field booti checks, as script/board-image checks it.
MAGIC="$(od -An -t x4 -j 56 -N 4 "$IMG" | tr -d ' \n')"
[ "$MAGIC" = "05435352" ] || { echo "build.sh: $IMG has no RISC-V Image header magic (got '$MAGIC')" >&2; exit 1; }
printf 'jh7110_entropy board check: a placeholder so the boot script takes both files from the network\n' \
    > "$OUT/serve/nife-initial-ramdisk.img"
if command -v shasum >/dev/null 2>&1; then SUM="$(shasum -a 256 "$IMG")"; else SUM="$(sha256sum "$IMG")"; fi
echo "build.sh: crate commit $RESOLVED"
echo "build.sh: payload $SUM"
echo "build.sh: serve it with: script/board-netboot --root $OUT/serve"
