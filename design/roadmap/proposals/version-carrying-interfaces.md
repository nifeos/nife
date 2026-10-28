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
moved package declares what it links". It replaces condition 4 of milestone 756 (the first code
to leave this repository): four settled weeks per linked interface. The reasoning: in a monorepo
a break is free, because the compiler catches every consumer in the same tree, so churn prices
nothing; the split is what prices breaks; waiting for stability inverts the causality.

It answers the ruling's five open questions with options and a recommendation each. It
sketches code and writes none: §311 (a contract is code, not prose) reserves the checks for an
implementing lane, and the ruling on the shape is what this lane stops for. Every new name below
is provisional.

Reuse: the checks reuse the tree's rustdoc-JSON extraction and `FORMAT_CONST` pattern
(`helpers/interface_stability.py`). No new code and no dependency; `cargo-semver-checks` and
`cargo-public-api` stay refused as milestone 610 records.

## What the tree does today

§151 (the goal of the repository split is independent release and third-party programs) named the
precondition in 2026-09. Per milestone 39 (repository structure for a loosely-coupled OS) and §14
(the project's direction): the wire protocols carry real version numbers and have stopped
churning weekly. Milestone 756 measured it on 2026-10-04 and found the first half unmet; this
lane re-measured on 2026-10-28:

- Five protocol crates carry a numbered magic on a shared-page layout: `TIMEBAS1`, `CURRCPU1`,
  `ENVCONF1`, `MACHSTA1`, `CLOCKv01`. Two carry an unnumbered one: `nifeargv`, `nifeklog`.
- No IPC operation set carries a version a peer could check. `filesystem_contract` packs the
  opcode into the first word's high byte and holds no version; `socket_contract` and
  `login_contract` hold none. In `hot_swap_contract`, `STATE_MAGIC` carries no number and
  `BOTH_VERSIONS` is a right bit, not an identity.
- Formats do better: `capability_manifest_note` (`VERSION = 1`, `ZONED_VERSION = 2`), `boot_slot`
  (`SLOT_VERSION = 1`), `package_archive` (`NIFEPKG1`), `nifefs` (`CRKR0002`).
- The ABI revision field §235 (the OS is built and updated from packages) ruled into the manifest
  note does not exist. The version 1 descriptor runs 56 bytes and byte 55 is its last zero;
  version 2 appends zones after it. A grep for `ABI_REVISION` over `crates/` returns nothing.

Milestone 610 (the interface's stability is measured weekly) measures that churn in its weekly
series: 157 breaking changes to the contract crates over 2026W37 to 2026W40, the first split's
reading. Under the restated boundary headline, 2026W39 to 2026W42 read 10 boundary moves and 49
syscall additions.

## Question 1: where the version constant lives, and what its scope is

Options: (a) one constant per crate; (b) one per operation set, module-scoped inside the crate;
(c) one per wire format.

**Recommendation: (c), one version constant per wire format, defined in the crate that owns the
format (AGENTS.md rule 7), module-scoped where a crate holds several.** The tree already does
this: `capability_manifest_note` carries three version constants, one per format.

Scope follows the check, and the check is per bind: `filesystem_contract` holds the block and
file protocols, so a per-crate constant would re-version file clients for a block-only break.

The first cut's four interfaces do not share one kind:

- `abi` is the syscall surface. One constant at the crate root, provisionally `ABI_VERSION`. Its
  scope is the syscall, method and error numbers a program matches.
- `address_space_map` is a set of layout constants the loader and the kernel compile in. One
  constant, provisionally `VERSION`; the scope is the bands and the pair-page arithmetic.
- `glob` is a library with no peer and no wire. One constant, so the declaration of question 3
  can name it. Nothing checks it at run time.
- `machine_statistics_contract` is a shared-page format whose first word is the magic `MACHSTA1`.
  The page gains a version word beside the magic (a new index in the `word` module, provisionally
  `VERSION`), because a shared page has no CALL to carry one.

New constants named `*VERSION*` join the measured surface for free: the helper's `FORMAT_CONST`
pattern reads any public `*VERSION*`, `*MAGIC*` or `*REVISION*` constant as a format identity, so
610's format-bump column counts these with no helper change.

Refusals. Embedding the version inside the magic word, as `MACHSTA1` and `CRKR0002` do, stays
the shared-page convention. The added word is for operation-set contracts, whose opcode fills the
whole first word and has nowhere to embed a number. A registry outside the crates loses: it holds
a fact about an interface away from the interface, rung 4 where rung 3 fits.

## Question 2: what the bind-time check compares, and where it lives

Options: (a) the client checks the server's served version; (b) the server checks the client's
declared version; (c) both, and each refuses a mismatch, naming both versions.

**Recommendation: (c) where a two-sided bind exists; the single-sided forms are (c) reduced to
the sides that exist.** The check compares the interface version the caller built against with
the interface version the peer serves. Both numbers come from question 1's constants: one
definition, no prose to drift (§311). Where the check lives depends on the interface's kind:

- A message-based contract: the caller asks once at bind. One reserved operation (provisionally
  `OPERATION_VERSION`, opcode 0 where the protocol leaves it free) returns the server's version,
  and the caller refuses, naming both numbers. The kernel routes these words without
  reading opcodes; the check belongs to the two endpoints.
- A shared-page format: the reader checks the magic and the version word before trusting any
  other word. One side writes and one reads; the check is one-sided by nature.
- The syscall ABI: the check happens at spawn and is already ruled. §235 puts the ABI revision a
  program built against into its manifest note, "so the progenitor can refuse one it no longer
  supports". The comparison pits the note's declared ABI version against the kernel's
  `ABI_VERSION` constant.
- A link-time library (`glob`): no run-time check exists; the declaration of question 3 records
  the version, and assembly checks the record.

A mismatch refuses cleanly and names both versions: "server speaks 2; the client built against
1". The progenitor already refuses a program whose note it cannot accept (the malformed-note
witness).

The check is code both sides use, in the contract crate. A sketch:
`agrees(own: u32, served: u32) -> Result<u32, Mismatch>`, with `Mismatch` carrying both numbers,
one per protocol. This lane writes none of it; the ruling on the shape is the gate.

Refusals. A client-only check loses: the server knows what it still serves, and Fuchsia's
component manager refuses a component whose declared ABI revision the platform does not cover. A
server-only check loses symmetrically: a client that binds an older server gets no refusal today
and misreads bytes tomorrow. Declaring the version on every message costs a header field per
call, the Model Context Protocol's 2026 shape.

## Question 3: what the package declares, and the note field's encoding

§235 ruled, on 2026-09-27 (UTC) in #1389, that the ABI revision is a field in
milestone 597's (a program carries its manifest in an ELF note) manifest note, not a separate
note. It left the encoding to whoever builds it: "the descriptor's reserved zero bytes, or a
longer layout". Amending version 1 in place holds
only while nothing outside the tree has acted on the note. Milestone 597 built the note; version
2 joined it on 2026-10-19; the field does not exist today.

What the field must hold: which interface versions the binary built against, at per-interface
granularity. The gate's words are plural ("what it links"), and one number cannot say which
interface moved.

Options: (i) one coarse number, with byte 55 as the ABI revision, amending versions 1 and 2 in
place; (ii) a version 3 descriptor: version 1's 56 bytes unchanged, then a counted list of
(interface, version) pairs.

**Recommendation: (ii).** The build stamps the list mechanically: it reads each external crate in
the binary's dependency graph, takes its interface constants, and writes the pairs. Nobody
maintains the declaration by hand, so it cannot rot. The encode and decode machinery and the
one-encoding property carry over from 597's gate.

Option (i) costs one byte and one comparison, and it matches Fuchsia's coarse stamp (RFC-0239
calls it coarse). It also spends the in-place amendment that §235's condition protects. On the
equal-cost test, (ii) survives: it answers the second interface bump as cheaply as the first.

