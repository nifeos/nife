---
status: PARTIAL
raised: 2026-11-09
milestone_dependencies: 814
decision_dependencies: none
machine_requirements: QEMU with an `nvme` device for the acceptance checks; one real controller for the hardware items; none for the rest
specific_machine: none
needs_person: yes
---
# 1058. `non_volatile_memory_express`: the outside review's findings land before its first release

Raised by the lane `milestone/1058-non-volatile-memory-express-outside-review`. A reviewer with no
prior exposure to this project read `nifeos/non_volatile_memory_express` at commit `87bbef8`
(`main`, after its pull request #12), against the step 5 section of
[the review package](../../notes/crate-reviews.md). Every item below is work for a lane there.
Milestone 814 (`non_volatile_memory_express`: proven NVMe queue logic, released on its own) owns
the extraction and the release path; this block owns the findings that land inside that path.

The review package settles the crate name, the version `0.1.0`, the dual license and every name it
records as ratified. It clears the move, and neither the tag nor publication.

Reuse: the shared crate workflows `nifeos/crate-workflows`, `cargo-mutants`, Kani, `cargo-fuzz`
and the crate's harness shapes. No new tool.

## Goal

A stranger who finds `nifeos/non_volatile_memory_express` can tell in one screen what it does and
what it leaves to them. They can build, test, prove and falsify it from the README alone, and write
a working polled driver against QEMU from its example without first reading the specification. Every name they meet says what it holds, and every claim of the README is a
harness that fails under its patch.

Publishing stays held. No crate from this organization publishes to crates.io until the
maintainers have extracted several crates and a review has decided whether any of them combine.
Nothing in this block lifts that hold, and a green gate here is not permission to publish.

## What the review covered

What the review read, ran and left unrun is in
[the coverage appendix](1058-non-volatile-memory-express-outside-review/what-the-review-covered.md).

## Findings, prioritized

P0 blocks the tag and any publish. P1 lands before the first release. P2 follows it, in
[the appendix](1058-non-volatile-memory-express-outside-review/after-the-first-release.md). A door
is one-way where somebody outside the repository acts on it after the release: a public name or
signature, a published behavior, a layout, a license. A public name is two-way until the tag and
one-way after it. The maintainers decide every two-way item provisionally.

### P0: the manifest and the register names

#### P0-1. The publish line of the manifest points outside the repository

`publish = false` in `Cargo.toml` is deliberate, and its comment sends the reader to
`crates/publish-allowlist.txt`, a file that does not exist in this repository. Change: a comment in
the crate's own words that says why it is unpublished, the publishing hold above. The release that
publishes, whenever the hold lifts, deletes the line. Two-way. XS.

Status: Landed in nifeos/non_volatile_memory_express#15.

#### P0-2. Four public register names needed a ruling before the tag

`src/registers.rs` spells four offsets differently from the spellings an earlier record of their
names carried. All four follow the register and address rules.

Ruled 2026-11-09. The four names stand as the code spells them:

- `CONTROLLER_CAPABILITIES_REGISTER`, with the alias `CAP`;
- `FIRST_DOORBELL_REGISTER`, which carries no `doc(alias)`;
- `ADMIN_SUBMISSION_QUEUE_BASE_PHYSICAL_ADDRESS_REGISTER`, with the alias `ASQ`;
- `ADMIN_COMPLETION_QUEUE_BASE_PHYSICAL_ADDRESS_REGISTER`, with the alias `ACQ`.

These, and not `CAPABILITIES`, `DOORBELL_BASE`, `ADMIN_SUBMISSION_QUEUE_BASE_ADDRESS_REGISTER` or
`ADMIN_COMPLETION_QUEUE_BASE_ADDRESS_REGISTER`, are the ratified names. The two queue base
registers hold physical addresses, and [the naming conventions](../naming.md) put
`_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS` on an address in every location. No code change follows.
One-way (public names). XS.

Status: Ruled, and `main` already spelled them so.

### P1: before the first release

#### P1-1. `Namespace` does not know its own transfer unit

`Namespace::new(capacity_bytes, blocks_per_transfer)` fixes how many logical blocks one unit
covers, yet `holds_whole_block` and `transfer_command` (`src/namespace.rs`) take `unit` again on
every call. The byte bound uses the caller's `unit`; the logical block address uses the stored
`blocks_per_transfer`. When the two disagree, the byte bound passes and the command addresses a
block past the end. The controller refuses such a command with a status, so this is not memory unsafety. The harness
`a_transfer_command_is_only_built_for_a_block_the_namespace_has` (`src/verification.rs`) states
only the byte bound, so it cannot see the mismatch.

Ruled 2026-11-09. The type carries the unit. A new constructor,
`Namespace::from_identify(&IdentifyNamespace, unit_bytes) -> Option<Namespace>`, returns `None`
for a unit the namespace cannot serve. `Namespace::new` stays for a caller with no parse, with the
unit as its second argument, and `holds_whole_block` and `transfer_command` drop `unit`. The harness
gains the block bound, `starting_logical_block + blocks <= namespace.blocks` for every accepted
command, with its falsification patch. One-way (three signatures). S.

Status: Landed in nifeos/non_volatile_memory_express#19.

#### P1-2. Parameters and methods hold a quantity or an address without saying so

- `Capabilities::minimum_page_size` (`src/capabilities.rs`) returns bytes with no unit in its name.
- `IdentifyNamespace::blocks_per_transfer(unit)` (`src/identify.rs`), `holds_whole_block` and
  `transfer_command` take byte counts named `unit` and `page_size`. The neighboring
  `physical_region_page_pair` says `length_bytes` and `page_size_bytes`.
- The four `Command` builders (`src/command.rs`) take an address named `physical_region_page`,
  while `transfer_command` says `data_physical_address`.
- `doorbell(queue_id, which, doorbell_stride)` (`src/doorbell.rs`) names its role parameter with a
  word that could name anything.
- The free function `logical_block_format_size_shift` and the field `logical_block_size_shift`
  spell one quantity two ways.

Ruled 2026-11-09: `minimum_page_size_bytes`, and `unit_bytes` and `page_size_bytes` everywhere.
Every address parameter takes a `_physical_address` suffix: `data_physical_address` for Identify,
`queue_physical_address` for the two queue builders. The doorbell parameter becomes `role`, and the
shift takes the one spelling `logical_block_size_shift`. One-way at the tag. S.

Status: Landed in nifeos/non_volatile_memory_express#19; `doorbell_stride` later became
`doorbell_stride_exponent` in nifeos/non_volatile_memory_express#35.

#### P1-3. Two harness names and their patch files carry `sq` and `cq`

`sq_push_stays_in_bounds` and `cq_pop_stays_in_bounds_and_flips_only_at_the_wrap`
(`src/verification.rs`) are the names the README advertises and the two files in `falsifications/`
carry. The abbreviation rule spells these out, and the abbreviation report of the template at its
head reads harness names. Change: `submission_push_stays_in_bounds` and
`completion_pop_stays_in_bounds_and_flips_only_at_the_wrap`, with the patch files and their
`Falsifies` lines renamed alongside. The functions are private, so the door is two-way and the
names are the maintainers' provisional call. XS.

Status: Landed in nifeos/non_volatile_memory_express#19.

#### P1-4. Most decoded fields lack the specification's mnemonic as an alias

The register offsets carry `CAP`, `CC`, `CSTS`, `AQA`, `ASQ` and `ACQ` as aliases, and the opcodes
carry theirs. The decoded fields carry none, so a reader who searches the documentation for the
specification's term finds nothing:

- in `Capabilities`: `maximum_queue_entries` (`MQES`), `doorbell_stride` (`DSTRD`),
  `minimum_page_size` (`MPSMIN`);
- in `IdentifyNamespace`: `blocks` (`NSZE`), `logical_block_size_shift` (`LBADS`, `FLBAS`);
- in `Completion`: `submission_queue_head` (`SQHD`), `submission_queue_id` (`SQID`), `command_id`
  (`CID`), `phase` (`P`), `status` (`SF`);
- `enabled_controller_configuration` (`CC`, `EN`, `MPS`, `IOSQES`, `IOCQES`).

Change: one `#[doc(alias)]` per mnemonic, and the mnemonic in the opening sentence of each doc
comment that lacks it. Two-way. XS.

Status: Landed in nifeos/non_volatile_memory_express#17.

#### P1-5. The direction of a transfer is a bare `bool`

`transfer_command(.., write: bool)` takes seven arguments and carries
`#[allow(clippy::too_many_arguments)]`, and a `true` at the call site says nothing. Every driver in
the prior art names the direction.

Ruled 2026-11-09: a two-variant enum, `Direction` with the variants `Read` and `Write`, replaces
the `bool`, and one builder stays rather than two methods. One-way at the tag. XS.

Status: Landed in nifeos/non_volatile_memory_express#19.

#### P1-6. Nothing states the memory ordering either ring needs

The example of the README writes the command, pushes, and says "ring the tail". Nothing says the
stores of the entry must reach the device before the doorbell write, which on a weakly ordered host
needs a barrier. On the completion side, `Completion::from_dwords` (`src/completion.rs`) takes all
four dwords at once, so a caller reads the whole entry and then asks `CompletionQueue::is_owned`
(`src/queues.rs`). On a weakly ordered host the loads of dwords 0 to 2 may precede the write of the
controller while dword 3 follows it, and a fresh phase then carries a stale head and command
identifier. Both consumers that pin this repository read the entry as one volatile `[u32; 4]` after a
single barrier before the poll loop.

Change: state the discipline in three sentences in the README and on `SubmissionQueue::push` and
`CompletionQueue::is_owned`. A write barrier goes between the entry and the tail doorbell. The
caller reads dword 3 first and tests the phase, then issues a read barrier, then reads the other
three dwords. A helper, `Completion::phase_of(dword_three: u32) -> bool` (name provisional), lets a
caller test ownership before it has the whole entry. The text is two-way; the helper is a new
public name. XS.

Status: Landed in nifeos/non_volatile_memory_express#16 (the text) and
nifeos/non_volatile_memory_express#23 (the helper, `Completion::is_phase_tag_set`).

#### P1-7. The example of the README teaches a completion that frees nothing

The worked completion is `[0, 1 | (1 << 16), 0, 1 | (1 << 16)]`. Dword 2 is zero, so
`submission_queue_head` is 0 while the comment says the controller consumed the command. The ring
accepts head 0 as no progress, and slot 0 stays in flight forever. Dword 1 is reserved and carries
an arbitrary value. The README also says `is_owned` gates the entry without saying that it checks
the phase alone. Matching `command_id` and `submission_queue_id` against what it issued is the
caller's job, and a hostile entry with a fresh phase and an unknown identifier passes `is_owned`.
Change: dword 2 becomes `1` (head 1, queue 1) and dword 1 becomes `0`. A sentence in the README's
BUGS and on `is_owned` says that ownership is the phase tag only and that the caller matches
identifiers. Two-way. XS.

Status: Landed in nifeos/non_volatile_memory_express#16.

#### P1-8. Two panics on caller input that no documentation names

`Command::create_io_completion_queue` and `create_io_submission_queue` (`src/command.rs`) compute
`entries as u32 - 1`, which overflows at `entries == 0` and panics in a debug build. Neither doc
comment says so. Neither checks `entries` against `Capabilities::maximum_queue_entries`, or
`queue_id` against zero, the admin pair. `physical_region_page_pair`
(`src/physical_region_page.rs`) asserts a power-of-two page size of at least 512 bytes, and its doc
comment does not say so. No value the controller writes reaches either panic. Change: an `assert!` with a message for `entries >= 2` in both queue builders and a sentence
in each doc comment, and the page-size condition in the doc comment of `physical_region_page_pair`.
The depth check against the capabilities stays with the caller, and the doc says so. Two-way. XS.

Status: Landed in nifeos/non_volatile_memory_express#16.

#### P1-9. The controller configuration fixes the memory page at 4 KiB

`enabled_controller_configuration()` (`src/configuration.rs`) returns `CC` with `MPS` 0, so a
kernel whose frames are 16 KiB or 64 KiB cannot use it. Nothing ties that `MPS` to the
`page_size_bytes` the page-pair builder takes.
`Capabilities` decodes `MPSMIN` and not `MPSMAX`, so a caller cannot check the upper bound. Ruled
2026-11-09: `enabled_controller_configuration(page_size_bytes) -> Option<u32>`. It encodes
`MPS = log2(page_size_bytes) - 12` and returns `None` for a size that is not a power of two or lies
outside 4 KiB to 128 MiB. `Capabilities::maximum_page_size_bytes` joins the minimum. A harness with
its falsification patch shows that the encoded value decodes back to the page size it came from.
One-way (a signature and a new
name). S.

Status: Landed in nifeos/non_volatile_memory_express#20.

#### P1-10. The admin queue attributes have a formula in a comment and no builder

The doc comment of `ADMIN_QUEUE_ATTRIBUTES_REGISTER` (`src/registers.rs`) gives the packing. Every
other value a driver writes has a builder, and a caller who writes this formula by hand repeats the
off-by-one the queue builders exist to prevent. Change: `admin_queue_attributes(submission_entries,
completion_entries) -> u32` (name provisional) with the `entries >= 2` assertion of P1-8, a unit
test of the packing, and a line in the example of the README. A new public name. XS.

Status: Landed in nifeos/non_volatile_memory_express#22 as `admin_queue_attributes`.

#### P1-11. The README lags the shared section order of the template

The README check of the template at its head expects a `## Minimum Rust version` section after
`## BUGS`; this README states the minimum in a sentence inside BUGS. The fuzz paragraph tells the
reader to run `cargo +nightly fuzz run` with no install line for `cargo-fuzz`. The license links
are relative paths, which docs.rs cannot resolve, since `src/lib.rs` includes the README as the
crate documentation. Change: the section, `cargo install cargo-fuzz` before the run line, and
absolute links to the two license files. Two-way. XS.

Status: Landed in nifeos/non_volatile_memory_express#16.

#### P1-12. The keywords miss the word a user searches for

`keywords = ["no-std", "kani", "driver"]` in `Cargo.toml`, so a crates.io search for `nvme` misses
the crate, whose name lacks those letters too. Change: add `nvme` and `storage`, within the limit of
five. Two-way. XS.

Status: Landed in nifeos/non_volatile_memory_express#16.

#### P1-13. The template pin predates the mutation stack, and the ruleset does not require it

Every caller workflow (`.github/workflows/ci.yml` and the six others) pins `nifeos/crate-workflows`
at `4e5b07dd`, 68 commits past the newest release of the template and 68 behind its head, so the
lag job passes. The head adds a `mutation.yml` caller, `check / mutation-diff` on every pull request, and a release
gate that wants a green full mutation run at the released commit. The `main` ruleset requires nine checks, and
`check / mutation-diff` is not among them.

Change: when the template releases, take the pin bump from Dependabot, and in the same pull request
add the scaffold's `mutation.yml` caller and budgets file. The settings script then makes the tenth
check required, and one dispatch on `main` gives the release gate a green run. Two-way. S.

Status: Landed: nifeos/non_volatile_memory_express#13 added the mutation caller, and
nifeos/non_volatile_memory_express#34 pins template release `v8`. The ruleset requires `check /
mutation-diff`, and `mutation / mutation` has a green full run on `main`.

#### P1-14. `IdentifyNamespace::default()` reads as a namespace

`Default` (`src/identify.rs`) yields zero blocks of 512 bytes, standing for "before Identify has
answered". A stranger reads `IdentifyNamespace::default()` as a real, empty namespace. Removing a trait implementation after publication breaks callers;
removing it before costs nothing.

Ruled 2026-11-09: the `Default` implementation goes before the tag, and the state before Identify
answers becomes `None` in an `Option<IdentifyNamespace>`. One-way at the tag. XS.

Status: Landed in nifeos/non_volatile_memory_express#19.

## Decisions this block needs

1. The four register spellings (P0-2). Ruled 2026-11-09: ratified as `src/registers.rs` spells
   them.
2. The tag. Ruled: `v0.1.0-pre.1` and `v0.1.0-pre.2` exist, and #37 prepares the `v0.1.0-pre.3`
   ruled on 2026-11-19. A plain `v0.1.0` waits with the first publish, which stays held until every
   outside review milestone has delivered (ruled 2026-11-18).
3. The public API of P1-1, P1-2, P1-5, P1-9 and P1-14. All five ruled 2026-11-09, as the items
   state, and landed.
4. The new names of P1-6, P1-10 and P2-1 to P2-5. Rulings of 2026-11-19 settled seven, and
   nifeos/non_volatile_memory_express#35 and #36 landed them. Ruled 2026-11-19: the other fourteen
   stand as built, ratified. They are `admin_queue_attributes`, `physical_region_page_list`,
   `Completion::is_success`, `status_code` and `status_code_type`, `STATUS_CODE_TYPE_GENERIC`,
   `GENERIC_STATUS_SUCCESSFUL_COMPLETION`, `GENERIC_STATUS_INVALID_FIELD_IN_COMMAND`,
   `CONTROLLER_STATUS_SHUTDOWN_OCCURRING`, `CONTROLLER_STATUS_SHUTDOWN_COMPLETE`,
   `CONTROLLER_OR_NAMESPACE_STRUCTURE_ACTIVE_NAMESPACE_LIST`,
   `IdentifyController::maximum_data_transfer_bytes`, `NotInFlightError` and
   `CompletionQueue::expects_phase_tag_set`.
5. Every two-way item, the harness names of P1-3 among them, is a provisional call of the
   maintainers and not a ruling. So are the two of #38: two kernel command line words that shape
   the QEMU example, and a BUGS line in place of a parse for P2-4's entry sizes.

6. Whether the first release waits for the two hardware rows. Ruled 2026-11-19: it waits for both.
   One is the driver on one real controller with a non-zero `FLBAS`, a drive with a 4096-byte
   logical block format. The other is the long ordering run on a weakly ordered host. Both must
   run on real hardware. The planned host is radon (VisionFive 2, JH7110, RVWMO) with an M.2 NVMe
   drive, once milestone 163 lands. An emulated NVMe device under QEMU may serve as interim
   evidence and satisfies neither row.

The publishing hold stands.

## Open decisions

None. The publishing hold stands.

## Prior art

How other drivers solve what the items touch, and where this crate differs, are in
[the prior art appendix](1058-non-volatile-memory-express-outside-review/prior-art.md).

## Done when

The checks, and the steps that need QEMU or a controller, are in
[the done-when appendix](1058-non-volatile-memory-express-outside-review/done-when.md). The review
claims none of the QEMU or hardware steps.

## BUGS

- Nobody has observed the hazard of P1-6 on hardware.
- P1-9 has run under QEMU with a 16 KiB controller page, not on a kernel with 16 KiB pages built
  against the crate.
- The prior-art rows for `nvme-driver`, `vroom` and Redox describe their front pages, not their code.
- The review did not run the name checks of the template at its head, only at the pin.
- The fuzz runs took 20 seconds per target and the weekly job gives five minutes, with no seed
  corpus, so each run starts cold.
- The P2 order is the reviewer's judgment of what a user asks for first.

## Where this stands

Every finding has landed but the queue entry sizes of P2-4 (#38). Two steps need the bench, and
they block the first release.

| item | what remains | blocker |
|---|---|---|
| P2-4, QEMU | The entry size BUGS line and the QEMU runs at depth 2 and with a 16 KiB page, all in #38 | #38 is a draft until `v0.1.0-pre.3` is tagged |
| Hardware | The driver on one real controller with a non-zero `FLBAS` | bench; blocks the first release |
| Ordering | A long ordering run on a weakly ordered host | bench; blocks the first release |
| Tag | `v0.1.0-pre.3`, ruled and prepared in #37 | another lane tags it |
| Release | `publish = false` goes, and `v0.1.0`, only when the hold lifts | publishing hold |

## Follow-on

- **Done.** Each finding's status line cites the pull request in
  `nifeos/non_volatile_memory_express` that landed it.
- **Done.** The tags `v0.1.0-pre.1` (`27aa19c`) and `v0.1.0-pre.2` (`73b335c`), and the stranger
  and QEMU runs of #33.
- **Outstanding.** Every row of the table above, checked on `main` and in the open
  pull requests. The two bench rows block delivery, and the first publish of every reviewed crate
  waits for them (decision 6).

## Index row

An outside reading of `non_volatile_memory_express`: two items block the tag (a manifest comment;
four register spellings, ruled). Fourteen go before the first release. Five of them change the
public API and are ruled: a namespace that carries its unit, unit and address names on parameters,
a `Direction` type, a page-size argument, and no `Default` on the Identify parse. The rest include
the ordering a weakly ordered host needs and the mutation stack of the template. Ten follow it.
Pre-release tags have a ruling, and publishing stays held.
