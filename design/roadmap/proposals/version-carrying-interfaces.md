---
status: PROPOSED
raised: 2026-10-28
milestone_dependencies: 597, 610
decision_dependencies: 235
machine_requirements: none
specific_machine: none
needs_person: no
---
# Interfaces a peer can check: version constants, bind-time checks, and what a package declares

Raised 2026-10-28 (UTC) by the lane `proposal/version-carrying-interfaces`. calef ruled the same
day that the project accommodates interface change rather than waiting for it to stop. The first
repository cut's gate becomes "each interface the cut links carries a checkable version, and the
moved package declares what it links", replacing condition 4 of milestone 756 (the first code to
leave this repository), four settled weeks per linked interface. In a monorepo a break is free,
because the compiler catches every consumer in the same tree, so churn prices nothing; the split
is what prices breaks; waiting for stability inverts the causality.

calef ruled the shape on 2026-10-29 (UTC), in PR #2852's comment. The five questions below record
what calef weighed and what calef ruled, and the ruling added a compatibility obligation and a
retirement provision that "The compatibility obligation, and retiring a generation" records. The
implementing work is milestone 956 (the linked interfaces carry checkable versions), promoted from
[the implementing proposal](../0956-the-linked-interfaces-carry-checkable-versions.md) on 2026-10-28.
This proposal writes no code: §311 (a contract is code, not prose) reserves the checks for an
implementing lane, and every new name below is provisional.

