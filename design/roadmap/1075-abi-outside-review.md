---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017, 1034
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
## Goal

A stranger who finds this crate can tell in one screen what it defines.
The stranger can build and test it from the README alone. Every method's contract reads without a path into another repository. A changed number cannot ship without the version changing with it. The repository's gates are green
on arrival for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1075-abi-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1075-abi-outside-review/prior-art.md).

## The items

Effort: S is under half a day for a lane, M a day or two, L more. A door is one-way when somebody
outside the repository acts on it after the release (a published name, a wire number, a layout, a
dependency); everything else is two-way and the maintainer decides it.

### P0: red on arrival

1. `cargo clippy -- -D warnings` fails in the staged repository.
   Finding: `src/survey.rs:39` writes `VisionFive 2's` without backticks, which `clippy::doc_markdown`
   (a lint the manifest turns on) reports. nife passes it only because the root `clippy.toml` lists
   `VisionFive` under `doc-valid-idents`; the template's scaffold carries no `clippy.toml`, so the
   `check / clippy` job is red on the first push. Verified on stable 1.99.1 and on the pinned nightly.
   Fix: write `` `VisionFive 2` `` (one edit, no new file). The alternative, shipping a
   `clippy.toml`, adds a file every future lint difference will grow.
   Effort: S. Door: two-way.

