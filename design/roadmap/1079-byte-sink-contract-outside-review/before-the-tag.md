# P1: before the tag

An appendix to [the milestone](../1079-byte-sink-contract-outside-review.md).

## P1: before the first release

3. The `fixture` module is this tree's test scaffolding in the public API.
   Finding: `pub mod fixture` exports `TRANSCRIPT`, `SINK_NAME` (the name of a file on a nife test
   image, `"sinkout"`), `READY`, `DONE`, `BAD` and `code`, documented against
   `fixtures/src/file_sink.rs`, `fixtures/src/sink_transcript_writer.rs`,
   `filesystem_contract::READY` and `filesystem_fixture`. Its consumers are three fixture programs
   and the kernel-side `sink_tests`, all in this tree. On docs.rs every reader meets them beside the
   wire; an alternative implementer has no use for them; and they are the only items in the crate
   that name nife files.
   Evidence: `src/lib.rs` lines 278 to 316; consumers `fixtures/src/sink_transcript_writer.rs`,
   `fixtures/src/file_sink.rs`, `fixtures/src/file_source.rs`, `system_tests/src/user/sink_tests.rs`
   lines 137 to 181.
   Change: move the module to a crate of its own in this tree, `byte_sink_fixture` (provisional),
   depending on `byte_sink_contract`, the shape `filesystem_fixture` already has; the four
   consumers change one `use`. The alternative, a cargo feature `fixture` defaulting off, keeps the
   code in the published crate and the nife file names in its documentation. The fixture crate stays
   in this tree: it is neither publishable nor used by a second repository.
   Decision B decides the separate crate, provisionally.
   Door: two-way before any publish, decision B. Effort: S.

4. The `abi` dev-dependency binds the move order to another repository for one assertion.
   Finding: `GONE` restates `abi::Error::Gone`, and the only test that pins the two together links
   `abi` as a dev-dependency. Milestone 1017 (the contract crates leave in this order) therefore sequences this crate after `abi`, and after
   the move the dev-dependency becomes a pin on a published `abi`. The assertion is about two crates
   nife links; nife already links both in `system_tests` and the kernel.
   Evidence: `Cargo.toml` lines 18 to 26; `src/lib.rs` lines 322 to 329; milestone 1017's table row
   for this crate.
   Change: move `the_restated_gone_code_is_the_kernels` to `system_tests` (or any nife crate that
   links both), leaving this crate with no dependency of any kind. The crate may then leave in any
   order relative to `abi`, and the `GONE` documentation says only that the value is the kernel's
   "object no longer exists" return, restated here so a writer needs no second crate. A dependency
   on the published `abi` is already approved for an organization crate, so the alternative is open. It costs a version bump here for every `abi` release and buys nothing an in-tree test does not.
   Door: two-way. Effort: S.

5. No proof, and the README the template requires has a "What is proven" section.
   Finding: the crate has no Kani harness. The decoder is total by construction (the fuzzing note
   says so and declines a fuzz target for that reason), which is exactly the kind of claim a
   harness states cheaply. The five properties below come from harnesses written and verified against the standalone copy; see the figures in "What was checked".
   Evidence: `src/lib.rs` (no `cfg(kani)`); `notes/fuzzing/not-fuzzed.md` line 12;
   `scaffold/README-crate.md` in the template.
   Change: a `proofs` module behind `cfg(kani)` with five properties. First, any prefix of up to sixteen symbolic bytes survives `pack` then `unpack` with `taken` equal to its length. Write it as a concrete loop over the seventeen lengths under `#[kani::unwind(18)]`, since a symbolic length does not finish (see "What was checked"). Second, `unpack` never returns `Bytes(n)` with `n` above `INLINE_MAXIMUM` for any three words. Third, `with_severity` then `severity` reads back the clamped level and leaves the count, opcode and kind unchanged for any first word. Fourth, `classify` is `Gone` exactly for `GONE` and `Ok` exactly for a non-negative return. Fifth, `eof()` unpacks as `Eof` for any second and third word and equals no bytes message of any length. Each with a falsification
   patch, as the template's `falsify` job requires. The `std-src` generator must keep stripping the
   module or ignoring it: a `#[cfg(kani)]` module is inert in std, and the generator already
   keeps `#[cfg(doctest)]` items for the same reason.
   Door: two-way. Effort: S.

