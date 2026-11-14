---
status: DECIDED
raised: 2026-08-30
decided: 2026-08-30
ratified_by: calef
---

# 136. A mature foreign implementation earns its place as an oracle, not as a dependency

calef, 2026-08-30. Decided for ext4 in milestone 190 (ext4, read and write: a
Rust implementation with libext2fs as the host-side oracle) and generalized here at his direction,
because the position is not about filesystems and was discoverable only by reading one. *(Section
number provisional until the merge queue lands it.)*

## What is being decided

When a mature implementation of a format or protocol exists in a language or under a license this
project would not ship, or falls short of the bar in §46 (thin primitives or whole subsystems; we
write everything in between), what role it may take.

The rule: it runs on the host, outside the shipping graph, as the reference our implementation is
differentially tested against. It is a tool, not a dependency.

An oracle stays an oracle. Meeting §46's bar is what turns a foreign implementation into a
dependency: a Rust crate that meets it is a candidate under §46 like any other, and an architect
rules on it there. One that cannot meet it, because it is C, needs a libc, or carries a license
nife does not link, serves only as an oracle. One that falls short only for now serves as an
oracle while we contribute upstream to bring it to the bar.

## The axis §46 and §83 leave open

§46 (thin primitives or whole subsystems; we write everything in between) decides write versus
take. §83 (when the same thing exists in C and in Rust, take the Rust one) decides which
implementation when both languages offer one. Neither says what to do with the implementation you
did **not** take, and the default answer has been *nothing*.

That default throws away the one thing the mature implementation is unambiguously best at. §46's own
rule 4 is the argument: prefer depending where correctness is won by exposure rather than by
reading a specification. A mature implementation is that exposure, accumulated. Using it as an
oracle captures the exposure without taking the dependency, which is the trade §46 was reaching for
and could not express, because it only had "write" and "take" as verbs.

## The tree already does this and never named it

Which is §46's own diagnosis about itself, one level over: the practice was unanimous in effect and
written down nowhere.

- Milestone 54 (a network file service a Mac can actually mount): the evidence is that a real
  Mac's `mount_smbfs` mounts it. macOS's SMB client is the oracle, and it is a far better one
  than any test we could write, because it was not written to agree with us.
- `tools/redoxfs_host`: its own workspace, `std`, deliberately not `fuse`, never in the shipping
  graph, running the round-trip test that keeps the vendored pin honest.
- `script/vendor-verify`: asks "is this tree what we say it is" rather than "does it build",
  which is the same instinct applied to source rather than behavior.
- Milestone 190's phase 4: proposes writing a jbd2-format journal and having Linux replay it,
  then `e2fsck -fn`. The oracle is the operating system we are trying to interoperate with.
- Borg repositories verify themselves cryptographically, so `borg check` after a round trip is an
  oracle that arrives free with the workload.

## What it dissolves

The objections that make a mature C implementation unusable are objections to shipping it, and
every one of them evaporates on the host.

For ext4 that is the worked example: `libext2fs` is LGPLv2, needs a libc, and is C in the storage
path, which §34 (RedoxFS is the primary filesystem) refused for littlefs on exactly those grounds. As a
host tool in `tools/`, none of the three is true. What survives is thirty years of exposure to real
images, which is the half worth having.

## What this does not license

It is not a way to take a dependency by calling it a test. The oracle lives outside the
workspace, like `tools/redoxfs_host`, and nothing in `kernel/`, `crates/` or `user/` may depend on
it. If it is in the shipping graph it is a dependency and §46, §83 and `deny.toml` govern it
unchanged.

It is not a reason to write our own where §46 says take. An oracle makes writing *safer*; it does
not make writing *right*. Writing still needs §46's stated reason, and this section is downstream
of it.

**It does not apply where no mature implementation exists**, which is most of this tree.

**And it is not free.** An oracle is a second thing to build, pin, and keep working, and a pinned
foreign tool that stops building is a gate that stops asking its question. `tools/redoxfs_host` is
the precedent for the cost as well as for the shape.

## BUGS

- Differential testing finds disagreements, not correctness. Where both implementations are wrong
  in the same way, it is silent, and the ones most likely to agree wrongly are the ones that read the
  same specification.
- The oracle can be wrong. `libext2fs` has bugs, and a disagreement is a question rather than a
  verdict about which side is at fault.
- Nothing enforces the host-only boundary except review and the workspace split. A future lane
  could add an oracle as a `dev-dependency` and nothing would fail.
- This says nothing about how much oracle agreement is enough, which is the question a claim
  would actually rest on, and it is left to the milestone that makes the claim.
