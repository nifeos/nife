---
status: NOT-STARTED
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

There is no review package for this crate in `notes/crate-reviews/`, so this block has no settled
baseline to defer to. It takes as settled only what the tree itself records.
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

Only the doors that someone outside this repository acts on after the first tag. Each is one
decision; the recommendation comes first.

1. The public names that the current rules reopen (item 4). Recommendation: rename all of them in
   one lane, before the tag, as below. And keep `status`, `network_step`, `Outcome`, `Denied`,
   `Control`, `Refusal`, `Grants`, `grant_messages`, `forget_message`, `reject_code`,
   `is_canonical_zone` and the `endowment` functions as they are. Background: three of the constants
   carried a ratification before the rule that a limit opens `MAXIMUM_` and ends with its unit. The template
   refuses `OFF_` and a bare `ADDRESS` at a ceiling of zero. The `socket_contract` package proposes
   `_OFFSET` for the same shape and names this crate's two offsets as names that would follow.

   | item now | proposed | rule |
   |---|---|---|
   | `NAME_TEXT_MAXIMUM` | `MAXIMUM_NAME_TEXT_BYTES` | a limit opens `MAXIMUM_` and ends with its unit |
   | `GRANTS_MAXIMUM` | `MAXIMUM_GRANTS` | a count names the things, limit word first |
   | `MAXIMUM_ADDRESSES` | `MAXIMUM_IP_ADDRESSES` | an address that is not a memory location says so; `domain_name_system::MAXIMUM_ADDRESSES` follows |
   | `OFF_NAME`, `OFF_ADDRESSES` | `NAME_OFFSET`, `IP_ADDRESSES_OFFSET` | `off` abbreviates; `_OFFSET` as the socket package proposes |
   | `OPERATION_ATTACH_PAGE_FRAME` | follow the socket contract's ruling (`OPERATION_ATTACH_SHARED_PAGE` is proposed there) | names that follow a renamed thing move with it |
   | `TRANSPORT_UDP`, `TRANSPORT_TCP` | keep; add `udp` and `tcp` to the template's accepted words | they pass the acronym test, as the socket package argues |
   | `w0`, `w1` (parameters) | `first_word`, `second_word` | an abbreviation; the rule covers parameters. This is the tree's convention for message words, so the ruling applies beyond this crate |
   | `text_len`, `text_total` (fields) | `text_bytes_received`, `text_bytes_total` | a length carries its unit |
   | `op` (two local bindings) | `operation` | `op` is on the spelled-out list |

   Prior art: smoltcp spells the same limits `MAX_NAME_SIZE` and an address-count limit (from
   memory, unverified); POSIX spells its host-name limit `NI_MAXHOST`. nife's rule differs from both
   in spelling the word and the unit out. Concrete example: a reader who sees
   `MAXIMUM_NAME_TEXT_BYTES = 253` beside `domain_name_system::MAXIMUM_NAME_BYTES = 255` can tell at
   once that one holds dotted text and the other wire bytes. `NAME_TEXT_MAXIMUM` beside
   `MAXIMUM_NAME_BYTES` reads as two spellings of one thing.

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

## Done when

The checks are in [the appendix](1086-domain-name-resolution-contract-outside-review/done-when.md).

## BUGS

- This block has no review package to defer to. If a package follows it, that package's rulings win where they differ, and this block then changes to say so.
- The dry run ran without its build half and against the template at `v7` from a local checkout.
  The template's head carries checks `v7` lacks (passive voice, history phrases, function length,
  spelled-out words, unit symbols), which this block ran by hand. A later template release may add
  more.
- The size of `Grants` is from reading. A size test (item 14) would make it measured.
- A spawner whose grant the resolver refuses fails the spawn without saying why, since nothing in the spawner
  writes a line once the shell runs (`crates/system_initializer` BUGS).
- Decision 1 asks for `first_word` and `second_word` where the tree writes `w0` and `w1` in many
  contracts. Ruling it for this crate alone leaves the tree inconsistent. Ruling it for the tree is
  a rename beyond this block, and the recording lane should say which the architect chose.
- Item 2 waits on the same question as the `socket_contract` package's decision 2. Whichever gets its ruling first settles the shape for both.

## Index row

An outside review of `domain_name_resolution_contract` before it leaves. Three items block the move
(54 mentions of nife, the boot-test fixture inside the crate, a manifest that does not stand alone).
Eight land before the first tag. They are the reopened names, the interface identity, an undefined
control reply word, three bounds for one field, an unchecked reply count, the README and a broken
doc table, a proof over the grant table, and the six equivalent mutants. Three more are for later.
Three decisions go to the architect: the names, the control reply word (ruled) and the identity
(ruled).