2. The crate cannot publish under its name.
   Finding: crates.io already has a crate named `abi` (version 0.1.4, 251 downloads, unrelated). The
   repository can be `nifeos/abi` and consumers can pin it by `rev`, but the publication step of the
   runbook (`notes/crate-move.md`, steps 16 to 19) and the README's badge row have no name to use.
   Fix: decision A below. The lane's part, once ruled, is the `[package] name` and a `[lib] name =
   "abi"` line so every `use abi::` stands.
   Effort: S. Door: one-way (a published name).

### P1: before the tag

3. The crate names nife and nife programs.
   Finding: `helpers/lint_externalized_crates.py` counts six hits, and the dry run's own
   nife-word check fails: `README.md:6` ("written for the nife kernel"), `src/lib.rs:270` (a comment
   about "nife's standard library" generator), `src/rendezvous.rs:30` ("makes nife a capability
   system"), and `src/virtio.rs:33` to `35`. These lines name `entropy`, `net_stack`, `gpu_driver`,
   `keyboard_driver` and `system_initializer` with their ring offsets.
   Fix: say "a capability kernel" and "the drivers that map the region"; the ring offsets belong
   in the drivers that own them, not in the constant that reserves the last eight bytes.
   Effort: S. Door: two-way.

4. Twenty-eight references point into nife's internals, which a stranger cannot follow.
   Finding: after the move strips the `Name:` blocks, the doc comments still cite
   `cap::CAPABILITY_TABLE_SLOTS` (`src/lib.rs:95`), `capability::Rights::ALL`, `::BIND` and
   `::ENUMERATE` (`src/lib.rs:210`, `src/rights.rs:9`, `14`, `19`), `user_mode_runtime::cycle_reading`,
   `::receive_request` and `::reply` (`src/lib.rs:226`, `src/rendezvous.rs:108`),
   `std_runtime_contract::SLOTS` (`src/memory_region.rs:26`), `current_cpu_contract::THREAD_PAGE_SLOTS` (`src/thread_control_block.rs:8`), `filesystem_contract` (`src/rights.rs:9`, `src/reboot.rs:7`) and `crates/byte_sink_contract` (`src/rendezvous.rs:10`). They also cite `entropy_contract` and `credential_contract`
   (`src/lib.rs:384`), `inter_process_communication::timer` (`src/timer.rs:41`), `sched::last_cpus`
   (`src/survey.rs:50`), `console::configure_from_dtb` (`src/uart.rs:6`), "AGENTS.md rule 7"
   (`src/lib.rs:227`), "claim 11" (`src/page_frame.rs:6`, `src/address_space.rs:8`), "(19c)"
   (`src/lib.rs:184`). The programs `pmap` and `ps` appear in `src/memory_region.rs:52`, `src/address_space.rs:28` and `src/survey.rs:13`. A raw HTML link into the kernel's rustdoc, `../user/struct.Spawn.html` (`src/fault.rs:3`), does not resolve in the crate's own docs, and rustdoc does not check it. `src/address_space.rs:4` cites `frame::MAP`, a module that does not
   exist (it is `page_frame`).
   Fix: restate each as a fact about the boundary ("the kernel asserts its own table size equal to
   this at compile time"; "a runtime wrapper tests `x4`, never `x1`"); delete the HTML link; name
   `page_frame::MAP`. The W^X claim gets its one sentence here instead of a claim number.
   Effort: M. Door: two-way.

5. The virtio contract documents an error that does not exist.
   Finding: `src/virtio.rs:19` says `SETUP_QUEUE` answers `BadQueue`/`WrongObject`. `Error` has no
   `BadQueue`; the kernel maps every transport refusal but a DMA escape to `WrongObject`
   (`kernel/src/syscall/object_methods.rs:234`). A reader who matches on `BadQueue` finds nothing to
   match.
   Fix: "`WrongObject` when `queue` is out of range, the block does not fit the region, or the
   queue was never set up".
   Effort: S. Door: two-way.

6. The crate root understates the register convention.
   Finding: `src/lib.rs:6` to `7` say `x0` carries the slot, `x1` the method, `x2` to `x4` the
   arguments and `x0` the result. The runtime's trap moves six words in both directions
   (`x0` to `x5`, `a0` to `a5`); `RECEIVE` writes `x3` and `x4`; a death delivers a sixth word
   (`src/fault.rs:26`); `CONFIGURE` reads "argument register 6" (`src/thread_control_block.rs:9`),
   a register the root never admits. The names are aarch64's throughout; the README says "or the
   architecture's equivalent" and the crate never says what that is beyond `START` (`src/thread_control_block.rs:29`)
   and `LIST` (`src/address_space.rs:31`). The x86_64 names of words 3 to 5 are not verified here.
   Fix: one table at the crate root: word, aarch64, riscv64, x86_64, meaning on entry, meaning on
   return; every module doc then says "word 3" or keeps `x3` with the table to read it by.
   Effort: S. Door: two-way (documentation of a wire that does not change).

7. Three statements contradict the crate.
   Finding: (a) `src/lib.rs:184` says of an address space that "nothing can run in it until TCBs
   arrive", while `thread_control_block::CONFIGURE` binds one and the same paragraph says a thread can take a binding. (b) `src/lib.rs:104` to `109` keep `console::WRITE` "so the ABI's history is
   legible", a method of an object no kernel serves, with no user in the tree; the tree's rule is
   that git holds history. (c) `README.md:65` says "BUGS: None known" while the crate carries five
   `# BUGS` sections (`src/rendezvous.rs:20`, `46`, `71`; `src/memory_region.rs:56`;
   `src/lib.rs:247`), two of them real wire warts (a message word that reads as an error; a death
   read through `RECEIVE_CAPABILITY` loses its `pc`).
   Fix: (a) delete the sentence; (b) delete the `console` module and its pinned line, under
   decision B; (c) the README's BUGS section lists the five in one line each.
   Effort: S. Door: (b) is decision B; the rest two-way.

8. Dates and rulings stand in doc comments as history.
   Finding: 23 dated phrases remain in `src/` after the move strips the `Name:` blocks, such as
   `src/lib.rs:101` ("the size was set on"), `:171` ("confirmed on"), `:307` ("ruled on") and `:357` ("were ratified on"). Others sit at `src/memory_region.rs:11` and `:54`, `src/rendezvous.rs:82` and `:109`, `src/survey.rs:19`, `src/notification.rs:33`, `src/reboot.rs:5`, `src/object_type.rs:29`, `src/cycle_counter.rs:31`, `src/timer.rs:39`, `src/reply.rs:7` and `src/thread_control_block.rs:43`, plus four "Index exception (date)" comments in `src/lib.rs`. §267 and §298 put these in commit
   messages; the template's `history phrases` gate reads only markdown today, so nothing refuses
   them, and a stranger reads them as noise.
   Fix: state each rule in the present tense; where the sentence is provenance, move it into the
   item's `Name:` block before the move strips it.
   Effort: S. Door: two-way.

