# Build output on its own volume

*Written 2026-10-06 (UTC) by lane/build-volume, after patagonia filled its disk while lanes were
building. It is a runbook for any Mac that builds nife, and nobody has run it yet: run it at a quiet
moment and correct this file with what the machine says. The file name and the volume name
`nife-build` are provisional and have not been put to calef.*

## Why

On macOS, Time Machine takes an hourly local APFS snapshot of each volume it backs up, and a
snapshot holds every block that existed when it was taken. **A snapshot is of the whole volume, so
a path excluded from Time Machine is still inside it.** Excluding `~/projects` keeps build output
off the backup disk. It does not keep it out of the local snapshots on the Mac itself.

That matters because lanes delete build output constantly. Every pruned worktree takes a `target/`
with it, and the blocks stay allocated until the last snapshot that saw them expires, about a day
later. Measured on patagonia (a 460 GiB APFS container, `disk3`) on 2026-10-06:

| Observation | Value |
| --- | --- |
| Local snapshots on the Data volume | 20 |
| Space they held, freed by `tmutil thinlocalsnapshots / 999999999999 4` | about 160 GiB |
| Free space right after that cleanup | 191 GiB |
| Free space one hour later, lanes building | 155 GiB |

Local snapshots cannot be turned off on current macOS. What can be done is to put the churn on a
volume Time Machine does not back up, because it does not snapshot a volume it does not back up.
An APFS volume added to the same container shares the container's free space, so nothing is
partitioned and no size has to be guessed.

## Before you start

Every lane is idle. These must print nothing:

    pgrep -l 'qemu|cargo|rustc|clippy|cbmc|kani'
    pgrep -lf 'script/test|script/lint|preflight-queue|xtask'

No worktree holds uncommitted work you have not accounted for. The copy below carries uncommitted
files across, but you want to know what is there before you delete the original:

    git -C <main checkout> worktree list --porcelain | sed -n 's/^worktree //p' |
    while IFS= read -r w; do
        [ -n "$(git -C "$w" status --porcelain 2>/dev/null | head -1)" ] && echo "DIRTY $w"
    done

(`while read` rather than `for w in $list`, because zsh does not word-split a variable.)

Free space for the copy. Build output is deleted rather than copied (step 3 says why), so the copy
needs only the source:

    du -sh <worktree root>                       # everything
    find <worktree root> -mindepth 2 -type d -name target -prune -exec du -sk {} + |
        awk '{s+=$1} END {printf "%.1f GiB of it is target/\n", s/1048576}'

On patagonia on 2026-10-06 that was 45 GiB across 65 worktrees, 36 GiB of it `target/`, so the copy
needs about 12 GiB.

## The runbook

### 1. Find the container and add the volume

Do not assume `disk3`. Ask which container holds the Data volume:

    C=$(diskutil info /System/Volumes/Data | awk -F': *' '/APFS Container:/ {print $2}')
    echo "$C"                                   # for example disk3
    diskutil apfs list "$C" | grep -E 'Name:|Role'

Then add the volume, with no quota and no reserve, so it draws on the container's free space like
every other volume there:

    diskutil apfs addVolume "$C" APFS nife-build
    ls -ld /Volumes/nife-build
    diskutil info /Volumes/nife-build | grep -E 'Owners|File System Personality|FileVault'

`APFS` is case-insensitive, the same as the Data volume, which is what the tree has always been
built on. If `ls -ld` shows root as the owner, take it: `sudo chown "$(id -un)":staff
/Volumes/nife-build`. If `Owners` reads `Disabled`, run `sudo diskutil enableOwnership
/Volumes/nife-build`.

### 2. Exclude it from Time Machine, and see that it gets no snapshots

Apple's documentation does not say whether Time Machine backs up an extra volume on the internal
disk by default, and `man tmutil` says erasing a volume makes Time Machine "apply default behavior"
to it, which implies the default is not necessarily exclusion. So exclude it explicitly. `-v` is a
volume exclusion keyed by the volume's UUID, and `man tmutil` calls it "the only supported way to
exclude or unexclude a volume". It needs root, and the terminal needs Full Disk Access.

    sudo tmutil addexclusion -v /Volumes/nife-build
    tmutil isexcluded /Volumes/nife-build        # [Excluded]  /Volumes/nife-build

Then prove it on the machine. Take a snapshot now rather than wait for the hourly one, and compare:

    tmutil localsnapshot
    tmutil listlocalsnapshots /System/Volumes/Data   # lists a snapshot from a moment ago
    tmutil listlocalsnapshots /Volumes/nife-build    # lists none

Optional: `sudo mdutil -i off /Volumes/nife-build` stops Spotlight indexing build output.

### 3. Move the worktrees, leaving a symlink

