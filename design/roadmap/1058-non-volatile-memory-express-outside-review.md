---
status: NOT-STARTED
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
[the review package](../../notes/crate-reviews.md). This block proposes work and changes nothing in
that repository. Every item below is work for a lane there. Milestone 814
(`non_volatile_memory_express`: proven NVMe queue logic, released on its own) owns the extraction
and the release path; this block owns the findings that land inside that path.

The review package settles the crate name, the version `0.1.0`, the dual license and every name it
records as ratified, and those stay closed here. It clears the move, and neither the tag nor
publication.

Reuse: the shared crate workflows `nifeos/crate-workflows` and their scaffold, `cargo-mutants`,
Kani, `cargo-fuzz` and the harness shapes of the crate. No new tool.

## Goal

A stranger who finds `nifeos/non_volatile_memory_express` can tell in one screen what it does and
what it leaves to them. They can build, test, prove and falsify it from the README alone. They can
write a working polled driver against QEMU from the example of the README without first reading
the specification. Every name they meet says what it holds, and every claim of the README is a
harness that fails under its patch.

Publishing stays held. No crate from this organization publishes to crates.io until the
maintainers have extracted several crates and a review has decided whether any of them combine.
Nothing in this block lifts that hold, and a green gate here is not permission to publish.

## What the review covered

The reviewer verified these firsthand, in a fresh clone of `main` on one Linux host:

- `cargo build`, `cargo test` (29 unit tests and 7 doctests), `cargo clippy`, `cargo doc` and
  `cargo fmt --check`, all with warnings denied: clean on 1.99.1 and on 1.88, the `rust-version`. The same
  tests pass on 1.85 with `--ignore-rust-version`.
- The crate builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`.
- Kani 0.68.0 verifies all ten harnesses in about 14 seconds in total. Each of the ten
  falsification patches applies, fails exactly its own harness at the assertion its header names,
  and reverses to a clean tree.
- `cargo mutants` over the whole crate: 247 mutants, 161 caught, 76 missed, 10 unviable, no
  timeout. The 76 misses sit on exactly the 28 source lines that `.github/mutants-exempts` lists:
  six production lines whose OR-to-XOR and shift swaps act on disjoint bits, and 22 lines inside
  `#[cfg(kani)]` harness bodies that the ordinary tests never compile.
- All three `cargo-fuzz` targets ran 20 seconds each with no finding. `typos` and
  `cargo deny check` pass, and `cargo package --list` ships 55 files.
- No `unsafe` in `src/`, and no mention of this tree beyond the App secret names in `release.yml`.
- Through the GitHub API: the last weekly run green in every job, nine required checks, no tag.

Inferred, not verified: the ordering hazard of P1-6, read from the API shape and two consumers.
The page-size mismatch of P1-9 is also inferred, since nobody built a kernel with 16 KiB pages
against the crate.

Not run: `cargo semver-checks` (no baseline exists), Miri (green in the weekly job), a long fuzz
campaign, the release dry run, the name checks of the template at its head, and anything that needs
a controller. The reviewer changed nothing in the repository.

Effort sizes: XS is under an hour, S half a day, M a day or two.

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

### P1: before the first release

#### P1-1. `Namespace` does not know its own transfer unit

`Namespace::new(capacity_bytes, blocks_per_transfer)` fixes how many logical blocks one unit
covers, yet `holds_whole_block` and `transfer_command` (`src/namespace.rs`) take `unit` again on
every call. The byte bound uses the caller's `unit`; the logical block address uses the stored
`blocks_per_transfer`. When the two disagree, the byte bound passes and the command addresses a
block past the end. The controller refuses such a command with a status, so this is a claim that is too
strong, not memory unsafety. The harness
`a_transfer_command_is_only_built_for_a_block_the_namespace_has` (`src/verification.rs`) states
only the byte bound, so it cannot see the mismatch.

Ruled 2026-11-09. The type carries the unit. A new constructor,
`Namespace::from_identify(&IdentifyNamespace, unit_bytes) -> Option<Namespace>`, returns `None`
for a unit the namespace cannot serve. `Namespace::new` stays for a caller with no parse, with the
unit as its second argument, and `holds_whole_block` and `transfer_command` drop `unit`. The harness
gains the block bound, `starting_logical_block + blocks <= namespace.blocks` for every accepted
command, with its falsification patch. One-way (three signatures). S.

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