9. The README fails the template's prose gates, and has no badge row.
   Finding: `prose caps` reports a 57-word sentence (the "Methods" bullet) and 28 bold spans per 1,000 words against a cap of 4. `passive voice` reports 6 passives in 459 words against 2 per 1,000. `readme_badges` wants the scaffold's row under the title, with the repository's name.
   The section order passes.
   Fix: split the bullet into a table of modules; unbold; rewrite the six sentences; add the row at
   the move. Reading as a stranger, the README also needs the one sentence item 6's table gives
   (which register holds what on each architecture) and the line `cargo test` already has.
   Effort: S. Door: two-way.

10. The manifest's publish comment points out of the repository.
    Finding: `Cargo.toml:10` says "the allowlist lint holds the exception (crates/publish-allowlist.txt)",
    a file the new repository will not have. The same finding as milestone 1048's first item.
    Fix: "`false` until the release that publishes; that release deletes the line".
    Effort: S. Door: two-way.

11. The version gate stays behind.
    Finding: `helpers/format_values_check.py` (`ABI_CRATE = 'crates/abi/'`) is the check that a
    changed or removed public number arrives with an `ABI_VERSION` bump. It reads nife's diff and
    will see nothing once the crate has left. The crate's own tests pin every value, and the
    test's doc claims the list is exhaustive, but nothing counts: 99 `pub const` `u64` items today
    against 99 pinned lines, by hand. The template's `semver` job watches API shape, not values.
    Fix: carry the rule into the crate. A test reads `src/**/*.rs`, counts every `pub const NAME:
    u64` and `pub const NAME: u32`, and fails unless each appears in the pinned lists (the lists
    then cannot drift). The version half becomes a `script/` check in the new repository, or a
    CHANGELOG rule the pull request template states: a pinned line changes only with `ABI_VERSION`.
    Effort: M. Door: two-way.

12. Two function names break rules a stranger reads as inconsistency.
    Finding: `futex::admitted(flags) -> bool` (`src/futex.rs:16`) is a bare participle. `design/naming.md` ("Functions that answer yes or no") wants `is_admitted`, and the template's
    `question_shape.py` refuses it at ceiling 0. `Error::from_ret` (`src/lib.rs:387`) abbreviates
    "return"; the standard library's spelling of the same conversion is `TryFrom<i64>`, which also
    gives `?` and `.try_into()` for free. Both names are provisional; `admitted` has one kernel
    caller, `from_ret` six.
    Fix: `is_admitted`; `impl TryFrom<i64> for Error` with the current body, and `from_ret` deleted
    or kept one release as a thin alias (the no-alias rule for old spellings applies before the
    first release, so delete).
    Effort: S. Door: two-way.

13. The names are not ready for the runbook's first step, and no package exists.
    Finding: `helpers/name_release.py --draft abi` lists 148 names: 17 ratified, 24 provisional,
    106 with no `Name:` block at all (every method constant of `address_space`, `memory_region`,
    `notification`, `page_frame`, `timer`, `usage`, `rights` and more). `notes/crate-move.md` step 1 needs the architect's ratification before the move. Also, `notes/crate-reviews/` has no package
    for this crate to start from. This is the largest gap between the crate and the move.
    Fix: a lane writes the package (`script/crate-review-packet abi` plus the naming table, in
    the shape of `notes/crate-reviews/socket-contract.md`), and marks every unmarked public item
    `provisional` so the worklist can see it. The rulings are the architect's, and this review does not list them one by one. The package asks them.
    Effort: M for the package. Door: each ratified name is one-way once published.

### P2: later

The P2 items are in [the appendix](1075-abi-outside-review/later-items.md).

## Decisions for the architect

Two one-way doors. Everything else above is the maintainer's.

### A. The published name

