#!/bin/sh
#
# helpers/pre-push-lint-cache.sh: the machine-local pass marker .githooks/pre-push consults before
# it runs `script/lint --no-cargo`, so a push that follows a passing lint on the same tree does not
# pay for the run again (fix 3 in notes/lint-cost-2026-10-18.md, section 5).
#
# Usage (from the hook):
#
#     helpers/pre-push-lint-cache.sh key          # print the key of this tree's lint inputs
#     helpers/pre-push-lint-cache.sh check <key>  # exit 0 when a marker for <key> exists
#     helpers/pre-push-lint-cache.sh write <key>  # record that these checks passed on this tree
#
# The hook asks for the key, skips the run when a marker for it exists, and otherwise runs
# `script/lint --no-cargo` and writes the marker. The verdict cannot move: the key names every
# input the checks read, so a skipped hook passes exactly when the run it skips would have. CI
# reruns everything and stays the authority; this marker only decides whether this machine repeats
# work it just did.
#
# # What the key covers
#
# Every input a `--no-cargo` check reads, each named for the checks that read it:
#
# - `tree`: the working tree's contents, untracked files included, via a temporary index
#   (`git add -A` with GIT_INDEX_FILE pointed outside the tree) hashed with `git ls-files -s`.
#   This is what nearly every check reads, script/lint and its helpers included, so the
#   `--no-cargo` partition itself (helpers/lint-no-cargo.awk) is covered too. Untracked files
#   count: the lint-cost lane's own lint failed on untracked notes naming a section.
# - `index`: the real index. `git grep`, `git ls-files` and the citations ratchet's "Commit first."
#   refusal read the staged state, which the working tree does not determine (a `git add` followed
#   by reverting the file leaves the tree at HEAD and the index ahead of it).
# - `head`: the committed tip. The citations ratchet judges the tip, not the working tree, and its
#   commit-first refusal reads `git status`, which tree, index and HEAD determine together.
# - `base`: `git merge-base HEAD origin/main`, empty when there is none. Every ratchet in
#   `--no-cargo` (citations, prose, module blocks, file length, comment blocks, house style, cap
#   abbreviation, names) compares against that commit; an empty base is also a state the ratchets
#   see (they skip), so it keys like any other.
# - `tools`: the binary content and `--version` of the out-of-tree tools the kept sections run:
#   python3, typos, shellcheck, git. A typos dictionary update rides inside the binary and can flip
#   the spelling verdict on an unchanged tree, so a version string alone would not hold.
#
# Everything else the checks read was judged not to be an input, and the pull request that added
# this file says why for each: origin/main beyond its merge base with HEAD (no kept check reads it
# directly), the branch name (the moved-block lookup that names branches is lint-fleet's, which
# still runs on every push), the contents of ignored files (target/, __pycache__/: no kept check's
# verdict reads them; "tracked files the tree ignores" reads the ignore rules and the index, both
# keyed), environment variables (the only ones the kept sections read are selftest hooks, unset in
# a normal push), the OS-provided tools (/bin/sh, grep, sed, awk and the rest change only when the
# OS does, which is a different machine as far as a machine-local marker is concerned), and the
# network (the no-cargo sections run no `gh`; the fleet half of the hook is not covered by this
# marker and still runs on every push).
#
# # Where the marker lives, and what failure means
#
# The markers are files in a machine-local cache directory, one per key, named
# `<sha256 of key>.ok` and holding the key and a UTC stamp for a reader who finds one. The default
# location follows script/effort's cache (XDG_STATE_HOME, then ~/.local/state), and
# `NIFE_PRE_PUSH_LINT_CACHE` overrides it. **Never inside the worktree**: a hook or a launchd job
# must not drop files into the tree (the script/effort precedent), and an in-tree marker would
# feed the next tree hash, so the marker would unmake itself.
#
# Every failure degrades to running the checks, never to skipping and never to a refused push: a
# key that cannot be computed exits nonzero with no output, `check` exits nonzero, and `write`
# exits nonzero leaving the tree alone. A missing helper (an older tree) lands the same way from
# the hook's side, because the hook treats any failed `key` as "no key, run the checks".
#
# BUGS
#
# - The key costs about a second on this tree (one `git add -A` over it), paid on every push
#   whether or not the marker matches. It buys the skip; making it cheaper means hashing less,
#   which means guessing what the checks do not read.
# - `git add -A` writes blob objects for new content into the repository's object store, as every
#   `git add` does; unreachable ones are garbage-collected and the real index is never touched.
# - A worktree edited while the checks run leaves a marker for the state the run ended on. That
#   race belongs to the run itself, not to the marker: the hook computes the key after the run for
#   exactly this reason. One lane per worktree is the rule that keeps it closed.
#
# Name: provisional. Minted 2026-10-28 (UTC) by lane maintainer/pre-push-lint-skip: `pre-push-lint`
# for the hook and the checks it serves, `-cache` for what it keeps, the checks' last verdict for a
# set of inputs rather than a run. calef has not ratified it.

set -u

me="pre-push-lint-cache"