Change, as the review recommends: `minimum_page_size_bytes`; `unit_bytes` and `page_size_bytes`
everywhere; `data_physical_address` for Identify and `queue_physical_address` for the two queue
builders; `role` for the doorbell parameter; and the one spelling `logical_block_size_shift`.
One-way at the tag. S.

#### P1-3. Two harness names and their patch files carry `sq` and `cq`

`sq_push_stays_in_bounds` and `cq_pop_stays_in_bounds_and_flips_only_at_the_wrap`
(`src/verification.rs`) are the names the README advertises and the two files in `falsifications/`
carry. The abbreviation rule spells these out, and the abbreviation report of the template at its
head reads harness names. Change: `submission_push_stays_in_bounds` and
`completion_pop_stays_in_bounds_and_flips_only_at_the_wrap`, with the patch files and their
`Falsifies` lines renamed alongside. The functions are private, so the door is two-way and the
names are the maintainers' provisional call. XS.

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

#### P1-5. The direction of a transfer is a bare `bool`

`transfer_command(.., write: bool)` takes seven arguments and carries
`#[allow(clippy::too_many_arguments)]`, and a `true` at the call site says nothing. Every driver in
the prior art names the direction. Change, as the review recommends: a two-variant enum,
`Direction { Read, Write }` (name provisional), in place of the `bool`. It keeps one builder and
reads at the call site, where two methods `read_command` and `write_command` would double the
builder. One-way at the tag. XS.

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

#### P1-8. Two panics on caller input that no documentation names

`Command::create_io_completion_queue` and `create_io_submission_queue` (`src/command.rs`) compute
`entries as u32 - 1`, which overflows at `entries == 0` and panics in a debug build. Neither doc
comment says so. Neither checks `entries` against `Capabilities::maximum_queue_entries`, or
`queue_id` against zero, the admin pair. `physical_region_page_pair`
(`src/physical_region_page.rs`) asserts a power-of-two page size of at least 512 bytes, and its doc
comment does not say so. No value the controller writes reaches either panic. Change: an `assert!` with a message for `entries >= 2` in both queue builders and a sentence
in each doc comment, and the page-size condition in the doc comment of `physical_region_page_pair`.
The depth check against the capabilities stays with the caller, and the doc says so. Two-way. XS.

#### P1-9. The controller configuration fixes the memory page at 4 KiB

`enabled_controller_configuration()` (`src/configuration.rs`) returns `CC` with `MPS` 0, so a
kernel whose frames are 16 KiB or 64 KiB cannot use it. Nothing ties that `MPS` to the
`page_size_bytes` the page-pair builder takes, so the two can disagree without a warning.
`Capabilities` decodes `MPSMIN` and not `MPSMAX`, so a caller cannot check the upper bound. Change, as the review recommends:
`enabled_controller_configuration(page_size_bytes) -> Option<u32>`. It encodes
`MPS = log2(page_size_bytes) - 12` and refuses a size that is not a power of two or lies outside
4 KiB to 128 MiB. `Capabilities::maximum_page_size_bytes` joins the minimum, and a harness with its
patch shows that the value decodes back to its page size. One-way (a signature and a new
name). S.

#### P1-10. The admin queue attributes have a formula in a comment and no builder

The doc comment of `ADMIN_QUEUE_ATTRIBUTES_REGISTER` (`src/registers.rs`) gives the packing. Every
other value a driver writes has a builder, and a caller who writes this formula by hand repeats the
off-by-one the queue builders exist to prevent. Change: `admin_queue_attributes(submission_entries,
completion_entries) -> u32` (name provisional) with the `entries >= 2` assertion of P1-8, a unit
test of the packing, and a line in the example of the README. A new public name. XS.

#### P1-11. The README lags the shared section order of the template

The README check of the template at its head expects a `## Minimum Rust version` section after
`## BUGS`; this README states the minimum in a sentence inside BUGS. The fuzz paragraph tells the
reader to run `cargo +nightly fuzz run` with no install line for `cargo-fuzz`. The license links
are relative paths, which docs.rs cannot resolve, since `src/lib.rs` includes the README as the
crate documentation. Change: the section, `cargo install cargo-fuzz` before the run line, and
absolute links to the two license files. Two-way. XS.

