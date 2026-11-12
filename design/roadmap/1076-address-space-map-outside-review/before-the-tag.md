# P1: before the tag

An appendix to [the milestone](../1076-address-space-map-outside-review.md).

## P1: before the first tag

4. The crate's documentation addresses a reader inside nife.
   Finding: the crate root and item docs cite `notes/address-space-map.md`,
   `crates/user_mode_runtime/link.ld`, `components/src/swish.rs`, `script/lint`,
   `kernel::user::USER_STACK_TOP`, `supervision_contract::CHILD_STACK_VA`. They also cite §171 (where a program image starts, and where the stack goes), milestones 206 and
   956, a pull request number, the architect by name, and five `Name:` blocks. The template's
   `standalone.py` counts 1 section citation, 4 paths into the tree, 5 provenance blocks, 2 lines
   naming the architect and 3 milestone citations. The template also states that a crate carries no `Name:` block. Two sentences are history ("Was `kernel::user::USER_STACK_TOP` ... both
   `0x50_1000`"), which nife's own rule for documents excludes. The root doc tells the reader to
   "see notes/address-space-map.md for the table of bands", so the one table a stranger needs is
   not in the crate.
   Evidence: `src/lib.rs` lines 2, 9, 13 to 15, 18, 23 to 27, 30 to 35, 111 to 112, 147 to 148, 152 to 153, 160 and 290 to 291;
   `src/verification.rs` lines 5 and 13 to 14.
   Change: the crate root doc carries the band table (eight rows, start, end, size, what goes in
   it) and the seven-question placement order, lifted from the note. It gives the reasons for the numbers in two or three sentences each (the 2 GiB code-model ceiling, the stack above the image,
   the process pages beside the stack). Every nife path, section, milestone, pull request and
   person's name goes; every `Name:` block goes; the two "Was ..." sentences go. The note in nife
   keeps the long form and links the crate.
   Door: two-way. Effort: M.

5. No README, licenses, changelog or scaffold.
   Finding: the crate directory holds `Cargo.toml`, `src/` and `falsifications/`. The template
   requires a README with `## EXAMPLES` and `## BUGS` in the scaffold's section order, `LICENSE-MIT` and `LICENSE-APACHE`, a `CHANGELOG.md` and `_typos.toml`. It also requires the nine caller workflows,
   `.github/mutants-exempts`, `.github/ci-job-budgets`, `.github/accepted-words` and a
   `docs/stranger-test.md` whose first recorded run gates the first release.
   Evidence: the crate's file list; `scaffold/README-crate.md` and `helpers/standalone.py` in the
   template.
   Change: scaffold the repository from the template. The README's EXAMPLES section shows a pair
   page placed in a `const`, a `check_image` refusal and its printed sentence. The "What is proven" section lists the nine harnesses in a user's words. The "Building and testing" section names the three bare-metal targets the `ci.yml` caller lists. The "Running the proofs" section gives the Kani install line on one line, the measured time (under ten seconds for nine harnesses on one machine here) and the paragraph on what a passing run prints that looks alarming. BUGS carries the three entries from the crate root
   plus the one in item 12. `.github/mutants-exempts` carries the one equivalent mutant with its
   reason (item 11). Then record a stranger run.
   Door: two-way. Effort: M.

6. Parameter and helper names the template refuses.
   Finding: the parameter `va` appears in `Band::contains`, `pair_page`, `runtime_window`,
   `service_window`, a test binding and four verification helpers; `va` is on the spelled-out list
   (`virtual_address`), and the rule covers parameters. The verification helper `allowed` returns
   `bool` and is not shaped as a question.
   Evidence: `src/lib.rs` lines 57, 171, 182, 196 and 430; `src/verification.rs` lines 29, 40, 85 and 91; the
   `spelled_out.py` (9 refusals) and `question_shape.py` (1 refusal) runs above.
   Change: `va` becomes `virtual_address` at every site, `allowed` becomes `is_allowed`. A parameter
   name is not part of the Rust API, so no caller changes.
   Door: two-way. Effort: S.

7. Constants that hold an address or a length do not say so.
   Finding: `PAGE` is a length in bytes. nife's crate rule spells a page contract's constants
   `PAGE_BYTES` and `PAGE_VIRTUAL_ADDRESS`. `IMAGE_BASE`, `STACK_TOP`, `STACK_TOP_PAGE`,
   `CURRENT_CPU_PAGE` and `CODE_MODEL_CEILING` are addresses. The rule says an address is
   `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS` in every location. `ImagePlacement`'s fields
   `image_start`, `image_end` and `stack_base` are addresses too. The template's `register_names.py`
   does not catch any of them because it reads only names that contain the word `address`, so this
   is a reading of `design/naming.md`, not a gate failure. These are public names with 90 call
   sites in nife (`IMAGE_BASE` 42, `PAGE` 23, `STACK_TOP_PAGE` 20, `CURRENT_CPU_PAGE` 2,
   `CODE_MODEL_CEILING` 2, `STACK_TOP` 1). `MAXIMUM_STACK_PAGES` has a ratified name and is a count, so it passes. The eight `Band` constants name bands, not addresses, and pass.
   Evidence: `src/lib.rs` lines 39, 70, 144, 149, 154, 165 and 216 to 226.
   Change: see decision 2 below. If ruled, the rename lands in the move pull request, where every
   caller's manifest changes anyway, with `script/names` enumerating the sites first.
   Door: one-way (public names). Effort: M across nife, S in the crate.

8. The crate's name uses a word the vocabulary rule sets aside.
   Finding: `address_space_map` carries `map`, which `design/naming/vocabulary.md` lists among the
   words to avoid where the kernel meaning differs from yours. The crate's own root doc uses the
   word in both senses in one paragraph: "the map" (the table of bands) and "pages the kernel maps
   into every process" (the verb). The crate root marks the name provisional and records that the minting set `user_layout` and `address_space` aside. The block does not give the reasons (unverified).
   Evidence: `src/lib.rs` lines 1, 18 and 124; `design/naming/vocabulary.md`.
   Change: see decision 1 below. A crate name becomes the repository name and the crates.io name
   at the first tag, so the move is the last cheap moment.
   Door: one-way. Effort: M across nife (26 manifests, 281 sites), S in the crate.

9. The band functions check a window's first page only.
   Finding: `pair_page`, `runtime_window` and `service_window` take one address and check that one
   page. A multi-page window whose first page sits near a band's top runs into the next band
   unseen: `service_window(0x5FFF_F000)` passes, and an 8 MiB window there would end in
   `IMAGE`. The crate's own BUGS and the note both record this. `Band::holds(start, end)` is
   already the whole-range check, and `std_runtime_contract` calls it directly for that reason.
   Evidence: `src/lib.rs` lines 11 to 14 and 171 to 206; `crates/std_runtime_contract/src/lib.rs` line 272;
   `notes/address-space-map.md`, BUGS, first entry.
   Change: see decision 3 below. The three one-page functions stay for their 222 call sites.
   Door: one-way (a new public name). Effort: S in the crate.

10. Five `const` assertions raise a future-incompatibility warning under Kani's toolchain.
    Finding: `const _: () = assert!(...);` puts a macro with a trailing semicolon in expression
    position. Stable rustc and clippy are silent; the nightly Kani drives prints
    `semicolon_in_expressions_from_non_local_macros` five times, "will become a hard error in a
    future release". A stranger running the proofs for the first time reads five warnings against
    the crate's own lines.
    Evidence: `src/lib.rs` lines 339, 340, 341, 342 and 344; the Kani run above.
    Change: wrap each in a block, `const _: () = { assert!(...) };`, as the tiling assertion at line
    298 already is.
    Door: two-way. Effort: S.

11. The one surviving mutant needs its exemption to travel.
    Finding: nife's census records one survivor, `check_image`'s `end > IMAGE.end` to `>=`, triaged
    equivalent: when `start` is inside the band and `end == IMAGE.end`, `holds` has already returned
    `Ok`, so the second branch never sees that boundary. Reading the function confirms the triage.
    The template's `mutation` verdict fails a survivor unless `.github/mutants-exempts` lists it
    with a reason, and the exemption lives in nife's `notes/project-metrics/mutation-triage.csv`,
    which does not move.
    Evidence: `src/lib.rs` lines 273 to 287; `notes/project-metrics/mutation-triage.csv`, the
    `address_space_map` rows.
    Change: one line in `.github/mutants-exempts` with the reason above. The alternative, rewriting
    `check_image` so the mutant is not equivalent, changes nothing a caller sees and trades a clear
    three-way `if` for a less legible one; not recommended.
    Door: two-way. Effort: S.

12. The map is the same in every process, and nothing says so to a stranger.
    Finding: every band and every fixed window sits at one address in every process on every
    architecture. That is the design (one layout for three architectures, compile-time checked
    windows), and it means there is no address-space layout randomization. A reader arriving from
    Linux, FreeBSD or Fuchsia, all of which randomize, will ask. The crate does not answer.
    Evidence: `src/lib.rs` lines 1 to 9; the BUGS at `src/lib.rs` lines 11 to 16.
    Change: one BUGS entry in the crate root and the README. It says the design fixes the layout, what that buys (a pair page checked at compile time, one linker script), and what it does not do.
    Door: two-way. Effort: S.