The pairs need an interface identity that survives a crate rename. Provisionally each interface
crate carries `INTERFACE_ID: u64` beside its version constant, values never reused; the id keeps
the declaration's meaning across a rename.

Who checks the declaration: the kernel's served versions are the same constants, under rule 7's
one definition. Until the split, the progenitor and the package link the same crates, so the
comparison runs at spawn. After the split, basalt's manifest carries the kernel's table, and
assembly refuses a package declaring a version the image does not serve.

## Question 4: what a version bump owes, and what an addition owes

Milestone 610 records the ledger discipline: a dated list in the format's own module
documentation, one item per in-place amendment, at AGENTS.md's rung 3. Nothing checks that an
entry exists; 610's BUGS says so.

Options:

- (a) A breaking change bumps the constant and owes a ledger entry plus a migration
  document.
- (b) A breaking change bumps the constant and owes one dated ledger item naming what moved and
  how a consumer migrates. An addition owes no bump and no item.
- (c) Every change, additions included, owes a ledger item.

**Recommendation: (b).** The migration note lives in the ledger item, beside the format it
migrates, not in a document a reader must find. An addition is a change an older peer ignores
without misreading: a new opcode, answered with the unknown-operation refusal, or a new optional
field. The definition is behavioral: a semantic change behind an unchanged number is a break and
owes the bump, which is also 610's blind spot about unchanged signatures.

