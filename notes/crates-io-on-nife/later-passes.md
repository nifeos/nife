# The later passes: what closed, what was declined, and why

*Name: provisional.*

An appendix to [notes/crates-io-on-nife.md](../crates-io-on-nife.md), moved there on 2026-10-11
(UTC) under §212 (a prose budget). It holds milestone 64 (enough `std` to run somebody
else's crate)'s second, third and fourth passes over the gap list, and the five bindings, each with
its reasoning.

## Six stale rows in the gap list

Milestone 259 (sweep)'s notes sweep, 2026-09-05, found six stale rows in this table, all in the
same direction: `Unsupported` where milestone 64 had since bound the call. The table's own
annotations are what made it obvious, because five of the six carried a verb exists
note. That is a gap list saying out loud that it is one binding away, and then not being reread
when the binding landed. The PAL's own header (`sys/fs/nife.rs`) had recorded all six under "Also
bound since milestone 64" and nothing propagated it here. Nothing gates this; the only mechanism is
somebody reading the PAL and this page together.

## The second pass, 2026-08-17: what closed, what was declined, and why

Milestone 64's first pass took the five bindings below. The second worked the ranked list from the
top and stopped where the reason to stop was a decision rather than an effort.

Closed: rank 1 (`getrandom`, above), rank 4 (`env`), rank 8 (`File::set_len`), rank 26
(`fs::copy`). Rank 9 turned out to need nothing: `fs::symlink_metadata` routes to `sys::fs::lstat`,
which this PAL has bound since milestone 27, so the row was recording a refusal that was not there.

Rank 4 is the one worth reading, because it is the sting in a second place. `env::var` was
recorded as "no PAL at all", which sounded like the harmless kind of gap: `getenv` falling through to
`sys::env::unsupported` answers `None`, and `None` is what a Unix box with the variable unset answers
too. But the same fallback's `env()` is `panic!("not supported on this platform")`, so
`std::env::vars()` aborted the process, and so did `Command::envs`, a logger dumping its
configuration, anything that filters the environment rather than asking for one name. Like
`tempfile`, it compiled perfectly. Unlike `tempfile`, the fix was ours.

The backend (`patches/std-nife/overlay/std/src/sys/env/nife.rs`) is a process-local table, empty at
start. Nothing endows a nife process with variables. `set_var` works because that is what `set_var`
means everywhere. `vars()` returns an empty iterator, which is the one answer here that is never
a lie. Milestone 47's namespace is where a *seeded* environment would come from, and the shape does
not change when it arrives.

Declined, and each for a reason rather than for time:

- Rank 2, `std::os::unix`. The three crates behind it want a uid (`gix-sec`) and a file
  mtime set (`filetime`), and this system has neither in the form they ask for. A `std::os::nife`
  that answered `geteuid()` would be inventing an identity nothing issues. A `MetadataExt` over
  a contract with no mode bits would be a Unix fiction over a capability refusal, which is exactly
  what the `InvalidFilename`-not-`PermissionDenied` choice in `sys/fs/nife.rs` exists to avoid. §42's
  rule is to declare what you offer. The honest answer here is that these crates cannot build, and
  the note's own observation stands: *the distinction between the passes and this class is entirely
  whether the crate author wrote a fallback.*
- Rank 3, `thread::spawn`. Not declined on the merits: it is a design fork and the roadmap
  block says so in its own BUGS. The kernel has everything the spawn needs (retype a TCB,
  CONFIGURE it into this address space, START it). What has never been decided is *what a `std`
  thread is* against the budget model, and a PAL that guessed would ship the answer as an
  implementation detail. It also has no build failures behind it at all: all four of `rayon`,
  `crossbeam-channel`, `tokio` and `ignore` compile and link today.
- Ranks 7, 10, 14, 15, 20, 23, 29 and 30 (`read_link`, `std::os::fd`, `hard_link`, `Permissions`,
  `set_permissions`, `Metadata::created`, `File::try_clone`, `File::lock`). Each refuses because
  nothing in §27 backs it, and inventing a backing is the failure mode. `try_lock` is the one that
  will hurt: `gix-tempfile` wants it.
- Ranks 19 and 28 closed on 2026-09-19, by path, and the two paragraphs below are what this note
  said before that. Milestone 47's `touch` lane added the three mtime verbs on 2026-08-24
  (DECISIONS §112), which made both rows PAL bindings; this note went on calling them wire-format
  decisions for three weeks. What is still refused is the open-`File` form of each, because the
  verbs take a name and a handle has none; that one genuinely is a wire change, and it is proposed
  in design/roadmap/0504-an-mtime-for-an-open-file.md.
- Rank 19, `Metadata::modified`. The nearest miss on the list. The FS server keeps an mtime and
  §43 (reading the clock is a page) gave us a clock to read it against. So the only missing piece is a field in `FSTAT`'s
  reply, which makes it a wire-format change, the expensive and irreversible kind, and not a
  lane's to make. It wants a `DECISIONS` section and calef.
- Rank 28, `File::set_times`, is the same shape as rank 19 and was never named as a decision by an
  earlier pass (found on milestone 64's next lane, 2026-08-22, while checking the list for anything
  still genuinely buildable). Setting an mtime needs a verb no less than reading one does; there is
  no `SET_TIME` (or equivalent) in `filesystem_contract`'s verb table today. It is a second wire-format row
  behind the same open decision, not a second question.
- Ranks 16, 18 and 27 (`env::temp_dir`, `env::current_dir`, `fs::canonicalize`) and everything
  else that needs to resolve a path against something. These are the `File::open` resolution fork,
  which the roadmap block reserves to be answered jointly with milestone 47's namespace half rather
  than twice. Nothing here routed around it.
- `remove_dir_all` keeps the existing refusal rather than gaining a recursion. `readdir` shows a
PAL *can* hold a capability per level, so the mechanism is not the obstacle. The recorded reasoning
  (the loop belongs where each step can be checked against that level's capability, in `components/src/rm.rs`)
  is a decision this lane had no evidence to overturn.

## The third pass, 2026-08-18: the gap list cannot see a `panic!`

The second pass worked the ranked list from the top. The third asked a different question, because
the second pass's own best finding had come from asking it by accident: what do the neighboring
functions do? `env::var` answered `None` honestly while `env::vars()` in the same fallback was a
`panic!`, and only one of the two was on this list.

Run against every module nife falls through rather than binds, that question found three more, and
every one of them kills the program:

| call | what it actually was | probes | packages |
|---|---|---|---|
| `std::env::temp_dir()` | `panic!("no filesystem on this platform")` | 11 | 9 |
| `std::env::split_paths()` | `panic!("unsupported")` | 8 | 4 |
| `std::process::id()` | `panic!("no pids on this platform")` | 5 | 5 |

None of them could ever have appeared on the ranked list, and that is the finding rather than
the three functions. This list is built from PAL functions that answer `Unsupported`; a function
that aborts never answers anything, so it is invisible to the method that produced every other row
here. `env::temp_dir` did have a row, at rank 16, and it read *"no PAL at all ... needs a namespace
answer, not a PAL one"*, which is how a fatal defect got filed as a design question. Its demand was
also undercounted by four probes.

What closed, and each stops where the signature stops. The rule the third pass followed, stated
so the next lane can apply it rather than re-derive it: fix the ones that abort, leave the ones
that refuse.

- `env::temp_dir` (`patches/std-nife/overlay/std/src/sys/paths/nife.rs`) answers `TMPDIR` if the
  program set one, otherwise `.`. `PathBuf` has no error channel, so *something* had to be named,
  and `.` is not a new decision: `sys/fs/nife.rs`'s `one_name` says *"./motd is motd: the
  current directory IS the granted one."* A process holds one directory and that is the only place a
  temporary file can go. `/tmp` lost because it names a filesystem root this system does not have,
  so every path built on it would be refused with `InvalidFilename`, turning an abort into a
  guaranteed failure rather than into working code. `TMPDIR` is first for parity and because it is
  the seam milestone 47's namespace arrives through, with no change to the file.
- `env::split_paths` and `env::join_paths` are pure string work over a separator with no
  platform in them at all, which is what makes the old `panic!` indefensible rather than merely
  unimplemented. Round-tripped in `std_exerciser`, because a splitter that agrees with nothing is
  worth less than no splitter.
- `process::id` (`sys/process/nife.rs`) answers `0`. There is no process identifier on this
  system to report, the four-call syscall surface issues none, and `u32` cannot say so; `0` is the
  one number no Unix assigns to a user process. The call sites make that the right answer rather
  than the least wrong one. Every reachable use in these fifty closures is a *fork* check
  (`gix-tempfile`'s `forksafe.rs` and its registry compare an owning pid so cleanup runs only in the
  creating process). nife has no `fork`, and a constant is what makes the comparison match.

Declined, and the line is the same one: `current_dir`, `current_exe`, `chdir` and `home_dir` all
*can* refuse in their own signatures and do. Each needs a namespace to resolve against, which is
milestone 47's unbuilt half and the `File::open` fork this milestone reserves. `std_exerciser` now
asserts those refusals as well as the two answers, so a later lane cannot quietly turn one into a
fabrication.

Nothing about the fifty-crate split moved: all three were runtime behavior, and every probe that
built before builds now.

## The fourth pass, 2026-08-18: the reading became a check, and it found a fifth

The third pass ended by saying its method was *"a sweep somebody has to run rather than a gate, and
nothing in the tree runs it."* The fourth built the gate, and the gate immediately found something
the sweep could not have.

`std::process::exit()` was a trap instruction. `sys/exit.rs`'s `_ =>` arm is
`crate::intrinsics::abort()`, so a nife program calling `std::process::exit(0)` compiled perfectly
and then executed `brk`; the kernel takes that as a fault, prints it, and reports `EVENT_FAULT` to
the process's supervisor. A clean exit arrived as a crash, which is how almost every CLI-shaped
program ends.

Why the third pass's method could not reach it, and this is the finding rather than the function.
That method was "read every module the PAL falls through and ask what its neighbors do". It works
on `sys/<module>/mod.rs` dispatchers, which is what `env`, `paths` and `process` are. `sys/exit.rs`
is not one: it is a single file whose `cfg_select!` lives *inside* `pub fn exit`, so there is no
module to read and no neighbors to compare. Two passes of careful reading walked past it, and a
third would have too.

It also hid behind a second thing worth recording: the two ways a Rust program ends took different
exits, and only one was wired. Returning from `main` reaches `_start`, which calls the PAL's
`rt::exit` directly; `std::process::exit` is the *only* caller of `sys::exit::exit` anywhere in std.
Every std test in this tree ended the first way, so the broken path never ran.

The check is `cargo xtask std-aborts` (provisional name), documented with its worked examples and
its limits in notes/std.md. It asks cargo's own dep-info which `library/std/src/sys/**` sources
rustc compiled for the nife targets and greps exactly those for process-ending bodies, comparing the
result against a list that carries a reason per entry. It is rung two of AGENTS.md's ladder where
this note's BUGS section had rung four, and it runs inside `script/test`.

Nothing about the fifty-crate split moved: this was runtime behavior, and every probe that built
before builds now. What moved is the count of process-ending calls that a green build hides, which
is now zero as far as a mechanism can see, and the mechanism's own blind spot is written down.

## Five of these were bindings, not verbs, and milestone 64 bound them

Ranks 5, 11, 12, 13 and 25 (`create_dir`, `read_dir`, `remove_file`, `remove_dir`, `rename`) were
each backed by a verb the FS server already implemented: `redoxfs_server/src/bin/redoxfs_server.rs` has
dispatched `MKDIR`, `OPENDIR`, `READDIR`, `UNLINK`, `RMDIR` and `RENAME` since milestones 47 and 48.
Nothing was missing from the contract and nothing was missing from the server; the client side in
`patches/std-nife/overlay/std/src/sys/fs/nife.rs` simply still refused, and its own comments
still said the verbs did not exist.

All five are bound now, and the `std_exerciser` demo walks them under a real directory
capability on both ISAs. See notes/std/fs.md for the behaviors (what `read_dir(".")` means with no
global namespace, why the listing is drained rather than streamed, and why `remove_file` refuses a
directory).

A sixth came with them and is not in the table above, because the census had no row for it:
`Path::is_dir()` was `false` for every directory that exists, since `OPEN` refuses a directory and
`stat` propagated the refusal. That made `std::fs::create_dir_all` non-idempotent, which is the
call every crate reaches for rather than `create_dir`. It costs no extra message: the `EISDIR` the
server already sends *is* the answer. Nothing in a gap list built from `Unsupported` counts would
have found it, because `stat` never returned `Unsupported`; it returned the wrong thing.

Milestone 47's block had already said this for `rename`, `unlink` and `rmdir` ("now a binding gap
rather than a missing verb"). The measurement added two, `read_dir` and `create_dir`, and put
`create_dir` above all of them by demand.

A refusal outlived its reason by two milestones and nothing caught it, which is the finding
worth more than the five functions. A refusal that looks correct reads exactly like a refusal that
is correct, and the comment explaining it is written once and then believed. The thing that found it
was not a review; it was asking fifty crates what they wanted.

And binding them turned up a bug that only a narrowed capability would have shown. `OPENDIR` and
`MKDIR` take the rights the caller wants on the child in the request's second word. The server
answers `EPERM` if the intersection with the parent's rights comes up short of what was asked
(§47's monotonicity is the intersection; the refusal is the server telling the truth about it). The
first version of this binding asked for `dir::ALL`, which works perfectly through the full-rights
grant every test uses and fails through every narrowed one. A PAL cannot know what its own
capability carries, so it has to ask for the minimum the operation needs: `ENUMERATE` for a listing,
and nothing at all for `create_dir`, which closes the handle it gets back. Caught by reading the
server rather than by a test, and there is still no test that would catch it (a std program spawned
with a narrowed directory capability does not exist yet; that wants a lane).
