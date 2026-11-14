---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 223, 563, 697, 857, 1017
decision_dependencies: 151, 197
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1107. `sealed_pair`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/sealed_pair` as a stranger to this tree would meet it. It
changes nothing; it lists what the next lanes change. The number is the recording lane's.

The review had no settled baseline to start from. `notes/crate-reviews.md` carries no package for
this crate. The only record is `notes/sealed-pair.md` plus the `Name:` block at the head of
`src/lib.rs`. That block marks the crate name provisional and records two refusals (`card_check`,
`seal`). Nothing below reopens those refusals. Every name in the crate is unratified, so every
naming item here is a proposal for the architect and not a rename a lane may perform.

Reuse: the template `nifeos/crate-workflows` and its helpers, `script/crate-review-packet`,
`script/names`, `helpers/name_checks.py`, the mutation census series. No new tool.

## Goal

A person who copies a kernel, an archive and a boot environment onto a card runs `script/card-check`
and gets the same answer the board will give. The crate says what the kernel and the progenitor
will each do, in their own words, and nothing it says is true only of one build shape. When it
leaves this tree, it leaves after the two crates it builds on, with a README that a stranger can
build and test from.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1107-sealed-pair-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1107-sealed-pair-outside-review/prior-art.md). The tree already recorded one weakness of a check that reads bytes: milestone 563 (A seal check that reads bytes cannot see a check that was dropped).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside this repository acts on it after a release (a public name, a published behavior,
a repository); everything else is two-way and the maintainer decides it.

### P0: blocks the move

1. The crate cannot leave before the two crates it builds on. LEFT: no change here; the order is Decision A's.
   Finding: both dependencies are path dependencies on sibling crates that have no repository of
   their own (`nifefs`, `measured_boot`). The ruling of milestone 1017 (the contract crates leave in this order) is that a crate leaves
   bottom-up and nothing that has left depends on a path or a pin into this tree. `measured_boot`
   is in the kernel's graph and `nifefs` is the kernel's archive format; neither is on journey 5's
   table yet. A repository for `sealed_pair` today would pin two unreleased crates by git revision
   into this tree, which the ruling forbids.
   Evidence: `crates/sealed_pair/Cargo.toml` lines 13 to 19; `notes/releasable-crates-2026-10-07/inventory.md`
   row `sealed_pair` (class C, graph `measured_boot, nifefs`); the second direction in
   `design/roadmap/1017-the-contract-crates-leave-in-this-order.md`.
   Change: none in this crate. Record the order: `nifefs` and `measured_boot` first, then this
   crate with or after `uefi_loader` (decision A).
   Door: one-way (a repository). Effort: none here.

### P1: before the move, and one before the next bench evening

2. The crate answers half of the kernel's question, and `card_check` says SEALED for a card that DONE (`stale_malformed_and_missing_loader_words_are_refused`; `card_check` reads the third file).
   halts.
   Finding: since the kernel began taking two digest words from the loader (`nife.progenitor`,
   `nife.base`), `kernel::trust::require` holds the progenitor to the loader's word before the trust
   root. `require_program_measurements` holds the table to the second word. A missing or disagreeing
   word halts at `MEASURED BOOT REFUSED` like a digest mismatch. On a card the words
   travel as a third file, `nife-initial-ramdisk.img.loader-digests`, and as `usr_bootargs` in the
   boot environment `script/board-image` writes. `inspect` reads two inputs and never the words;
   `card_check` reads only `nife-vf2.img` and `nife-initial-ramdisk.img`, and its fix text says to
   copy "the set" of two. A card whose kernel and archive are one build and whose words are
   another's reads as SEALED and halts after the power cycle, which is the walk to the board
   the crate exists to save. The stick has cover, because `uefi_loader/build.rs` checks the words
   against the archive before embedding them, and the QEMU paths write the words beside the
   archive they pack. The crate's `BUGS` names boot scripts and `extlinux.conf` and not this.
   Evidence: `kernel/src/trust.rs` lines 1 to 9, 39 to 42, 83 to 87 and `require_from_loader`;
   `script/board-image` lines 256 and 293; `crates/sealed_pair/src/lib.rs` lines 8 to 13 and 276
   to 310; `xtask/src/card_check.rs` lines 23 to 24 and 213 to 262.
   Change: a third question beside the two: `inspect_words(words: &str, archive: &[u8]) ->
   WordsSeal` (or an `Option<&str>` third argument to `inspect`, which the two existing callers pass
   as `None` for the stick and `Some` for a card). Parse the words with
   `measured_boot::loader_digests::parse`, so this crate refuses a word the kernel would refuse and
   gives the kernel's reason. Compare the result to the archive's `progenitor` and table digests. `Seal`
   gains the verdict, `is_sealed` includes it, `explain` says which word disagrees, and
   `card_check` reads the third file when it is there and says the set is three files. Add the
   gap to `BUGS` until the change lands.
   Door: two-way (an API addition before any release). Effort: S.

3. A table line the parser rejects makes the tool report the whole table unreadable, and the words say DONE (`a_bad_table_line_is_reported_and_does_not_condemn_the_console`, `a_table_that_is_not_utf8_is_still_unreadable`).
   the progenitor will read an empty table. It will not.
   Finding: `inspect_table` returns `TableSeal::Unreadable` on the first bad line, and `explain`
   prints "the progenitor measures against an empty table and refuses the console". The progenitor
   reads the table through `measured_boot::verdict`, which reaches `expected_in_manifest`, which
   flattens the parse and skips a bad line; only the program that line named gets refused, and the
   console boots when its own line is good. `TableSeal::Unreadable` is accurate for bytes that are
   not UTF-8 (`boot_steps.rs` makes those the empty table) and inaccurate for a bad line. The host
   check is stricter than the board, which is the safe direction, and the report sends a person to
   fix the wrong thing. Such a table reaches a card only by hand, because `kernel/build.rs` panics
   on the same line at build time.
   Evidence: `crates/sealed_pair/src/lib.rs` lines 110 to 112, 200 to 207 and 321 to 329;
   `crates/measured_boot/src/lib.rs` lines 426 to 432; `crates/system_initializer/src/boot_steps.rs`
   lines 183 to 188 and `measurement.rs` line 16.
   Change: keep `Unreadable` for the UTF-8 case. Add `TableDisagreement::Unparsable { line: String
   }` for each bad line. `progenitor_refuses` counts it (the program on that line is gone), and
   `explain` prints it with the line and the parser's reason, in the words `BadLine` already
   carries.
   One test with one bad line beside a good console line, asserting that the console is not named.
   Door: two-way. Effort: S.

4. The crate does not stand alone, in the ten ways the template's checker lists. LEFT: move preparation waits on Decision A.
   Finding: `standalone.py` reports no README, no `LICENSE-MIT`, no `LICENSE-APACHE`, a license
   field inherited from the workspace, and no `rust-version`. It also reports `publish = false` with
   a comment that points at `crates/publish-allowlist.txt`, `lints.workspace = true`, five lines
   that cite a path into this tree, one provenance block and six milestone citations. The `Cargo.toml` has no
   `description`, `repository`, `keywords` or `categories`. These are the move lane's ordinary
   items; they size the move.
   Evidence: `crates/sealed_pair/Cargo.toml` lines 1 to 10; `src/lib.rs` lines 6, 13, 15 to 16, 24
   to 27, 31, 238 to 239, 247 to 248, 312, 362 to 363.
   Change: at the move, add the scaffold's files, measure `rust-version` by a build, and inline the
   lints. Rewrite the citations as the crate's own words (the kernel's behavior stated, not a
   milestone named). Strip the `Name:` block as the move rule says. A README with `## EXAMPLES`
   (the two doctests, which already exist) and `## BUGS` (the four entries plus item 2).
   Door: two-way until the tag. Effort: S.

