# P1: before the first tag

An appendix to [the milestone](../1073-counter-frequency-contract-outside-review.md).

## P1: before the first tag

3. The crate's BUGS section states a kernel fact that is no longer true.
   Finding: the module documentation and the README both say the invariant time stamp counter bit
   (`CPUID.80000007H:EDX[8]`) is not checked. The kernel's `x86_64` ISA discovery documents the
   opposite: it refuses a part that does not advertise an invariant TSC and warns under a
   hypervisor. A stranger reading the contract learns a wrong fact about a kernel the crate will no
   longer be able to see. The deeper problem is that a contract's BUGS section describes one
   kernel's behavior at all.
   Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 9 to 11; `README.md` lines 55
   to 57; `kernel/src/arch/x86_64/isa.rs` lines 4 and 10; `kernel/src/arch/x86_64/timer.rs` line 5.
   Change: rewrite the three BUGS entries in the contract's own terms. The page carries one number
   for the life of a process, so the contract cannot express a counter whose rate changes after the
   page is written, and the writer establishes whether a given counter is invariant. A reader of a
   process with no mapping at the fixed address faults, by design. Drop the sentence about QEMU's
   code generator; it is a fact about one writer's environment.
   Door: two-way. Effort: XS.

4. `hertz` returns whatever number the page carries, and no reader in the tree applies the band.
   Finding: `hertz` answers `Some(n)` for any page with the magic, so `build_page(0)` reads back
   as `Some(0)` and `build_page(u64::MAX)` as `Some(u64::MAX)`. The review package proposes to
   keep that and document that the reader calls `is_plausible`. The readers do not:
   `user_mode_runtime::cntfrq_checked` returns `page.hertz()` directly on both architectures, and
   `process_builder` relays that number into a child's page unchecked. In nife this is safe only
   because the kernel asserts the band before `build_page`; a third-party writer has no such
   assertion, and a `Some(0)` becomes a division by zero in any reader that computes seconds.
   Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 208 to 216;
   `crates/user_mode_runtime/src/clock.rs` lines 82 to 94 and 137 to 147;
   `crates/process_builder/src/lib.rs` line 335; `kernel/src/arch/x86_64/timer.rs` lines 257 to
   267.
   Change, within the baseline: say at `hertz` and in the README's contract list that `hertz`
   returns the number as written and the band is the reader's to apply. Add one unit test pinning
   `build_page(0)` to `Some(0)` so the behavior is explicit. Have `cntfrq_checked` in
   `user_mode_runtime` apply `is_plausible` before returning. The stricter alternative, `hertz`
   answering `None` outside the band, is compatible with a later release and the maintainer can
   take it then; the package already leaves this with the maintainer.
   Door: two-way. Effort: S (the reader change is in nife, not in the crate).

5. `new` truncates its address on a target narrower than 64 bits.
   Finding: `new(virtual_address: u64)` casts with `as *const u8`. On a 32-bit target both fixed
   addresses (`0x7000_0000_0000` and `0x38_0000_0000`) are above `u32::MAX`, so the cast yields
   address 0 and the reader dereferences a null page. This leaves every target the crate names
   unaffected; the three bare-metal targets are all 64-bit. It is a hole in the claim, not a defect a current user
   can reach.
   Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 106 to 122 and 175 to 179.
   Change: a `const` assertion that `PAGE_VIRTUAL_ADDRESS <= usize::MAX as u64`, so a narrow
   target fails to compile instead of reading address 0, and one sentence in `new`'s safety
   section saying the address must be representable as a pointer on the target. Keeping `u64` as
   the parameter type is right: the kernel's mapper and the fixed constant are `u64`, and a
   `usize` parameter would move the cast to every caller.
   Door: two-way. Effort: XS.

6. The manifest's `publish` comment points into nife.
   Finding: the `publish = false` line cites `crates/publish-allowlist.txt`, a file the new
   repository will not hold. The dry run's citation check did not report it (BUGS).
   Evidence: `crates/counter_frequency_contract/Cargo.toml` line 11.
   Change: one sentence a stranger can act on, in the shape the `generational_table` review
   settled: the line stays until the release that publishes, which this crate's own release
   milestone owns under the publishing hold.
   Door: two-way. Effort: XS.