Reuse: the checks reuse the tree's rustdoc-JSON extraction and `FORMAT_CONST` pattern
(`helpers/interface_stability.py`, milestone 610's machinery). No new code and no dependency;
`cargo-semver-checks` and `cargo-public-api` stay refused as milestone 610 records.

## What the tree does today

§151 (the goal of the repository split is independent release and third-party programs) named the
precondition in 2026-09. Per milestone 39 (repository structure for a loosely-coupled OS) and §14
(the project's direction): the wire protocols carry real version numbers and have stopped
churning weekly. Milestone 756 measured it on 2026-10-04 and found the first half unmet; this
lane re-measured on 2026-10-28:

- Five protocol crates carry a numbered magic on a shared-page layout: `TIMEBAS1`, `CURRCPU1`,
  `ENVCONF1`, `MACHSTA1`, `CLOCKv01`. Two carry an unnumbered one: `nifeargv`, `nifeklog`.
- No IPC operation set carries a version a peer could check: `filesystem_contract` packs the
  opcode into the first word's high byte, `socket_contract` and `login_contract` hold none, and
  in `hot_swap_contract` `STATE_MAGIC` carries no number.

- Formats do better: `capability_manifest_note` (`VERSION = 1`, `ZONED_VERSION = 2`), `boot_slot`
  (`SLOT_VERSION = 1`), `package_archive` (`NIFEPKG1`), `nifefs` (`CRKR0002`).
- §235 (the OS is built and updated from packages, and the tree divides by what releases
  together) ruled an ABI revision field into the manifest note; it does not exist. The version 1
  descriptor is 56 bytes, byte 55 its last zero;
  version 2 appends zones after it. A grep for `ABI_REVISION` over `crates/` returns nothing.

Milestone 610 measures that churn in its weekly series. The cells read
reads 157 breaking changes to the contract crates over 2026W37 to 2026W40, the first split's
reading, and 10 boundary moves with 49 syscall additions over 2026W39 to 2026W42.

## Question 1: where the version constant lives, and what its scope is

Options: (a) one constant per crate; (b) one per operation set, module-scoped inside the crate;
(c) one per wire format.

**Ruled (calef, 2026-10-29 UTC, PR #2852): (c), one version constant per wire format, in the
crate that owns the format (AGENTS.md rule 7), module-scoped where a crate holds several;
`filesystem_contract` scopes block and file.** Per-crate scope and an external registry stay
refused. `capability_manifest_note` already carries three version constants, one per format.

The check is per bind, so the scope is per bind. `filesystem_contract` holds the block and
file protocols; a per-crate constant would re-version file clients for a block-only break. The
first cut's four interfaces do not share one kind:

- `abi` is the syscall surface. One constant at the crate root, provisionally `ABI_VERSION`; its
  scope is the syscall, method and error numbers a program matches.
- `address_space_map` is layout constants the loader and the kernel compile in. One constant,
  provisionally `VERSION`; the scope is the bands and the pair-page arithmetic.
- `glob` is a library with no peer and no wire. One constant, so the declaration of question 3
  can name it; nothing checks it at run time.
- `machine_statistics_contract` is a shared-page format; its first word is the magic `MACHSTA1`.
  The page gains a version word beside it (a new index in the `word` module, provisionally
  `VERSION`); a shared page has no CALL to carry one.

New constants named `*VERSION*` join the measured surface for free: the helper's `FORMAT_CONST`
pattern reads any public `*VERSION*`, `*MAGIC*` or `*REVISION*` constant as a format identity,
so 610's format-bump column counts these.

Refusals. Embedding the version inside the magic word, as `MACHSTA1` and `CRKR0002` do, stays
the shared-page convention; the added word is for operation-set contracts, whose opcode fills the
whole first word.

## Question 2: what the bind-time check compares, and where it lives

Options: (a) the client checks the server's served version; (b) the server checks the client's
declared version; (c) both, and each refuses a mismatch, naming both versions.

**Ruled (calef, 2026-10-29 UTC, PR #2852): (c), both sides check, and the refusal names both
versions; placement by kind as proposed.** One amendment rides the compatibility ruling: the
check compares sets, and the server's served set must contain a client's declared version; the
compatibility section below defines that set. The caller asks once at bind; the server answers
with its set; the client binds or refuses, naming both sides. Both come from question 1's constants, one
definition, no prose to drift (§311). Placement follows the interface's kind:

- A message-based contract: one reserved operation (provisionally `OPERATION_VERSION`, opcode 0
  where the protocol leaves it free) returns the server's served set. The caller binds only if
  its version is in it, refusing with both numbers. The kernel routes these words without reading
  opcodes; the check belongs to the two endpoints.
- A shared-page format: the reader checks the magic and the version word before trusting any
  other word. One side writes and one reads; the check is one-sided by nature.
- The syscall ABI: the check happens at spawn, already ruled. §235 puts the ABI revision a program
  built against into its manifest note, "so the progenitor can refuse one it no longer supports";
  the comparison pits the note's declared version against the kernel's `ABI_VERSION` constant.
- A link-time library (`glob`): no run-time check exists; the declaration of question 3 records
  the version, and assembly checks the record.

A refusal names both sides: "the server serves 2 and 1; the client built against 3". The
progenitor already refuses a program whose note it cannot accept (the malformed-note witness).

The check is code both sides use, in the contract crate, as a sketch:
`member(own: u32, served: &[u32]) -> Result<u32, Mismatch>`, `Mismatch` carrying both; the
implementing proposal writes it. Refusals: a client-only check loses, because the server knows
what it still serves; a server-only check loses symmetrically, because a client that binds an
older server gets no refusal today and misreads bytes tomorrow.

## Question 3: what the package declares, and the note field's encoding

§235 ruled, on 2026-09-27 (UTC) in #1389, that the ABI revision is a field in the manifest note
of milestone 597 (a program carries its manifest in an ELF note), not a separate note. It left
the encoding to whoever builds it: "the descriptor's reserved zero bytes, or a longer layout".
Amending version 1 in place holds only while nothing outside the tree has acted on the note.
597 built the note and version 2 joined it on 2026-10-19.

The field holds which interface versions the binary built against, at per-interface granularity:
the gate's words are plural ("what it links"), and one number cannot say which interface moved.

Options: (i) one coarse number, byte 55 as the ABI revision, amending versions 1 and 2 in
place; (ii) a version 3 descriptor: version 1's 56 bytes unchanged, then a counted list of
(interface, version) pairs.

**Ruled (calef, 2026-10-29 UTC, PR #2852): (ii), descriptor version 3, a counted
(interface, version) list stamped mechanically at package build from the dependency graph; the
ruling refuses byte 55, and versions 1 and 2 stay untouched.** Nobody maintains the declaration by hand, so
it cannot rot. The encode and decode machinery and the one-encoding
property carry over from 597's gate.

The refused coarse byte matches Fuchsia's stamp, which RFC-0239 calls coarse. It spends the
in-place amendment §235's condition protects, and it fails the equal-cost test: it cannot answer
a second interface bump, where (ii) answers as cheaply as the first.

The pairs need an interface identity that survives a crate rename. Provisionally each interface
crate carries `INTERFACE_ID: u64` beside its version constant, values never reused.

Who checks the declaration: the kernel's served versions are the same constants, under rule 7's
one definition, so the progenitor's comparison runs at spawn. After the split, basalt's manifest
carries the kernel's table, and assembly refuses a package declaring a version the image does not
serve.

## Question 4: what a version bump owes, and what an addition owes

Milestone 610 records the ledger discipline: a dated list in the format's own module
documentation, one item per in-place amendment, at rung 3, and nothing checks that an entry
exists (610's BUGS says so).

Options: (a) a break bumps the constant and owes a ledger entry plus a migration document. (b) A
break bumps the constant and owes one dated ledger item naming what moved and how a consumer
migrates; an addition owes no bump and no item. (c) Every change, additions included, owes a
ledger item.

**Ruled (calef, 2026-10-29 UTC, PR #2852): (b), and the break carries a one-generation dual
path; the section below records it.** The migration note lives in the ledger item, beside the
format it migrates. An addition is a change an older peer ignores without misreading: a new
opcode, answered with the unknown-operation refusal, or a new optional field. The definition is
behavioral: a semantic change behind an unchanged number is a break and owes the bump, which is
also 610's blind spot about unchanged signatures.

The prior art splits the same way. ext4 separates compatible from incompatible feature sets at
mount; Fuchsia expands an existing revision's semantics when an element arrives without breaking
the ABI, so no new revision mints.

## Question 5: the frozen-identifier rule, and what enforces it

The rule: a syscall number is never renumbered or removed, and a format never changes in place;
a change is a new version. AGENTS.md already makes a brand-new syscall number a design fork. The
question is what catches a violation besides the weekly 610 series, which records a week late and
refuses nothing.

Options: (a) nothing beyond 610 and review. (b) A pinned-values test in `abi`, plus a lint rule
where a pull request changing a `FORMAT_CONST` value must change its crate's version constant.
(c) A full interface diff at pull time, reusing the helper's rustdoc-JSON extraction against base
and head.

**Ruled (calef, 2026-10-29 UTC, PR #2852): (b) now; (c) waits for a consumer outside the tree.**
The `abi` test spells every public number as an expected literal: six syscall constants today,
the object modules' method numbers, the `Error` codes. Updating it is the deliberate act a
renumber should be; skipping it fails CI. The lint reads a pull request's changed lines and fails
when a magic or version value moved without its constant.

Option (c) costs a nightly rustdoc run per pull request and inherits 610's blind spots: 
`#[doc(hidden)]` items, `cfg(target_arch)` items for the other two architectures, glob re-exports,
a macro's fingerprint being its source text. Its trigger is a consumer outside the tree.

Refusals: a tree-wide test listing every format constant's value is (b)'s lint done by hand.

## The compatibility obligation, and retiring a generation

Heading provisional. The section records calef's ruling of 2026-10-29 (UTC, PR #2852) beyond the
five questions.

A message-contract server owes the current generation and the last one (N-1). A break bumps the
version, owes the dated ledger entry (question 4), and carries a one-generation dual path;
shipping the next generation closes the window. The rationale on record: 610's wire-level series
shows the wire effectively frozen already, one format change in sixteen weeks and zero amendments,
so N-1 costs a dual path a few times a year, not weekly. The weekly churn is the Rust API surface,
not the wire. (The columns read zero over 2026W29 to 2026W44; the window's one layout change
arrived as a new constant.)

The carve-outs, ruled the same day:

- Shared pages stay single-version. One writer and one reader leaves no served set to hold; a
  page-format break rides the update path (§235's U4 coherent set), changing both sides together.
- The syscall ABI never dual-serves: §235's spawn-time refusal is the hard edge.
- Stateful services cannot swap across versions (§116 (live component state handoff is declined,
  for want of a customer) governs; §208 (installing is granting) names the revisit). Their version
  changes ride the update path, where clients re-bind at restart.
- Hot swap may carry a version change only on an N-1 contract, only for a stateless component,
  with the operator verifying that the new instance serves every bound client's version.

The swap test's two invariants generalize into the contract crates' tests: `hot_swap_contract`
asserts them for the swap run already, the version per sequence number never going backwards
(`log_checks::MONOTONE`) and both versions appearing (`log_checks::BOTH_VERSIONS`). A
dual-serving contract's test asserts both across its own swap.

Retiring the legacy generation is itself a version event, never silent. It is a contract-version
bump with a dated ledger entry naming the migration, riding the update path (§235's U4 coherent
set), so in-tree v1 clients move in the same release. The compiler proves that while nothing
outside the tree consumes these interfaces. The spawn-time refusal then fires on a stale client
binding, naming both versions. The ledger records each retirement and whether any client
exercised the N-1 shim; that is the recorded evidence for revisiting the N-1 rule
(event-triggered) if shims go unused.
Posture on record: no customers yet, learn in practice; the rule permits breaking changes where
it bites.

## The implementing milestone

The eight-step plan lives in milestone 956, re-aimed at the ruled shape; this proposal keeps the
survey, the prior art and the costs.

## The seven questions

1. Refused options, and why each lost: per-crate scope (question 1). The single-byte ABI field
   (question 3, refused by the ruling). Client-only and server-only checks (question 2). A
   separate migration document per break (question 4). The rustdoc diff at pull time now
   (question 5). A registry outside the crates (question 1). Version on every message
   (question 2). Each section holds its reason.
2. What the tree already does: the survey above, dated 2026-10-28. One version constant per
   format in `capability_manifest_note`; magics beside their formats; no ABI revision field.
3. Prior art, read not recalled, one lesson per system: 9P refuses cleanly and names both
   versions. Fuchsia stamps its ABI revision into packages. seL4 breaks by coordinated semver
   release, no runtime check. Mach's MIG carries an interface-version byte that nothing compares.
   Linux's syscall promise is add-only; ext4 splits compatible from incompatible sets at mount.
   glibc's loader binds the symbol version a binary names. MCP's 2026 revision declares the
   version per request. Citations, and what this lane did not verify, are in
   [the appendix](version-carrying-interfaces/prior-art.md).
4. The premise, checked: each sub-claim comes from a command run on 2026-10-28. No operation set
   carries a checkable version; five numbered magics and two unnumbered (756 measured the same on
   2026-10-04); no ABI revision field (grep, and byte 55); the churn is real (610's series, cited
   above). The ruling's causal claim matches the tree's words: the `abi` crate exists so a change
   "fails to compile on both sides at once".
5. Costs. A version constant is one line per interface and joins the measured series with no
   helper change. The note's version 3 costs encode, decode, and the Kani property. The
   pinned-values test is small today and grows with the surface.
6. Reversibility, and who has acted: nothing outside the tree has acted on any interface here.
   No SDK archive exists (756's table), and three programs carry manifest notes, all in-tree.
   Every choice is a rename or a version word away until the S2 archive hands one to a stranger.
   The descriptor's released layouts are the one acted-on surface, and question 3's ruling leaves
   them untouched.
7. Same cost, same choice: yes for per-interface scope over per-crate (one protocol's bump would
   falsify the other's version). Yes for the version 3 list over the single byte (it answers the
   second bump as cheaply as the first). Yes for both-sided checks (each side knows its own
   truth). Yes for the pinned-values test over the rustdoc diff today, which is the right tool
   once a consumer lives outside the tree.

## What the ruling leaves open

The implementing proposal carries the work. Eventual, not blocking: the ledger's first entries,
the rustdoc diff's trigger (a consumer outside the tree), the N-1 rule's event-triggered revisit,
and the interface ids' ratification.

## BUGS

- The declaration derives from the link graph, so an interface a program consumes only through a
  grant is invisible to the stamp: a program that maps a shared page by hand declares nothing
  about it. Every program in the first cut links the contracts of the pages it reads.

- The graceful reading of an unversioned peer, where its unknown-operation refusal reads as
  version 0, holds only where the server answers an unknown opcode cleanly. The contract crates
  define the reply's shape; each server's behavior is its own.
- The diff-grep lint reads changed lines, so a value change spelled as a reorder or an enum
  renumbering reaches it only as a version-constant miss. The rustdoc diff would catch it.
- The 610 series reads the new constants as additions in their first week; that is true, not a
  defect.
- One recollection stands unverified in the prior art, and the appendix marks it.