5. `BOOT_PROGRAMS` restates a string `measured_boot` already owns. DONE (`the_boot_programs_are_the_measured_boot_names`).
   Finding: `BOOT_PROGRAMS` is `["progenitor"]`, a literal, while `measured_boot::PROGENITOR` is
   the constant the kernel (`PROGENITOR_ENTRY`) and the loader read. `notes/sealed-pair.md`
   argues for one list so a kernel cannot measure a name the checkers do not know; the list's one
   member is a second copy of that name. A rename of the entry would change the kernel and leave
   this crate reading a name no archive carries, and every archive would then be `NoProgenitor`.
   Evidence: `crates/sealed_pair/src/lib.rs` line 28 and line 278; `crates/measured_boot/src/lib.rs`
   line 381; `kernel/src/user/progenitor_boot.rs` line 16.
   Change: `pub const BOOT_PROGRAMS: [&str; 1] = [measured_boot::PROGENITOR];` and `inspect` reads
   `measured_boot::PROGENITOR` where it means the progenitor. No behavior changes today.
   Door: two-way. Effort: XS.

6. The notice constant is a sentence, and it names a board in a crate that also judges QEMU images. LEFT: a public name, Decision B.
   Finding: `THE_BOARD_IS_THE_AUTHORITY` is a `&str` holding two sentences. A constant's name says
   what it holds. This one is the opening of its own text. It has a leading `THE_` the naming page
   refuses for assertion constants, and it says "board" where the crate's documentation says
   "kernel" (its own doc comment on the same item says the kernel is the authority). `card_check` prints it after
   every refusal.
   Evidence: `crates/sealed_pair/src/lib.rs` lines 238 to 242; `xtask/src/card_check.rs` line 79.
   Change: a name for what it is, `AUTHORITY_NOTICE`, with the text saying "the kernel" where it
   says "the board". The architect names it (decision B).
   Door: two-way until a release; a published name after. Effort: XS.

