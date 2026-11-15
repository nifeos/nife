# P1: before the tag

An appendix to [the milestone](../1083-clock-contract-outside-review.md).

## P1: before the move is declared done

3. The mutation verdict needs the ten recorded outcomes written as exemptions.
   Finding: the template's `mutation` verdict (`helpers/mutants_gate.py`) fails on any survivor or
   timeout not listed in `.github/mutants-exempts` with a reason, and the release gate reads that
   verdict. The baseline row for this crate records 3 survivors and 7 timeouts. The regression
   note for this crate under `notes/mutation-testing/` (the one that triages `clock_contract`,
   `swish` and `filesystem_contract` together) already gives the reasons. `spin_hint` replaced by `()` is a hint with no host-observable effect. The claim's `+` under `*` is single-threaded blindness that the loom gate kills. `decide`'s `>` under `>=` is equivalent at equality because both arms fall through to `ACCEPTED`. The seven timeouts are the two retry loops' conditions turned into loops that never exit. Nobody knows the mutants' line numbers in the new repository until the first full run, so the note supplies this
   inference, not a measurement.
   Why it matters: the weekly run is red for a known reason until the lines exist, and a red verdict
   blocks the release gate when the release is eventually unheld.
   Change: the move lane dispatches `mutation.yml` once, copies each survivor's and timeout's line
   from `mutants.out` into `.github/mutants-exempts` with the note's reason, and runs the
   verdict again. The loom gate that kills the claim mutant is nife's `script/interleaving-check`;
   the new repository has no such job, so either the `tests` job runs the loom models under
   `--cfg loom` or the exemption's reason says the models run in nife only. The first is better and
   costs twelve seconds.
   Door: two-way. Effort: S.

