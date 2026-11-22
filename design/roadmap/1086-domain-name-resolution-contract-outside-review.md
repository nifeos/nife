---
status: BUILT
built: 2026-11-22
raised: 2026-11-11
milestone_dependencies: 1017, 956
decision_dependencies: 248, 252
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1086. `domain_name_resolution_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review lane that met `crates/domain_name_resolution_contract` the way a
maintainer outside this tree would, on a fresh clone of `main`. It changes nothing; it lists what
the next lanes change before the crate moves to `nifeos/domain_name_resolution_contract`.

The review package for this crate is in `notes/crate-reviews/`
([domain-name-resolution-contract.md](../../notes/crate-reviews/domain-name-resolution-contract.md)).
Where it and this block differ, the package wins.
The architect ratified the crate name (`src/lib.rs` lines 22 to 24). Pull request #2797 ratified
the three constants `NAME_TEXT_MAXIMUM`, `MAXIMUM_ADDRESSES` and `GRANTS_MAXIMUM`. The architect
froze the interface identity, `INTERFACE_ID` 11 and `VERSION` 1 (decision 3). Milestone 956 (the linked
interfaces carry checkable versions) minted it, and
`design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md` lists it. The
crate has its place in the order of extraction, step 6 of milestone 1017 (the contract crates leave
in this order), after `domain_name_system` has left in step 5. The rule that
a release review rechecks names ratified before a later naming rule applies here, as it did to
`DATA_MAXIMUM` in the `socket_contract` package.

Reuse: `script/crate-move --dry-run`, `helpers/lint_externalized_crates.py`, the template
`nifeos/crate-workflows` and its helpers (`standalone.py`, `spelled_out.py`, `register_names.py`,
`abbreviations.py`, `prose_acronyms.py`), cargo-mutants' recorded census, Kani and cargo-fuzz for
the proofs this block asks for. No new tool.

## Finding status

Items 1 to 3 are under [P0](#p0-blocks-the-move), 4 to 11 in [the before-the-tag
appendix](1086-domain-name-resolution-contract-outside-review/before-the-tag.md), and 12 to 14 in
[the later items](1086-domain-name-resolution-contract-outside-review/later-items.md). Read against
nife `main`; the crate has not yet left.

- Item 1 (P0), the crate describes its consumers in 54 lines: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641) and
  [#3838](https://github.com/nifeos/nife/pull/3838). `helpers/lint_externalized_crates.py` counts
  one hit, the ratified crate-name `Name:` block, which the move strips with the others.
- Item 2 (P0), `fixture` is nife's boot-test data: Done,
  [#3838](https://github.com/nifeos/nife/pull/3838). The cases live in
  `crates/domain_name_resolution_fixture`, which stays in nife and is never published.
- Item 3 (P0), the manifest does not stand alone: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641),
  [#3838](https://github.com/nifeos/nife/pull/3838) and
  [#3818](https://github.com/nifeos/nife/pull/3818). `Cargo.toml` carries every field the shape
  asks for and explicit lints, the crate carries a README and the two license files, and
  `domain_name_system` is a `git` dependency pinned at its `v0.1.0-pre.1` tag.
- Item 4 (P1), the limits and offsets break the current rules: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641),
  [nifeos/crate-workflows#76](https://github.com/nifeos/crate-workflows/pull/76) and
  [#3865](https://github.com/nifeos/nife/pull/3865). The template's `spelled_out`,
  `register_names`, `unit_symbols` and `question_shape` judge 319 names in the crate and refuse
  none.
- Item 5 (P1), the interface identity: Done, [#3583](https://github.com/nifeos/nife/pull/3583).
- Item 6 (P1), the reply word to a control `CALL`: Done,
  [#3583](https://github.com/nifeos/nife/pull/3583).
- Item 7 (P1), one field with three bounds: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641) and
  [#3838](https://github.com/nifeos/nife/pull/3838). The resolver, the grant table and the std
  client all read `MAXIMUM_NAME_FIELD_BYTES`, and two layout assertions bound the page.
- Item 8 (P1), `Outcome::from_word` decodes words the resolver cannot send: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641).
- Item 9 (P1), the documentation does not stand alone: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641) and
  [#3838](https://github.com/nifeos/nife/pull/3838). The template's `prose_acronyms` finds every
  acronym expanded.
- Item 10 (P1), the grant table has no proof: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641) and
  [#3838](https://github.com/nifeos/nife/pull/3838). Two Kani harnesses with falsification
  patches, and the fuzz target `domain_name_resolution_grants_control` in the crate's own `fuzz/`
  workspace.
- Item 11 (P1), the six surviving mutants: At the move. `script/crate-move` carries the census row
  into the new repository's `.github/mutants-exempts`, where the six equivalent mutants are listed
  with the reason, and deletes the row from `.cargo/mutants-baseline.txt`.
- Item 12 (P2), `wire` carries the spawner's half into the standard library: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641).
- Item 13 (P2), the root zone has three spellings: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641).
- Item 14 (P2), small shapes: Done, [#3641](https://github.com/nifeos/nife/pull/3641) and
  [#3785](https://github.com/nifeos/nife/pull/3785), which built decision 4.
- Decision 1, the reopened names: Done, ruled as built,
  [#3641](https://github.com/nifeos/nife/pull/3641) and
  [#3785](https://github.com/nifeos/nife/pull/3785). The `Name:` blocks that still say provisional
  go at the move with the others.
- Decision 2, the control reply: Done, [#3583](https://github.com/nifeos/nife/pull/3583).
- Decision 3, the identity: Done, [#3583](https://github.com/nifeos/nife/pull/3583).
- Decision 4, the reply to an unknown transport: Done,
  [#3785](https://github.com/nifeos/nife/pull/3785).
- Decision 5, the attach operation's name: Done, [#3785](https://github.com/nifeos/nife/pull/3785).
- Decision 6, the review and the move: Done, [#3798](https://github.com/nifeos/nife/pull/3798),
  which wrote [the review package](../../notes/crate-reviews/domain-name-resolution-contract.md).
- Decision 7, the fixture's names and the repository go: Done,
  [#3838](https://github.com/nifeos/nife/pull/3838).
- F-attach, the attach operation's name: Done, decision 5,
  [#3785](https://github.com/nifeos/nife/pull/3785).
- F-log, a runtime log channel for the spawner: Done,
  [#3583](https://github.com/nifeos/nife/pull/3583), as an entry in the BUGS list of
  `crates/system_initializer`.
- F-package, the review package: Done, [#3798](https://github.com/nifeos/nife/pull/3798).
- F-clearance, review clearance: Done, [#3838](https://github.com/nifeos/nife/pull/3838). The
  architect cleared the review package and approved `nifeos/domain_name_resolution_contract`.

## Open decisions

None.

## Goal

A stranger who opens `nifeos/domain_name_resolution_contract` can tell in one screen what the three
parties on the resolver's endpoint are (the spawner, a client, the resolver). The same stranger can
see which words each sends, where the name and the addresses sit in the page, and what every reply
word means. The crate names nothing that is not in its own repository. Its one stateful parser, the
grant table, provably never panics on a message and never puts a half-written zone in force.
Its names pass the template's gates at a ceiling of zero.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1086-domain-name-resolution-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1086-domain-name-resolution-contract-outside-review/prior-art.md).

## The items

Effort: S is half a day for a lane, M a day or two, L longer. A door is one-way when somebody
outside this repository acts on it after the first tag (a public name, a wire word, a layout, a
dependency). Everything else is two-way and the maintainer decides it. Every name proposed below is
provisional; the architect names.

### P0: blocks the move

1. The crate describes its consumers, in 54 lines. Finding: the externalization lint counts 54 lines
   that name nife, a nife program or a nife path, and the dry run's `mentions nife` and `citations`
   checks fail on the same lines. They fall into four classes: the fixture's zone `nife.test` and
   every test name under it (`src/fixture.rs` lines 6, 45 to 74; `src/tests.rs`, 34 lines; the doc
   example in `src/grants.rs` lines 87 to 97). Paths into this tree (`src/lib.rs` line 5 to
   `notes/name-resolution/the-contract.md`, line 62 to `system_tests/`, `src/endowment.rs` line 32
   to `components/`, `src/wire.rs` line 2 to the std farm's `sys/pal/nife/resolveproto.rs`). Program
   names (`domain_name_resolver` at `src/lib.rs` line 23, `net_stack` at `src/endowment.rs` line
   30). And citations of milestones and sections (`Cargo.toml` lines 8 to 14, `src/wire.rs` lines
   31, 88, `src/grants.rs` line 2 and others: 9 milestone lines and 4 section lines by the dry run's
   count). Why it matters: the runbook's rule is that a crate that has left never mentions nife, and
   the template's `standalone` check refuses the citations. Change: a documentation pass in nife
   before the move. Example zones become reserved names (`example.org`, `example.test`). Each path
   becomes a sentence about the role ("the program that spawns the resolver", "a standard library
   that speaks this contract"). Each milestone or section citation becomes the reason it stood for,
   in one clause, or goes. The fixture's lines go with item 2. The `Name:` blocks go by themselves
   at the move. Door: two-way. Effort: M.

2. `fixture` is nife's boot-test data inside a crate that is to stand alone. Finding:
   `src/fixture.rs` holds the zone the boot test grants (`nife.test`) and the address of the test
   runners' name-server peer (`10.0.2.9`, which is a QEMU user-network address, line 9). It also
   holds four report codes for the test client's last message (lines 13 to 22) and the ten cases the
   client asks in order (lines 44 to 75). Its three users are all in nife:
   `fixtures/src/name_resolver_test_client.rs`, `system_tests/src/user/name_resolver_tests.rs` and
   `std_exerciser/src/bin/std_resolve.rs`. The module is where 14 of the 54 nife mentions sit, and
   the template's register check refuses `ECHO_NO_ADDRESS` (an address that is neither virtual nor
   physical). Nothing in the protocol depends on it: the crate's own tests import nothing from it.
   Why it matters: a contract crate published for others to implement against ships a list of host
   names that only one test harness answers, and an address only one virtual network has. Change:
   move the module to a crate that stays in nife. The test client, the kernel test and the std
   exerciser must all reach that crate (the `fixtures` package, or a small `name_resolver_fixture`
   crate beside it, the way `filesystem_fixture` sits beside `filesystem_contract`). The
   `package_index` doc comment that cites the module (`crates/package_index/src/lib.rs` line 795)
   follows it. Door: two-way. The `socket_contract` review package puts the same question to the
   architect as its decision 2 (where `socket_contract::fixture` lives). One ruling should settle
   both, and this block does not ask it twice. Effort: S.

3. The manifest does not stand alone.
   Finding: `Cargo.toml` inherits `license.workspace` (line 6) and `lints.workspace` (lines 16 to
   17) from a workspace that will not be there. It has no `rust-version`, `description`, `readme`,
   `keywords` or `categories`. Its `publish = false` comment (line 5) points at
   `crates/publish-allowlist.txt`. Its one dependency is a `path` into this tree (line 20). The
   crate has no README, and the template's `standalone` check lists ten lines, the first `no
   LICENSE-MIT`. The dry run stages the scaffold's README shape and says it needs writing. Why it
   matters: the template refuses the tree as staged, before any workflow runs. Change: the shape
   `crates/block_roster_contract/Cargo.toml` already has: `rust-version`, `description`, `license =
   "MIT OR Apache-2.0"`, `readme`, `keywords`, `categories`, and an explicit `[lints.rust]` and
   `[lints.clippy]` block carrying `unexpected_cfgs` for `cfg(kani)`, `unsafe_op_in_unsafe_fn`,
   `missing_docs` and `undocumented_unsafe_blocks`. The dependency on `domain_name_system` becomes a
   `git` dependency with `rev` and `version` once that crate has left (step 5 of milestone 1017
   comes before this crate's step 6; nothing here reopens the order). A README with `EXAMPLES` and
   `BUGS` (item 9). The license files are the template's two, copied by the move lane (unverified
   whether the runbook names that step; the dry run does not stage them). Door: two-way, except the
   license, which the organization has already ruled and this block does not reopen. Effort: S.

### P2: later

The P2 items are in [the appendix](1086-domain-name-resolution-contract-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1086-domain-name-resolution-contract-outside-review/before-the-tag.md).

## Decisions for the architect

The doors that someone outside this repository acts on after the first tag. Every decision here is a rule
in force. The architect ruled 2 and 3 on 2026-11-14 (UTC), 1 and 4 to 6 on 2026-11-19 (UTC), and 7
on 2026-11-20 (UTC).

1. The names (item 4). Ruled 2026-11-19 (UTC), option A. The 30 provisional names stand as built. A
   limit opens `MAXIMUM_` and ends with its unit, an address that is not a memory location says
   which kind, and an offset follows the family-wide `<FIELD>_OFFSET_BYTES`.

   | was | built |
   |---|---|
   | `NAME_TEXT_MAXIMUM`, `GRANTS_MAXIMUM` | `MAXIMUM_NAME_TEXT_BYTES`, `MAXIMUM_GRANTS` |
   | `MAXIMUM_ADDRESSES` | `MAXIMUM_IPV4_ADDRESSES`, as in `domain_name_system` |
   | `OFF_NAME`, `OFF_ADDRESSES` | `NAME_OFFSET_BYTES`, `IPV4_ADDRESSES_OFFSET_BYTES` |
   | `w0`, `w1`; `text_len`, `text_total`; `op` | `first_word`, `second_word`; `text_bytes_received`, `text_bytes_total`; `operation` |

   `TRANSPORT_UDP`, `TRANSPORT_TCP` and the rest of the public names keep their spelling.
   `first_word` and `second_word` are ratified for this crate only. Other contracts that still
   write `w0` and `w1` follow when their own names come up. Prior art: smoltcp spells its limits
   `MAX_NAME_SIZE` (recalled, unverified); POSIX spells `NI_MAXHOST`. nife spells the word and the
   unit out. Example: `MAXIMUM_NAME_TEXT_BYTES = 253` beside
   `domain_name_system::MAXIMUM_NAME_BYTES = 255` shows that one holds dotted text and the other
   wire bytes.

2. The reply word to a control `CALL` (item 6). Ruled 2026-11-14 (UTC), option A. The reply uses
   `Outcome`'s packing, a status byte and a detail byte, exactly as a resolve reply does.
   `status::OK` (0) means the resolver accepted the message. `status::BAD_REQUEST` (10) means it
   refused the message, and the detail byte carries the `Refusal` by its number in
   `wire::refusal`: `UnknownOperation` 1, `BadgeZero` 2, `OutOfOrder` 3, `TooLong` 4, `BadZone` 5,
   `Full` 6, `NoSuchGrant` 7. The numbers stay fixed, and `DENIED` (1) does not answer a control
   message. The resolver sends the packed word (`Control::reply`). The spawner `CALL`s the last
   piece of each grant and reads the answer (`control_answer`); earlier pieces stay one-way `SEND`s.
   A refusal means no grant: the spawner forgets the entry and the spawn fails.

3. The interface identity (item 5). Ruled 2026-11-14 (UTC), option A. The ruling freezes the identity at
   `INTERFACE_ID` 11 and `VERSION` 1. Version 1 includes the control reply of decision 2, with no
   bump, because the earlier bare reply was never part of the contract.

4. The reply to an unknown transport (item 14). Ruled 2026-11-19 (UTC), option A. The spawner
   hands the resolver `TRANSPORT_UDP` (0) or `TRANSPORT_TCP` (1). The resolver refuses any other
   word at start and answers every resolve with `status::NETWORK` and
   `network_step::UNKNOWN_TRANSPORT` (5), as it does for a missing socket page. The step number is
   an addition inside interface version 1, which stays. The restrictive option follows the
   precedent a missing socket page set. Door: one-way at the tag, since the step number is a wire
   word. Prior art: glibc's resolver reads `options use-vc` as a flag, so no third value can exist
   (recalled, unverified); systemd-resolved refuses an unknown D-Bus enum value with an error. nife
   differs by carrying the transport as a number a spawner writes. Example: a resolve against a
   resolver started with transport 2 returns `Outcome { status: NETWORK, detail:
   UNKNOWN_TRANSPORT }`, and the standard library reports which step failed.

5. The attach operation's name. Ruled 2026-11-19 (UTC), option A. The operation that hands the
   resolver its page is `OPERATION_ATTACH_SHARED_PAGE` here and in the socket contract, with its
   number unchanged. The two contracts must agree on one act's name, and in the socket contract
   `frame` also means a network frame (`polled_wait`), though `page frame` stays the kernel's own
   term. Door: one-way at the tag. Prior art: Linux names `mmap` once for every file type.
   Example: both crates read `OPERATION_ATTACH_SHARED_PAGE`.

6. The review and the move. Ruled 2026-11-19 (UTC), option A. A lane writes the review package
   once the rulings pull request merges, with the names as ruled, `UNKNOWN_TRANSPORT` and
   `OPERATION_ATTACH_SHARED_PAGE` included. The move does not wait for the socket contract. The
   repository go is asked when the package exists, and the move runs at step 6 of milestone 1017
   (the contract crates leave in this order), after `domain_name_system` leaves in step 5. Door:
   two-way until the repository exists. Prior art: `file_allocation_table` and `e1000e` cleared
   from a written package. Example: the move runs `script/crate-move` on a tree whose `standalone`
   check reports nothing.

7. The fixture's names and the repository go. Ruled 2026-11-20 (UTC). The crate that holds the
   boot-test fixture keeps the ratified name `domain_name_resolution_fixture`, and the code for a
   client whose first name did not resolve is `ECHO_UNRESOLVED`, with no alias. The architect
   cleared the review package: the go creates `nifeos/domain_name_resolution_contract` with `publish
   = false`, no tag and no publish, and the move waits for the `domain_name_system` tag. Door:
   one-way once the repository exists. Prior art: `device_tree_blob` and `interfaces_note` cleared
   the same way. Example: the test client reports `ECHO_UNRESOLVED` (3) when `packages.nife.test`
   does not resolve.

## Done when

The checks are in [the appendix](1086-domain-name-resolution-contract-outside-review/done-when.md).

## BUGS

- The dry run ran without its build half and against the template at `v7` from a local checkout.
  The template's head carries checks `v7` lacks (passive voice, history phrases, function length,
  spelled-out words, unit symbols), which this block ran by hand. A later template release may add
  more.
- A spawner whose grant the resolver refuses fails the spawn without saying why, since nothing in the spawner
  writes a line once the shell runs (`crates/system_initializer` BUGS).
- `first_word` and `second_word` (decision 1) are ratified for this crate only, so the tree still
  writes `w0` and `w1` in other contracts, which follow when their own names come up.

## Follow-on

- **Recorded.** Item 11 is at the move: the move lists the six equivalent mutants in the new
  repository's `.github/mutants-exempts`.

## Index row

An outside review of `domain_name_resolution_contract` before it leaves. Three items block the move
(54 mentions of nife, the boot-test fixture inside the crate, a manifest that does not stand alone).
Eight land before the first tag. They are the reopened names, the interface identity, an undefined
control reply word, three bounds for one field, an unchecked reply count, the README and a broken
doc table, a proof over the grant table, and the six equivalent mutants. Three more are for later.
The architect has ruled all seven decisions: the control reply word, the identity, the names, the
reply to an unknown transport, the attach operation's name, the review package and move, and the
fixture's names with the repository go.