Delete build output first. It is rebuilt on demand, and copying it would cost more than the rebuild
it saves: tens of gigabytes through the disk, carrying absolute paths baked under the old location.
No tracked path in this repository is named `target` (`git ls-files | grep -E '(^|/)target/'`
prints nothing), so every directory of that name is build output.

    cd ~/projects
    find nife-worktrees -mindepth 2 -type d -name target -prune -exec rm -rf {} +
    ditto nife-worktrees /Volumes/nife-build/nife-worktrees
    (cd nife-worktrees && find . | wc -l); (cd /Volumes/nife-build/nife-worktrees && find . | wc -l)

The two counts must match. Then swap in the symlink:

    mv nife-worktrees nife-worktrees.old
    ln -s /Volumes/nife-build/nife-worktrees nife-worktrees
    git -C <main checkout> worktree list | grep -c prunable      # 0

Keep `nife-worktrees.old` until the checks under "After the move" pass, then delete it and reclaim
what the snapshots still hold:

    rm -rf <worktree root>.old
    tmutil thinlocalsnapshots / 999999999999 4
    df -h /System/Volumes/Data

### 4. The main checkout's `target/` (optional, recommended)

The main checkout holds 10 GiB in `target/` and about 5 GiB more in its sub-workspaces'
`target/` directories (`fs_server`, `tools/redoxfs_host`, `redoxfs_server` and the rest). Its
`target/` churns at every merge, because the maintainer rebuilds the farm there. Move it with a
symlink:

    cd <main checkout>
    rm -rf target
    mkdir /Volumes/nife-build/nife-main-target
    ln -s /Volumes/nife-build/nife-main-target target
    git status --short | grep target            # prints nothing
    cargo xtask std-src                         # rebuilds the farm and links its toolchain name

**Symlink rather than `CARGO_TARGET_DIR`.** Exported in a shell profile, `CARGO_TARGET_DIR` would
send every worktree to one target directory. It also moves only what cargo writes. The farm
(`xtask/src/farm.rs`, `farm_dir()`) and the images under `target/` are spelled relative to the
workspace root, and `std_exerciser`'s target directory is pinned on purpose, so build output would
land on two volumes. The symlink moves all of it.

The sub-workspaces' `target/` directories stay where they are. They are smaller and rarely deleted.

## Hazards, checked against this tree on 2026-10-06

### Hard links between the farm and `~/.rustup`

The likeliest breakage, and it does not break. `std_src()` in `xtask/src/farm.rs` builds each worktree's patched toolchain by hard-linking the
nightly's `bin/` and `lib/` with `cp -al`, and a hard link cannot cross volumes. The code already
handles that: when the `cp -al` calls fail, it deletes the half-made `bin/` and `lib/` and copies
them with `cp -R`. That fallback was written for containers that mount the checkout on a different
filesystem. `xtask`'s `run()` returns false on any nonzero exit, and BSD `cp` exits nonzero when a
link fails, so the fallback is reached. Two costs follow:

- Each farm takes real space, about 1.2 GiB (the nightly of 2026-10-06 has a 1.1 GiB `lib/` and a
  61 MiB `bin/`), where a hard-linked farm costs almost nothing. Four worktrees had farms on
  2026-10-06. The copies live on the unsnapshotted volume and go away when the worktree is pruned.
- Before this lane, the failed `cp -al` would have printed one "Cross-device link" error per file
  before falling back, hundreds of lines that read like a broken build. `std_src()` now compares the
  device numbers of the sysroot and the farm first, skips `cp -al` when they differ, and prints one
  line that names this note.

### Git worktree metadata

Each `.git/worktrees/<name>/gitdir` names its worktree by absolute path,
and each worktree's `.git` file names `~/projects/nife/.git/worktrees/<name>`. The main checkout does
not move, so the second kind is untouched. The first kind still resolves, through the symlink.
Tested with a symlinked directory: git records the *physical* path when a worktree is added through
a symlink, and `git worktree remove` accepts either spelling. So worktrees made after the move
appear in `git worktree list` under `/Volumes/nife-build/...` while older ones keep the old
spelling. Both work. `git worktree repair` is not needed.

### Paths baked into cached artifacts

The memory behind this one: after the 2026-08-15 rename,
cached test binaries carried a dead path from `env!("CARGO_MANIFEST_DIR")`, and build scripts had
baked absolute linker-script paths. Here the old path stays valid through the symlink, so a stale
artifact would still find its files. Step 3 deletes every `target/` anyway, which removes the
question.

### Cargo canonicalizes

Cargo finds the workspace from the process's working directory, which the
kernel reports as the physical path. Measured: from a directory reached through a symlink, `cargo
metadata` and `git rev-parse --show-toplevel` both print the physical path. So the first build after
the move is a full rebuild, every artifact records `/Volumes/nife-build/...`, and `xtask`'s
`workspace_root()` (read at run time from `CARGO_MANIFEST_DIR`) returns the physical path too.

### The toolchain links