7. `EntrySeal` can say "refused" with no digest, and the report then prints an empty hash. DONE (`Vouch` carries the digest; the impossible pair no longer compiles).
   Finding: `digest: Option<Digest>` and `vouch: Vouch` are independent public fields. `inspect`
   never builds `vouch: Vouch::No` with `digest: None`, but the type allows it, a test builds it,
   and `explain` prints "hashes to , which is not in the kernel image's trust root" for it.
   Evidence: `crates/sealed_pair/src/lib.rs` lines 80 to 87, 182 to 189 and 484 to 489.
   Change: carry the digest in the variants that have one: `Vouch::Yes(Digest)`, `Vouch::No(Digest)`,
   `Vouch::Absent`, and drop the `digest` field. The test that builds the impossible pair then
   fails to compile, which is the point.
   Door: two-way. Effort: XS.

8. The refusal text asserts a cause. DONE (`a_kernel_from_another_build_is_refused_and_the_refusal_names_the_entry` reads the new text).
   Finding: `explain` says "They are from different builds" for every `Vouch::No`. One flipped
   byte on a card produces the same verdict, and the crate's own test
   `one_stale_half_is_named_on_its_own` builds that case. Milestone 563 (A seal check that reads bytes cannot see a check that was dropped) recorded the same sentence
   as false for a diverted build, and its `BUGS` entry stands.
   Evidence: `crates/sealed_pair/src/lib.rs` lines 177 to 181;
   `design/roadmap/0563-a-seal-check-that-reads-bytes-cannot-see-a-check-that-was-dropped.md`,
   "Why the tool then says the wrong thing".
   Change: say what the tool knows: "the kernel image does not carry the digest of this archive's
   `progenitor`", then the two usual causes (a kernel and an archive from different builds; a
   copy that changed one of them) as causes, and the halt as the consequence.
   Door: two-way. Effort: XS.

