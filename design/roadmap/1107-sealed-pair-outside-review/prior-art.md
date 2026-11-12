# Prior art

An appendix to [the milestone](../1107-sealed-pair-outside-review.md).

Other systems answer the crate's two questions as follows. This review gives no download figures and
measured none.

| tool | what it checks on the host | how it finds the expected value | what a user coming from it will miss here |
|---|---|---|---|
| systemd `systemd-measure calculate` and `ukify inspect` | predicts the PCR values a unified kernel image will produce before it boots | parses the UKI's sections | a parsed structure, a predicted value to compare against, JSON output |
| Android `avbtool verify_image` | that `vbmeta`'s descriptors match the partitions beside it | parses the descriptor table | per-partition hash tree checks, a signature |
| U-Boot `mkimage -l`, `fit_check_sign` | that a FIT image's hash and signature nodes match its images | parses the device tree | signatures, a listing of every node |
| `sha256sum -c`, BSD `cksum -c`, `mtree -p` | a manifest of digests against a directory's contents | the manifest | a standard manifest format, an exit code per line |
| sbsigntools `sbverify --list`, `osslsigncode verify` | a UEFI PE image's Authenticode signature | parses the PE | signatures, certificate chains |
| Linux IMA/EVM `evmctl ima_verify`, `fsverity digest` | a file's measurement against the policy or the Merkle root | computes from the file | kernel-enforced policy, a tree over large files |
| Rust `memchr::memmem` | a byte needle in a haystack, SIMD | not applicable | a faster search than `windows().any()` (item 13) |

Where this crate differs: it parses nothing out of the kernel. It hashes each archive entry and
looks for the 32 raw bytes in the image. One function therefore reads a flat `booti` image, an ELF
and a UEFI boot file. That is cheaper than every tool above. It is weaker in one way the tree
already recorded (see the main page): a digest present does not show that a check ran. It also answers the
second question, the table against the archive. `sha256sum -c` answers that for a directory, and
nothing above answers it for an archive inside a boot file. What it costs: the vocabulary is this
tree's (`progenitor`, `MEASURED BOOT REFUSED`, the console). Both inputs are this tree's formats. A
third input the kernel now checks is not read at all (item 2). A concrete example: `avbtool
verify_image` fails when a partition is absent that `vbmeta` describes. This crate reports the
mirror case (`TableDisagreement::Missing`) and calls it harmless. The progenitor does the same, so the call is right.
