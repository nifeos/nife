# ext4, read and write: the appendix to milestone 190

[Milestone 190 (ext4, read and write: a Rust implementation with libext2fs as the host-side
oracle)](../0190-ext4-read-write.md) carries the status, the plan for `rsext4`, the bar, what goes
upstream, what runs here, the done-when conditions and its `BUGS`. This directory carries the
survey of candidates and the reason each one lost, and the read and write oracles' full results, which the
main page's prose budget has no room for.

- [`survey.md`](survey.md): every candidate for writing ext4, the two facts that decided most of
  them, each refusal with its reason, and the prior art.
- [`read-oracle.md`](read-oracle.md): phase 1's read oracle, the generated corpus, what `rsext4`
  reads equal to `debugfs` and what it does not, and why it refuses 1 KiB and 2 KiB blocks on
  4,096-byte sectors.
- [`write-oracle.md`](write-oracle.md): the write oracle, its cases and bases, what `rsext4`
  writes that `e2fsck` and a `debugfs` readback accept, and the three bugs they do not.
- [`upstream/`](upstream/): one draft per submission to `rsext4` upstream, each a text for the
  architect's review with the change beside it as a `.patch`: so far
  [`2a-free-empty-extent-index.md`](upstream/2a-free-empty-extent-index.md) and
  [`2b-restore-truncate-tail.md`](upstream/2b-restore-truncate-tail.md), the two truncate fixes.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default,
`design/roadmap/X/` beside `design/roadmap/X.md`. calef names directories.*

- `survey` Name: provisional. For the candidates and refusals the parent cites. calef names files.
- `read-oracle` Name: provisional. For the measurement the parent's phase 1 cites. calef names files.
- `write-oracle` Name: provisional. For the measurement the parent's phase 3 cites. calef names files.
