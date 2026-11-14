---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 972, 1017
decision_dependencies: 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1078. `boot_ladder`: what an outside maintainer would fix before it leaves

*(The number is the recording lane's. The title and slug are provisional.)*

Raised by a read-only outside review of `crates/boot_ladder` as a stranger to this tree would meet
it. It changes nothing; it lists what the next lanes change before the crate moves to its own
repository and before any tag.

The crate has no review package in `notes/crate-reviews/`, so this review has no settled baseline
to inherit. The one ruling it does inherit is the crate name, `boot_ladder`, ratified in the
crate's own `Name:` block with four refusals, and it does not reopen it. Every other public name
in the crate is unratified (`script/names boot_ladder` and `helpers/name_release.py --draft`
list eight items with no `Name:` block and two marked provisional).

Reuse: the crate template `nifeos/crate-workflows` and its scaffold, `script/crate-standalone`,
`script/crate-review-packet`, `cargo kani`. No new tool.

## Goal

A stranger who finds `boot_ladder` in its own repository can tell in one screen what it holds. It holds the fixed heads of the lines a nife kernel and shell print while booting, and the list of self-test check names. A program reading a serial log, a CI transcript or a photograph of a screen then keys on the same bytes the kernel wrote. The same stranger can build and test it from the README
alone, and the repository's gates are green for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1078-boot-ladder-outside-review/what-was-checked.md).

## The crate in one paragraph

`src/lib.rs` is 142 lines: eight `&str` constants (`BANNER`, `MACHINE`, `SELF_TEST`,
`SELF_TEST_FAILED`, `SELF_TEST_ABSENT`, `TOUR`, `PROMPT`, `SCREEN_HELD`), one `&[&str]`
(`SELF_TEST_CHECKS`, 15 names) and one `&[usize]` (`SELF_TEST_CHECKS_EARLIER`, the two older list
lengths). It is `no_std`, has no dependencies, no `unsafe`, no functions, no tests. The kernel
prints the heads; `swish` prints `PROMPT`; `serial_console` and `xtask` match them and parse the
verdict's tail. Soundness has nothing to review. Correctness lives in the agreement between the
printer and the readers, and that is where the items below sit.

## The items

Effort: S is half a day for a lane, M a day or two, L more. A door is one-way when somebody
outside the repository acts on it after a release (a public name, a printed wording that logs and profiles already hold, a layout, a license). Everything else is two-way, and the maintainer decides it.

### P0: blocks the move

1. The manifest is not standalone.
   Finding: `crates/boot_ladder/Cargo.toml` inherits `license` (line 9) and `lints` (lines 20
   to 21) from the nife workspace, carries no `rust-version`, no `description`, no `repository`,
   no `readme`, no `keywords` and no `categories`. The `publish = false` line (line 8) points the
   reader at `crates/publish-allowlist.txt`, a file that will not be in the repository. The header
   comment (lines 1 to 3) describes a `Name:` block the template strips. The body comment (lines 11 to 18) says the crate is "six `&'static str`s" when it holds eight and two slices. It also cites a milestone and a rule of `AGENTS.md`.
   Why it matters: `check / standalone` fails on each of the first three. `cargo publish --dry-run` refuses a manifest without `description` and `license`. A stale count is the first thing a
   stranger reads and the first thing they stop trusting.
   Change: write `license = "MIT OR Apache-2.0"`, `rust-version = "1.88"` (the value every other
   package in `notes/crate-reviews.md` states), a one-sentence `description`, `repository`,
   `readme = "README.md"`, and the template's required `[lints]` table (`unsafe_op_in_unsafe_fn`,
   `missing_docs`, `unexpected_cfgs` naming `cfg(kani)`) as literals. Replace every comment with
   one sentence a stranger can act on. The `publish` line stays until the release that publishes,
   with the comment "set to `true` only in the release that publishes".
   Door: two-way, except the license, which is already ruled for every crate. Effort: S.

