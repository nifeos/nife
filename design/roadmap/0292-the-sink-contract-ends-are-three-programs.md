---
status: BUILT
raised: 2026-09-14
built: 2026-09-14
---
# 292. `fixtures/src/sink.rs` is three programs wearing one name

Built 2026-09-14. calef ruled the split; the three names are **provisional** and gathered
below for him to rule together. *(Number provisional until the merge queue lands it.)*

One binary dispatched three roles on `arg0` (`ROLE_WRITER`, `ROLE_FILE`, `ROLE_VERIFY`) and was
packed into both archives as `sink`. It is three programs now, and all three stay in `fixtures/`:

| was | is (provisional) | where | why there |
|---|---|---|---|
| `ROLE_WRITER` | **`sink_transcript_writer`** | `fixtures/` | writes a pinned transcript so a test can assert a classification by value |
| `ROLE_FILE` | **`file_sink`** | `fixtures/` | exercises the contract's receiving end against a real image; `>` at a prompt is `fs_file_caretaker` |
| `ROLE_VERIFY` | **`file_source`** | `fixtures/` | reads the file back so `<` and `|` can be compared |

This is the third application of calef's 2026-09-14 ruling, in his own words: *"I don't understand
why we would want to write a program that does multiple things. Part of the beauty of Unix that I
think we want to retain is small programs with specific functions."* One concurrent lane is splitting
`components/src/ntp.rs` and another `fixtures/src/hello.rs`'s thirty-one roles, and he asked for this
case to have its own milestone rather than being folded into the second. Neither of those lanes had
landed when this block was written, so their milestone numbers are not cited here; the integrator
can wire them at merge, and this note is what says a pointer is missing rather than absent.

## The argument is not that three programs are tidier

**It is that a dispatch number nobody can get wrong is one that does not exist.**

The role numbers were three hand-maintained copies of a fact two binaries had to agree on, which
is what AGENTS.md rule 7 exists to prevent, and all three were invisible to `script/lint` check 5
because none of them was a `#[path]` module:

| file | what it declared |
|---|---|
| `fixtures/src/sink.rs:86-88` | `ROLE_WRITER = 0`, `ROLE_FILE = 1`, `ROLE_VERIFY = 2` |
| `kernel/src/user/sink_tests.rs:9` | `ROLE_WRITER = 0` |
| `kernel/src/user/fs_service.rs:1272-1273` | `SINK_ROLE_FILE = 1`, `SINK_ROLE_VERIFY = 2` |

