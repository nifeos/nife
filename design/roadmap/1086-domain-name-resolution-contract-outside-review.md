---
status: PARTIAL
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

## Finding status

Items 1 to 3 are under [P0](#p0-blocks-the-move), 4 to 11 in [the before-the-tag
appendix](1086-domain-name-resolution-contract-outside-review/before-the-tag.md), and 12 to 14 in
[the later items](1086-domain-name-resolution-contract-outside-review/later-items.md).

- 1, the crate describes its consumers in 54 lines: Open.
  [#3641](https://github.com/nifeos/nife/pull/3641) cleared all but 14, which sit in
  `src/fixture.rs` and leave with item 2; the externalization waiver's ceiling of 54 then falls to
  0.
- 2, `fixture` is nife's boot-test data: Open. The module is still in the crate, and
  `register_names` refuses its `ECHO_NO_ADDRESS`. The door is two-way, and the maintainer rules the
  fixture's home provisionally, with the `socket_contract` package's decision 2, which asks the same
  question and has no ruling.
- 3, the manifest does not stand alone: Open. [#3641](https://github.com/nifeos/nife/pull/3641)
  wrote the manifest. The pinned `git` dependency on `domain_name_system` and the two license files
  wait for the move.
- 4, the limits and offsets break the current rules: Open.
  [#3641](https://github.com/nifeos/nife/pull/3641) built the renames, which wait on
  [ratification](#ratify-the-provisional-names). `udp` and `tcp` still need adding to the template's
  accepted words; they stay abbreviated, as ruled 2026-11-16 (UTC).
- 5, the interface identity: Done, [#3583](https://github.com/nifeos/nife/pull/3583).
- 6, the reply word to a control `CALL`: Done, [#3583](https://github.com/nifeos/nife/pull/3583).
- 7, one field with three bounds: Done, [#3641](https://github.com/nifeos/nife/pull/3641). The
  layout assertion at `src/wire.rs` line 44 writes the page size as a literal 4096, a two-way nit
  for the next lane in the crate.
- 8, `Outcome::from_word` decodes words the resolver cannot send: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641).
- 9, the documentation does not stand alone: Open. [#3641](https://github.com/nifeos/nife/pull/3641)
  wrote the README and fixed the table. `prose_acronyms` still flags RFC, TTL and NXDOMAIN
  (`src/lib.rs`), GB (`src/verification.rs`) and OK (`src/wire.rs`), for expansion or the accepted
  words.
- 10, the grant table has no proof: Open. [#3641](https://github.com/nifeos/nife/pull/3641) added
  the two harnesses and their falsification patches. A fuzz target over `Grants::control` lands in
  the crate's own repository.
- 11, the six surviving mutants: Open. The move lists them in `.github/mutants-exempts` after a
  census.
- 12, `wire` carries the spawner's half into the standard library: Done,
  [#3641](https://github.com/nifeos/nife/pull/3641).
- 13, the root zone has three spellings: Done, [#3641](https://github.com/nifeos/nife/pull/3641).
- 14, small shapes: Open. [#3641](https://github.com/nifeos/nife/pull/3641) added `Debug`, the size
  test and the dead branch's removal. [The reply to an unknown
  transport](#the-reply-to-an-unknown-transport) waits for a ruling.
- Decision 1, the reopened names: Open, under [ratification](#ratify-the-provisional-names).
- Decision 2, the control reply: Done, [#3583](https://github.com/nifeos/nife/pull/3583).
- Decision 3, the identity: Done, [#3583](https://github.com/nifeos/nife/pull/3583).
- F-attach, `OPERATION_ATTACH_PAGE_FRAME`: Open, under [Open
  decisions](#name-the-attach-operation-with-the-socket-contract).
- F-log, a runtime log channel for the spawner: Open. The spawner reads why the resolver refused a
  grant, but cannot write a line once the shell runs. No lane holds it.
- F-package, the review package: Open. None exists under `notes/crate-reviews/`.
- F-clearance, review clearance: Open, under [Open decisions](#clear-the-review-for-the-move).

## Open decisions

### The reply to an unknown transport

The spawner hands the resolver a transport word, `TRANSPORT_UDP` (0) or `TRANSPORT_TCP` (1). The
resolver tests only for TCP (`components/src/domain_name_resolver.rs` line 133), so any other value
reads as UDP. The contract defines no reply for a word it does not know, and a stranger's spawner
that writes 2 for a transport of its own gets UDP without a word.

Recommendation: A. It is the restrictive option, and it follows the precedent the resolver set for
a missing socket page: refuse at start, then answer each resolve with `status::NETWORK` and a step
that names why.

- A. Refuse the transport at start; answer every resolve with `status::NETWORK` and a new
  `network_step::UNKNOWN_TRANSPORT` (5).
- B. Keep reading every value but 1 as UDP, and document it.
- C. Have the spawner refuse the value before it spawns, with no word on the wire.

Door: one-way at the tag, since the step number is a wire word. Prior art: glibc's resolver reads
`options use-vc` as a flag, so no third value can exist (recalled, unverified); systemd-resolved
refuses an unknown D-Bus enum value with an error. nife differs by carrying the transport as a
number a spawner writes. Example: under A, a resolve against a resolver started with transport 2
returns `Outcome { status: NETWORK, detail: UNKNOWN_TRANSPORT }`, and the standard library reports
which step failed. Blocks: the tag.

### Ratify the provisional names

[#3641](https://github.com/nifeos/nife/pull/3641) built decision 1, which the maintainer decided
provisionally on 2026-11-14 (UTC), and the crate holds 30 `Name: provisional` blocks. A limit opens `MAXIMUM_` and ends with its unit, an
address that is not a memory location says which kind, and an offset follows the family-wide
`<FIELD>_OFFSET_BYTES`.

| was | built |
|---|---|
| `NAME_TEXT_MAXIMUM`, `GRANTS_MAXIMUM` | `MAXIMUM_NAME_TEXT_BYTES`, `MAXIMUM_GRANTS` |
| `MAXIMUM_ADDRESSES` | `MAXIMUM_IPV4_ADDRESSES`, as in `domain_name_system` |
| `OFF_NAME`, `OFF_ADDRESSES` | `NAME_OFFSET_BYTES`, `IPV4_ADDRESSES_OFFSET_BYTES` |
| `w0`, `w1`; `text_len`, `text_total`; `op` | `first_word`, `second_word`; `text_bytes_received`, `text_bytes_total`; `operation` |

`TRANSPORT_UDP` and `TRANSPORT_TCP` keep their spelling, and the rest of the public names keep
theirs. Recommendation: A.

- A. Ratify the names as built.
- B. Ratify all but `first_word` and `second_word`, and rule the message-word spelling for the
  whole tree first, since other contracts still write `w0` and `w1`.

Door: one-way at the tag. Prior art: smoltcp spells its limits `MAX_NAME_SIZE` (recalled,
unverified); POSIX spells `NI_MAXHOST`. nife spells the word and the unit out. Example:
`MAXIMUM_NAME_TEXT_BYTES = 253` beside `domain_name_system::MAXIMUM_NAME_BYTES = 255` shows that one
holds dotted text and the other wire bytes. Blocks: the tag.

### Name the attach operation with the socket contract

`OPERATION_ATTACH_PAGE_FRAME` (`src/wire.rs` line 53) is the operation that hands the resolver its
page. It names the same act as the socket contract's attach, whose package proposes
`OPERATION_ATTACH_SHARED_PAGE`. Recommendation: A, because two contracts that spell one act two ways
teach a reader two acts.

- A. Rename it to whatever the socket contract's operation-name ruling settles, in the same lane.
- B. Ratify `OPERATION_ATTACH_PAGE_FRAME` here on its own.

Door: one-way at the tag. Prior art: Linux names `mmap` once for every file type. Example: under A,
both crates read `OPERATION_ATTACH_SHARED_PAGE`. Blocks: the tag, after the socket contract's
ruling.

### Clear the review for the move

No review package exists, and clearance follows one. Recommendation: A, write the package once
items 1 to 3 close and ratification settles the names, then clear the move at step 6 of milestone 1017
(the contract crates leave in this order), after `domain_name_system` leaves in step 5.

- A. Clear from a written package, as other crates did.
- B. Hold the move until the socket contract leaves, since the two share the fixture question.

Door: two-way until the repository exists. Prior art: `file_allocation_table` and `e1000e` cleared
from a written package. Example: under A, the move runs `script/crate-move` on a tree whose
`standalone` check reports nothing. Blocks: the move.

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

The doors that someone outside this repository acts on after the first tag. Decision 1, the names,
waits on [ratification](#ratify-the-provisional-names). Decisions 2 and 3 are the rules in force.

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
- A spawner whose grant the resolver refuses fails the spawn without saying why, since nothing in the spawner
  writes a line once the shell runs (`crates/system_initializer` BUGS).
- `first_word` and `second_word` (decision 1) apply in this crate only, so the tree still
  writes `w0` and `w1` in other contracts. Ruling the spelling for the tree is a rename beyond this
  block.
- Item 2 waits on the same question as the `socket_contract` package's decision 2. Whichever gets its ruling first settles the shape for both.

## Follow-on

- **Outstanding.** The open lines under [Finding status](#finding-status): items 1 to 4, 9 to 11
  and 14, F-log and F-package, and the four open decisions.

## Index row

An outside review of `domain_name_resolution_contract` before it leaves. Three items block the move
(54 mentions of nife, the boot-test fixture inside the crate, a manifest that does not stand alone).
Eight land before the first tag. They are the reopened names, the interface identity, an undefined
control reply word, three bounds for one field, an unchecked reply count, the README and a broken
doc table, a proof over the grant table, and the six equivalent mutants. Three more are for later.
The architect has ruled the control reply word and the identity. Four decisions go to the
architect: the names, the reply to an unknown transport, the attach operation's name and the review
clearance.
