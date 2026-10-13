# §34 appendix: the crash-consistency evidence

Evidence for [§34 (RedoxFS is the primary filesystem, on three conditions)](../0034-redoxfs-primary.md): the fault
injection behind condition 1, and the extended-attribute atomicity check. Moved out of that page on
2026-10-12 (UTC) under [§212 (a prose budget)](../0212-a-prose-budget-for-every-document.md). The
main page is complete without this one, which exists to verify or challenge it. Headings were added;
no ruled word changed.

## The measurements behind milestone 37's amendment

From [the amendment of milestone 37 (crash consistency)](../0034-redoxfs-primary.md), which keeps the claim, the limit and
what is still a design claim.

The numbers, host side, exhaustive (`fs_server/tests/crash_consistency.rs`, 0.6 s):

| injection | fault points | result |
|---|---|---|
| power cut, every write | 93 | all prefix-consistent, `p` monotonic, `p` = 7 with nothing lost |
| power cut with the last write **torn**, 4 offsets | 372 | all prefix-consistent |
| a device that **lies** (drop or tear one write, keep persisting after) | 186 | 112 recovered, 1 refused at the mount, 73 refused at a read, **0 silently wrong** |

The controls, which are the reason the rest counts. Three, of increasing directness. The
lying-device sweep needs no tampering at all and still produces 74 images the filesystem refuses, so
the injector is demonstrably destructive. Removing the header ring's older generations leaves 92 of
93 fault points unmountable, which isolates the fallback as the mechanism rather than a guess. And
a commit torn at 2048 bytes fails `Header::valid()` outright while the previous generation's slot
stays valid and stays older, which is the whole recovery argument in three assertions and no mount.

A fourth control is worth more than the other three: `ENOENT` is the only error the harness reads
as absence. A harness that read *any* failed lookup as "the name is absent" would turn a dropped
write to a directory's tree block into what looks like a filesystem that never existed, empty root
and all. It would report a filesystem bug that is a test bug. The engine refusing to guess at a block
whose checksum did not match is the property working.

Two mechanisms we had named wrong, corrected here. `cleanup: true` is **not** the header-ring
replay; `FileSystem::open` scans all 256 slots and keeps the newest valid one unconditionally, and
`cleanup` only releases unused nodes and commits on top. And the recovery is not "the newest
consistent generation" in any sense the engine computes: it is the newest generation whose *header*
still hashes, which is enough only because a commit's blocks are all written before it.

On device, both ISAs, on a disk of its own. The FS server is killed one block write into its
second transaction, with that block torn in half by a real virtio write, announcing the cut on its
readiness endpoint so the kill is provably the injector's. A different FS-server process then
mounts the same disk through the same block server, which is endpoint-only naming doing its job: the
block server never learns its client died and was replaced, because it never knew who its client was.
Its readiness sentinel is the consistency result, since `Server::open` refuses an image it cannot
make sense of. Both legs recover the acknowledged payload, whole, and the host tool re-reads the image
afterwards with the pinned engine and agrees.

The crash test owns its disk, and that is §27's lesson applied before the fact rather than after.
This test deliberately leaves a filesystem half-written. On the shared fixture, every other FS test's
result would have depended on whether this one ran first, which is precisely the order-coupled gate
that manufactured three incompatible root causes from three honest investigations. The fixture is
regenerated every run, `NIFE_KEEP_REDOXFS` deliberately does not apply to it, and on the host every
fault point starts from a byte-identical clone of one image built in-process.

## Why the harness matters more than the engine

From the page's "The alternative that could supersede this, and is not a filesystem choice".

Note that switching engines would not address condition 1 at all: **no candidate's crash consistency
is tested here.** That is a gap in our harness, not in RedoxFS, and it is why the conditions matter
more than the choice. *That sentence was true when it was written and is now the thing milestone 37
fixed; the harness exists, and it would measure any engine put behind the same trait.*

## The check that let §34 stand against the xattr requirement

From the page's 2026-07-30 xattr amendment.

The check that decided it, and its result. The question was whether a layer *above* the filesystem
could be crash-atomic, since a rename must move a file and its metadata together or a crash leaves
them inconsistent (§42's territory). RedoxFS groups arbitrary mutations into one transaction:
`fs.tx(|tx| …)` exposes `create_node`, `write_node`, `rename_node`, `truncate_node` and `remove_node`
on one `Transaction`, committed together, and `fs_server` already relies on it ("existence is checked
inside the same transaction as the create"). So the layer is safe, the format stays unforked, and
§34 stands.

The layer is also the *anti*-lock-in option rather than the sunk-cost one: xattrs implemented in the
FS server work over any backing filesystem, so they decouple the requirement from this decision.
Normally such a layer is bypassable and therefore worthless; here nothing can bypass it, because all
access goes through `fs_proto`. That is a capability-system property doing real work.

Also found, and it corrects a claim made in milestone 55: `rename_node` and
`rename_node_no_replace` already exist in RedoxFS. `fruit:posix_rename` and §42's rename work are
blocked on `fs_proto` lacking the verb, **not** on the engine.