#### P1-12. The keywords miss the word a user searches for

`keywords = ["no-std", "kani", "driver"]` in `Cargo.toml`, so a crates.io search for `nvme` misses
the crate, whose name lacks those letters too. Change: add `nvme` and `storage`, within the limit of
five. Two-way. XS.

#### P1-13. The template pin predates the mutation stack, and the ruleset does not require it

Every caller workflow (`.github/workflows/ci.yml` and the six others) pins `nifeos/crate-workflows`
at `4e5b07dd`, 68 commits past the newest release of the template and 68 behind its head, so the
lag job passes. The head adds a `mutation.yml` caller, `check / mutation-diff` on every pull request, and a release
gate that wants a green full mutation run at the released commit. The `main` ruleset requires nine checks, and
`check / mutation-diff` is not among them.

Change: when the template releases, take the pin bump from Dependabot, and in the same pull request
add the scaffold's `mutation.yml` caller and budgets file. The settings script then makes the tenth
check required, and one dispatch on `main` gives the release gate a green run. Two-way. S.

#### P1-14. `IdentifyNamespace::default()` reads as a namespace

`Default` (`src/identify.rs`) yields zero blocks of 512 bytes, standing for "before Identify has
answered". A stranger reads `IdentifyNamespace::default()` as a real, empty namespace. Removing a trait implementation after publication breaks callers;
removing it before costs nothing. Change,
as the review recommends: remove the implementation. The alternative keeps it, with a doc comment
that says `blocks == 0` means "not yet identified". One-way at the tag. XS.

## Decisions this block needs

1. The four register spellings (P0-2). Ruled 2026-11-09: ratified as `src/registers.rs` spells
   them.
2. The tag. No tag is ruled for this crate. Which commit carries one, and when, is a separate
   question for the architect. It is one-way, since a tag is a number others compare against.
3. `Namespace` carries its unit (P1-1). Ruled 2026-11-09, as the item states.
4. The public API of P1-2, P1-5, P1-9 and P1-14, each one-way at the tag, with the
   recommendation each item states. These need the architect before the tag.
5. The new names of P1-6, P1-10 and P2-1 to P2-5. A lane mints each provisionally in its own pull
   request, and the release review ratifies them together. Only those that land before the tag
   need a ruling then.
6. Every two-way item, the harness names of P1-3 among them, is a provisional call of the
   maintainers and not a ruling.

The publishing hold stands.

## Prior art

How other drivers solve what the items touch, and where this crate differs, are in
[the prior art appendix](1058-non-volatile-memory-express-outside-review/prior-art.md).

## Done when

The checks, and the steps that need QEMU or a controller, are in
[the done-when appendix](1058-non-volatile-memory-express-outside-review/done-when.md). The review
claims none of the QEMU or hardware steps.

## BUGS

- Nobody has observed the hazard of P1-6 on hardware. Its fix is documentation and one helper,
  cheap enough to land without the observation.
- P1-9 comes from reading the code, not from a kernel with 16 KiB pages built against the crate.
- The prior-art rows for `nvme-driver`, `vroom` and Redox describe their front pages, not their
  code. A user of one of them may miss less, or more, than the table says.
- P1-13 waits on the next release of the template.
- The review did not run the name checks of the template at its head. The review package ran them
  at the pin, and P1-2 and P1-3 are what reading the rules against the code found beyond that run.
- The fuzz runs took 20 seconds per target, and the weekly job gives five minutes. Neither is a
  long campaign, and the repository commits no seed corpus, so each run starts cold.
- The P2 order is the judgment of the reviewer on what a user asks for first. Nobody outside has
  asked for anything yet.

## Index row

An outside reading of `non_volatile_memory_express`: two items block the tag (a manifest comment;
four register spellings, ruled). Fourteen go before the first release, among them a namespace that
cannot hold a wrong unit, unit and address names on parameters, the ordering a weakly ordered host
needs, a page-size argument, and the mutation stack of the template. Ten follow it. No tag is
ruled, and publishing stays held.