# sha256 of a file, or of stdin when no file is named. sha256sum is GNU (CI); shasum is what
# macOS ships. script/build-is-reproducible resolves the pair the same way.
if command -v sha256sum >/dev/null 2>&1; then
    hash_of() { sha256sum "$@" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
    hash_of() { shasum -a 256 "$@" | cut -d' ' -f1; }
else
    echo "$me: no sha256sum and no shasum on PATH" >&2
    exit 2
fi

# The marker's home, outside every worktree. NIFE_PRE_PUSH_LINT_CACHE overrides, so a selftest can
# point it at a throwaway directory and a person at a volume of their own. A relative override
# would resolve against the hook's working directory, which is a worktree, so it is refused: the
# degradation is a run, which is what a misconfigured cache earns.
cache_dir() {
    dir="${NIFE_PRE_PUSH_LINT_CACHE:-${XDG_STATE_HOME:-$HOME/.local/state}/nife/pre-push-lint}"
    case "$dir" in
    /*) printf '%s\n' "$dir" ;;
    *) echo "$me: $dir is not an absolute path; refusing a cache inside the tree" >&2; exit 1 ;;
    esac
}

# The worktree guard for check and write: an absolute path can still name somewhere inside the
# tree, and a marker there would feed the next tree hash.
refuse_inside_tree() {
    root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 1
    case "$1" in
    "$root"|"$root"/*)
        echo "$me: $1 is inside the worktree at $root; the marker would feed the next tree hash" >&2
        exit 1
        ;;
    esac
}

# The tools the kept `--no-cargo` sections run and this machine installs on its own. Everything
# else they run is the OS's (see the header).
LINT_TOOLS="python3 typos shellcheck git"

tools_hash() {
    tools=""
    for tool in $LINT_TOOLS; do
        path="$(command -v "$tool" 2>/dev/null || true)"
        if [ -z "$path" ]; then
            tools="$tools $tool:missing"
            continue
        fi
        # A missing --version leaves an empty field rather than failing the key: the binary hash
        # below already pins the tool, and a tool that cannot print a version still judges.
        version="$("$path" --version 2>/dev/null | head -n 1 || true)"
        binary="$(hash_of "$path" 2>/dev/null || true)"
        tools="$tools $tool:$path:$version:$binary"
    done
    printf '%s' "$tools" | hash_of
}

case "${1:-}" in
key)
    root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 1
    # The working tree, untracked non-ignored files included, hashed through a temporary index so
    # the real index is never touched. The temporary index itself is a scratch file outside the
    # tree: an in-tree scratch would feed the next tree hash.
    # mktemp pre-creates its file, and git refuses to read a zero-length index ("smaller than
    # expected"), so the name is reserved and then removed: git add starts an index that does not
    # exist and writes it whole.
    tmpidx="$(mktemp "${TMPDIR:-/tmp}/nife-lint-key.XXXXXX")" || exit 1
    rm -f "$tmpidx"
    tree="$(GIT_INDEX_FILE="$tmpidx" git -C "$root" add -A >/dev/null 2>&1 &&
        GIT_INDEX_FILE="$tmpidx" git -C "$root" ls-files -s 2>/dev/null | hash_of)" || {
        rm -f "$tmpidx"
        exit 1
    }
    rm -f "$tmpidx"
    # The staged state the git-backed checks read. An index mid-merge lists its unmerged entries
    # with their stages and keys as its own state, which is right: that tree is not the one on
    # either side of the merge.
    index="$(git -C "$root" ls-files -s 2>/dev/null | hash_of)" || exit 1
    head="$(git -C "$root" rev-parse HEAD 2>/dev/null)" || exit 1
    # Empty when there is no merge base, matching what the ratchets compute in the same situation.
    base="$(git -C "$root" merge-base HEAD origin/main 2>/dev/null || true)"
    tools="$(tools_hash)" || exit 1
    printf 'tree=%s index=%s head=%s base=%s tools=%s\n' "$tree" "$index" "$head" "$base" "$tools"
    ;;
check)
    [ -n "${2:-}" ] || { echo "$me: check needs the key" >&2; exit 2; }
    dir="$(cache_dir)" || exit 1
    refuse_inside_tree "$dir"
    file="$dir/$(printf '%s' "$2" | hash_of).ok"
    [ -f "$file" ] || exit 1
    ;;
write)
    [ -n "${2:-}" ] || { echo "$me: write needs the key" >&2; exit 2; }
    dir="$(cache_dir)" || exit 1
    refuse_inside_tree "$dir"
    # No marker is better than a refused push or a wrong skip, so every failure here is honest and
    # the hook ignores it. mktemp and mv keep a partial marker from ever being read as a whole one.
    mkdir -p "$dir" || exit 1
    file="$(printf '%s' "$2" | hash_of).ok" || exit 1
    tmp="$(mktemp "$dir/.write.XXXXXX")" || exit 1
    {
        date -u '+%Y-%m-%dT%H:%M:%SZ'
        printf '%s\n' "$2"
    } >"$tmp" || { rm -f "$tmp"; exit 1; }
    mv -f "$tmp" "$dir/$file" || { rm -f "$tmp"; exit 1; }
    # Markers stop being matched the moment any input moves, so an old one is dead weight. Two
    # weeks is generous against every cadence this machine's lanes run on; a failure to prune is
    # not a failure to cache.
    find "$dir" -name '*.ok' -mtime +14 -delete 2>/dev/null || true
    ;;
*)
    echo "$me: usage: helpers/pre-push-lint-cache.sh key|check <key>|write <key>" >&2
    exit 2
    ;;
esac
