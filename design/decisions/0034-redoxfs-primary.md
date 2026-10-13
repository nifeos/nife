---
status: AMENDED
raised: 2026-07-29
decided: 2026-07-29
ratified_by: calef
---

# 34. RedoxFS is the primary filesystem, on three conditions

**Decided 2026-07-29 (calef), with the conditions attached deliberately so the label and its caveats
land together.** RedoxFS is the primary on-disk filesystem. It is not yet the *root* filesystem, and
§34.3 below is why that is a separate piece of work rather than a relabelling.

## Why this commitment is cheaper here than the words suggest

In a monolithic kernel, choosing a primary filesystem means linking tens of thousands of lines of
someone else's code into the TCB, where a bug in it is a kernel bug. Here the FS server is a confined
userspace component holding a capability to a block device, so a RedoxFS defect is a **data-integrity
bug, not a system compromise**: the kernel does not trust it and cannot be broken by it. That is what
the structure was for, and it means this decision is revisable at the cost of one component, which
milestone 23 exists to demonstrate. Recording it as a decision is still right, because a default
nobody wrote down is a decision nobody can revisit.

## What earns it the role

- It is already somebody's root filesystem. Redox OS runs on it. That is the exact use being asked
  of it, exercised by a real system rather than inferred from a design document, and it is the single
  strongest argument in its favor.
- Copy-on-write with transactions, so crash consistency is designed in rather than bolted on. That
  is the one property a primary filesystem must have and the most expensive thing to write oneself.
- Rust, and no_std on both bare targets, proven by us. It does not drag a libc into the FS server.
  (§31 makes a C component possible now, but possible is not free.)
- Maintained upstream, pinned at 0.9.1 with a patch discipline (`patches/`, two `Vec` imports).
- It is the reuse thesis made concrete: a real filesystem we did not write, running confined.

## The three conditions

1. **Crash consistency must be tested, not asserted (milestone 37). MET, 2026-07-30**; the
   measurement and the exact claim it earns are in the amendment below. It is RedoxFS's central
   selling point and we had never injected a torn write or a power cut. The claim rested on the
   upstream design description. For a project whose rule is measure rather than argue, that gap was
   worse than the missing verbs were, and it is the first thing a skeptic should ask about.
2. Throughput must be measured (milestone 38). `fs_read` reports the whole-path cost of a real
   read (~204 us under HVF, device-dominated, with `relay_rtt` putting the isolation tax three orders
   of magnitude below it), and it is deliberately ungated because the path is interrupt-driven. What
   does not exist is any MB/s figure, or any comparison against ext4 or APFS. The phrase "primary
   filesystem" invites a comparison we currently cannot make.
3. The write path must be honestly complete, which is `CREATE` and `TRUNCATE` (milestone 31 phase
   two, in flight). §27 records why `TRUNCATE` is not merely a feature: a write that half-works reads
   as a write that failed, and that sharp edge cost a day and produced three wrong root causes.

One encouraging measurement already in hand: the real engine under the FS server's own allocator, at
the 8 MiB cap, held a high-water of 352 KiB across thirty mount-and-write cycles. So the budget is
generous headroom rather than a requirement, which was the main worry about it on small hardware.

## What would reverse this

If RedoxFS turned out to need `std`, or to need an allocation guarantee the budget model cannot give,
or if its repair and recovery tooling is absent. The first two have been probed and came out fine.
The third is unchecked, and for a primary filesystem "what do you do with a corrupted one" is a fair
question that deserves an answer before the label hardens.

## Alternatives considered

`nifefs` is not among them, because it is not a competitor: a boot archive and a read-write
filesystem are different jobs, and the initrd wants exactly what nifefs is. It stays.

- Write our own. Rejected on the same grounds as milestone 32 (a real filesystem: RedoxFS behind a
  capability FS server): the thesis is the kernel confining the filesystem, not the filesystem. A
  crash-consistent CoW filesystem is a large, subtle project that proves nothing the thesis needs.