7. Documentation a stranger reads, beyond the three edits the package already assigns.
   Finding, four parts. (a) The `PAGE_VIRTUAL_ADDRESS` documentation says the address-space map's
   table of fixed pages "should list it, and does not yet". The map's own documentation now says
   that above 2 GiB only this page has an assignment, so the sentence is stale, and it describes
   another crate's internals. (b) The same item's `Name:` block says `VA` is the abbreviation alias,
   but no `#[doc(alias)]` attribute exists. The rule that a crate carries no alias for this tree's
   own earlier spelling before its first release means none should. This is the one place a later
   rule contradicts the baseline's text, and the move strips the block, so the fix is cosmetic on
   the nife side. (c) `build_page`'s example comment promises an "aligned buffer" while `new` requires
   only byte alignment; the two should agree, and byte alignment is the true requirement because
   the reader copies into a local array. (d) `build_page` and `hertz` say "the calibrated TSC
   rate" and `new`'s safety section says "every `x86_64` process" and opens a sentence in
   lowercase; the package's proposed items 1 and 2 cover these and they belong in the same lane.
   Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 102, 104 to 105, 119 to 120,
   124, 134, 170 to 171 and 190; `crates/address_space_layout/src/lib.rs` lines 4 to 5.
   Change: delete the stale sentence and the alias sentences, say "byte-aligned" in the example,
   and apply the package's items 1 and 2 in the same pull request.
   Door: two-way. Effort: S.

8. The template's next release refuses the prose.
   Finding: at the template's head, `check / standalone` runs `prose_acronyms` over the README and
   every doc comment at ceiling zero, and `passive_voice` over the README at two per thousand
   words. Over the standalone copy, `prose_acronyms` refuses `GHz` (README line 23, `src/lib.rs`
   line 56), `MHz` and `CSR` (`src/lib.rs` line 50), and `passive_voice` refuses the README (six
   passive constructions in 392 words, which allows none). The pinned template release `v7` does
   not run either check, so the dry run passes today; the weekly `lag` job turns red when the
   next release lands, Dependabot opens the bump, and the bump is red until the prose changes.
   Evidence: `crate-workflows/.github/workflows/check.yml` lines 255 to 286;
   `crates/counter_frequency_contract/README.md` lines 3, 8, 19, 23, 56 and 59;
   `crates/counter_frequency_contract/src/lib.rs` lines 50 and 56.
   Change: write "100 kilohertz" and "100 gigahertz" in prose and keep the symbol only inside a
   code span where it names a value. Expand "control and status register (CSR)" at first use. Put
   the six README sentences into the active voice; the first is the baseline's "was written for the
   nife kernel" sentence, which the move lane rewrites anyway.
   Door: two-way. Effort: XS.

9. Nothing is machine-checked, and the manifest says a model checker proves the crate's harnesses.
   Finding: the README says so plainly, and the package leaves a harness to the maintainer. The
   reader's one decision is the magic compare, and the whole value of the crate is that a frame
   without the magic reads as `None`. A harness proving it over every 16-byte frame costs little
   and is the kind of claim a stranger wants checked before trusting the `unsafe` reader. The
   manifest's `check-cfg` comment says "the Kani model checker, which proves this crate's harnesses"
   sets `cfg(kani)`, which is not true until a harness exists. Nobody verified whether the
   template's `proofs / kani` job is green on a crate with no harness; `falsify.py` reports
   "nothing to falsify" and exits 0.
   Evidence: `crates/counter_frequency_contract/README.md` lines 45 to 48; `Cargo.toml` lines
   20 to 21; `crates/counter_frequency_contract/src/lib.rs` lines 185 to 216.
   Change: two harnesses in a `verification` module behind `cfg(kani)`, each with a falsification
   patch the template replays red. The first takes `hertz` over an arbitrary
   `[u8; PAGE_CONTENT_BYTES]` and asserts `None` exactly when the first eight bytes differ from
   `MAGIC`, and otherwise the little-endian `u64` of the next eight. The second asserts
   `build_page(x).hertz() == Some(x)` for an arbitrary `x`. The
   README's "What is proven" section then says what the two prove and that no proof covers the
   caller's mapping promise, which the package already states. The pattern is
   `block_roster_contract`'s `verification` module and its `falsifications/` directory.
   Door: two-way. Effort: S.