Background. The crate's name is `abi`, ratified, and `design/naming.md` says "`abi` takes no
suffix". A crates.io name is global and first-come: `abi` belongs to another author's crate (0.1.4,
251 downloads), so `cargo publish` of this manifest fails whatever the version. The repository
name is a separate namespace and `nifeos/abi` is free. A Cargo package may carry one name on
crates.io and another as the library a program writes in `use`: `[package] name = "x"` with `[lib]
name = "abi"`. Redox does exactly this (`redox_syscall` on crates.io, `use syscall::` in code).
Hubris keeps a crate named `abi` and never publishes it. The hold on publishing stands either way;
this decision shapes the manifest, the badge row and the runbook's step 16 before any publish.

Recommendation. Keep `abi` as the library name, so every `use abi::` in nife and in basalt's programs stands and the ratified name stays as it is. Publish the package under a name that says what it is. Of the names free on crates.io at the time of the review (`syscall_abi`, `kernel_abi`,
`capability_abi`, `capability_kernel_abi`, `invoke_abi`), `capability_kernel_abi` is the one that
passes the generic-word test: `syscall_abi` and `kernel_abi` could name any kernel's. The cost is one
line in `design/naming.md` saying the package name and the library name differ for this crate, and
why.

Alternatives. Never publish and consume by `rev` only (Hubris's path; it forecloses the step 16
publication and crates.io is where a second implementation would find the contract). Rename the
library too (every consumer and every README example changes; a rename of a ratified name is its own
decision).

Concrete example. `Cargo.toml` reads `name = "capability_kernel_abi"` under `[package]` and
`name = "abi"` under `[lib]`; `kernel/Cargo.toml` reads
`abi = { package = "capability_kernel_abi", version = "0.1.0", git = "https://github.com/nifeos/abi", rev = "<commit>" }`
and `kernel/src/syscall.rs` still reads `abi::SYSCALL_INVOKE`.

### B. Removing `console::WRITE` without bumping `ABI_VERSION`

Background. `console::WRITE` is a method number of a kernel object that no kernel serves and no
program invokes (item 7b). `ABI_VERSION` is the number a program declares and a kernel compares at
spawn; a mismatch refuses the program. nife's version gate refuses any pull request that removes a
public number of this crate without bumping `ABI_VERSION`, and a bump to 2 would refuse every
program basalt has built against 1. The deletion changes nothing a program can observe.

Recommendation. Delete the module and its pinned line without a bump. Record in the gate (and later in the crate's own rule, item 11) that the version follows the numbers a kernel dispatches, so a removal of a number no kernel serves is not a renumber. The stricter alternative, bumping on every
removal, would make the first deliberate cleanup of the surface a flag day for basalt with no
behaviour behind it.

Prior art. seL4 and Zircon remove nothing from a released ABI; Linux reserves a retired syscall's
number forever (`sys_ni_syscall`). None of them has a version a program declares at spawn, which is
what makes this crate's case different: here the version is what the kernel compares, so it should
mean what the kernel serves.

Concrete example. After the deletion, a program built against `ABI_VERSION` 1 spawns on a kernel
built against the trimmed crate, invokes nothing numbered `console::WRITE` because nothing ever
could, and `Error::BadMethod` is what any object would have answered before and after.

## Done when

The checks are in [the appendix](1075-abi-outside-review/done-when.md).

## BUGS

- Item 6's x86_64 column for the fourth and fifth words comes from reading the runtime's aarch64 and riscv64 traps and the x86_64 `START` doc. The lane that writes the table reads the
  x86_64 trap and the kernel's entry before it writes a register name.
- Item 20 names three survivors of six; the other three are what the first weekly run reports, and
  this block does not guess them.
- Item 21 does not know whether the proofs job is green with no harness; the lane that stages the
  move finds out on the first push.
- The coverage figure was not measured; every function in the crate has a test that calls it, so the 85 floor should pass, unverified.

## Index row

An outside review of `abi` before its move found these problems. One lint is red on arrival. Another crate holds the crates.io name. Six nife references and twenty-eight internal citations need a rewrite. The docs name an error the kernel never answers. The root states the register convention too narrowly. A version gate stays behind in nife. Two function names break the rules. 130 names lack a ratified block, and no review package exists. It leaves two one-way doors for the architect, the published name and whether a
dead method's removal bumps the version.