`rustup toolchain link` stores a path string. Each worktree links its own farm as
`nife-dev-<directory>-<hash>` (notes/std.md), so step 3 leaves a moved worktree's link dangling,
and its next `cargo xtask std-src` relinks it; any worktree's `std-src` removes the dangling links
of worktrees that are gone. A link spelled through `~/projects/nife/target` keeps working after
the main checkout's `target/` becomes a symlink. `link_own_toolchain()` and `foreign_std_sources()`
canonicalize both sides before comparing, so the mixed spellings do not cause a relink on every
call or a false accusation. The legacy shared `nife-dev` link (on 2026-10-06 it named
`779-confined-fuzz`) is the maintainer's to remove once no lane predates 2026-10-10.

### Scripts that spell the worktrees path or resolve paths

Every hit in `script/`, `helpers/`,
`xtask/` and `briefs/`:

| Where | What it does | After the move |
| --- | --- | --- |
| `script/preflight-queue` | `WT` defaults to `~/projects/nife-worktrees/preflight`; it then matches `WT` against `pgrep -f` (QEMU's command line) and `lsof`'s working directories | Broke silently: both report the physical path, so the "somebody is using it" refusal and the QEMU sweep would never match. Fixed in this lane: `WT` is resolved with `realpath` |
| `script/effort` | counts an opencode session as nife's if its directory contains `/projects/nife-worktrees/` | Undercounted: opencode records the physical directory. Fixed in this lane: it matches `/nife-worktrees/` anywhere, with a selftest case |
| `script/claim` | default worktree is `$HOME/projects/nife-worktrees/<slug>` | Works; git records the physical path |
| `script/preflight-queue` (`--git-common-dir`), `script/lint`, `.githooks/pre-push`, `helpers/*.py` | `realpath` of, or `git rev-parse` for, the repository root | Works; both sides of each comparison resolve the same way |
| `script/nanny`, `helpers/at-risk-check.sh` | read paths out of `git worktree list` and use them | Works; either spelling is a valid path |
| `script/stranger-test` | matches QEMU by its clone path under a temporary directory | Not affected |
| `briefs/merge-and-cleanup.md` | example paths under `~/projects/nife-worktrees` | Works through the symlink |

### `.gitignore` and a symlinked `target`

`target/` with a slash matches only a directory, and git
does not follow symlinks, so before this lane a symlinked `target` in the main checkout showed up in
`git status` as untracked and `git add -A` would have committed it. Tested in a scratch repository.
This lane adds an anchored `/target` line.

## After the move

In one worktree (any lane's, or a fresh one from `script/claim`):

    git -C <main checkout> worktree list
    cd <worktree root>/<one>
    cargo build -p xtask && script/lint
    cargo xtask std-src            # prints the "different volumes ... copied" line once
    cargo xtask std-stamp          # must equal the main checkout's

Then the point of the exercise. Delete a large `target/` on the new volume and watch the space come
back at once rather than a day later:

    df -h /Volumes/nife-build; du -sh target
    rm -rf target
    df -h /Volumes/nife-build      # free space is up by about what du said

## Rollback

Every lane idle, as before. Then reverse step 3 and step 4:

    cd ~/projects
    rm nife-worktrees                                   # the symlink only
    find /Volumes/nife-build/nife-worktrees -mindepth 2 -type d -name target -prune -exec rm -rf {} +
    ditto /Volumes/nife-build/nife-worktrees nife-worktrees
    cd <main checkout> && rm target && cargo xtask std-src
    diskutil apfs deleteVolume /Volumes/nife-build

Deleting the volume also drops its Time Machine exclusion, which is keyed by its UUID.

## BUGS

- **Source files and `~/.rustup` stay on the snapshotted Data volume.** Only the worktrees and the
  main checkout's `target/` move. The main checkout's own source stays put deliberately: the
  harness's project directory for this repository is named after the checkout's physical path, so
  moving it would orphan the session records `script/effort` reads and the agents' memory. A nightly
  toolchain bump still leaves its predecessor in snapshots for a day.
- **`cargo clean` in the main checkout deletes the symlink, not what it points at.** Measured with a
  scratch crate: `cargo clean` reported "Removed 1 file, 115B total" and left the build output on the
  other volume. `rm -rf target` does the same. The next build then makes a real `target/` on the
  Data volume and quietly undoes step 4. After either, check `ls -ld <main checkout>/target`, empty
  `/Volumes/nife-build/nife-main-target` by hand, and redo the `ln -s`. Nothing gates this; it is
  machine state, not tree state.
- **A farm on the new volume is a copy, not a hard link**, at about 1.2 GiB each. Moving
  `RUSTUP_HOME` onto the same volume would restore the hard links and take toolchain churn out of
  the snapshots too, but it changes every Rust project on the machine and has not been tried.
- **The sub-workspaces' `target/` directories in the main checkout stay on the Data volume**, about
  5 GiB on patagonia.
- **The new volume is not protected by the FileVault password.** On Apple Silicon its contents are
  still encrypted at rest by the hardware, but it unlocks at boot without a login. It holds build
  output and checkouts of a public repository. If that ever changes, `addVolume` takes
  `-passprompt`.
- **None of this has run yet.** Each command was checked against `man` pages, this tree's code and
  scratch experiments on patagonia, not by performing the move.
