# Upstream drafts for milestone 190

[Milestone 190 (ext4, read and write: a Rust implementation with libext2fs as the host-side
oracle)](../../0190-ext4-read-write.md) sends its `rsext4` fixes to `rcore-os/tgoskits`.
Upstream takes one task per pull request and processes none without a linked Issue, so each
submission has a draft here holding both: the Issue to file first and the pull request, each with
its Chinese text and an English rendering, then the base commit and the test evidence. A `.patch`
beside it holds the `git format-patch` output. Nobody files a draft until the architect has
reviewed its text and given a separate go.

- [`2a-free-empty-extent-index.md`](2a-free-empty-extent-index.md): upstream item 2's first
  defect, a depth-2 truncate that fails with `extent:empty_index`.
- [`2b-restore-truncate-tail.md`](2b-restore-truncate-tail.md): item 2's second defect, a failed
  truncate that leaves the file's tail zeroed.

*Name: provisional. For the drafts the parent's upstream round cites. calef names directories.*

- `2a-free-empty-extent-index` Name: provisional. The upstream item's number, a letter per
  submission, and what it fixes. calef names files.
- `2b-restore-truncate-tail` Name: provisional. As for `2a`. calef names files.
