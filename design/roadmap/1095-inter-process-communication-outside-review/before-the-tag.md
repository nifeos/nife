# P1: before the tag

An appendix to [the milestone](../1095-inter-process-communication-outside-review.md).

## P1: before the first tag

4. A public type named `Send` shadows `core::marker::Send`.
   Finding: `pub use rendezvous::{Receive, Rendezvous, Send}` puts a type named `Send` at the
   crate root. Types and traits share one namespace, so `use inter_process_communication::*` or
   `use inter_process_communication::Send` in a user's module hides the prelude's `Send` trait
   there; `T: Send` then fails to parse as a bound. The kernel never imports it, which is why
   nothing has noticed. A stranger's first `use` will.
   Evidence: `src/lib.rs` line 37; `src/rendezvous.rs` line 66.
   Change: the names the milestone's D1 decides, provisionally. The crate's own vocabulary is "verdict"
   (`src/rendezvous.rs` lines 64, 81; `src/notification.rs` line 79), so `SendVerdict` and
   `ReceiveVerdict` are the names the documentation already uses in prose; `Signal`, `Wait` and
   `Arm` would follow for one shape across the four modules.
   Door: two-way before the first tag. Effort: S.

5. Names the template's ceiling-zero gates refuse.
   Finding, one line each:
   - `Rendezvous::one_queue_invariant` returns `bool` and is not shaped as a question
     (`question_shape`); its twin `Notification::invariant_holds` already follows the rule, and the
     item's own block says the rule postdates it.
   - `futex::Key::address` and the test helper parameter `address`: an address is
     `_virtual_address` or `_physical_address` in every location (`register_names`). The field is a
     virtual address, and the kernel's `futex_wait` passes a `va`.
   - `a_cancelled_or_rearmed_timer_never_fires_stale`: British spelling (`british_spelling`);
     `canceled`.
   - Test bindings `pa` and `pb` in `notification.rs` tests: `spelled_out` reads `pa` as the
     abbreviation of physical address. They are pointers to `a` and `b`; `pointer_a` says so and
     passes.
   - Eight prose spellings `typos` (en-us) refuses: the British forms of signaled (twice) and signaling,
     `queueing` (twice), and the British forms of modeled, neighboring, and behavior (in one patch header).
   Evidence: `src/rendezvous.rs` line 180; `src/futex.rs` lines 51 and 172; `src/timer.rs`
   line 250; `src/notification.rs` lines 456 and 518; `src/verification.rs` line 76;
   `src/timer.rs` line 13; `src/futex.rs` line 5; `src/rendezvous.rs` line 38;
   `src/notification.rs` lines 58, 246 and 325;
   `falsifications/verification.send_rendezvous_iff_a_receiver_waited.patch` line 5.
   Change: `one_queue_invariant_holds` (the name the block itself recommends), `Key::virtual_address`,
   the test and prose spellings. All but the first two are mechanical.
   Door: the two public names are one-way at publish; the rest two-way. Effort: S.

6. The design record does not travel with the crate.
   Finding: `notes/inter-process-communication.md` is the only place that states the token discipline. It also states the two obligations the harnesses do not check (every node outlives its object; one mint per node), why there are two `remove_*` methods, and why the timer proves a cache invariant instead of keeping a heap. Every doc comment points at it. The note stays in
   nife and moves to the record site, so a crates.io reader has nothing. The scaffold's
   `docs/stranger-test.md` reads `Last run: none` and the release gate refuses on that line.
   Evidence: `src/rendezvous.rs` lines 5, 9, 212; `src/notification.rs` line 8; `src/futex.rs`
   line 8; `src/timer.rs` line 4; `crate-workflows/scaffold/docs/stranger-test.md`.
   Change: `docs/design.md` in the crate's repository, the note's content without nife
   vocabulary, and a README from `scaffold/README-crate.md` whose "Running the proofs" section states what this review measured. `cargo kani` takes about half a minute and about 220 MB. One harness runs as `cargo kani --harness verification::send_preserves_the_invariant --exact`. A passing run prints two `trailing semicolon in macro` warnings from Kani's toolchain (item 17
   removes them). Then a stranger run on record.
   Door: two-way. Effort: M.

7. Manifest metadata for a crate that readers will open on crates.io.
   Finding: no `description`, `repository`, `readme`, `keywords`, `categories` or
   `rust-version`. The edition is 2024, which needs Rust 1.85; `Option::is_none_or` in the
   proofs needs 1.82 and the inline `const` block in `Futexes::new` needs 1.79, so 1.85 is the
   floor, and the template's weekly `msrv` job builds at whatever the manifest says.
   Evidence: `Cargo.toml`; `src/verification.rs` line 201; `src/futex.rs` line 69.
   Change: the fields, `rust-version = "1.85"`, and a one-line description that says what the
   crate decides ("the wait-queue decisions of a kernel's rendezvous, notification, timer and
   futex, as safe state machines over intrusive queues").
   Door: two-way (the description is prose). Effort: XS.

8. A harness recorded as unfalsifiable has a minimal falsification.
   Finding: `signal_preserves_the_invariant` carries a paragraph arguing that no minimal defect
   in `signal` can turn it red, under §134 (a harness carries a machine-replayable falsification
   record, or it is not evidence). Replacing `self.pending.saturating_add(1)` by
   `self.pending + 1` turns it red: the seed makes `pending` symbolic, so Kani's overflow check
   fails at `u32::MAX`. The harness therefore proves something the paragraph does not claim: a
   signal never panics, whatever the count. That is worth stating and recording.
   Evidence: `src/verification.rs` lines 71 to 90; `src/rendezvous.rs` line 318.
   Change: either add `falsifications/verification.signal_preserves_the_invariant.patch` with
   that one-line defect and rewrite the paragraph to say what the harness guards (no enqueue path,
   and no overflow), or keep the paragraph and say why an overflow failure does not count. The
   first is the §134 shape.
   Door: two-way. Effort: XS.

9. The mutation record is stale and no one has triaged the three survivors.
   Finding: the mutation index row says 24 mutants; `cargo mutants --list` today says 178,
   because the futex and timer modules arrived after that count. The latest census row records 3
   missed, all classed "algebraic", 3 unclassified, 1 copied. No note says which three.
   Evidence: `notes/mutation-testing/README.md` row for the crate;
   `notes/mutation-testing/survivor-classification.md` line 53; `cargo mutants --list` output.
   Change: one full `mutation.yml` run in the new repository inside the window, the three
   survivors named and either killed by a test or exempted in `.github/mutants-exempts` with a
   reason. Item 14 is one likely kill.
   Door: two-way. Effort: S, plus the run.
