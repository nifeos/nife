---
status: REFUSED
promoted_from: ufs2-read-and-write-against-freebsds-tools
raised: 2026-10-30
refused_by: §34
---
# 981. UFS2, read and write: a Rust server with FreeBSD's tools as the oracle

Refused 2026-10-31 (UTC) on #2927, by §34 (RedoxFS is the primary filesystem, on three conditions).
RedoxFS stays the launch store, and nife does not build a UFS2 server. The next measurement for the
launch store is ext4 through `rsext4` under milestone 37's injector, which the proposal
`ext4-through-rsext4-under-the-crash-injector` carries. This branch's name still says UFS2 because
the pull request that holds it predates the refusal.

## What it proposed

A Rust UFS2 server for the launch store, built from `rufs` and `ufs-core`. FreeBSD's `newfs`,
`makefs` and `fsck_ffs` would judge it on a host. Its four phases were the builds, an image writer
and reader, the write path, and a crash claim that the injector of milestone 37 (Prove RedoxFS's
crash consistency) measures. Milestone 982 (Retire RedoxFS once a launch store proves itself) would then have
removed RedoxFS.

## What was measured before the refusal

Phase 1 ran on 2026-10-31 (UTC), and [`ufs2-builds-2026-10-31.md`](../../notes/ufs2-builds-2026-10-31.md)
keeps it with its probe script. `ufs-core` builds for the three bare targets with a 6-line patch and
for nife `std` unchanged. `rufs` builds for nife `std` with a 49-line patch, and a host round trip
over FreeBSD's own test image prints the same output before and after that patch. `rufs` depends on
bincode 2.0.1, which RUSTSEC-2025-0141 marks unmaintained; its unreleased main moved to
bincode-next.

## Why it is refused

The four-route comparison on #2927 priced UFS2 against RedoxFS, ext2 and ext4, and
[the launch filesystem's candidates](../../notes/launch-filesystem-candidates.md) keeps its result.
UFS2 needed a writer, an on-device `fsck` and a crash mechanism that no Rust implementation has. No
data block carries a checksum, so a lying device can never reach milestone 37's zero silently wrong.
And few drives other systems made are UFS2. RedoxFS already meets the launch store's requirements,
and the pin bump that reopened the question applies without a conflict.

## Revisit

- **Condition.** The ext4 measurement through `rsext4` fails, and a RedoxFS pin bump starts to
  cost more than a few lines, so that nife would have to own the engine outright.

## Index row

A Rust UFS2 server as the launch store, judged by FreeBSD's tools. Refused 2026-10-31: RedoxFS
stays the launch store, and ext4 through `rsext4` is the next measurement. Phase 1's builds live in
notes/ufs2-builds-2026-10-31.md. It reopens if that measurement fails and RedoxFS stops being
bumpable.
