# Prior art

An appendix to [the milestone](../1090-filesystem-contract-outside-review.md).

How the crate sits against what a reader of other systems already knows. Figures marked
unverified were not measured in this review.

| system | what it has | how this crate differs |
|---|---|---|
| Plan 9's 9P2000 and the `rs9p`, `rust-9p` crates (downloads unverified) | a file protocol over a byte stream: `Twalk` up to sixteen names at once, fids chosen by the client, `Rerror` as a string (9P2000.L: an errno) | one hop per `OPENDIR`, handles minted by the server, bytes in a shared region rather than in the message, the errno negated in the first reply word |
| Fuchsia's `fuchsia.io` | directory connections carry rights; `Open` must ask for a subset of the parent's rights; `Clone` with same-rights | the closest match for `dir::Rights::attenuate` and `PARENT_RIGHTS`; here the intersection is one `&` and is proven, there it is a check that refuses |
| FreeBSD's Capsicum (`cap_rights_limit`) | `CAP_READ`, `CAP_WRITE`, `CAP_LOOKUP`, `CAP_CREATE`, `CAP_UNLINKAT`, `CAP_FUTIMES`, `CAP_FSYNC`, `CAP_FSTATFS`, `CAP_EXTATTR_*`, `CAP_RENAMEAT_SOURCE` and `_TARGET`; limiting is monotone | the same ladder with fewer rungs: `STATFS` needs no right here, extended attributes ride on `READ` and `WRITE`, `SYNC` rides on `WRITE`; rename needs `REMOVE` at the source and `CREATE` at the destination, as Capsicum's pair does |
| WASI preview 1 | `rights_base` and `rights_inheriting` on a directory descriptor, `path_open` relative to it, no absolute paths; dropped in preview 2 | the same no-global-namespace shape; this crate keeps one rights word per handle and resolves one component per verb |
| Linux `getdents64`, `listxattr`, `statfs`, `getxattr` | `linux_dirent64` with inode, offset, record length, type and a NUL-terminated name; `listxattr` as NUL-separated names; `XATTR_NAME_MAX` 255, `XATTR_SIZE_MAX` 65,536; `statfs` with eleven fields | a two-byte record header and length-prefixed names (a name is bytes, a NUL inside one is legal); the same 255 for a name, 3,072 for a value because name and value share one page; three `statfs` fields with the length as the version |
| Linux's syscall return convention | `-errno` in the return register, and a reply is an error only in `-4095..=-1` (`MAX_ERRNO`); `IS_ERR_VALUE` | the same negation; no lower bound, so every negative word is an errno (item 6) |
| virtio-blk and the `virtio-drivers` crate; `embedded-sdmmc`'s `BlockDevice` | `VIRTIO_BLK_T_IN`, `_OUT`, `_FLUSH` over 512-byte sectors; a `read`/`write`/`num_blocks` trait on 512-byte blocks | a 4,096-byte block, eight sectors, up to sixteen per request, and a `FLUSH` whose reply counts completed flushes so a client can see one happened |
| Genode's `File_system::Session` (unverified in detail) | directory and file handles, a packet stream for bytes, policy by session label | the same split of control words from a shared byte region; here the policy is the rights word and the caretakers, not a label |

Where this crate differs from all of them: the rights ladder is machine-checked (three Kani
harnesses with falsification patches). Every limit refuses rather than truncates and says which
limit with its own errno. The verb table is data a proxy dispatches off, with compile-time
assertions that a verb without a row does not build. What it costs: the crate carries nife's
process-start conventions (`grant`, `nameset`, `READY`, `DESCENT_REFUSED`, `rm`) beside the two
protocols, which a stranger implementing only the protocols has to read past.
