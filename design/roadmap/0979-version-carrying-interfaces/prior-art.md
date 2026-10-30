# Milestone 979 appendix: the prior art, read for this block

[Milestone 979](../0979-version-carrying-interfaces.md) keeps the questions, the options and the
recommendations. This appendix holds what this lane read on 2026-10-28 about how five systems and
one protocol handle ABI and wire-protocol versioning and bind-time checks, with the citation
beside each lesson. The appendix marks anything the lane recalls rather than read.

## Plan 9

The 9P `version` exchange (http://9p.io/magic/man2html/5/version, also
https://man.cat-v.org/plan_9/5/version). `Tversion` must be the first message on a connection, the
client proposes a version string and a message size, and the server answers `Rversion` with the
version it picks, at or under the client's size. A server that does not understand the client's
string answers with its own version string, not with an error, and the client walks away if it
cannot speak that. The refusal is clean and names both versions. This is the bind-time shape of
the proposal's question 2, and it is the oldest one this lane found still in the manual.

## Fuchsia

Fuchsia splits the question in two, which is the separation the proposal's question 1 makes.

- FIDL versioning is API versioning only, compile-time, with `@available` annotations and a
  `--available` flag; the reference states "FIDL versioning provides API versioning, not ABI
  versioning. There is no way to query versions at runtime"
  (https://fuchsia.googlesource.com/fuchsia/+/5a61768d7ec37b02fdb0914ba4bc6a2307795203/docs/reference/fidl/language/versioning.md).
- The ABI is a separate surface with its own revision number. `fuchsia.version` carries
  `AbiRevision` as a `uint64` whose values are "chosen at random"
  (https://fuchsia.dev/reference/fidl/fuchsia.version), so a revision orders nothing; support
  treats a revision as a member of a set, not a step in an order.
- Packages carry the stamp. RFC-0135 (https://fuchsia.dev/fuchsia-src/contribute/governance/rfcs/0135_package_abi_revision)
  encodes the target ABI revision into each package. It names three consumers: the assembler can
  refuse a package the system does not support, the resolver can refuse to download one, and a
  petal can check its whole tree against a target release. This is the proposal's "the moved
  package declares what it links", at package granularity.
- The launcher checks the stamp. RFC-0188 (https://fuchsia.dev/fuchsia-src/contribute/governance/rfcs/0188_component_abi_compatibility)
  has the component manager read a component's ABI revision and refuse to launch what the platform
  does not cover, at resolution time, before any component code runs.
- The platform's own rule for additions (RFC-0002,
  https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/docs/contribute/governance/rfcs/0002_platform_versioning.md):
  when an element arrives without breaking the ABI, for example a new system call, the semantics
  of the existing ABI revisions expand to include it, and no new revision mints. This is the
  proposal's question 4 distinction between an addition and a break.
- The stamp is coarse by its own admission. RFC-0239 (https://fuchsia.dev/fuchsia-src/contribute/governance/rfcs/0239_platform_versioning_in_practice)
  calls the ABI revision stamp coarse: it answers yes or no per component. That is prior art for
  the proposal's question 3 option (i), and for why that option loses.

## seL4

seL4 versions the kernel release by semantic versioning
(https://docs.sel4.systems/releases.html: "seL4, Microkit, and rust-sel4 follow semantic
versioning"). Compatibility is a coordinated release, not a runtime check: the 5.0.0 release notes
say "This release breaks both API and ABI and is not source compatible"
(https://docs.sel4.systems/releases/sel4/5.0.0.html), and consumers pin the release and move when
it moves. This lane found no runtime version exchange in seL4's IPC. That is the shape the 2026-10-28
ruling declines: seL4 waits for nobody, but its users are all in one release stream, and its
release notes carry the break rather than any peer checking a number.

## Mach

Every MIG message carries an NDR record, eight bytes holding `mig_vers` ("MIG version"),
`if_vers` ("interface version"), and the representation bytes, from `mach/ndr.h` as quoted in
https://github.com/XSLabs/mach-docs/blob/main/docs/17-mach-and-mig.txt. The same source calls the
record "largely vestigial", and every MIG-generated stub fills in the local `NDR_record`
constant. The field exists on the wire and nothing compares it across ends, so far as this lane
read. A version field nobody checks is the cautionary case behind question 2's requirement that
the check be code both sides call: a constant beside the format that no bind reads is this
failure. One recollection, marked: this lane did not verify whether any real MIG consumer ever
compared `if_vers` in practice; the claim here is only that the field survives and the source
calls the record vestigial.

## Linux

- The syscall interface carries a published add-only promise. `Documentation/ABI/stable/syscalls`
  (https://github.com/torvalds/linux/blob/master/Documentation/ABI/stable/syscalls): the interface
  only grows, and nothing leaves it. This is the proposal's frozen-identifier rule as a published
  promise, with the documentation as the enforcement and new numbers as the only growth.
- ext4's superblock carries `s_rev_level` plus compatible and incompatible feature sets, and the
  mount rule refuses an unknown bit in the incompatible set while it ignores an unknown
  compatible bit (https://kernel.org/doc/html/next/filesystems/ext4/super.html). This is the
  addition-versus-break split of the proposal's question 4, at bind time, with the refusal naming
  the thing refused by feature rather than by number.
- glibc's versioned symbols make the dynamic loader a bind-time checker. A binary names the
  symbol version it built against (`glob64@@GLIBC_2.27`), the loader binds that version and
  refuses one it does not find
  (https://developers.redhat.com/blog/2019/08/01/how-the-gnu-c-library-handles-backward-compatibility).
  The toolchain stamps the declaration at build time, and nobody maintains it by hand, which is
  the proposal's question 3 recommendation.

## The Model Context Protocol

The 2026-07-28 revision (https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)
removed the initialize handshake: "There is no negotiation handshake. Every request carries its
protocol version, and the server accepts or rejects each request independently." The client
declares a version on every call, and the cost lands per message. The proposal's question 2
considered this shape and refused it for cost: one round trip at bind buys the same refusal and
leaves every later call unchanged.