2. The files a stranger needs do not exist.
   Finding: the crate has no `README.md`, no `LICENSE-MIT`, no `LICENSE-APACHE`, no
   `CHANGELOG.md`, no `_typos.toml`, no `docs/stranger-test.md`.
   Why it matters: `check / standalone` requires the README with `EXAMPLES` and `BUGS` headings,
   the two license files and the typos configuration; `release / gate` refuses while
   `docs/stranger-test.md` reads `Last run: none`.
   Change: the scaffold's files, filled in. The README's one screen says what the crate is for in the words of item 3. It shows the verdict line and how a matcher keys on `SELF_TEST` and `SELF_TEST_FAILED`. It carries `EXAMPLES` (a reader that finds the verdict in a transcript) and `BUGS` (the crate's current `BUGS` list, reworded for a stranger).
   Door: two-way. Effort: S.

3. Shipped text points into this tree.
   Finding: `script/crate-standalone` counts 10 milestone citation lines (first `Cargo.toml` line 11). It counts 3 lines naming a path in nife (first `src/lib.rs` line 8, "See notes/boot-ladder.md"; also lines 79 and 141). It counts one line naming the architect outside a `Name:` block (`src/lib.rs` line 62 to 63, a ruling cited by a pull request number). Beyond what
   the script counts, the doc comments name `kernel_main`, `kernel/src/machine_description.rs`, `kernel/src/self_test.rs`, `components/src/swish.rs`, `kernel::console::yield_screen`,
   `machine_discovery::framebuffer::SCREEN_HOLD`, `cargo xtask swish-check` and `cargo xtask
   uefi-boot`, and `AGENTS.md` rule 7 (lines 5, 31, 42, 50, 109 to 111, 119, 127, 139 to 140).
   Lines 82 to 83 and 108 to 111 carry a dated history of the check list, which the documentation
   standard sends to git.
   Why it matters: the move cannot strip these as it strips `Name:` blocks; each is a link a
   stranger cannot follow, and the ruling citation names a person in text that leaves the tree.
   Change: rewrite each doc comment to say who prints the line and who reads it in role words ("the
   kernel", "the shell", "a console reader"), with no path. State the ruling behind `absent`
   without the name or the pull request number. Keep the table of earlier list lengths, keyed by
   which names each list held rather than by date. Move the design narrative (why the banner is
   generic, why the machine line comes last) to the README.
   Door: two-way. Effort: S.

### P2: later

The P2 items are in [the appendix](1078-boot-ladder-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1078-boot-ladder-outside-review/before-the-tag.md).

## Decisions for the architect

Only one-way doors. Each states the background first, then the recommendation, prior art and a
concrete example. The repository itself follows the standing rule that clearing this review is the
go for creating it; that is not a new decision.

### The heads are fixed shapes after the caller's system name

Ruled 2026-11-14 (UTC), option A. The crate names no system. It fixes the shape of each head and
takes the system's name from its caller: `Heads::new(name)` yields the seven heads that begin with
the name (`banner`, `machine`, `self_test`, `self_test_subject`, `tour`, `prompt`, `screen_held`),
each a `Head` that prints through `Display` and matches a line in place, so neither side allocates
and the kernel can use it. nife passes `nife`, so every byte printed and matched is the one stored
logs, board profiles and gates already hold, and a test in `serial_console` pins the nife heads to
those bytes.

The nine shapes are ratified as printed: `<system> on `, `<system> machine: `,
`<system> self-test: `, `<system> self-test subject: thread `,
`<system>: the capability core runs on `, `<system> capability shell`,
`<system> screen: held for the host; `, and inside the verdict `FAILED: ` and `absent: `. The shell
banner's text is unchanged and takes no period. The names of the accessors and types are
provisional, under the decision below.

Prior art (recalled, not read in this review, unless stated): seL4's `sel4test` ends a green run with one fixed sentence that its CI harness matches. Zephyr's `twister` console harness matches `PROJECT EXECUTION SUCCESSFUL` or `FAILED`. Linux's KTAP (`Documentation/dev-tools/ktap.rst`)
freezes `KTAP version 1` as the head of every report. In each, the first release of the reader freezes the sentence. Where nife differs: the heads live in a crate both sides link rather than
in a document, and the system's name is a parameter of that crate.

Example: a transcript holding
`nife self-test: 12 of 15 passed, 3 absent: block_device clock true_random_number_generator` is
green under the released crate and under every later one, because the verdict's shape and the
grammar behind it never change. Only the list length and the names may change, and the
earlier-lengths list says which totals an older kernel printed.

### The ten public names

Background. Ten items are public and unratified. Seven say what they hold. `PROMPT` holds the
shell's banner, which the crate's `BUGS` concedes. `SELF_TEST_CHECKS_EARLIER` holds counts.
`TOUR` holds the line that says the kernel's middle stages finished, named for a concept
(`Stage::Tour`) a stranger never meets. The name of `TOUR` and `PROMPT` is also the name of a
`serial_console` stage, which follows whatever ruling comes. The seven heads are now accessors on
`Heads` (decision above), so each recommendation below applies to the accessor's lower-case
spelling (`prompt` to `shell_banner`, `tour` to `core_runs`).

Recommendation: ratify `BANNER`, `MACHINE`, `SELF_TEST`, `SELF_TEST_FAILED`, `SELF_TEST_ABSENT`,
`SELF_TEST_CHECKS` and `SCREEN_HELD`. Rename `PROMPT` to `SHELL_BANNER` (it is the shell's first
line, and the real prompt is `$ `), `SELF_TEST_CHECKS_EARLIER` to `EARLIER_SELF_TEST_CHECK_COUNTS` (a count names the things counted), and `TOUR` to `CORE_RUNS` (the line's own words: the
capability core runs). Refused for `TOUR`: `HANDOVER`, since milestone 972 (the boot stick's shared names are code) spends that meaning on
the progenitor line; `KERNEL_UP`, since the banner already says the kernel is up.

Prior art: systemd's readiness protocol names its message for what it says (`READY=1`, `STATUS=`), and its reader and writer share `sd_notify`'s definition. U-Boot's `bootstage` names
each stage for the event (`BOOTSTAGE_ID_START_UBOOT_F`, `..._BOOTM_START`) in one header its tools
include. Where nife differs: a name here is a Rust item a third-party reader imports, so a
misnomer is in their code too.

Example: a third-party console reader written against the released crate reads
`boot_ladder::SHELL_BANNER` and knows without the `BUGS` entry that it is not waiting for a `$ `.

### The printed check names spell out their words

Ruled 2026-11-14 (UTC). Of the fifteen names the verdict prints, `blockdev` becomes `block_device`
and `trng` becomes `true_random_number_generator`. The other thirteen, `device_interrupt` included,
are ratified as printed. The self-test program's slot and the kernel half's flag that name the
generator follow: `TRUE_RANDOM_NUMBER_GENERATOR` and `TRUE_RANDOM_NUMBER_GENERATOR_ANSWERED`. No
list of old spellings is kept: a reader matches the current names only, and a stored log that
prints an older one is history.

Prior art: Linux kselftest names its suites by directory and abbreviates freely (`bpf`, `kvm`,
`mm`); nife's own naming rule refuses that. seL4test names tests by code (`SCHED0001`), which
needs a decoder. Zephyr names them dotted (`kernel.common`). Where nife differs: a reader takes a check name off a photograph with no decoder at hand, so a word beats a code and a spelled word beats an abbreviation.

Example: `nife self-test: 12 of 15 passed, 3 absent: block_device clock true_random_number_generator`,
and radon's profile forbids `entropy`, `true_random_number_generator` and `clock` absent.

## Prior art

The comparison with nearby work is in [the appendix](1078-boot-ladder-outside-review/prior-art.md).

## What needs the architect

The ten public names, above; the heads and the check names are ruled. Nothing else here is one-way: the manifest, the README, the tests, the
`Verdict` type and the nife-side gate are the maintainer's.

## Done when

The checks are in [the appendix](1078-boot-ladder-outside-review/done-when.md).

## BUGS

- The prior-art rows come from memory and say so. A lane that writes the README should read each source before citing it.
- Item 7's gate is nife's, not the crate's. After the move the crate cannot know that a consumer
  spelled a head by hand; only the consumer's own lint can.
- Item 11 adds no checkable version. A tenth rung that printed one would itself be a wording to
  freeze, and this review found no reader that needs it yet.
- The review did not run the template's name-check helpers over the crate; `script/names` and
  `helpers/name_release.py` in nife cover the same ten names, and the printed check names are
  string literals no helper reads.
- `xtask/src/time_record.rs` line 929 and 1010 (`"nife on riscv64"`, `"nife on x86_64"`) spell the
  banner inside `#[cfg(test)] mod tests`, so item 7 omits them; item 7's gate must exempt test
  modules the same way.

## Index row

An outside review of `boot_ladder` before it leaves found three manifest and file items that block the move. It found six to land before the first tag. They are a shared `Verdict` type so the printer and its three readers agree by construction, `const` invariants on the check list, real tests, eight hand-spelled literals in `xtask`, ten names to ratify, and a false boundary claim. It found four items for later. The heads are ruled to be fixed shapes after a system name the caller supplies, ratified as printed, and the check names `block_device` and `true_random_number_generator` are ruled with the other thirteen ratified. One ruling remains for the architect: three renames among the ten public names.
