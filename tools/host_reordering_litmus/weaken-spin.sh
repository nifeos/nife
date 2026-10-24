#!/bin/sh
# weaken-spin.sh: points the workspace's `spin` 0.12.3 at a copy with weakened orderings, so the
# kernel suite runs with every IrqSafeMutex (kernel/src/sync.rs wraps spin::Mutex) broken.
#
#   release  the guard's unlock store is Relaxed instead of Release
#   both     that, and the lock's compare-exchange is Relaxed instead of Acquire
#
# It edits Cargo.toml and Cargo.lock in place; it is for a throwaway checkout (a CI job), never a
# lane worktree's committed state. See notes/benchmarks/host-reordering-under-tcg.md.
set -eu
cd "$(dirname "$0")/../.."
variant="${1:?usage: weaken-spin.sh release|both}"
cargo fetch --quiet
src="$(find "${CARGO_HOME:-$HOME/.cargo}/registry/src" -maxdepth 2 -type d -name spin-0.12.3 | head -1)"
[ -n "$src" ] || { echo "weaken-spin: spin-0.12.3 is not in the registry" >&2; exit 1; }
dst=target/weakened-spin
rm -rf "$dst"
mkdir -p target
cp -R "$src" "$dst"
chmod -R u+w "$dst"
f="$dst/src/mutex/spin.rs"
grep -q 'self.inner.lock.store(false, Ordering::Release);' "$f"
sed -i.bak 's/self.inner.lock.store(false, Ordering::Release);/self.inner.lock.store(false, Ordering::Relaxed);/' "$f"
if [ "$variant" = both ]; then
    sed -i.bak 's/compare_exchange\(_weak\)\{0,1\}(false, true, Ordering::Acquire, Ordering::Relaxed)/compare_exchange\1(false, true, Ordering::Relaxed, Ordering::Relaxed)/' "$f"
fi
rm -f "$f.bak"
sed -i.bak 's|^\[patch.crates-io\]$|[patch.crates-io]\nspin = { path = "target/weakened-spin" }|' Cargo.toml
rm -f Cargo.toml.bak
echo "weaken-spin: $variant; the orderings left in the lock are:"
grep -n 'Ordering::' "$f" | grep -v '^.*//' | grep -v SeqCst
cargo update --quiet -p spin@0.12.3 2>/dev/null || true
