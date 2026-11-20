---
status: DECIDED
raised: 2026-08-18
decided: 2026-08-18
ratified_by: calef
---

# 99. Where Apple's metadata lands: stream or sidecar

Raised 2026-08-18 by a milestone 55 (Time Machine: SMB3 with Apple's extensions, and mDNS) lane, which was asked to answer the fork
rather than build it. Decided the same day by calef: option 1, plus a new milestone for the
file-server workload (milestone 137).

Minted 99 by the integrator at merge. The lane proposed 97, which had been taken in the meantime
by two sections on another branch (97 is advisory CI checks, 98 the `OPENDIR` attenuation fork),
both raised earlier. They are written without the section sigil here on purpose: until that branch
lands they name nothing, and a well-formed citation to an absent section is what `script/decisions`
exists to catch. That is the ordinary collision between lanes that cannot see each other, and it is
why a lane ships a number it expects to lose.

## The decision

**Option 1. This server does not claim `FILE_NAMED_STREAMS`, and Apple metadata lands in the `._`
sidecars the Mac writes for itself.** Zero new code, zero new wire surface, and it is already in
force: it is what the tree does today, and the evidence below is that it works because *the client*
implements the fallback rather than because we implemented anything.

And the scope split, which is the half that changes what happens next. The lane's real finding
was not that one option beat the others. It was that **Time Machine does not touch this surface at
all**, so the metadata work was never a backup prerequisite and only looked like one because
milestone 55 carried two workloads under one block. Those workloads are now two milestones: 55 stays
the Time Machine target, and milestone 137 is the share as a Mac file server, which is where
options 2, 3 and 4 go and where the question reopens on purpose.

**What this does not decide.** It does not refuse named streams forever. It says the customer path
does not need them and that the milestone which does need them should be the one to choose, with its
own reader in mind. Milestone 137's gate is `DECISION` for exactly that reason.

The cost of being wrong is bounded, which is what made a decision cheap here despite the
irreversible category. Nothing is written to disk in a format we chose, because the sidecar is the
Mac's format and the Mac both writes and renames it. Reversing this is claiming a bit we currently
leave clear.

## What is being decided

Milestone 55's block names its remaining metadata work as "the stream-versus-sidecar choice, then
the SMB stream surface". Concretely, three questions that have to be answered together, because the
answer to the first determines whether the other two exist:

1. Does this server tell a Mac that it supports named streams? One bit,
   `FILE_NAMED_STREAMS` in `FileFsAttributeInformation`, currently clear. It is the switch, and the
   evidence below is that it is the *only* switch: macOS decides everything else from it.
2. If yes, what does a stream become on the disk? An extended attribute in milestone 57's store,
   an AppleDouble file the server writes, or a file of its own under a reserved name.
3. If yes, what is the on-disk name? A stream called `AFP_Resource` stored as an attribute has
   to be stored under *some* attribute name, and `redoxfs_host` and any future recovery host read
   it. That is a thing two programs agree on in the strict sense.

## Is the premise true? Time Machine does not need any of this

The question AGENTS.md puts fourth is the one that moved the most here, so it goes first.

A Time Machine backup over SMB writes a sparse bundle, and a sparse bundle is directories and
band files. The metadata a Mac cares about lives inside the disk image's own filesystem, which
the server never sees. From the most specific write-up found on the point:

> no extended attributes or forked files. The sparse bundle format provides a virtual disk, whose
> content is stored in relatively small (around 1GB) files called bands.

That author's minimum working Samba configuration is `vfs objects = fruit` plus
`fruit:time machine = yes`, and their reason for dropping the rest is the same one: `catia` and
`streams_xattr` serve a Mac using the share as a file server, which a backup is not.
(<https://ddaa.net/2025/time-machine-samba-config.html>, opened 2026-08-18.)

The second-hand corroboration is Samba's own module. `vfs_fruit`'s header comment says the
module intercepts `AFP_AfpInfo` and `AFP_Resource` and defers every other named stream to
`vfs_streams_xattr`, and its `fruit:metadata` and `fruit:resource` settings decide where those two
go. None of that machinery is reached by a client writing band files.
(<https://raw.githubusercontent.com/samba-team/samba/master/source3/modules/vfs_fruit.c>, opened
2026-08-18.)

The third piece of evidence is a whole server. `ksmbd`, the in-kernel Linux SMB server, is used
as a Time Machine target. Its entire handling of Apple's create context is four lines: find the
`AAPL` tag, set `conn->is_aapl`, and then use that flag in exactly two places, both of which zero
`UniqueId` in a directory-information record. It emits **no** answering `AAPL` context and claims no
`FULL_SYNC`. Its `FILE_NAMED_STREAMS` bit is set only when a share carries
`KSMBD_SHARE_FLAG_STREAMS`, which is off by default.
(<https://raw.githubusercontent.com/torvalds/linux/master/fs/smb/server/smb2pdu.c>, lines 3958-3964,
4447, 4467 and 6020, opened 2026-08-18.)

**What this does not dissolve.** Two things survive, and they are why this is still a decision
rather than a deletion:

- A Mac that uses the share for anything else does write metadata, and `smb-serve` exists to be
  mounted by hand. The moment a person drags a folder onto the share, the question is live.
- calef's reference implementation has it turned on, so "what the working system does" and "what
  the backup needs" are not the same answer, and this proposal should not pretend they are.

**So the honest framing of the fork is not "which one does milestone 55 need".** It is: this is a
file-server feature that milestone 55 inherited by proximity. Whether it belongs in milestone 55
at all is part of what calef is being asked.

## The evidence, in appendices

The evidence behind the options moved, unchanged, to two appendices on 2026-10-12 (UTC) under
[§212 (a prose budget)](0212-a-prose-budget-for-every-document.md). This page is complete without
them; they exist to verify or challenge it.

- [`client-and-prior-art.md`](0099-apple-metadata-at-rest/client-and-prior-art.md): what macOS
  does, read from Apple's SMB client and XNU, and what Samba, calef's router and ksmbd do.
- [`this-tree.md`](0099-apple-metadata-at-rest/this-tree.md): what this tree already does in the
  analogous case.

## The options

Costs are stated per option in four currencies: lines of new code, methods added to the `Share`
seam, wire surface added, and bytes at rest per file that carries metadata. Where a number is an
estimate rather than a measurement it says so.

### Option 1: say nothing, and let the client write sidecars

Leave `FILE_NAMED_STREAMS` clear. macOS's own VFS writes `._name` files, which this share already
stores, lists, renames and deletes as ordinary files.

| | |
|---|---|
| New code | **Zero.** This is the tree's current behavior. |
| `Share` methods added | 0 |
| Wire surface added | 0 |
| Bytes at rest | one extra directory entry, one extra node, and at least one 4096-byte RedoxFS block per file that carries metadata. XNU's `ATTR_BUF_SIZE` is 4096, described in its own comment as "default size of the attr file and how much we'll grow by", so the sidecar is unlikely to be smaller than the block it occupies. |
| The 3 KiB attribute ceiling | does not apply. A resource fork of any size is an ordinary file. |
| `MAX_COMPONENT = 64` | **bites at the edge.** A 63-byte name has no expressible sidecar, and the create is refused with `STATUS_OBJECT_NAME_INVALID`. Band files are 1 to 8 hex characters, so the backup path never meets it. |

**What it costs that is not code.** The metadata is opaque to everything on this side of the wire.
`redoxfs_host extract` recovers `._` files as files, which a Mac can merge with `dot_clean(1)` and
nothing else can read. This tree's own `mv` orphans a sidecar. A listing shows twice as many entries
as a user expects, which Samba hides with `fruit:veto_appledouble` and calef's router does **not**
(his stanza sets it to `no`, so the working system already shows them).

**Two things it does not cost, which the tree's existing prose implies it would.** It does not throw
away milestone 57's work, because the attribute store serves every non-SMB holder of a directory
capability and always did. And it does not lose metadata on rename in the case that matters, because
the renaming party is the client that wrote both files.

Optional additions are each independently cheap, and none is required. Filter `._*` from
`QUERY_DIRECTORY` (roughly 10 lines in `smb_proto::server`, and it makes the share lie about its own
contents, which is a §42 (a filesystem declares what it offers) question). Or refuse to create `._*`
(which would break the client rather than tidy it).

### Option 2: named streams, stored as extended attributes

Claim `FILE_NAMED_STREAMS`, parse `name:stream:$DATA` at the wire, and map a stream onto the
milestone 57 attribute store.

| | |
|---|---|
| New code | **estimated 500 to 700 lines** plus tests: a stream-name parse in `path.rs` and the colon fix beside it, stream-aware CREATE with dispositions, READ/WRITE/CLOSE and `SET_INFO`'s end-of-file on a stream handle, `FileStreamInformation` enumeration, delete-on-close as a remove. The estimate's basis is ksmbd's equivalent at roughly 200 lines of C over an existing xattr system call, times this tree's usual ratio of documented Rust with host tests to reference C. |
| `Share` methods added | **4 to 6**, in all three implementations. |
| Wire surface added | one volume-attribute bit, one `QUERY_INFO` class widened, one path grammar. **No new `fs_proto` op**, which is the option's best property: ops 14-17 already do the work. |
| Bytes at rest | one extra directory entry, one extra node, one 4096-byte block for the node's blob under `.extended-attributes`. **The same order as option 1**, which was not obvious before it was counted. |
| The 3 KiB ceiling | **decides this option.** `AFP_AfpInfo` is 60 bytes and fits. A resource fork is unbounded and does not. Every write is a whole-value read-modify-write of up to 3 KiB, because the contract has no partial-value verb. |
| `MAX_COUNT = 16` | a file may carry at most 16 attributes, and macOS turns *every* extended attribute into a stream, so a file with 16 xattrs meets `ENOSPC`. |

The unavoidable sub-decision. A stream needs an on-disk attribute name. `user.DosStream.<name>:$DATA`
is what Samba's `streams_xattr` and ksmbd both use, so a disk written this way is readable by both;
anything else is ours alone. That is a thing two programs agree on and it is calef's.

**What it buys that option 1 does not.** Metadata survives a rename performed by anything, because
the store keys on the node. `redoxfs_host xattr` already prints it. And it is the precondition for
`READ_DIR_ATTR`, which is the one place a measurable performance win might be hiding, since the
client's bulk-listing path is gated on the same bit. Unmeasured, and it would move server cost up
while moving round trips down, so it should not be claimed until it is measured.

Recorded honestly: this option ships a share that refuses a large resource fork with an error the
client will surface as a failed copy. That is §42-correct and it is still a user-visible refusal that
option 1 does not have.

### Option 3: named streams, hybrid at rest, matching the reference

`AFP_AfpInfo` and ordinary extended attributes become attributes; `AFP_Resource` becomes a `._name`
AppleDouble file that the server writes and parses. This is `fruit:metadata = stream` with
`fruit:resource = file`, which is what calef's router runs.

| | |
|---|---|
| New code | **option 2 plus an estimated 250 to 400 lines**: an AppleDouble header encoder and decoder, entry table handling, and the empty-fork cases Samba spends two configuration options on. |
| `Share` methods added | option 2's, plus a path for the resource fork. |
| Wire surface added | option 2's. |
| Bytes at rest | option 2's, plus option 1's sidecar for any file with a resource fork. |
| The 3 KiB ceiling | **retired for the case that hits it.** The resource fork is a file. |

Its one real argument is that the on-disk result is byte-compatible with the working reference,
so a disk could move between the two servers. Whether anyone would ever move one is a question for
calef, and it is the only question that makes the AppleDouble encoder worth writing.

**Its cost that is not lines**: AppleDouble is a format two programs agree on, and this is the option
that puts *us* on the writing end of it rather than the storing end.

### Option 4: named streams, streams stored as files under a reserved name

Claim the bit, and store every stream as a real file under a reserved directory the way
`.extended-attributes` already holds attribute blobs. No AppleDouble, no 3 KiB ceiling, no compatibility with
anything outside this tree.

| | |
|---|---|
| New code | option 2's, plus a store: **estimated 300 to 500 lines** in the FS server or in the adapter. |
| `Share` methods added | option 2's. |
| Wire surface added | option 2's, **plus a reserved name and possibly a new `fs_proto` verb**, since a stream is a file-shaped object with offsets and the attribute verbs are not. |
| Bytes at rest | one node and one block per stream. |
| The 3 KiB ceiling | gone. |

The honest reading: this is the option with the fewest limits and the most invention, and its
argument is a general SMB file server rather than a Time Machine target. Milestone 55's scoping
decision already went the other way once, on the ranking principle.

## How reversible is it, and who has already acted

Nobody has acted on it yet, which is the whole reason to decide it now. No Mac has written a byte
of metadata to this share; `smb-serve` has been mounted read-only once and writable never, and the
`AAPL` context has never met a real client. The window in which this is cheap is open and it closes
the first time somebody's backup disk has bytes on it.

By AGENTS.md's test it is irreversible in two of the listed categories at once. It is a thing two
programs agree on, and it is bytes at rest that a customer depends on. The code is not the expensive
part in any of the four options.

The migration exists and costs a converter. Samba's `fruit:convert_adouble` proves that sidecar
to stream is possible and that somebody has to write it. The reverse direction, stream to sidecar,
has no reference implementation that this lane found.

One asymmetry worth weighing. Option 1 is the only option in which the *client* owns the format,
so a change in what macOS writes is not a change we have to chase. Options 2 through 4 put this tree
on the hook for a format Apple can change.

## What is blocked until this is answered

- The `FILE_NAMED_STREAMS` bit, and with it `READ_DIR_ATTR` and the client's bulk-listing path.
- The colon in `smb_proto::path`. It is recorded as a compatibility gap today and becomes a
  correctness bug the moment any option other than 1 is chosen. It should be fixed in the same lane
  that claims the bit, not before and not after.
- Nothing on the Time Machine path. Stated again because it is the finding most likely to be
  forgotten between reading this and acting on it.

## What this lane could not answer

- Whether macOS in practice writes any extended attribute onto a Time Machine share. The
  sparsebundle's contents do not, by construction. Whether `backupd` or Finder stamps the bundle
  directory or the share root is a question for a real Mac on real hardware, which milestone 55
  already records as its own gap.
- The `._` file's actual byte length. XNU's `ATTR_BUF_SIZE` of 4096 is the constant in the
  header; the writer itself now lives in a userspace `doubleagent` service that this lane did not
  read. The claim above is bounded rather than exact: whatever the length, it occupies at least one
  4096-byte RedoxFS block.
- Whether `READ_DIR_ATTR` is a net win. It moves work from round trips to the server and this
  tree has no measurement of either side.
- The line estimates for options 2 through 4 are estimates. The only measured comparable is
  ksmbd's C, and the ratio applied to it is a judgment.

## See also

- `design/roadmap/0055-time-machine.md`, whose Apple-metadata paragraphs this answers.
- `notes/smb.md`, whose BUGS entry "Apple metadata is not implemented at all" is the reader-facing
  statement of the same gap, and whose Apple section is the `AAPL` claim table.
- `notes/xattr.md` for the store, its ceilings, and the rename property.
- §42 for the rule that a verb which is offered must be truthful, which is what makes the 3 KiB
  refusal in option 2 a stated cost rather than a bug.
- §57 for the caretaker forwarding, whose BUGS list already carried "`MAX_VALUE` at 3 KiB is
  untested against real Time Machine traffic".