- ext2. Simple, well documented, Rust implementations exist, and it buys real interop (mount the
  image on Linux). Rejected for a *root*: no journaling, so power loss means `fsck` and possible loss,
  which is a step down from CoW. ext4 has no serious no_std Rust write implementation, and writing one
  correctly is its own multi-month project.
- FAT32 / exFAT. `fatfs` is mature and no_std. Rejected for a root on semantics: no crash
  consistency at all, no permissions, no symlinks. It is the right answer for a future *boot* partition
  where interop is the point, and wrong for anything that must survive a power cut.
- littlefs. Genuinely power-fail-resilient, and wrong on two axes: it targets raw NAND/NOR with
  wear leveling rather than a block device, at microcontroller scale, and it is C, so it would put a
  foreign component in the storage path for no thesis gain.
- btrfs / ZFS / F2FS. No no_std Rust implementation, and a size that would dominate the project.
  The `no_std` half of this expired on 2026-09-14; see the 2026-09-20 amendment below, which also
  says why bundling btrfs with ZFS was the bigger mistake.
- Build on a proven transactional store (SQLite being the most battle-tested crash-consistency
  implementation in existence, needing only a VFS shim, which is precisely the seam §31 built).
  Interesting and not recommended: file-data performance would be poor and the novelty would need
  defending for no thesis benefit. Recorded because the crash-consistency argument for it is real.

## Amendment (2026-07-30): the xattr requirement, measured rather than argued