Three sites, six constant definitions, and the split deleted every one. Counted by grepping the
values and not only the names, because a bare `arg0: 1` would not have shown up in a search for
`ROLE_`; there were none. (`components/src/disk_partitioner.rs` and
`fixtures/src/os_primitives_benchmarker.rs` have their own role numbers. They are different programs
with the same defect and are not this milestone's.)

`fs_service.rs`'s copy carried its own warning, which is the tell that the tree already knew this was
too low a rung:

> `fixtures/src/sink.rs`'s roles. Kept in sync with that file by name and by this comment; a
> mismatch spawns the wrong role and hangs, which is why they are named here rather than spelled as
> bare integers at the two call sites.

A comment asking the next person to remember is rung four of AGENTS.md's ladder. Three programs are
rung one: the wrong state is unrepresentable, because there is no number to get wrong. The failure it
guarded against was real and nasty (a mismatch hangs rather than fails, since the wrong role
blocks on a rendezvous nobody is serving), and a hang in a kernel test boot is diagnosed from a
timeout rather than from an assertion.

A second duplicate went with them, unasked. The single binary needed two report-slot constants,
`REPORT_WRITER = 1` and `REPORT_FS = 2`, because the writer role held no FS endpoint and so its
report sat one slot lower than the other two roles'. Each program now has one `REPORT`, at the slot
it actually uses.

## Where each one landed, and why all three stayed in `fixtures/`

Milestone 175 spent 363 files drawing the line between `components/` (what a distribution ships
because somebody wants its function) and `fixtures/` (what exists to exercise the system), so the
directory is part of the ruling rather than tidying afterwards.

The brief said to check whether `ROLE_FILE` belongs in `components/`, on the grounds that
`kernel/src/user/fs_service.rs` spawns it "in the live path". That premise is false, and checking it
was the point of asking. `fs_service.rs` carries its own answer at the module declaration in
`kernel/src/user.rs`:

> `#[cfg_attr(not(test), allow(dead_code))] // spawned only by the phase-2 test`

and both entry points are attributed the same way. `start_file_sink` and `start_file_source` are
called from exactly one file, `kernel/src/user/sink_tests.rs`, and from nowhere else in the tree.

The live `>` path is `components/src/fs_file_caretaker.rs`, which is a different program and a
different shape: it **narrows** `filesystem_proto` for a client that speaks `filesystem_proto`, where
`file_sink` **translates**, speaking the sink contract to its client and `filesystem_proto` to the FS
server. `file_sink` is the fixture that proves the translation works against a real RedoxFS image.
Nothing at a prompt builds one, which notes/pipes.md has recorded since milestone 50.

So: three fixtures, and the directory question is answered rather than assumed.

## The names, and what was refused

Each program's own header carries its provenance block and its refusals (`script/names`), which is
where a reader meets them. Gathered here because calef rules them together:

- `sink_transcript_writer` (was `ROLE_WRITER`). Refused `writer` (generic), `indifferent_writer`
  (names the property being proved rather than the program, which is `flaky`'s mistake applied to a
  conclusion), `sink_writer` (reads as "writes a sink" rather than "writes to one", and says nothing
  about what it writes).
- `file_sink` (was `ROLE_FILE`). It is the name the kernel side already spelled:
  `fs_service::FileSink`, `fs_service::start_file_sink`. Refused `file_sink_caretaker`, which nearly
  won on analogy with `terminal_sink_caretaker` (the same adapter for a different backend) and lost
  on collision with the existing and different `fs_file_caretaker`.
- `file_source` (was `ROLE_VERIFY`). `source` is this tree's own word for the reading end of the
  sink contract: `grant_plan::spawnproto::Wiring` carries `sink` and `source` as the two directions,
  and `sink_tests` already names the endpoint `source` at the call site. Refused `sink_verifier` and
  `verify` (they name the *test* this serves; the program verifies nothing, it reads a file and sends
  bytes), `file_reader` (generic), and `cat` (a standard term a reader knows from outside, and
  therefore one that would promise a name argument and a general program, which this is not).

`sink` survives as the contract word, under the structural-versus-current test calef set on
2026-09-13 (design/naming.md). `byte_sink_proto` is a wire contract named for what it carries and
makes no disposal claim; `file_sink`'s terminus is structural, because its client holds a capability
over which no message but *append* is expressible and no grant anybody could make would change that.
What did not survive the test is one binary wearing that word for three different jobs.
`byte_sink_proto` is untouched.

**One naming inconsistency this milestone found and did not settle, because it is an architect's.**
The tree spells one sink adapter `terminal_sink_caretaker` and this one `file_sink`, and `caretaker`
is defined in two incompatible ways depending on which file you read. The old `sink.rs` header said
`fs_file_caretaker` is a caretaker *because* it serves the protocol its client speaks, and that this
program was deliberately not one; `sink_tests` says the terminal adapter is "`fs_file_caretaker`'s
shape", and it is named `caretaker`. Both cannot be the definition. It is recorded in `file_sink`'s
own header, where the next person to touch the name is already reading.

## What the tests prove is unchanged, and what changed under them

`kernel::user::sink_tests` is four `#[test_case]`s and **not one assertion moved**. What changed is
which image each spawn names:

- `spawn_writer` takes `sink_transcript_writer` and passes `repeat` as `a0` rather than as `a1`
  behind a role number.
- `fs_service::start_file_sink` takes a `file_sink_image` and spawns it with `arg0: 0`.
- `fs_service::start_sink_verify` is `start_file_source`, takes a `file_source_image`, and spawns it
  with `arg0: 0`. Renaming that function is a judgment call this lane made and flags: leaving
  `start_sink_verify` pointing at a program called `file_source` is exactly the drift this milestone
  is about, and a `pub fn` inside one kernel module with one caller is the reversible end of the
  naming rule. It is provisional like the rest.

The three empty roles that `_start` used to dispatch are gone, and with them the `_ => writer(a1)`
default arm, which silently ran the writer for any unknown role number and would have turned a
mistyped role into a plausible-looking wrong answer rather than a failure.

## Three entries in the archive where there was one

The initrd tables in `xtask/src/main.rs` pack `sink_transcript_writer`, `file_sink` and
`file_source` on both ISAs, so the measured-boot manifest (`kernel/src/trust.rs`, the kernel's
`build.rs`) gains two lines. No name is near `nifefs`'s `NAME_LEN = 32`: the longest is
`sink_transcript_writer` at 22 bytes.

## Records this milestone deliberately did not edit

`design/roadmap/0050-pipes-and-redirection.md` and `design/decisions/0051-sink-protocol.md` both cite
`fixtures/src/sink.rs`, and both keep the old path. Milestone 50 is BUILT, which design/naming.md
makes an account of what happened under the names it happened under, and a decision records what was
decided in the words used then. A reader arriving from either lands here, because this block names
both. The live documents (notes/sink-protocol.md, notes/pipes.md, notes/shared-page-audit.md,
notes/unsafe-obligations.md, `crates/byte_sink_proto`, `components/src/swish.rs`) were repointed,
because a reader picks those up to act on.

`design/naming.md`'s 2026-09-13 table row for `sink` (the program) is a record of a ruling made when
the program had three roles, so the row stands and carries a note saying what the split did to its
reasoning. The ruling itself is unaffected: `sink` survived that sweep for reasons that survive the
split too.

## BUGS

- The shared-frame duplication got one file wider rather than narrower. `file_sink` and
  `file_source` each carry their own `PAGE_VA`, their own `MappedWindow`, and their own `put`/`get`,
  which is the same dozen lines every other FS client in the tree carries. The split did not create this and could
  not honestly avoid it: a crate for it is a naming and dependency decision, and the `ntp` split lane
  has already proposed exactly that crate. Recorded in both programs' `BUGS` sections rather than
  only here.
- `file_sink` and `file_source` both open one hard-coded name, `byte_sink_proto::fixture::SINK_NAME`.
  They are two halves of one fixture rather than general programs, and nothing takes a name from
  anywhere. Recorded in both headers.
- **`file_source` reports the size it found, not the size it delivered.** Every caller compares what
  it received, so this has never mattered; it is recorded where a future caller meets it.

## Follow-on

- **Milestone 413.** `caretaker`
  carries two incompatible definitions in this tree, one that narrows a protocol and one that
  translates between two, and this lane had to name a translating adapter without being able to say
  which. It shipped as `file_sink` on a collision argument, which is a local answer to a global
  question.
- **Recorded.** The three limitations in this block's `BUGS` are written where a reader meets them:
  the shared-frame duplication and the single hard-coded name in both `fixtures/src/file_sink.rs`
  and `fixtures/src/file_source.rs`, and the reported-size caveat in `fixtures/src/file_source.rs`.
- **Done.** The shared-frame duplication has a home already: the concurrent `ntp` split lane has
  proposed the crate that would end it across every program that maps that frame, and this milestone
  added one more copy of a shape that proposal already covers rather than a new problem. The proposal
  is not cited by number because that lane had not landed when this was written.
- **Refused.** Lifting `put`, `get` and `fs_call` into a crate here, to avoid the two copies this
  split creates. It is the right fix and it is not this milestone's: a crate is a name and a
  dependency, it would touch six programs this lane has no business in, and doing it badly in a
  hurry is how a tree gets a shared module nobody can restructure later. The proposal that owns it
  already exists.

## Index row

An application of calef's ruling that a program does one thing (2026-09-14). One binary picked `ROLE_WRITER`, `ROLE_FILE` or `ROLE_VERIFY` out of `arg0`. The argument for splitting is not tidiness: the role numbers were three hand-maintained copies of a fact two binaries had to agree on (the program, `kernel/src/user/sink_tests.rs`, `kernel/src/user/fs_service.rs`), six constant definitions in all, invisible to `script/lint` check 5 because none was a `#[path]` module, and a mismatch hangs rather than fails. Three programs deleted all six, and the writer's two report-slot constants with them. All three stay in `fixtures/`: the brief's premise that `fs_service` spawns the file role in the live path is false (`// spawned only by the phase-2 test`), and `>` at a prompt is `fs_file_caretaker`, which narrows a protocol where this one translates between two. Names `sink_transcript_writer`, `file_sink`, `file_source` are **provisional**, gathered in the block with their refusals; `byte_sink_proto` is untouched. Found and did not settle: `caretaker` is defined two incompatible ways across `fs_file_caretaker` and `terminal_sink_caretaker`.
