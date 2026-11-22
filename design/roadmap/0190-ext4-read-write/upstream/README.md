# Upstream drafts for milestone 190

[Milestone 190 (ext4, read and write: a Rust implementation with libext2fs as the host-side
oracle)](../../0190-ext4-read-write.md) sends its `rsext4` fixes to `rcore-os/tgoskits`.
Each submission has a draft here for the architect's review: a `.md` with the base commit, the
title, the body in Chinese and in English and the test evidence, and a `.patch` beside it holding
the `git format-patch` output. Nobody files a draft until the architect has reviewed its text and
given a separate go.

- [`2-truncate-fix.md`](2-truncate-fix.md): upstream item 2, a depth-2 truncate that failed with
  `extent:empty_index`, and a failed truncate that left the file's tail zeroed.

*Name: provisional. For the drafts the parent's upstream round cites. calef names directories.*

- `2-truncate-fix` Name: provisional. The upstream item's number and what it fixes. calef names
  files.
