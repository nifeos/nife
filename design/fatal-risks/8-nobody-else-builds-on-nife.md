---
experiment_status: NOT-RUN
---
# 8. Nobody else builds on nife's mechanisms

*Risk 8 of [the nine](README.md). The status vocabulary, the rule an entry meets and the running order are there. Name: provisional, for this file's stem (`8-nobody-else-builds-on-nife`), minted 2026-10-18 (UTC) by lane maintainer/risk-8-restatement; naming is an architect's.*

The claim: nife's mechanisms, released as crates or offered to existing ones, are not taken up by
anyone else. Then nife cannot shrink, because nothing it builds finds a home outside it, and it stays
a large, self-maintained tree that demonstrates the approach to nobody.

calef, 2026-10-18 (UTC): *"Ideally nife would shrink by pushing its mechanisms out into the rust
ecosystem. I have no desire to maintain 50 crates. Rather I desire 50 crates that I would confidently
use to build a nife."* nife is a demonstrator (§14 (project direction)), and for a demonstrator the
fatal outcome is that nothing leaves it. Whether anybody runs nife instead of Linux is a non-fatal
note in the appendix, and it returns as a fatal risk only if nife turns back toward being a product.

Not run, and no verdict.

Four observations count from the start, each only when it is real:

1. Adopted. A nifeos crate has a dependent outside nifeos that is itself maintained and used, not a
   demo, a fork or another nife lane's test project.
2. Absorbed. A nife OS mechanism or proof is merged into a crate nife does not maintain, and nife
   then depends on that crate and deletes its own copy. This is the one that shrinks nife. A
   toolchain contribution is not a mechanism, so it is counted under lines nife owns instead.
3. Handed off. A nifeos crate gets a maintainer outside nifeos who takes over its releases.
4. Composed. An OS project that is not a fork of nife builds on nife crates, or on a derivative that
   adopted a nife mechanism. It counts double when what it uses is a mechanism crate (capabilities,
   revocation, untyped allocation, DMA confinement) rather than plumbing such as boot, paging or a
   FAT driver.

Taught is a fifth: people working through a course built on nife's crates. Until a course chapter
is live it is a leading indicator only, so the entry does not wait on one. Once a chapter is live it
counts as use of the crates, toward green's three adopted or composed, and never as something
leaving nife (calef, 2026-10-18 (UTC)). So taught alone cannot keep the year mark from red.
Publishing a crate or a chapter counts as none of the five.

**The experiment, which has not been run:** honest attempts at releasing or contributing nife's
mechanisms, each with its own 6-month clock, run in parallel. An attempt is a release or a pull
request, a post or issue explaining what the proof rules out, and a follow-up. The first three
attempts were ruled on 2026-10-27 (UTC), and all three are crates nife publishes itself (publish-first,
below). Measured monthly: outside dependents per crate, mechanisms
absorbed, crates handed off, and lines nife owns (kernel and crates) against a fixed baseline. Is
nife shrinking while it keeps booting and passing its gates?

The thresholds, ruled by calef on 2026-10-18 (UTC):

- green: at least one absorbed or handed off, and at least three adopted or composed. Readable the
  day it is met.
- amber: before 2027-10-18, some adoption or composition, but nothing has left nife.
- red: five honest attempts, each past its 6 months, with no uptake of any of the four; or nothing
  has left nife at 2027-10-18, no mechanism absorbed and no crate handed off. Use alone stops being
  amber after a year.

The first attempts, ruled 2026-10-27 (UTC):

- Publish-first. The first attempts are crates nife publishes itself. A contribution to another
  project waits until a published crate has users who ask for it upstream. The reason is uptake so
  far: the Kani pull request (#4913) has been open and unmerged since 2026-09-30, and the QEMU PLIC
  patch went unanswered. An unanswered contribution would count toward red's five expired attempts
  and measure the counterpart's attention instead of the value of the mechanism.
- First: `file_allocation_table`. The incumbent `fatfs` has 422k downloads over 90 days and 58
  dependents, and no release since 2023-01.
- Second: `extensible_host_controller_interface`, released with `usb`, after it gains Kani proofs.
  The incumbent `xhci` has 39k downloads over 90 days and no release since 2023-07.
- Third: `generational_table`, released on its own. Before release, confirm whether `slotmap`
  (maintained) already covers allocation-free fixed capacity, and say what was found in the release
  record. The incumbents are `generational-arena` (1.18M downloads over 90 days, 57 dependents,
  last release 2023-05-22, archived) and `thunderdome` (222k, 32 dependents, last release
  2023-06-25). Ours is 302 lines with no dependencies and no allocation, and it is proved that a
  stale name never resolves. The crate name is free.
- Dropped from the first attempts: `capability` (32 downloads) and `non_volatile_memory_express`
  (under 1k). Considered and not chosen for the third: `elf` (the largest demand, but ours reads
  only static executables, the name is held by the leader, and matching the leader means a new
  parser) and `device_tree_blob` (ready, but demand is split across maintained crates).

Crates.io figures are as measured on 2026-10-27 (UTC) and age quickly.

The cost is low next to the project, and nothing here waits on milestone 801 (packages over the
internet) or milestone 802 (the trivial install). Thirty crates can leave today, 23 standalone and
7 one small cut away ([`notes/releasable-crates-2026-10-07.md`](../../notes/releasable-crates-2026-10-07.md)),
and the release gates exist in `nifeos/crate-workflows`.

Three caveats. Linux takes Rust code into its own tree rather than from crates.io, so a nife crate
reaching Linux is a long road, and this entry measures whether anyone takes the crates at all.
Counterparts that refuse AI-written work cannot receive an honest attempt, so a first attempt goes
where that work is accepted, or the experiment measures policy instead of value. And the mechanism
crates travel, but the kernel glue that makes them a system (revocation, the one-shot Reply,
mailboxes) does not, which bounds what composition can use today.
[Appendix](what-leaves-nife.md).