Milestone 55 surfaced a requirement §34 never considered: Time Machine over SMB wants extended
attributes (Samba's `streams_xattr`), and RedoxFS has none. calef's read was that this suggested the
choice was wrong rather than something to patch around, and the strongest half of that is correct:
extending RedoxFS's on-disk format would destroy two of the four reasons for choosing it, since it
would no longer be the filesystem Redox actually runs and every pin bump would pay for the divergence.
"We will patch xattrs in" is doubling down and should not have been offered as a co-equal option.

**What the requirement is not, stated precisely:** the application is not blocked. Samba's
`fruit:metadata = netatalk` keeps the identical Apple metadata in AppleDouble sidecar files and needs
no xattrs at all. So Time Machine runs on RedoxFS today; what is unavailable is the clean way to do
it. The preference for xattrs is right (we will want them anyway) but there is no time pressure, and
that distinction is what let this be decided by measurement.

The extended-attribute atomicity check and the rename finding are in
[`crash-consistency.md`](0034-redoxfs-primary/crash-consistency.md).

## Amendment (2026-07-31): the xattr fork is closed, the layer, and it is reversible

The amendment above left the mechanism open. **Decided: extended attributes are implemented as a
layer in the FS server, not as an extension to RedoxFS's on-disk format.**

The argument that decides it is reversibility, and it outranks the others. `fs_proto` is the
contract; whether an attribute lives in a node's on-disk structure or in a store the server manages is
invisible above that boundary. So choosing the layer does not foreclose the format extension, if
attributes later prove central enough to justify diverging from a pinned upstream, or if the change is
accepted upstream, the implementation moves and no client changes. That makes this a low-regret
decision rather than a bet, which is the right shape for a mechanism nobody has exercised yet.

The supporting reasons, in order:

1. It is crash-atomic, and that was measured rather than assumed. `fs.tx(|tx| …)` groups arbitrary
   mutations into one commit, so a file write and its attribute write land together, and a delete
   removes both or neither. This was the check that had to pass before the layer was viable at all.
2. **Attributes key on `TreePtr<Node>`, not on a path**, because that is what the FS server already
   works in (`handles: Vec<Option<TreePtr<Node>>>`). Rename is therefore free and correct: a
   rename changes a directory entry, not the node, so attributes follow the file without any code
   knowing that a rename happened. AppleDouble sidecars get exactly this wrong, and so would any
   path-keyed store. This correctness property is only available inside the FS server, which is an
   argument for the layer rather than merely a consolation for not forking the format.
3. It decouples the requirement from the filesystem choice. A layer works over any backing
   filesystem, so if RedoxFS is ever replaced the attributes come along.
4. It preserves §34's upstream reason. The pin stays at 0.9.1 with a two-import divergence patch
   rather than a format fork every future bump would pay for.
5. Nothing can bypass it, because all access goes through `fs_proto`. On Linux this style of layer
   is worthless (anything can open the file directly), and here it is authoritative. A capability
   property doing real work.

Three things to get right when building it, recorded now because each is a way to get it subtly
wrong:

- The attribute store must be invisible to enumeration. A directory listing must not show it, or
  it becomes part of the namespace clients can name.
- Deletion must be in the same transaction as the file's, or a deleted file leaks its attributes
  and a later node reusing that pointer inherits them. That is a correctness bug wearing a
  housekeeping costume.
- Recovery sees it. `redoxfs_host extract` and upstream's FUSE mount will show the store as
  ordinary data. That is acceptable and arguably good (the attributes come out with the backup), but
  it must be a decision rather than a surprise, and `notes/host-recovery.md` should say so.

What was rejected: extending the on-disk format. Correct, and atomic by construction, and it costs
two of §34's four reasons: RedoxFS would stop being the filesystem Redox actually runs, and every pin
bump would pay for the divergence. Reversibility makes that cost avoidable rather than merely
deferred. calef's objection to "patching it in" was right, and the answer is not to patch it in.

## Amendment (2026-07-30): ZFS and XFS, and why RedoxFS is better-shaped than its size suggests

The ZFS and XFS comparison is in [`zfs-and-xfs.md`](0034-redoxfs-primary/zfs-and-xfs.md).

## The alternative that could supersede this, and is not a filesystem choice

A read-only measured root plus a writable layer. §22 already gives us measured boot, so hashing a
read-only root image would extend integrity verification from init to the entire system, with writes
landing in a smaller, less critical layer (RedoxFS or anything else). That is a *stronger security
story* than a writable RedoxFS root, it sidesteps the repair question above by making the root
reproducible rather than repairable, and it is the shape Android and ChromeOS chose (dm-verity plus an
overlay). It is recorded here as the thing most likely to make this section a footnote, and it competes
on architecture rather than on engine quality, which is why choosing RedoxFS now costs little.

## Amendment (milestone 37, 2026-07-30): condition 1 is met, and the claim is narrower than the words it replaces

RedoxFS is crash consistent, in a sense that is now measured rather than described. The docs may
stop saying "designed for crash consistency". They should not start saying it without the scope
below, because the scope is where the interesting part is.

What is proven. Take a workload of operations, each acknowledged only after the engine commits
it, and call the filesystem after the first `p` of them `S(p)`. Then:

> **For every point at which the device could stop, a fresh mount recovers exactly `S(p)` for some
> `p`.** Never a blend of two states, never a half-applied operation, never a length nobody wrote,
> never a mount that fails.

That is prefix consistency, and it is deliberately stated as a stronger property than the one the
milestone asked for. "Every acknowledged write is either wholly present or wholly absent" falls out
of it, and so does the thing that phrasing leaves open: a state where a later operation survived and
an earlier one did not. Two further assertions make it a measurement rather than a shape. `p` must be
non-decreasing as the cut point advances, so a later crash can never lose more than an earlier
one; and at the last cut point `p` must be the whole workload, so a filesystem that recovered the
initial state every time (perfectly prefix-consistent, perfectly useless) fails.

The fault-injection table the next paragraph cites, its controls and the on-device run are in
[`crash-consistency.md`](0034-redoxfs-primary/crash-consistency.md).

The limit, stated as plainly as the guarantee, because it is real. RedoxFS's `Disk` trait has no
flush and no barrier, so write *ordering* is the device's job. A device that acknowledges a write it
has not persisted and then persists later ones can leave a valid commit pointing at a block that
never landed, and no filesystem promises otherwise. What RedoxFS does promise, and what the third row
measures, is that this is **never silent**: every `BlockPtr` carries a seahash of the block it names,
checked on every read, so a lost or torn block is an error rather than a wrong answer. Our block
server issues no `VIRTIO_BLK_T_FLUSH`, so on real hardware with a volatile write cache the durability
of the *last* acknowledged write is the device's word rather than ours; that is a gap in our driver,
not in the engine, and it is recorded here rather than in a footnote.

What is still a design claim. Ordering and durability at the device, above. Repair and recovery
tooling for an image the checksums *do* reject, which "What would reverse this" already names as
unchecked, and which this milestone sharpens rather than answers: we can now produce such an image
deliberately, and there is still nothing to hand a user who has one. Condition 2 (throughput,
milestone 38) is untouched.

## Amendment (2026-09-20): the `no_std` clause expired, and it was doing work it should not have been

calef asked whether btrfs needs `no_std` at all, given a filesystem server runs in userspace. **It
does not, and this section has been saying otherwise for seven weeks.**

What changed. This section was decided 2026-07-29, when nothing in this tree had `std`. The
alternatives list above dismisses "btrfs / ZFS / F2FS" in one line, and the first half of that line
is *"No `no_std` Rust implementation"*. Milestone 27 (Rust `std` on the native ABI) retired that
premise, and milestone 184 (extend the `std` port (milestone 27) to x86_64, BUILT 2026-09-14)
finished the job: all three targets now declare `"std": true`, the `nife-dev` farm builds it, and an
`fs_server` is an ordinary EL0 program exactly as `redoxfs_server` is. **A userspace filesystem may
use `std` here, so `no_std` is no longer a filter on anything.**

**What replaces it, because the bullet should not simply be deleted.** Three questions, none of
which is the one that was asked:

1. Does a usable Rust implementation exist, and at what completeness: read-only, read-write,
   maintained? This is now the live question and it is unanswered. It should be measured against
   `targets/*-unknown-nife.json` rather than looked up, which is the lesson milestone 442 (a crypto
   provider `rustls` can use on all three bare-metal targets) paid for: DECISIONS §196 (nife carries
   TLS: `rustls` for the protocol, and a crypto provider we make work)'s table was measured against
   stock `-none` targets and every row of it was wrong about ours.
2. **Our `std` is not Linux's.** The targets are `singlethread = true` with `panic-strategy = abort`,
   so a crate that spawns threads or expects unwinding fails here whatever its filesystem logic is.
   That is a sharper filter than `no_std` ever was, and it is the one a probe actually hits.
3. Read-only against read-write, which the original bullet never separated. Reading a foreign
   btrfs volume belongs to milestone 140 (mount a drive this system did not create), and a read-only
   implementation is a fraction of a read-write one. btrfs appears nowhere in 140's ordering, which
   is FAT32, exFAT, NTFS and ext4.

The ZFS half of that bullet survives, and for a reason the 2026-07-30 amendment above already
gives rather than for the one written here: OpenZFS is not a component you confine, it is a subsystem
you host, needing a Solaris Porting Layer against a seam built to confine a narrow interface. That
argument is untouched by userspace `std`. btrfs was bundled into a one-line dismissal with it and
should not have been, which is the more useful half of this correction: a shared bullet let one
crate's disqualifier stand in for another's.

## Amendment (2026-10-07): btrfs does not replace RedoxFS

Measured: btrfs and ZFS readers build here; no Rust writer of either exists. calef ruled (#1803):
Linux's btrfs is roughly 150,000 lines of C, and RedoxFS already has copy-on-write under a
checksum Merkle tree. Re-probe in three months. Data, and LKL as an alternative:
[notes/filesystem-crates-2026-10-07.md](../../notes/filesystem-crates-2026-10-07.md).
