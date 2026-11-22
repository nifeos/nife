# Review package: `terminal_contract`

*Name: provisional, minted by lane lane/line-editor-review-packages. This is the
`terminal_contract` package of [crate-reviews.md](../crate-reviews.md). It has its own file because
the packages together outgrow the page. Its twin is [`line_editor`](line-editor.md), the engine
this crate's protocol serves.*

The architect reads this to clear the crate for its move to its own repository. It describes the
crate as `main` leaves it at `97d16ae8b`. The crate came out of `line_editor` by decision 1 of
[milestone 1098 (`line_editor`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1098-line-editor-outside-review.md),
which holds the rulings this package relies on. No outside review of this crate alone exists, so
the findings below come from this package's own reading of the source and the template's checks.

## In one paragraph

The opcodes, flags, badges and reply words of the terminal protocol, which the terminal, its
clients, its supervisor and the kernel's tests speak over a rendezvous. A request is a `CALL` whose
first word carries an opcode in bits 63 to 56 and a length or count in bits 31 to 0. Short payloads
travel packed little-endian in the second word, and longer ones in a page the client already maps.
Four badge values gate which operations a copy of the endpoint may send, and `admits` is the one
table of that gate. A supervisor quiesces a terminal on its own badge, and three control words and
four notes carry a live replacement. The crate is `no_std`, has no dependency and no `unsafe` site,
and five unit tests pin the values.

## Identity

| item | value |
|---|---|
| crate | `terminal_contract`; ratified by decision 5 of milestone 1098, but its `Name:` block still reads provisional (see What still blocks the move) |
| identity values | none yet. Decision 4 of milestone 1098 rules an interface number and version 1, with no magic, and leaves minting the number to the architect. The next unused number in the id table of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md) is 25: rows 1 to 24 are taken, 3 retired, no `INTERFACE_ID` in the tree exceeds 24, and no open pull request edits the table |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, none in the manifest (the move writes it) |
| license | inherited from the nife workspace; no license file in the crate (the move writes both) |
| `publish` | `false` |
| dependencies | none, of any kind |
| size | `src/lib.rs` 269 lines and `src/tests.rs` 118 lines |
| files | 3 tracked; no README and no falsification patch |
| listed for the move | `packages/contracts.package.toml` names it; `crates/externalization-list.txt` does not, and the order of [milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md) has no row for it |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency | `components`, `crates/system_initializer`, `fixtures`, `kernel` (`kernel/src/user/terminal_print.rs` and `display_service.rs` pack `OPERATION_WRITE` and `OPERATION_BYTES` requests); `crates/std_runtime_contract` and `system_tests` as development dependencies |
| prose and tables | `packages/contracts.package.toml`, `helpers/trust_boundary_crates.tsv`, `notes/terminal-contract.md` (the protocol's design note) |

46 Rust and manifest files outside the crate name it. The `interfaces_note` check
(`helpers/interface_declarations_check.py`) does not list it in `OWED` or `NOT_A_FORMAT`, because no
program of the first cut's three packages links it.

## What was run

All commands ran in this lane's worktree, at `main` with this lane's edits to documents only.

| check | result |
|---|---|
| `script/crate-review-packet terminal_contract` | no dependency, no type from another crate in a public signature, no `unsafe` site, no Kani harness; nine paths into the nife tree, seven section citations, nine milestone citations and three lines naming the architect in shipped text |
| `script/crate-standalone terminal_contract` | 13 failures: the crate name and 33 items not ratified, inherited `license` and `lints`, no `rust-version`, no README or license files, the four citation counts above, and no Kani harness. The build passes on the host and on the three bare-metal targets on stable |
| `cargo test -p terminal_contract` | 5 unit tests pass; no doctest |
| `cargo clippy -p terminal_contract --all-targets -- -D warnings` | no warning |
| `cargo doc -p terminal_contract --no-deps` with warnings denied | no warning |
| `script/names terminal_contract` and `python3 helpers/name_release.py --draft terminal_contract` | `NOTE_NOT_ABSORBED` ratified; the crate and 12 items provisional; 20 items with no block |
| the template's gating name checks (`spelled_out`, `unit_symbols`, `register_names`, `british_spelling`, `service_model`, `contract_names`, `question_shape`, and `service_model --prose`), from `nifeos/crate-workflows` at `56437de` | all pass, 52 names judged |
| the template's `abbreviations` report | 14 components: `readline`, `intrcount`, `rawmode`, `readraw`, `stdin`, `quiesced`, `ctl` (three names), `handoff` (two), `req`, `eof`, and `proto` in a test name |

Not run, and so not claimed: the stranger test (there is no README), `cargo-mutants`, Kani (no
harness exists), Miri, `cargo package` and `cargo-semver-checks`. No repository exists, so the
template's workflows did not run. The QEMU-side tests that speak the protocol (`system_tests`) did
not run in this lane.

## Public API

| kind | items |
|---|---|
| the first word | `OPERATION_SHIFT` (56), `req(operation, len)`, `operation(w0)`, `len(w0)` |
| opcodes | `OPERATION_WRITE` 1, `OPERATION_READLINE` 2, `OPERATION_BYTES` 3, `OPERATION_INTRCOUNT` 4, `OPERATION_PRINT` 5, `OPERATION_RAWMODE` 6, `OPERATION_READRAW` 7, `OPERATION_QUIESCE` 8, `OPERATION_SUSPEND_COUNT` 9, `OPERATION_READ_LINE_BYTES` 10 |
| badges | `RAW_ONLY_BADGE` 1, `STDIN_BADGE` 2, `SUPERVISOR_BADGE` 3, and `admits(badge, operation)` |
| read replies | `FLAG_EOF`, `FLAG_INTERRUPTED`, `FLAG_RETRY`, `is_retry(r0, r1)`, `BAD_REQUEST` (`u64::MAX`) |
| replacement | `QUIESCED` ("QUIT"), `CTL_RESUME`, `CTL_QUIT`, `CTL_HANDOFF`, `START_HANDOFF`, `START_ABSORB`, `NOTE_ABSORBED`, `NOTE_NOT_ABSORBED`, `NOTE_WRITTEN`, `NOTE_NOT_WRITTEN` |

No nife type appears in a signature. What a stranger may find surprising:

- The first word's bits 55 to 32 are reserved, and the documentation says a client sends zero there, but nothing refuses a word
  that sets them. `operation` shifts them away and `len` masks them off, and the two servers
  (`components/src/line_editor.rs` and `display_terminal.rs`) decode with those two functions alone.
  A client that sets a reserved bit today works, so a later version cannot give those bits a
  meaning without breaking it.
- The documentation of several opcodes names programs a stranger does not have (`program_table`,
  `terminal_supervisor`, `login`) and types from other nife crates (`moho_abi::rendezvous::BADGE`,
  `grant_contract::Manifest::console_input`, `std_runtime_contract::STDIN_SLOT`,
  `line_editor_component::HANDOFF_VA`). None appears in a signature.
- `START_HANDOFF` and `START_ABSORB` are bits of `line_editor`'s third start argument, and
  `QUIESCED` equals `hot_swap_contract::QUIESCED` by a compile-time assertion in the `line_editor`
  program, not in this crate.
- The crate root carries no `#![forbid(unsafe_code)]`, which `line_editor` carries.

## What landed

| item | state | evidence |
|---|---|---|
| Decision 1 of milestone 1098, the cut | landed ([#3587](https://github.com/nifeos/nife/pull/3587)) | the crate exists with no dependency; the kernel and the protocol's clients depend on it, not on `line_editor` |
| Decision 4, an interface identity | ruled, not built | no `INTERFACE_ID` or `VERSION` in `src/lib.rs`; the number is not minted |
| Decision 5, the name | landed ([#3965](https://github.com/nifeos/nife/pull/3965)) | the crate root's `Name:` block records the ratification |
| Item 4, the crate's own name worklist | open | the 14 components the `abbreviations` report lists |
| The externalization list | open | `crates/externalization-list.txt` names `line_editor` and not this crate |

## Names

The crate name carries decision 5's ratification, in milestone 1098 and at the crate root.
The architect ratified `NOTE_NOT_ABSORBED`. Everything else below is provisional or has no block. This lane
ratifies nothing; the architect ratifies, changes or refuses each row. The proposals in the third
column follow rulings already in force and are not built.

| item | status | rule, reason or proposal |
|---|---|---|
| `INTERFACE_ID`, `VERSION` | not built | the identity pair every contract carries; the architect ratified these two names for `block_roster_contract` |
| `req` | no block | `byte_sink_contract` renamed its `req` to `first_word` for the same packing; the same name here keeps the two contracts alike |
| `OPERATION_READLINE`, `OPERATION_RAWMODE`, `OPERATION_READRAW` | no block | run-together words; `OPERATION_READ_LINE`, `OPERATION_RAW_MODE`, `OPERATION_READ_RAW` match `OPERATION_READ_LINE_BYTES` beside them |
| `OPERATION_INTRCOUNT` | no block | an abbreviation; `OPERATION_INTERRUPT_COUNT` matches `OPERATION_SUSPEND_COUNT` |
| `CTL_RESUME`, `CTL_QUIT`, `CTL_HANDOFF` | no block, `CTL_HANDOFF` provisional | `CONTROL_` spells out the endpoint they travel on |
| `STDIN_BADGE` | provisional | `stdin` is the standard library's spelling (`std::io::stdin`), which the rule of 2026-11-03 keeps |
| `FLAG_EOF` | no block | the standard library's spelling, kept for `byte_sink_contract` and `line_editor` alike |
| `OPERATION_QUIESCE`, `SUPERVISOR_BADGE`, `admits`, `RAW_ONLY_BADGE`, `OPERATION_SUSPEND_COUNT`, `OPERATION_READ_LINE_BYTES`, `START_HANDOFF`, `FLAG_RETRY`, `NOTE_WRITTEN`, `NOTE_NOT_WRITTEN` | provisional | plain words |
| `OPERATION_SHIFT`, `OPERATION_WRITE`, `OPERATION_BYTES`, `OPERATION_PRINT`, `START_ABSORB`, `NOTE_ABSORBED`, `QUIESCED`, `FLAG_INTERRUPTED`, `BAD_REQUEST`, `is_retry`, `operation`, `len` | no block | plain words; `len` is the standard library's spelling |

Every value above is wire: renaming a constant changes no value, so the renames are two-way until
the first tag even though the values are not.

## What still blocks the move

| item | owner |
|---|---|
| The architect mints the interface number (decision 4), and a lane adds `INTERFACE_ID` and `VERSION` 1, the row in milestone 956's table, and the declaration in every program that carries an `INTERFACES` note and links the crate. | architect, then a lane |
| The crate root's `Name:` block records decision 5's ratification. | the lane that records milestone 1098's rulings |
| Add the crate to `crates/externalization-list.txt` and give it its place in milestone 1017's order. | maintainer |
| README in the template's section order, both licenses, literal `license`, `lints` and `rust-version`, the manifest rows, the `Name:` strip, the citations of this tree, and the three lines that quote the architect. | move lane |
| Convert the six dependents to `git` and `rev` with `version = "0.1.0"`, the kernel included, and read every file `script/crate-move --dry-run` lists. | move lane |
| Run the stranger test and the template's workflows in the new repository. | move lane |

## Recommendation

Ready for review, not yet ready to move. The crate is a dependency-free leaf with no `unsafe` and no
nife type in a signature, and the template's gating name checks pass. The move waits for its
interface number, its place on the externalization list and milestone 1017's order, and the
reserved-bit question below, which changes what the wire promises.

## Open questions

- No Kani harness covers the crate. `admits` and the first word's packing are small enough for an
  exhaustive harness over the opcode byte and the four badges, with a falsification patch each, as
  `byte_sink_contract` carries. Two-way; the maintainer can rule it.
- The documentation that names nife programs and other crates' items states context a stranger
  cannot check. The move lane rewrites it as the protocol's own rule or links
  `notes/terminal-contract.md`'s successor in the new repository.

## Decisions for the architect

1. The interface number. Recommended: A.
   - A. 25, the next unused row of milestone 956's table, at version 1 with no magic.
   - B. Another number the architect chooses for a reason this package does not see.
2. Whether a server refuses a first word with a reserved bit (55 to 32) set. One-way once a client
   outside nife builds against the published crate. Recommended: A, the stricter wire, which
   `entropy_contract` took in decision 2 of milestone 1071 (`entropy_contract`: what an outside
   maintainer would fix before 0.1.0): its service answers a request word with a reserved bit
   set by refusing it.
   - A. Refuse it with `BAD_REQUEST`, and decode through one function that checks the bits.
   - B. Keep ignoring the bits, and state that a future version may not give them a meaning.
3. Clear the review, which is the go to create `nifeos/terminal_contract` and move the crate
   (one-way: the template's workflows and nife's pin act on the repository from its first day).
   Recommended: A.
   - A. Clear it now; the move waits for a lane to build decisions 1 and 2, and for the crate's place on
     the externalization list and milestone 1017's order.
   - B. Hold the clearance until a lane builds the identity, and review the crate with it.
4. Ratify the Names table, in one pass or row by row, including the proposed renames of `req`, the
   run-together opcodes, `OPERATION_INTRCOUNT` and `CTL_`. Two-way before the first tag; one-way
   after it.

Nothing here asks for a tag or a publication.
