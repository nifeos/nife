#!/bin/sh
# weaken-spin.sh: weakens the orderings of the kernel's lock, `crates/spin_lock`, in place, so the
# kernel suite runs with every IrqSafeMutex (kernel/src/sync.rs wraps spin_lock::SpinLock) broken.
#
#   release  the unlock store is Relaxed instead of Release (the guard's drop and force_unlock)
#   both     that, and the lock's compare-exchanges are Relaxed instead of Acquire on success
#
# It edits crates/spin_lock/src/lib.rs in place; it is for a throwaway checkout (a CI job), never a
# lane worktree's committed state. See notes/benchmarks/host-reordering-under-tcg.md.
set -eu
cd "$(dirname "$0")/../.."
variant="${1:?usage: weaken-spin.sh release|both}"
f=crates/spin_lock/src/lib.rs
grep -q 'self.locked.store(false, ordering);' "$f"
sed -i.bak 's/self.locked.store(false, ordering);/let _ = ordering; self.locked.store(false, Ordering::Relaxed);/' "$f"
grep -q 'self.locked.store(false, Ordering::Relaxed);' "$f"
if [ "$variant" = both ]; then
    grep -q '(false, true, Ordering::Acquire, Ordering::Relaxed)' "$f"
    sed -i.bak 's/(false, true, Ordering::Acquire, Ordering::Relaxed)/(false, true, Ordering::Relaxed, Ordering::Relaxed)/' "$f"
    if grep -q 'Ordering::Acquire' "$f"; then
        echo "weaken-spin: an Acquire survived the edit" >&2
        exit 1
    fi
fi
rm -f "$f.bak"
echo "weaken-spin: $variant; the orderings left in the lock are:"
grep -n 'Ordering::' "$f" | grep -v '^[0-9]*: *//' | grep -v '^[0-9]*: *use '