The prior art splits the same way. ext4's superblock separates compatible from incompatible
feature sets, and the mount rule refuses an unknown incompatible bit while ignoring an unknown
compatible bit. Fuchsia's platform versioning agrees: an element that arrives without breaking
the ABI, a new system call for example, expands the existing revisions' semantics, and no new
revision mints.

## Question 5: the frozen-identifier rule, and what enforces it

The rule as discipline: a syscall number is never renumbered or removed, and a format never
changes in place; a change is a new version. AGENTS.md already makes a brand-new syscall number a
design fork a lane raises before building. The question is what catches a violation besides the
weekly 610 series, which records a week late and refuses nothing.

Options:

- (a) Nothing beyond 610 and review.
- (b) A pinned-values test in `abi`, plus a lint rule: a pull request changing a `FORMAT_CONST`
  value must change its crate's version constant.
- (c) A full interface diff at pull time, reusing the helper's rustdoc-JSON extraction to compare
  base and head.

**Recommendation: (b), priced.** The `abi` test spells every public number as an expected literal:
six syscall constants today, the object modules' method numbers, the `Error` codes.
Updating it is the deliberate, visible act a renumber should be; skipping it fails CI. The
lint reads the pull request's changed lines, not a build, and fails when a magic or version value
moved without its constant. Together they put the rule at rung 2, a gate that fires, not
rung 4, somebody notices.

Option (c) costs a nightly rustdoc run per pull request, minutes of CI at the tree's pull rate.
It also inherits 610's blind spots: `#[doc(hidden)]`
items, `cfg(target_arch)` items for the other two architectures, glob re-exports, and a macro's
fingerprint being its source text. The compiler already catches every in-tree consumer in the
same pull request; (c) buys intent detection, and (b) buys that for the two rules that matter
without any rustdoc. The trigger for (c) is the SDK's event: a consumer outside the
tree, whose breakage the compiler no longer sees.

Refusals. A tree-wide test listing every format constant's value is (b)'s lint done by hand: it
duplicates the surface to check it and rots with every addition.

## The plan an implementing lane follows

1. Re-derive the cut's linked interface list from `packages/process-tools.package.toml`
   membership and the crate dependency graph, not from 756's inline list of four (one derivation,
   2026-10-04). The lane starts from the graph.
2. Add the linked interfaces to the helper's `CONTRACTS` set, so 610's per-crate view measures
   them. Today `abi` is in; `address_space_map`, `glob` and `machine_statistics_contract` are
   not, so three of the four have no row.
3. Add the version constants and the page's version word (question 1).
4. Build the note encoding the ruling picks (question 3): encode, decode, the one-encoding Kani
   property, and the progenitor's refusal naming both versions.
5. Add the bind-time checks where a peer exists (question 2), as code in the contract crates,
   with the reserved version operation where the protocol leaves opcode 0 free.
