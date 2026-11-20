# P0: blocks the move or any publish

An appendix to [the milestone](../1079-byte-sink-contract-outside-review.md).

## P0: blocks the move or any publish

1. The crate does not stand alone, and the documentation that explains it does not travel.
   Finding: there is no README, no `LICENSE-MIT` or `LICENSE-APACHE`, no `rust-version`. The manifest inherits `license` and `lints` from the workspace, and its `publish = false` comment points at `crates/publish-allowlist.txt`, a file that will not exist in the new repository. The
   crate root documentation sends the reader to notes/abi.md, notes/sink-protocol.md ("the module
   note", twice), §16 (object revocation: reclaim the objects a process built), §242, milestones 613 and 956, `filesystem_contract`, `line_editor::proto`, `filesystem_fixture`, `cargo xtask std-src` and two files under `fixtures/src/`. It also names the architect in four `Name:` blocks. The reasons for the shape (register-only rather than a
   shared page; `SEND` rather than `CALL`; no per-write error; what the protocol deliberately does
   not carry) live only in the note. A stranger holding the published crate cannot learn why the contract has its shape, and the template's `standalone` gate refuses every one of those
   references.
   Evidence: `crates/byte_sink_contract/Cargo.toml` lines 5 to 6, 8 to 13, 16 to 17, 20 to 26. `src/lib.rs` lines 4, 8, 10 to 11, 15 to 16, 22, 26, 32, 35, 58, 61, 66, 153 to 158, 162, 248, 278 to 281, 285, 291, 297. Also the `standalone` findings above, and the two `lint_externalized_crates` hits (`line_editor` at line 16, `b"nife"` at line 360).
   Change: copy the scaffold (README with the template's section order, both license files, `CHANGELOG.md`, `_typos.toml`, `deny.toml`, `.github/`). Write `license`, `rust-version` and the three lints as literals. Delete the `publish` line in the release that publishes, and until then give it a comment a stranger can act on. Strip the `Name:` blocks (ruled for every moved crate). Rewrite the crate root documentation as the specification. It carries the wire table and the two shape decisions with their reasons. It carries the `Gone` versus `NoSink` split with std's mapping (`BrokenPipe` versus `Unsupported`) as the worked example. It also lists what the contract does not carry, in the crate's own words with no path into this tree. Rename the test string at line
   360.
   Door: two-way (the license is already ruled). Effort: M.

2. The `VERSION` documentation specifies a bind-time wire check that does not exist and cannot
   exist in this shape.
   Finding: the comment says a writer `CALL`s `OPERATION_VERSION` and binds only when its own
   version is in the served set. No such opcode exists in this crate or anywhere in the tree. Proposal 1000 (message contracts carry bind-time version checks), not started, reserves opcode 0 for it "where the protocol leaves it free", and this contract gives opcode 0 to `OPERATION_BYTES`. A sink is `SEND`-only by the contract's own first paragraph, so there is
   nothing a writer could `CALL`, and the one receiver that would answer is a pipe's ordinary
   reader. The check that does exist today is static: the program's `INTERFACES` note carries
   (6, 1), and `interfaces_note` says the progenitor compares only the system call ABI's entry at spawn
   and carries the others. An outside implementer who trusts the comment reserves opcode 0 for a
   version reply and collides with every bytes message.
   Evidence: `src/lib.rs` lines 19 to 27 (the comment) and 46 (opcode 0 taken);
   `crates/interfaces_note/src/lib.rs` lines 4 to 7; `design/roadmap/1000-*.md` line 23.
   Change: the comment states what is true: a program's note declares the pair (`INTERFACE_ID`, `VERSION`). The contract has no wire operation for versions and, being one-way, cannot have one without a reply. A framing change bumps `VERSION`, and a sink that serves two versions does so by accepting both framings, which is what "serves N and N-1" means for a stream. Whether a
   wire version operation is ever added is decision A, decided provisionally: none.
   Door: two-way, decision A. Effort: S.
