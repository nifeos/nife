# The Apple half: the `AAPL` create context (2026-08-17)

*Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `apple-half`.*

An appendix to [notes/smb.md](../smb.md). The code it describes was removed on 2026-08-30; read every
present tense as "as of `685900ec`". It holds what milestone 55 (Time Machine: SMB3 with Apple's
extensions, and mDNS) added for macOS, what the server claimed, and the durability gap the claim
exposed and closed.

## What macOS looks for

macOS mounts a plain SMB2 share and never offers one as a Time Machine destination. What it looks for
is a create context. It hangs an `AAPL`-tagged blob off the first CREATE of a tree connect, and reads
the server's answering context off the response. That is the whole of `fruit:aapl = yes` on the
reference implementation. It is the first line of the working configuration
design/roadmap/0055-time-machine/reference.md records.

Two modules, because they are two things:

- `crates/smb_proto/src/create_context.rs` is the chain ([MS-SMB2] section 2.2.13.2): generic, and
  reusable. A real macOS CREATE carries `DHnQ` (durable handle), `MxAc` (maximal access), `QFid`
  (on-disk id) and `RqLs` (lease), and the server has to walk past them to find the one it answers.
- `crates/smb_proto/src/apple.rs` is what the `AAPL` tag means. There is no public specification.
  [MS-SMB2] defines the container and says nothing about this tag. So the layout is the one Samba's
  `vfs_fruit` puts on the wire, and macOS has been talking to it for a decade. That file says so at
  the top, and every constant in it is there because the reference emits it.

## What this server claims

This is the expensive half, because a claim is something a client acts on:

| word | set | left clear, and why |
|---|---|---|
| server capabilities | `UNIX_BASED` | `READ_DIR_ATTR` would promise Apple's extended listing (Finder info and fork sizes inside the dirinfo) and there is no Finder info here; `OSX_COPYFILE` would promise a server-side copy that arrives as an `FSCTL` this server refuses; `NFS_ACE` is off on the reference too (`fruit:nfs_aces = no`) |
| volume capabilities | **`FULL_SYNC`** | `CASE_SENSITIVE` would be untrue in the other direction: the backing filesystem is case-sensitive but this server folds every name to lower case at the wire, so what a client can observe is a share that is not. `RESOLVE_ID` would promise resolving a file by an on-disk id nothing here mints |
| model | `TimeCapsule` | matching `fruit:model = TimeCapsule`. **Not** the `_device-info` mDNS model, which the reference sets to `MacSamba`; notes/mdns.md's capture found the working reference running with the two disagreeing, so they are two knobs and not one |

`FULL_SYNC` is `fruit:time machine = yes`. It is the single bit on the SMB side that makes macOS
willing to hold a backup here.

## The claim ran ahead of the stack, and as of 2026-08-18 it does not

The gap is worth keeping on the record, rather than quietly deleting. It is the shape of mistake this
project is most exposed to: two layers, one of them genuinely covered, and a claim written against
the covered one.

- Always true: the FS server puts every `fs_proto` write through one RedoxFS transaction that
  commits to the header ring *before* the reply. There is no write-back cache above the block device
  for a flush to push, so SMB2's `FLUSH` has nothing to do at that layer.
- Not true until milestone 55's durability half: the block server issued no `VIRTIO_BLK_T_FLUSH`. So
  the durability of the last acknowledged write was the device's word rather than ours. A host that
  lost power could lose a write this server had acknowledged, and nothing in the stack was even asking
  the device about it.

What closed it is two new opcodes on two contracts:

| contract | opcode | what it does |
|---|---|---|
| `fs_proto::blk` | `FLUSH` (4) | the block server issues `VIRTIO_BLK_T_FLUSH` and waits for the device's completion. `EOPNOTSUPP` if the device never offered `VIRTIO_BLK_F_FLUSH`, so a device with no flush is a loud refusal rather than a quiet success |
| `fs_proto::fs` | `SYNC` (19) | the file-service verb behind SMB2's `FLUSH`. Any handle the server minted, `dir::WRITE` required, refused with `EROFS` |

Both answer with a count of completed device flushes rather than a zero. That is what makes the gate
falsifiable: two syncs that return the same number mean the second never reached the device. See
`fs_proto::fs::SYNC` for the full argument, including why the rights are write-side, and why the
`EOPNOTSUPP` travels to the client unmapped.

The honest sentence now: after a successful `FLUSH`, every write this server acknowledged is on the
medium the device calls durable. What "durable" means is still the device's definition. A device that
lies about its own flush is outside anything a protocol can check.
