# Why the roadmap is markdown in the tree and not GitHub Issues

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/roadmap.md](../roadmap.md), which summarizes it.

Decided 2026-07-30 by calef. Issues would buy PR linkage, a home for discussion, and a board view.
They would cost the things this table is actually for:

- The roadmap stops being version-controlled alongside the code, so a status change is no longer a
  diff in the commit that caused it.
- `script/roadmap --check` has nothing to validate. That gate is what caught milestone 34 (GPU
  acceleration via virtio-gpu 3D) having no row.
- The cross-references to `DECISIONS §N` and `notes/*.md` decay from one grep into URLs.

The deciding argument is that a second place where status lives is a second source of truth. That
is the exact failure this project spent 2026-07-30 cleaning up. `bench.rs` contradicted
notes/benchmarks.md about what `fs_read` measures. Status prose was phrased a dozen ways, which made
a sweep mis-report eight milestones. And §27 (the filesystem service: a capability-shaped contract)
was corrected four times.

The linkage only starts paying when more than one person files work, so revisit if that changes.
With external contributors, the shape would be markdown canonical and issues generated one-way from
it, never synced back.

Note also that GitHub's own *Milestones* feature is a name collision with this list. It is a poor
fit besides, being built for dated release grouping; these are capability-shaped and deliberately
undated.