4. The manifest's `publish` comment names a file that is not in the repository.
   Finding: `Cargo.toml` line 11 reads `publish = false # the default; the allowlist lint holds the
   exception (crates/publish-allowlist.txt)`. The file exists in nife and nowhere else. The manifest
   also has no `repository` key, which `cargo package` warns about and the move adds. The same
   finding appears in the outside reviews of the four crates that moved before this one.
   Why it matters: a stranger follows the path and finds nothing; the sentence also says a lint holds the `publish` line, and the new repository does not have that lint.
   Change: one sentence a stranger can act on ("stays `false` until the release milestone that
   publishes; nife pins the crate by `rev` meanwhile"). The move adds `repository`.
   Door: two-way. Effort: S.

5. The reader's spin has no bound, and the layout is about to freeze.
   Finding: `ClockPage::read` (`src/lib.rs` line 272 to 299) spins while the sequence word is odd. The
   crate says so in its `BUGS` twice (lines 209 and `README.md`) and the clock note says the same.
   The readers include the standard library's `SystemTime::now` in every process
   (`patches/std-nife/overlay/std/src/sys/time/nife.rs` line 130), which calls `read` with no bound of its
   own. Decision 1 asks whether the page layout should make readers wait-free before `VERSION` 1 freezes. This item is the lane that follows the ruling.
   Why it matters: after the identity freezes, changing the layout costs a `VERSION` and a new magic
   and every pin moves. Before it, the change costs one lane and a re-run of the loom models.
   Change: per Decision 1. If the ruling keeps the seqlock, the item closes by stating the hazard
   once in the `ClockPage` doc with the words "by design" and the reason, so a stranger does not
   file it as a bug.
   Door: one-way if the layout changes (the whole point of deciding it now). Effort: M for the
   layout change (the algorithms, the four loom models, the host tests, the overlay's generated copy,
   the clock program and `date`); S for the documentation-only outcome.

6. The docs call `initialize` a once-only step, and its only caller runs it on every start. DONE (first option: a page that carries the magic is left as it is, and an abandoned odd sequence is released forward; `initializing_a_page_again_keeps_what_was_published`, `initializing_a_page_with_an_abandoned_claim_releases_it_forward`, and the loom model `a_restarted_initialize_never_shows_a_reader_a_reset_page`).
   Finding: `initialize` (`src/lib.rs` line 254) says the writer does it "once, before anyone else has a
   mapping, so it needs no sequence claim". `components/src/clock.rs` line 63 calls it unconditionally at
   program start. The review did not establish whether this tree ever restarts the `clock` program on the same frame (it found no supervisor entry for it). If it does, readers already hold
   mappings when the restart stores the sequence as zero, and a reader comparing `generation` across the
   restart sees it go backward. The loom model for a racing reader
   covers the first initialize only.
   Why it matters: the contract's precondition is one the caller cannot promise in general, and a
   stranger writing a second clock service will copy the call.
   Change: make `initialize` idempotent (return without writing when the magic is already present,
   and say so), so a restart continues the generation count. The alternative is to state in the doc that a restart on
   a published frame resets the generation and readers must tolerate a smaller one. The first is
   one compare and keeps `generation`'s meaning.
   Door: two-way (no reader checks for the reset today). Effort: S.

7. Doc comments that assume the reader lives in nife.
   Finding: `propose::OPERATION_SHIFT` (`src/lib.rs` line 352) says "the same position its sibling
   message contracts use". `rtc::CMOS` (lines 554 to 558) says the service reads the ports "through
   a `PortRange` capability over both, which the wiring grants with `WRITE` because the I/O
   permission bitmap cannot permit `in` without `out`". `INTERFACE_ID` (lines 168 to 171) names "a
   program's `INTERFACES` note" and "the project that links these crates". `ClockPage` (line 202)
   and the `state` module (line 125) lean on "the capability layout" and "the no-silent-degradation
   rule" as if the reader knew them. None is wrong; none has a referent outside this tree.
   Why it matters: the crate is the one definition three programs share, and its rustdoc is where a
   stranger reads the contract.
   Change: one sentence each that stands alone ("the opcode occupies the top byte of the first
   word", "two fixed I/O ports the service is granted together", "a program declares the pair of
   this id and `VERSION`; the system that links the programs keeps the table of ids"), and keep the
   nife reasoning in nife's notes, which already hold it.
   Door: two-way. Effort: S.

8. The crate needs 64-bit atomics and does not say so.
   Finding: every word of the page is an `AtomicU64`. On a target without `target_has_atomic = "64"`
   (`riscv32imac-unknown-none-elf`, the Cortex-M targets) the build fails with an error about a
   missing type rather than a sentence. The review inferred this from reading; the host has no such target.
   Why it matters: the manifest's categories (`embedded`, `no-std`) invite exactly those targets.
   Change: `#[cfg(not(target_has_atomic = "64"))] compile_error!("clock_contract needs 64-bit
   atomics: the clock page is four 64-bit words")` and one README sentence under "Building and
   testing".
   Door: two-way. Effort: S.

9. Two Kani harnesses, each with its falsification patch.
   Finding: the crate has no harness; `cargo kani` and `falsify.py --check` pass by finding nothing,
   so the template does not block. nife's `script/crate-standalone` fails its `proofs` check, and
   the move runbook (`notes/crate-move.md`, the step that wants `crate-standalone` to exit 0 with
   Kani) names that check. The package's open question 1 recommends two harnesses and leaves the
   call to the maintainer.
   Why it matters: `policy::decide` is the one function a compromised proposer can aim at, and it
   is a pure function of three words, which is the cheapest kind of proof there is. The request
   word's round trip is the wire format the service decodes with its own copy of the constant.
   Change: harness one: for any `current_state`, `current` and `proposed`, `decide` returns
   `ACCEPTED` only if `is_plausible(proposed)`, and when the state holds a known time only if the step lies
   inside the bound on its side. Harness two: `operation(req(x)) == x` for every `x` below 256, and
   `wall_nanoseconds(offset_for(w, m), m) == w` whenever `w >= m`. Each with a patch under
   `falsifications/` that `proofs / falsify` replays red.
   Door: two-way. Effort: M.
