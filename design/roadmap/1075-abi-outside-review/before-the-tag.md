# P1: before the tag

An appendix to [the milestone](../1075-abi-outside-review.md).

## P1: before the tag

3. The crate names nife and nife programs.
   Finding: `helpers/lint_externalized_crates.py` counts six hits. The dry run's own
   nife-word check fails on `README.md` ("written for the nife kernel"), `src/lib.rs` line 270 (a comment
   about "nife's standard library" generator), `src/rendezvous.rs` line 30 ("makes nife a capability
   system"), and `src/virtio.rs` line 33 to 35. These lines name `entropy`, `net_stack`, `gpu_driver`,
   `keyboard_driver` and `system_initializer` with their ring offsets.
   Fix: say "a capability kernel" and "the drivers that map the region"; the ring offsets belong
   in the drivers that own them, not in the constant that reserves the last eight bytes.
   Effort: S. Door: two-way.

4. Twenty-eight references point into nife's internals, which a stranger cannot follow.
   Finding: after the move strips the `Name:` blocks, the doc comments still cite
   `cap::CAPABILITY_TABLE_SLOTS` (`src/lib.rs` line 95), `capability_table::Rights::ALL`, `::BIND` and
   `::ENUMERATE` (`src/lib.rs` line 210, `src/rights.rs` line 9, 14, 19). They cite `user_mode_runtime::cycle_reading`,
   `::receive_request` and `::reply` (`src/lib.rs` line 226, `src/rendezvous.rs` line 108),
   `std_runtime_contract::SLOTS` (`src/memory_region.rs` line 26). They cite `current_cpu_contract::THREAD_PAGE_SLOTS` (`src/thread_control_block.rs` line 8), `filesystem_contract` (`src/rights.rs` line 9, `src/reboot.rs` line 7) and `crates/byte_sink_contract` (`src/rendezvous.rs` line 10). They also cite `entropy_contract` and `credential_contract`
   (`src/lib.rs` line 384), `inter_process_communication::timer` (`src/timer.rs` line 41), `sched::last_cpus`
   (`src/survey.rs` line 50), `console::configure_from_dtb` (`src/uart.rs` line 6), "AGENTS.md rule 7"
   (`src/lib.rs` line 227), "claim 11" (`src/page_frame.rs` line 6, `src/address_space.rs` line 8), "(19c)"
   (`src/lib.rs` line 184). The programs `pmap` and `ps` appear in `src/memory_region.rs` line 52, `src/address_space.rs` line 28 and `src/survey.rs` line 13. A raw HTML link into the kernel's rustdoc, `../user/struct.Spawn.html` (`src/fault.rs` line 3), does not resolve in the crate's own docs, and rustdoc does not check it. `src/address_space.rs` line 4 cites `frame::MAP`, a module that does not
   exist (it is `page_frame`).
   Fix: restate each as a fact about the boundary ("the kernel asserts its own table size equal to
   this at compile time"; "a runtime wrapper tests `x4`, never `x1`"); delete the HTML link; name
   `page_frame::MAP`. The W^X claim gets its one sentence here instead of a claim number.
   Effort: M. Door: two-way.

5. The virtio contract documents an error that does not exist.
   Finding: `src/virtio.rs` line 19 says `SETUP_QUEUE` answers `BadQueue`/`WrongObject`. `Error` has no
   `BadQueue`; the kernel maps every transport refusal but a DMA escape to `WrongObject`
   (`kernel/src/system_call/object_methods.rs` line 234). A reader who matches on `BadQueue` finds nothing to
   match.
   Fix: "`WrongObject` when `queue` is out of range, the block does not fit the region, or the
   queue was never set up".
   Effort: S. Door: two-way.

6. The crate root understates the register convention.
   Finding: `src/lib.rs` line 6 to 7 say `x0` carries the slot, `x1` the method, `x2` to `x4` the
   arguments and `x0` the result. The runtime's trap moves six words in both directions
   (`x0` to `x5`, `a0` to `a5`). `RECEIVE` writes `x3` and `x4`. A death delivers a sixth word
   (`src/fault.rs` line 26). `CONFIGURE` reads "argument register 6" (`src/thread_control_block.rs` line 9),
   a register the root never admits. The names are aarch64's throughout; the README says "or the
   architecture's equivalent" and the crate never says what that is beyond `START` (`src/thread_control_block.rs` line 29)
   and `LIST` (`src/address_space.rs` line 31). The x86_64 names of words 3 to 5 are not verified here.
   Fix: one table at the crate root: word, aarch64, riscv64, x86_64, meaning on entry, meaning on
   return; every module doc then says "word 3" or keeps `x3` with the table to read it by.
   Effort: S. Door: two-way (documentation of a wire that does not change).

7. Three statements contradict the crate.
   Finding: (a) `src/lib.rs` line 184 says of an address space that "nothing can run in it until TCBs
   arrive", while `thread_control_block::CONFIGURE` binds one and the same paragraph says a thread can take a binding. (b) `src/lib.rs` line 104 to 109 keep `console::WRITE` "so the ABI's history is
   legible", a method of an object no kernel serves, with no user in the tree; the tree's rule is
   that git holds history. (c) `README.md` says "BUGS: None known" while the crate carries five
   `# BUGS` sections (`src/rendezvous.rs` line 20, 46, 71; `src/memory_region.rs` line 56;
   `src/lib.rs` line 247). Two of them are real wire warts (a message word that reads as an error; a death
   read through `RECEIVE_CAPABILITY` loses its `pc`).
   Fix: (a) delete the sentence; (b) keep the `console` module as reserved (ruled, decision B; done); (c) the README's BUGS section lists the five in one line each.
   Effort: S. Door: (b) is decision B; the rest two-way.

8. Dates and rulings stand in doc comments as history.
   Finding: 23 dated phrases remain in `src/` after the move strips the `Name:` blocks, such as
   `src/lib.rs` line 101 ("the size was set on"), `:171` ("confirmed on"), `:307` ("ruled on") and `:357` ("were ratified on"). Others sit at `src/memory_region.rs` line 11 and `:54`, `src/rendezvous.rs` line 82 and `:109`, `src/survey.rs` line 19, `src/notification.rs` line 33, `src/reboot.rs` line 5, `src/object_type.rs` line 29, `src/cycle_counter.rs` line 31, `src/timer.rs` line 39, `src/reply.rs` line 7 and `src/thread_control_block.rs` line 43. There are also four "Index exception (date)" comments in `src/lib.rs`. Section §267 (A comment states the constraint as it is now) and §298 (A document states what is in force now, and its history is in git) put these in commit
   messages. The template's `history phrases` gate reads only markdown today, so nothing refuses
   them. A stranger reads them as noise.
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
    Finding: `Cargo.toml` line 10 says "the allowlist lint holds the exception (crates/publish-allowlist.txt)",
    a file the new repository will not have. The same finding as milestone 1048 (what an outside maintainer would fix before 0.1.0)'s first item.
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
    Finding: `futex::admitted(flags) -> bool` (`src/futex.rs` line 16) is a bare participle. `design/naming.md` ("Functions that answer yes or no") wants `is_admitted`, and the template's
    `question_shape.py` refuses it at ceiling 0. `Error::from_ret` (`src/lib.rs` line 387) abbreviates
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
