# Upstream drafts for milestone 190

[Milestone 190 (ext4, read and write: a Rust implementation with libext2fs as the host-side
oracle)](../../0190-ext4-read-write.md) sends its `rsext4` fixes to `rcore-os/tgoskits`.
Each submission has a draft here for the architect's review: a `.md` with the base commit, the
title, the body in Chinese and in English and the test evidence, and a `.patch` beside it holding
the `git format-patch` output. Nobody files a draft until the architect has reviewed its text and
given a separate go.

- [`4-fuzz-targets.md`](4-fuzz-targets.md): upstream item 4, seven `cargo fuzz` targets over the
  parsers, mount and journal replay, with an `mke2fs` seed corpus; it files after item 5.
- [`7-mount-refusal-reason.md`](7-mount-refusal-reason.md): upstream item 7, a refused mount
  reports its reason instead of a bare `Io`.
- [`8-whole-inode-times.md`](8-whole-inode-times.md): upstream item 8, `InodeInfo` reports whole
  times, nanoseconds and epoch bits included.

*Name: provisional. For the drafts the parent's upstream round cites. calef names directories.*

- `4-fuzz-targets` Name: provisional. The upstream item's number and what it adds. calef names
  files.
- `7-mount-refusal-reason` Name: provisional. The upstream item's number and what it fixes. calef
  names files.
- `8-whole-inode-times` Name: provisional. The upstream item's number and what it fixes. calef
  names files.