6. The crate promises the `kind` byte as the extension point, and the decoder ignores it.
   Finding: `KIND_SHIFT`'s documentation says a later structured body "arrives as a new kind rather
   than a new framing". `unpack` returns `Bytes(n)` for any kind. Every sink that decodes through it (the terminal adapter, the shell's two readers, `wc`, std's stdin, the benchmark contract) would print a future structured body as text. Only `system_log` reads `kind` first and
   refuses another kind; it does so outside `unpack`. So the extension point exists for one
   consumer and is a silent misread for the rest.
   Evidence: `src/lib.rs` lines 162 to 169 and 232 to 246; `crates/system_log/src/log.rs` line 113;
   `components/src/terminal_sink_caretaker.rs` line 50; `components/src/swish/running.rs` line
   1258; `components/src/wc.rs` line 137; the std PAL `sys/stdio/nife.rs` line 120.
   Change: `unpack` returns `Malformed` for a bytes message whose kind is not `KIND_TEXT`. Every
   writer today sends zero there, so nothing on the wire changes and every existing test still
   passes; `system_log`'s own check becomes redundant and may stay. The alternative, `Msg::Bytes {
   len, kind }`, makes every consumer decide, which is the misread by another route. Refusing now
   and relaxing later is compatible; the reverse is not.
   Door: two-way before the release (decoder behavior, no wire change). Effort: S.

7. A British spelling fails the template's gate at a ceiling of zero.
   Finding: the British spelling of "neighbors" appears in a test function's name, and the British
   spellings of "authorize" and "neighbor" appear in prose. The house-style check refuses them in new text.
   Evidence: `src/lib.rs` lines 207, 442 and 454.
   Change: `neighbors`, `authorize`, `neighbor`.
   Door: two-way. Effort: XS.

8. The contract leaves the end-of-stream message's second and third words undefined, and one program uses them.
   Finding: the contract says an `OPERATION_EOF` first word has a zero count and says nothing about `w1` and `w2`. `unpack` discards them. `rm` sends its exit status and its removal count in them,
   and `filesystem_contract::rm` documents that as `rm`'s report format. Every generic reader drops
   them: the shell's pipeline reader and its direct reader return on `Eof` without looking, so past a
   `|` the verdict reaches nobody; only the kernel-side filesystem fixture tests read it. An
   alternative sink implementer cannot tell whether to preserve, ignore or refuse non-zero words
   there.
   Evidence: `src/lib.rs` lines 48 to 55, 87 to 90, 243; `components/src/rm.rs` lines 327 to 337;
   `crates/filesystem_contract/src/rm.rs` lines 1 to 8; `components/src/swish/pipeline.rs` line
   843; `components/src/swish/running.rs` line 1260; `crates/filesystem_fixture/src/tests.rs`
   lines 498 to 506.
   Change: the contract states what the two words are, decision C. Under the recommended ruling the documentation says a writer sends zero and a sink ignores them. Either `rm` records its use as a private agreement between `rm` and its test reader that this contract does not cover, or the verdict moves to a channel of its own.
   Door: one-way (wire format), decision C. Effort: XS for the text; S if `rm` changes.

9. Names that a later rule reopens at release review.
   Finding: `INLINE_MAXIMUM` is a limit without its unit, and the units rule says a limit opens
   `MAXIMUM_` and ends with its unit. The name predates that rule, and a release review rechecks such names. `req` is not a request (a `SEND` has no reply) and the abbreviations report
   flags it. `Msg` is an abbreviation the same report expands. `ret` is a parameter name the naming
   rules cover. `w0`, `w1`, `w2` are the ABI's own words for the three message registers.
   `OPERATION_EOF`, `eof` and `Msg::Eof` name a file where the thing is a stream, but `Eof` is std's
   spelling (`ErrorKind::UnexpectedEof`).
   Evidence: `src/lib.rs` lines 62, 73, 174 to 197, 268; `design/naming.md`, "Units" and "Shapes";
   the `abbreviations` report in "What was checked".
   Change: as decision D decides, provisionally: `MAXIMUM_INLINE_BYTES`, `first_word`, `Message`,
   `returned`, the rest kept. One rename lane applies it across every caller, with the std PAL
   regenerated.
   Door: two-way before the first tag, decision D. Effort: S.

10. The documentation and the decoder disagree about a zero-length bytes message.
    Finding: `OPERATION_BYTES`'s documentation says the count is 1 to `INLINE_MAXIMUM` and the note
    says `len = 1..=16`, while `unpack` accepts 0 and a test asserts that `pack(&[])` unpacks as
    `Bytes(0)`. std's stdout path calls `pack` on whatever slice the caller supplies, so an empty write would send one.
    Evidence: `src/lib.rs` lines 38, 232 to 242, 385 to 394; `notes/sink-protocol.md` line 77.
    Change: the documentation says 0 to `INLINE_MAXIMUM`, and that a zero-count bytes message is legal and a sink treats it as nothing. That is `write(2)`'s rule for a zero-byte write and what every decoder in the tree already does. Refusing it would turn an empty `write` into a broken
    stream.
    Door: two-way (states the decoder's existing behavior). Effort: XS.