9. One British spelling in a public parameter. LEFT: a public name, Decision B.
   Finding: the one parameter of `test_support::pair`, a `&str`, is spelled the British way. The naming page spells every name the American
   way and the template's `british_spelling.py` refuses it at a ceiling of zero, so the template's
   `check` job would be red on the first push. The parameter is public because the module is.
   Evidence: `crates/sealed_pair/src/lib.rs` lines 406 and 410.
   Change: `flavor`, or better `build: &str`, which is what the argument names (one build's pair
   against another's). Provisional until the architect rules.
   Door: two-way until a release. Effort: XS.

10. `Unreadable` is not a `std::error::Error`, and its `Display` prints a `Debug` form. DONE (`an_unreadable_error_is_a_std_error_in_words`).
    Finding: the crate is a `std` crate (`String`, `Vec`, `std::fmt::Write`) whose one error type
    implements `Display` and not `std::error::Error`, so a caller cannot put it in a `Box<dyn
    Error>` or behind `?` in a `main` that returns one. `NotAnArchive` writes `{e:?}` because
    `nifefs::Error` has no `Display`, so the text reads "not a nifefs archive (Truncated)".
    Evidence: `crates/sealed_pair/src/lib.rs` lines 45 to 63.
    Change: `impl std::error::Error for Unreadable {}`; a `match` over `nifefs::Error`'s six
    variants with one phrase each ("the image is shorter than the directory", and so on), or a
    `Display` on `nifefs::Error` itself, which is that crate's call.
    Door: two-way. Effort: XS.

### P2: later

The P2 items are in [the appendix](1107-sealed-pair-outside-review/later-items.md).

## Decisions for the architect

Only the one-way doors. Each with the background first and a recommendation.

### A. Whether `sealed_pair` leaves at all, and in whose company

Background. The crate is class C in the inventory: bound to this tree's formats. Its two
dependencies are the kernel's archive format and the kernel's measured-boot primitive, neither of
which has a repository. Its two readers are `xtask` (the card checker) and `uefi_loader`'s build
script. Its vocabulary is this tree's: the progenitor, the console, `MEASURED BOOT REFUSED`. The
standing rule is that a crate gets a repository of its own when it is publishable or used by more
than one repository, and otherwise stays with the package that uses it. Whether more
than one repository uses it depends on where `uefi_loader` and `xtask` end up, which is not yet ruled.

Recommendation. Do not create a repository for `sealed_pair` now. Record that it leaves after
`nifefs` and `measured_boot`, with or after `uefi_loader`, and only if `uefi_loader` and the card
checker land in different repositories. If they land together, it moves into that repository as a
module and never publishes. Nothing in the crate needs a repository to get fixed; items 2 to 10
land here.

Prior art. systemd keeps `systemd-measure` inside the systemd repository beside `ukify`, the tool
that builds what it measures; Android keeps `avbtool` in the `avb` repository with the library it
verifies against. Neither publishes its checker on its own. A concrete example: a card checker
published to crates.io with `nifefs = "0.1"` and `measured_boot = "0.1"` in its manifest would
force both of those crates to publish first, and `measured_boot` is in the kernel's graph.

### B. Two public names, if and when it leaves

Background. Names are the architect's. Two of this crate's public names fail a reading a stranger
would give them, and a release makes them permanent. One is `THE_BOARD_IS_THE_AUTHORITY` (item 6: a
sentence, a `THE_` prefix, "board" for "kernel"). The other is the parameter of `test_support::pair` (item 9: a
British spelling the template refuses). Every other name reads as what it is: `inspect`, `Seal`,
`EntrySeal`, `TableSeal`, `TableDisagreement`, `Vouch`, `Unreadable`, `embedded_archive`,
`is_sealed`, `kernel_refuses`, `progenitor_refuses`, `hex_string`, `test_support`.

Recommendation. `AUTHORITY_NOTICE` for the constant, and `build` for the parameter (`pair(build:
&str)`, since it names one build against another). Rule them together with the crate name when the
move gets its ruling; until then they stay provisional and no lane renames them.

Prior art. The standard library names a constant for what it holds (`EPSILON`, `MAX`), never for
its opening words; this tree's naming page refuses `THE_` on an assertion constant for the same
reason. A concrete example: `card_check` prints the constant after every refusal, so a reader meets the name at one call site and the text at every bench.

## Done when

The checks are in [the appendix](1107-sealed-pair-outside-review/done-when.md).

## BUGS

- Item 2 came from reading the kernel and `script/board-image`, not by booting a card with
  stale words. The change is cheap enough to land without a bench run, and a bench run would
  confirm the halt text.
- Nobody has measured item 13's cost. A `windows(32)` scan over a 10 MiB image is on the order of a few
  hundred million byte compares, which is tens of milliseconds; the item is for the record, not a
  defect.
- The review did not run `script/crate-standalone` or the template's `check` workflow; the packet
  and `standalone.py` list what they would flag, and item 1 says the move cannot happen yet.
- No review package exists for this crate, so the baseline this review respected is the `Name:`
  block's two refusals and nothing else. A package would normally come first; the recording lane
  may want to say whether it owes one.

## Follow-on

What is left of this review: Decisions A and B are not ruled, and the items marked LEFT in the
appendices wait on them or on a lane of their own.

- **Milestone 1017.** The contract crates leave in its order; items 1, 4 and 15 wait on Decision A.
- **Recorded.** Items 11 and 12 are in `crates/sealed_pair/src/lib.rs`'s `BUGS`.
- **Outstanding.** Items 6 and 9 (names, Decision B), 13 (unmeasured) and 14 (a harness): checked
  against the tree, none exists yet.

## Index row

An outside review of `sealed_pair` before any move: one blocker (its two dependencies have no
repository), nine items before the move, six for later. The nine cover these: the loader's two
digest words, which the kernel now also requires and the crate never reads. A bad table line
misreported as an empty table. A duplicated `progenitor` literal. The standalone checker's ten
findings. A notice constant named by its opening words. A refusal type that can carry no digest. A
refusal text that asserts a cause. One British spelling. An error type without `std::error::Error`.
