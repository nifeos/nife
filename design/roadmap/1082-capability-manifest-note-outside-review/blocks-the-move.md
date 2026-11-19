# P0: blocks the move

An appendix to [the milestone](../1082-capability-manifest-note-outside-review.md).

## P0: blocks the move

1. The manifest cannot stand alone.
   Finding: `license.workspace = true` and `[lints] workspace = true` inherit a workspace the new
   repository does not have, so `cargo generate-lockfile` fails in the staged tree. There is no
   `rust-version`, no `description`, no `repository`, no `readme`, no `keywords`, no `categories`.
   The `publish = false` comment sends the reader to `crates/publish-allowlist.txt`, a file outside
   the repository. `domain_name_resolution_contract` is a path dependency with no `version`, so the
   move cannot rewrite it to a `git` or crates.io line. The crate carries no `LICENSE-MIT` or
   `LICENSE-APACHE`.
   Evidence: `crates/capability_manifest_note/Cargo.toml` lines 5, 6, 8, 9, 16; the dry run's
   `build` and `standalone` failures (10 lines).
   Change: write `license = "MIT OR Apache-2.0"` and the lint table as literals, add
   `rust-version`, `description` (one sentence: the ELF note a program carries to ask for authority,
   and its decoder), `repository`, `readme = "README.md"`, `categories = ["no-std",
   "no-std::no-alloc", "parsing"]`, `keywords`. Give every dependency a `version`. Copy the two license files. Replace the `publish` comment with one sentence a stranger can act on. The
   dependencies are the organization's own, approved in advance.
   Door: the license is one-way and already ruled for the organization's crates; the rest is
   two-way. Effort: S.

2. There is no README.
   Finding: the dry run stages the scaffold's shape and fails on it. The module documentation is
   the only description, and it opens with a project-internal citation (item 5).
   Evidence: `crates/capability_manifest_note/` holds no `README.md`; dry run `readme: FAIL`.
   Change: a README in the scaffold's sections. It says what a manifest note is in one paragraph. It lists the three note types and the three descriptor versions as the module tables already give them. It shows how to carry one (`carry!`) and how to read one (find the `PT_NOTE` with owner `OWNER` and type `MANIFEST` with any ELF reader, then `decode`, `zones`, `service`). It covers building and proving (`cargo test`, `cargo kani`, the three commands around a falsification patch) and the no_std and no-allocation facts. It states the refusal policy: the reader refuses a later version and does not skip it. Then
   record a stranger run in `docs/stranger-test.md`.
   Door: two-way. Effort: M.

3. The encoders accept two manifests the decoder refuses.
   Finding: the crate makes three claims. The compiler builds a program's note from a constant. `encode*` panics (a compile error) on what `decode` refuses. A reader therefore never meets a note its own program could not have carried. Two holes, each probed:
   (a) `encode_serviced` never calls `domain_name_resolution_contract::is_canonical_zone`. Its
   documentation says it panics on "a zone the resolver would not spell"; it does not.
   `encode_serviced(&m, &["Example.NET"], true)` and `encode_serviced(&m, &["example.net."],
   false)` both evaluate at compile time, and `decode` of each returns `Err(BadField(57))`.
   `encode_zoned` has the check (line 649) and panics on the same input. A zone longer than 255
   bytes also writes `zone.len() as u8` silently truncated (line 932). The canonical check's 253-byte bound guards `encode_zoned`; `encode_serviced` has no such guard.
   (b) `encode` writes `MemoryGrantDeclaration::Required { minimum, maximum }` without checking
   `minimum <= maximum`; `decode_mem` refuses the result. `encode(&Manifest { memory_grant:
   Required { minimum: 8, maximum: 4 }, ..NO_NOTE_MANIFEST })` evaluates, `Note::of` of it builds
   a 76-byte note, and `decode` returns `Err(BadField(8))`. `grant_plan` places no invariant on
   the pair (its `MemoryGrantDeclaration` is a plain enum with two public `u64` fields), so a
   program source can state it.
   Why it matters: a program built with `carry!` from either manifest compiles, links and ships
   a note that the progenitor and the shell refuse at run time, with no diagnostic at build time.
   Nothing gains authority it should not have (the readers fail closed), so this is not a security finding. It is the crate's central guarantee failing in one direction, and the Kani proof
   covers only the other (`decode` then `encode`).
   Evidence: `src/lib.rs` lines 900 to 947 against 630 to 669 (`is_canonical_zone` at 649 to 652
   has no counterpart in `encode_serviced`); lines 298 to 302 against 496 to 499; the probe
   results above.
   Change: one `const fn` that writes a zone block and checks each zone, shared by `encode_zoned`
   and `encode_serviced`, so the two cannot disagree again. Add `assert!(minimum <= maximum, ...)` in `encode` and a test for each. Add a Kani harness in the encoding direction, `any_manifest` built field by field with `kani::any` under the type's invariants (`ShortOptions::try_new`, a declared subtree letter, the slot). It asserts `decode(&encode(&m)) == Ok(m)`, and a falsification patch removes the new `minimum <= maximum` assertion. The harness is cheap:
   `encode` has no data-dependent loops beyond 16 letters.
   Door: two-way (a stricter encoder refuses only what the reader already refused). Effort: S for
   the fixes, M with the harness.
