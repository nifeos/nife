# Prior art

An appendix to [the milestone](../1082-capability-manifest-note-outside-review.md).

How a program tells its loader what it needs, in the binary itself.

| system | note or section | owner, type | descriptor | writer | what nife does differently |
|---|---|---|---|---|---|
| FreeBSD | `.note.tag`, `NT_FREEBSD_FEATURE_CTL` | `FreeBSD`, 4 | one 32-bit bitmask (`ASLR_DISABLE` 0x01, `PROTMAX_DISABLE` 0x02, `STKGAP_DISABLE` 0x04, `WXNEEDED` 0x08, `LA48` 0x10, `LA57` 0x40); the kernel reads it at `execve` | the linker, then `elfctl -e +noaslr file` edits it in place | nife's descriptor is a typed record, not a bitmask, and it is refused unless canonical; nothing edits it after the link, by design, because the digest covers it |
| FreeBSD | `.note.tag`, `NT_FREEBSD_ABI_TAG` | `FreeBSD`, 1 | one 32-bit `__FreeBSD_version` | crt | the version word of nife's descriptor plays the same role for the layout, not for the operating system |
| Linux / GNU | `.note.gnu.property`, `NT_GNU_PROPERTY_TYPE_0` | `GNU`, 5 | a list of (type, size, data) properties, 8-byte aligned on ELF64; the kernel reads `GNU_PROPERTY_X86_FEATURE_1_AND` (IBT, SHSTK) and the AArch64 BTI bit (details unverified) | the assembler and `ld`, which merge the lists of every object | nife has no extensible list; every version is a fixed grammar, and a reader refuses a later version instead of skipping what it does not know |
| systemd / UAPI | `.note.package` | `FDO`, `0xcafe1a7e` | one JSON object as a NUL-terminated UTF-8 string, NUL-padded to four bytes, with well-known optional keys (`type`, `os`, `name`, `version`, `debugInfoUrl`) | `ld --package-metadata=<json>` (bfd and gold 2.39, mold 1.3.0, lld 15) | JSON has many spellings of one meaning (key order, whitespace); nife's "one manifest, one encoding" is the opposite choice, taken so the bytes can be hashed |
| OpenBSD, NetBSD | `.note.openbsd.ident`, `.note.netbsd.ident`; NetBSD `.note.netbsd.pax` | `OpenBSD` 1, `NetBSD` 1, `PaX` 3 (unverified) | a version word; for PaX a flags word edited by `paxctl` (unverified) | crt; `paxctl` | the same per-binary toggles that FreeBSD's feature control carries; nife's note is the program's request for authority rather than a mitigation toggle |
| Go | `.note.go.buildid` | `Go`, 4 (unverified) | a build identifier string | the Go linker | an identity, not a request |
| Android | `AndroidManifest.xml` in the package, not the executable | | XML `<uses-permission>` elements, granted at install | the build | the word "manifest" here is Android's sense; nife puts it inside the hashed executable so the package and the request cannot drift apart |
| Rust readers | `object` 0.41 (143M downloads in 90 days, 710M all-time), `goblin` 0.10 (23M / 88M), `elf` 0.8 (4.0M / 14M), `xmas-elf` 0.10 (0.7M / 3.4M) | | each walks `PT_NOTE` and yields (owner, type, descriptor) | | none decodes an owner's descriptor; this crate starts where they stop, which is the right seam, and its README should say so and name `object` as the way to find the note outside nife |

Concrete example of the difference: two `.note.package` descriptors, `{"os":"x","name":"y"}` and
`{"name":"y","os":"x"}`, mean one thing and hash differently. Two `capability_manifest_note`
descriptors that decode to one `Manifest` are the same bytes, and Kani proves it for version 1.
Item 3 is where the crate fails its own standard in the other direction.