6. Stamp the declaration at package build (question 3).
7. Add the enforcement of question 5, option (b).
8. Aim 756's condition 4 at what now exists: each linked interface carries its constant, the note
   carries the declaration, the checks refuse a mismatch, and 610's per-crate view reads all
   four. Every part checkable by a command.

## The seven questions

1. Refused options, and why each lost: per-crate scope (question 1); the single-byte ABI field
   (question 3); client-only and server-only checks (question 2); a separate migration document
   per break (question 4). The rustdoc diff at pull time now (question 5). A registry outside the
   crates (question 1). Version on every message (question 2).
2. What the tree already does: the survey above, dated 2026-10-28. One version constant per
   format in `capability_manifest_note`; magics beside the formats they number; no ABI revision
   field. And 610's `FORMAT_CONST` pattern already reads `*VERSION*` constants as identities.
3. Prior art, read not recalled, one lesson per system: 9P's exchange refuses cleanly and names
   both versions. Fuchsia splits the interface constant from the launcher's check, and stamps its
   ABI revision into packages. seL4 breaks by coordinated semver release, with no runtime check.
   Mach's MIG carries an interface-version byte that nothing compares. Linux's syscall promise is
   add-only, and ext4 splits features into compatible and incompatible sets at mount. glibc's
   loader binds the symbol version a binary names. The Model Context Protocol's 2026 revision
   declares the version per request. Citations, and what this lane did not verify, are in
   [the appendix](version-carrying-interfaces/prior-art.md).
4. The premise, checked: each sub-claim comes from a command run on 2026-10-28. No operation set
   carries a checkable version (grep over `crates/`). Five numbered magics and two unnumbered
   (grep; 756 measured the same on 2026-10-04). The ABI revision field does not exist
   (grep, and the descriptor table's byte 55). The churn is real (610's series, cited above). The
   ruling's causal claim matches the tree's own words: the `abi` crate exists so a change "fails
   to compile on both sides at once".
5. What each option costs. A version constant is one line per interface and joins the measured
   series with no helper change. The note's version 3 costs encode, decode, and the Kani
   property; 597's gate is the shape (four reader harnesses, two format harnesses, the
   malformed-note witness). The version operation is one opcode and one reply word per protocol.
   The pinned-values test is small today and grows with the surface.
6. Reversibility, and who has acted: nothing outside the tree has acted on any interface here.
   No SDK archive exists (756's table), and three programs carry manifest notes, all in-tree.
   Every choice here is a rename or a version word away until the S2 archive hands one to a
   stranger. The descriptor's released layouts are the one acted-on surface; question 3's
   recommendation leaves them untouched.
7. Same cost, same choice: yes for per-interface scope over per-crate (the check is per bind, and
   one protocol's bump would falsify the other's version). Yes for the version 3 list over the
   single byte (it answers the second bump as cheaply as the first). Yes for both-sided checks
   (each side is the only one that knows its own truth). Yes for the pinned-values test over the
   rustdoc diff today; the diff is the right tool once a consumer lives outside the tree.

## What a no does

A no leaves the cut's gate unmet and the split waits. The proposal
returns, re-aimed at whatever shape the ruling picks instead. Blocking now: the version's scope
(question 1), the check's placement (question 2), the manifest field's granularity and encoding
(question 3), and the enforcement tier (question 5). Eventual: the ledger's first
real entries, the rustdoc diff's trigger, and the interface ids' ratification.

## BUGS

- The declaration derives from the link graph, so an interface a program consumes only through a
  grant is invisible to the stamp: a program that maps a shared page by hand declares nothing
  about it. Every program in the first cut links the contracts of the pages it reads.
- The graceful reading of an unversioned peer, where its unknown-operation refusal reads as
  version 0, holds only where the server answers an unknown opcode cleanly. The
  contract crates define the reply's shape; each server's behavior is its own.
- The diff-grep lint reads changed lines, so a value change spelled as a reorder or an enum
  renumbering reaches it only as a version-constant miss. The rustdoc diff would catch it.
- The 610 series reads the new constants as additions in their first week; that is true, not a
  defect.
- One recollection stands unverified in the prior art, and the appendix marks it.
