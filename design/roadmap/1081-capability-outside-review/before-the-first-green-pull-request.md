# P1: before the first green pull request in the new repository

An appendix to [the milestone](../1081-capability-outside-review.md).

## P1: before the first green pull request in the new repository

3. The proved `derive` is not the delegation the kernel runs.
   Finding: `CapabilityTable::derive`, the subject of `derive_never_widens_rights` (which the
   crate, `notes/verification/core-harnesses.md` and the lifecycle note all call the central
   theorem), has no caller in the kernel. Neither do `put` and `get_with`. Delegation runs in
   `kernel/src/sched/capabilities.rs` (`Delegation::derive`, lines 366 to 383), which rereads the
   source, requires `GRANT`, requires the subset, and builds the copy by hand; the two `REVOKE`
   arms in `kernel/src/syscall.rs` (lines 198 and 212) check `GRANT` the same way. None of those
   three sites is under a Kani harness (`kernel/src/syscall/proofs.rs` has none for them). Inside the crate, the docs describe `Rights::GRANT` as the right to pass a capability on, and nothing in
   the crate consults it: an outside user who calls `derive` gets a copy of a `GRANT`-less
   capability. Within nife this widens nothing, because the kernel never calls `derive`; it means
   the proof and the code it describes have drifted apart, and the crate's own API contradicts its
   own documentation of `GRANT`.
   Evidence: `src/lib.rs` lines 62 to 64, 453 to 472, 717 to 758; `kernel/src/sched/capabilities.rs`
   lines 340 to 400; `kernel/src/syscall.rs` lines 196 to 214; `grep -rn '\.derive(' kernel/src`.
   Change: put the rule in the crate once, as a method on `Capability<O>`: `delegate(&self,
   rights: Rights) -> Result<Capability<O>, Error>` (name provisional), refusing without `GRANT`
   and refusing a widening, with a new error variant for the first refusal (`GrantRequired`,
   provisional). Have `Delegation::derive` and `CapabilityTable::derive` call it, so the kernel's
   lock discipline stays where it is and the rule has one body. Prove it: a harness over all 2^32
   source and requested rights that `Ok` implies `GRANT` held and the result's bits are within the
   source's, stated in raw bits as `derive_never_widens_rights` is, with a falsification patch that
   drops the `GRANT` check. Then either give `get_with` a caller or delete it; `put` stays as the
   proof builder's door and says so.
   Door: two-way (no behavior changes on the kernel's path when the rule is the same). Effort: M.

4. A slot number truncates on a 32-bit target.
   Finding: `get`, `put`, `delete` and (through them) `derive` cast `slot as usize`
   (`src/lib.rs` lines 389, 439, 448, 476; its range check guards `insert_at` first and `insert_at_or_above` uses `try_from`). On a target with 32-bit `usize`, slot `2^32 + k` names
   slot `k`. The caller only reaches its own table, so the caller gains no authority. The claim that "the
   entire security mechanism is a bounds check" stops being true as stated, and `get(u64::MAX)`
   passing the test (`usize::MAX` is out of range) hides it. No target nife builds is 32-bit, so
   this is a hole in the claim, not a defect a current user can reach.
   Evidence: the four lines above; `cargo clippy -W clippy::pedantic` names five
   `cast_possible_truncation` sites; `tests.rs` line 103.
   Change: make `storage::slot`, `fill` and `empty` take `u64` and convert with
   `usize::try_from(slot).ok()?`, so an unrepresentable slot is `NoSuchSlot` everywhere; one test
   with `1 << 32` beside `u64::MAX`; one sentence on `get`.
   Door: two-way. Effort: S.

5. `len` means capacity.
   Finding: `CapabilityTable::len` returns `N`, the capacity, while `is_empty` and `used` speak of
   occupancy, so a table with `len() == 64` is `is_empty()`. The naming rule says a method takes
   std's spelling and meaning where std has one, and `len` in std is the number of elements held.
   The ruling that standard-library names win came after this name, so this review rechecks it as the release-review rule allows. The kernel reads it as capacity at `kernel/src/sched/
   capabilities.rs` lines 88, 149, 303 and 328.
   Evidence: `src/lib.rs` lines 377 to 385; `design/naming.md`, "Shapes".
   Change: rename to `capacity` (std's word for exactly this on `Vec` and `HashMap`), keep
   `is_empty` and `used`, add `len` only if it returns `used`. Four kernel call sites move with it.
   Door: two-way before the release; a published name after it. Effort: S. Name provisional.

6. The move tool breaks every regenerated patch and skips the manifest step while reporting both
   as done.
   Finding: in the staged tree all 14 regenerated patches fail `git apply --check` with "corrupt
   patch" (the template's `proofs / falsify` uses `git apply`), while the tool's own `patch -p1
   --dry-run -F0` passes them. The regenerated hunk header for `subset_is_reflexive` says 7 old
   lines and the body carries 6: the trailing blank context line is gone. `regenerate_patch` calls
   `run`, which returns `(stdout + stderr).strip()`, and `strip()` removes a final line that is a
   single space, which is how `diff -u` writes a blank context line. Separately, `add_repository`
   matches only `^license\s*=` and this manifest has `license.workspace = true`, so the tool writes no `repository` line while the "rewrite" step reports it set.
   Evidence: `helpers/crate_move.py` lines 72 to 118 (`regenerate_patch`), 156 to 163
   (`add_repository`), 574 to 576 (`run`); the staged tree's `falsifications/
   verification.subset_is_reflexive.patch` lines 26 to 32; `crate-move-static.log` of the dry run.
   Change: have `regenerate_patch` take `diff`'s output without stripping (or strip newlines
   only), add a selftest patch whose hunk ends in a blank line, and make `add_repository` insert
   after `license.workspace` too, or fail loudly when it inserted nothing. Any crate whose hunk
   ends at a blank line hits the first; any crate that inherits its license hits the second, so
   both belong in the tool, not in this crate. The template's `standalone` job would have caught
   both on the first pull request; the point of the dry run is to catch them before it.
   Door: two-way. Effort: S. This item is against nife's tooling, not the crate.

7. Two mutators are outside every harness.
   Finding: `insert_at_or_above` (the floor insert the `std` runtime relies on) appears in no
   harness, so no proof covers the free word or the count after it. Only the builder's first fill exercises `put` over an occupied slot. One assertion covers the mark's monotonicity, as `peak() >= used()` after a delete, not as "never falls across any operation". A test, not a proof, reaches the `floor >= 64` arm of `first_free_from`.
   Evidence: `src/lib.rs` lines 294 to 302, 414 to 427, 601 to 677.
   Change: add `insert_at_or_above(kani::any(), ...)` and a second `put` to
   `the_free_mask_is_the_empty_slots` and `the_count_is_the_slots`. Add a harness that applies one
   symbolic operation of any kind to a symbolic table and asserts `peak` did not fall and
   `insert_at_or_above` never landed below its floor. Each new harness arrives with its
   falsification patch. The unwind bound stays N + 1 by the existing argument.
   Door: two-way. Effort: S.
