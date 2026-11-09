# P2: after the first release

An appendix to [the milestone](../1058-non-volatile-memory-express-outside-review.md).

## P2-1. A transfer longer than two pages has nowhere to go

`physical_region_page_pair` (`src/physical_region_page.rs`) refuses anything that needs a PRP list,
and the README says so. That forces one command per 4 KiB block on every consumer, and a user coming
from any driver in the prior art will miss the list first. Change:
`physical_region_page_list(base, length_bytes, page_size_bytes, list: &mut [u64]) ->
Option<PhysicalRegionPagePair>` (name provisional). It fills a page the caller supplies with the
entries and returns PRP1 and a PRP2 that points at the list, and it refuses a list that needs a
second list page. A harness shows that every entry is page-aligned, that the entries cover exactly
the transfer, and that the count fits the page, with its patch. A new public name; the behavior of
the pair stays. M.

## P2-2. The completion status is a raw field

`Completion::status` (`src/completion.rs`) is the 15-bit field, and its doc comment gives the bit
positions of the status code type and the status code. A caller decides success with `status == 0`
and shifts by hand for anything else, and `do_not_retry` and `more` stay unreadable without the
specification open. Change: `is_success`, `status_code_type`, `status_code`, `do_not_retry` and
`more` as methods (names provisional, spelled out), total by construction. The generic status code
type and the two codes the example of the README meets become constants, and a harness shows that
the methods rebuild the field. New public names. S.

## P2-3. No value for a clean shutdown, and no status bits for it

The crate gives the enable value of `CC` and the `RDY` and `CFS` bits of `CSTS`. It has no `SHN`
value for `CC` and no `SHST` bits for `CSTS`, so a driver built from it cannot tell the controller
to flush and stop, and a real drive loses its volatile cache at power off. Evidence:
`src/configuration.rs` and the status bits in `src/registers.rs`. Change:
`shutdown_controller_configuration` (normal shutdown notification) and the two
`CONTROLLER_STATUS_SHUTDOWN_*` bits, each written as a shift, with the mnemonics of the
specification as aliases. New public names. XS.

## P2-4. Identify stops at two facts of one namespace

`parse_identify_namespace` (`src/identify.rs`) reads `NSZE` and `LBADS`. The selectors in
`src/command.rs` have no entry for the active namespace list, and nothing parses Identify
Controller: `MDTS`, the maximum data transfer size every transfer must respect; `NN`, the namespace
count; the queue entry sizes. A user with more than one namespace, or with a controller that caps
transfers below two pages, parses the structures by hand. Change:
`CONTROLLER_OR_NAMESPACE_STRUCTURE_ACTIVE_NAMESPACE_LIST` (`CNS` 0x02) and a
`parse_identify_controller` that reads `MDTS` and `NN`, total for any 4096 bytes, with a harness
and patch in the shape of the namespace parse. New public names. S.

## P2-5. Nothing tracks which command identifiers are in flight

The caller mints `command_id`, and the crate never sees it again; `src/queues.rs` holds no
identifier state. Every driver in the prior art owns a tag set, and the hostile completion of P1-7
needs one. Change, after P1-1 and P1-7: a fixed-capacity `InFlight<N>` bitmap (name provisional)
that mints an identifier on push and releases it on an owned completion, and refuses an identifier
it did not mint. A harness shows that no identifier releases twice. A new public name. S.

## P2-6. No test double shows the whole loop

The example of the README shows one command and one hand-written completion, and the repository
has no `examples/` directory. Nothing models a controller that consumes a submission ring and
writes completions with the right phase, so a user cannot see the wrap, the stale lap or a refused
head happen end to end without hardware. Change: `examples/software_controller.rs`, about a
hundred lines that model a controller over two `Vec`s of entries. It drives both rings through
several laps, reports `SQHD`, and once reports an impossible head, under `cargo test --examples`.
It doubles as the test double a consumer copies. Two-way. M.

## P2-7. The mutation ledger carries 22 lines that one setting would retire

Of the 28 lines `.github/mutants-exempts` lists, 22 sit in `#[cfg(kani)]` harness bodies.
`cargo-mutants` reads `exclude_globs` from `.cargo/mutants.toml`, so excluding `src/verification.rs`
whole turns the rule in the ledger's header into configuration. The census line of the header
carries counts that predate the move (179 caught, 7 missed), while the crate now has 247 mutants.
Change: `.cargo/mutants.toml` with `exclude_globs = ["src/verification.rs"]`, the 22 lines deleted,
and the census line refreshed or dropped. Check first that the mutation gate of the template reads
the configuration. Two-way. XS.

## P2-8. The package ships its CI

`cargo package --list` shows 55 files. The workflows, hooks, Dependabot and issue templates,
`deny.toml`, `_typos.toml` and `docs/stranger-test.md` sit beside the source, the falsification
patches, the README, the changelog and the licenses. Only the second group serves a crates.io user,
and `Cargo.toml` has no `include`. Change: `include = ["src/**", "falsifications/**", "README.md",
"CHANGELOG.md", "LICENSE-*", "Cargo.toml"]`, which keeps the patches, since shipping them is the
point. Two-way. XS.

## P2-9. `rust-version` may sit higher than the code needs

The manifest states 1.88. The tests pass on 1.85 with the version check bypassed, and the one
method stable after 1.85 (`is_multiple_of`) sits in `#[cfg(kani)]` code, which builds on Kani's own
toolchain. A lower minimum widens who can take the crate, and lowering later is compatible while
raising is a change users feel. Change: state 1.85, confirm it with the weekly `msrv` job, and
update the badge and the section of P1-11 together. Two-way. XS.

## P2-10. Small text fixes for a stranger

- The README has no section on how the crate differs from what a user knows, so a user who knows
  `nvme-driver` or Linux infers the scope from BUGS. Change: a short
  `## How it differs from Linux and the Rust drivers` section after BUGS, built from the prior-art
  table.
- `CODEOWNERS` says its team may not exist; it does. Change: drop the caveat.
- `deny.toml` carries a `Zlib` allowance that matches nothing, so `cargo deny check` warns on a
  crate with no dependency. The list comes from the scaffold. Change: none here; the template owns
  it, and this line exists so nobody chases it.
- `CompletionQueue::new` (`src/queues.rs`) says "see the struct docs" for its minimum of two
  entries, and the struct docs do not state it. Change: one sentence there.

Two-way. XS.
