---
status: BUILT
built: 2026-09-27
raised: 2026-08-31
milestone_dependencies: none
decision_dependencies: 170
machine_requirements: none
specific_machine: none
needs_person: no
---
# 205. How a foreign program is told what to do

§170 (how a foreign program is told what to do), the decision this block depends on, was decided by
calef on 2026-09-26, with two rulings of 2026-09-27 (UTC) recorded below: "N1" and "no mark". The
argument page's layout is still his, built provisionally and proposed.

In brief. Unmodified `ripgrep` ran on nife and stopped at argument parsing, because the native ABI
had no argument vector and `std::env::args()` compiled std's `unsupported` backend and yielded
nothing. The argv is now built,
so a stranger's program hears its line. What it may reach with those words is the half still to
build. Everything milestone 121 (`ripgrep` on nife) still owes is behind that half: the confined
`rg`, the loud `ENUMERATE` refusal, and the walk benchmark.

## What the ruling asks this milestone to build

§170 has the ruling in full. In short:

1. An argv of plain bytes in one page, carrying no authority. The transport was priced at about 50
   lines and no new system call in notes/foreign-program-arguments.md (#1314), which also holds the
   layout this milestone proposes.
2. A word that resolves to an existing file is granted that file as the program's manifest
   declares: read-only, read-write, or the file's directory. The manifest travels in the ELF (§197
   (a package is one archive file), M2), which is the proposal
   `design/roadmap/proposals/a-program-carries-its-manifest-in-an-elf-note.md`.
3. A manifest may declare "may create the named path" for a word that does not yet resolve.
4. An unvouched program gets every named file read-only. Read-write and create each need a mark
   on the word, spelled provisionally here.
5. The directories granted on the line bound everything else.

## What is built (2026-09-26, lane `milestone/205-foreign-program-arguments`)

Clause 1, the transport, on all three architectures. A `std` program run from the prompt hears the
line's words through `std::env::args()`, and nothing it hears is authority.

- `crates/program_argument_contract` (provisional) is the page: a magic, a count, a total, then each word
  as a length and its bytes. A page that does not parse whole reads as no arguments.
- `std_runtime_protocol` gains slot 8 and `ARGS_PAGE` (`0x1400_0000`), and the `std` PAL's
  `sys/args/nife.rs` reads the page from there, zero-copy. `cargo xtask std-src` generates the
  layout into the PAL and routes `sys/args` to it.
- `grant_plan::ArgSpec::Words` (provisional) is a manifest saying "my line is my argv".
  `std_exerciser` declares it. The planner then classifies nothing on the line: `-i` and
  `--color=never` are words, not refusals. `grant_plan::argv` writes the page. `check_words` is
  the same walk without a page, run at plan time, so a bad line is refused before anything spawns.
- The shell writes the page into a frame of its own, for a plain line and for each pipeline stage.
  It sends the frame `READ`-only under `spawnproto::ARGS_BIT` (provisional). The progenitor copies
  the frame into a page from the child's region and places it at slot 8. `caps` prints that slot.
- An unquoted pattern on such a line is refused (`Refusal::PatternInArguments`). The words carry
  no authority, so expanding one would grant names and nothing they name. Quoting passes it through.

Proven by `script/swish-check` on every architecture. `std_exerciser one 'two words'` prints
`args ["std_exerciser", "one", "two words"]`. `std_exerciser redirected > args.txt` takes the
pipeline path, and `wc` counts what it wrote. `std_exerciser *.rs` is refused with nothing spawned. `caps std_exerciser` shows
`cap 8  frame     args`. Host tests in `argument_protocol` show that every byte round-trips and
that no damage to the header or a length yields a partial argv. `grant_plan`'s tests show that
every word arrives unclassified, past the sixteen tokens `parse_run` keeps.

A `std` program run by path hears its words too (lane `milestone/205-args-by-path`). The shell reads
the program's manifest note, and when it declares `ArgSpec::Words` sends the line as the argv. The
note also has to declare the `std` runtime; `grant_plan::image_can_carry` ties the two. The
progenitor sizes the image's region from the argv bit, because the region is split before the frames
carrying the note arrive, and then checks that the note agrees. Bytes nobody vouched for get
`UNVOUCHED_STD_MANIFEST`: §219 (how the shell names an installed program to the spawner)'s clock and configuration pages at the `std` slots, and the words,
which carry no authority. Proven by `installed/std-echo one 'two words'` on every architecture,
which prints `words ["installed/std-echo", "one", "two words"]`. `std_echo` is a second, small
binary in the `std_exerciser` workspace, with a provisional name.

## What is built (2026-09-27, lane `milestone/205-designation`)

Clauses 2 to 5: a word that names something here is granted it. calef ruled two questions first.

- N1, at 2026-09-27T06:27Z, on
  [`0665-designating-a-foreign-programs-words.md`](0665-designating-a-foreign-programs-words.md):
  a line that names no file grants nothing. To search here, a person types `rg pattern .`.
- No mark, at 06:35Z, option 1 on
  [`0675-the-mark-on-a-foreign-programs-word.md`](0675-the-mark-on-a-foreign-programs-word.md).
  An unvouched program gets every named word read-only, plus `>` for its output. To widen that, a
  person installs (vouches for) the program, and its manifest applies. There is no mark to spell.

How it works:

- The shell designates. For each word after the name, `swish::designation` looks the first path
  component up in the shell's directory, with the same directory read `echo *` uses. A name that
  is there is designated, `.` designates the directory itself, and anything else is inert bytes.
- The planner turns that into one `DirGrant` at the shell's directory: the names, or the whole
  directory for `.`. Nothing designated, nothing granted.
- The manifest says what a named word may do: `ArgSpec::Words(WordGrant)`, read-only, read-write,
  or read-write and create (clause 3). The note encodes them as 2, 3 and 4 in its `arg` byte.
- The shell sends the grant as a directory grant. Named entries also send a name-set frame
  (`spawnproto::NAMESET_BIT`), and the progenitor builds `fs_nameset_caretaker`. The whole
  directory gets `fs_subtree_caretaker`. The caretaker's endpoint lands at `std`'s slot 4.
- The progenitor clamps the caretaker's rights to the manifest it endows. Bytes nobody vouched for
  get `UNVOUCHED_STD_MANIFEST`, which is read-only, whatever their note asks.
- Plain lines, pipeline stages and images run by path all carry it.

Proven by `script/swish-check` with `std_grep`, a small `std` program that searches like `rg` and
runs from the disk unvouched. In a directory holding `docs/n.txt`, `std_grep needle docs` prints
`docs/n.txt:find the needle here`. `std_grep needle` prints `std_grep: .: no directory was granted
to search`. `caps` previews the grant as `docs` only, read-only. Host tests cover designation,
the planner's grant and the note's round trip.

## What stays open

- The layout: where the page sits, `argv[0]`, bytes rather than UTF-8, the 4,080-byte ceiling.
  Built as [`0672-the-argument-page-layout.md`](0672-the-argument-page-layout.md)
  proposes, and provisional until a ruling.
- Environment variables and exit codes for a foreign program, which §170 does not rule.

## Provisional names

`argument_protocol`, `ArgSpec::Words`, `argv`, `check_words`, `ARGS_BIT`, `ARGS_SLOT`,
`ARGS_PAGE`, the magic `nifeargv`, `Refusal::PatternInArguments`, `Refusal::ArgumentsTooLong`,
`image_hears_words`, `UNVOUCHED_STD_MANIFEST`, `std_echo`, `installed/std-echo`, `WordGrant`,
`Designation`, `designation`, `each_word`, `NAMESET_BIT`, `std_grep` and `installed/std-grep`.

## BUGS

- It says nothing about environment variables or exit codes, which are the same family of
  question and arrive right behind it.
- **The shell's own words win on a foreign line.** `--mem N` is the shell's grant on any line, so
  `rg --mem 5` is refused as a memory grant the program does not take. `time` and `xargs` go
  down their own paths and send no argv, so `xargs std_exerciser` runs it with none. Quoting
  passes `"--mem"` as a word.
- **A backslash is a pattern byte here**, as it is to `glob`, so a regex such as `\d+` has to be
  quoted. That matches what another shell would do to it unquoted, but it is not what a person
  typing a regex expects.
- **An installed `rg` still cannot run by path, because it is too big to travel.** An image
  goes from the shell to the progenitor as frames staged in the shell's own budget. That is at
  most 64 pages (256 KiB, `spawnproto::IMAGE_MAX_PAGES`), out of a 128-page shell budget
  (`SH_BUDGET_PAGES`). `ripgrep` is about 2.6 MiB and `std_exerciser` is 347 KiB, which is why
  the gate proves the path with `std_echo` (166 KiB on aarch64). Raising the ceiling moves the
  shell's image window and both budgets; milestone 206 (a program image has under 896 KiB)'s
  address-space map is where the window would go.
- **There is no mark, by design** (calef, 2026-09-27T06:35Z). An unvouched program cannot be
  given write on a word from the prompt. Installing it is how trust widens. A note's read-write or
  create grants nothing until then.
- **A word grants its whole first component.** `std_grep needle docs/n.txt` grants all of `docs`,
  because the caretaker filters at the shell's directory and nowhere below. And a pattern that
  happens to be a file's name here is granted too, read-only.
- The shell must stand exactly one directory down. At its root, or deeper, a word grant is
  refused with a sentence saying so, because the progenitor builds one caretaker per grant from a
  single directory name (`dir_grant`'s limit, shared).
- The progenitor's install path has almost no stack to spare. In a debug build, `package install
  uptime` runs `package_archive`'s parser under `spawn_service`, and 416 bytes more in that frame
  overflowed the 32 KiB stack (kernel `INIT_STACK_PAGES`); CI found it on 2026-09-27. Moving the
  grant's locals into `build_grant` left 112 bytes over main, which passes, but the margin was never
  measured. The next change to `spawn_service` can trip it again.
- **A named word's filter can be raced on a real boot.** The prompt's `fs_nameset_caretaker`
  forwards through the file service's one shared frame, which the shell also writes while it drains
  a redirected job. So `std_grep needle docs > out.txt` can open a name the set never approved, if
  a shell write lands between the caretaker's re-staging and the server's read. Reachable, not
  observed. `notes/a-set-grant-at-the-prompt.md` has the case, and `notes/shared-page-audit.md`
  finding 1 the mechanism, which it still calls unreachable: correcting that audit trips the prose
  ratchet (108 bold spans, over its word baseline). Milestone 599 (a frame per
  filesystem client channel)'s production pool closes it.
- A word grant holds at most eight names, the name-set page's ceiling; a ninth is refused.
- **A pipeline with a `std` program at its head strands that program's region until reboot.**
  The job pool is a stack: a region's pages go back only when it is the most recent carve
  (`memory_regions`' `return_to_parent`). A pipeline carves its head first, and the head exits
  and is reaped first, so its region is never the top when it goes. For a `std` head that is 384
  pages, and the pool holds one `std` region, so after `std_exerciser | wc` no `std` job runs
  again. Found by this milestone's CI on 2026-09-27, when a plain `std_exerciser` line after a
  pipeline answered "could not spawn (the progenitor is out of memory)". It predates this
  milestone, since a `std` head was already legal, and native heads strand 40 pages the same
  way. Milestone 26 (object revocation) built the LIFO case and recorded non-LIFO return as
  having "no reason to build one"; this is the reason. `script/swish-check` proves the pipeline
  path with `std_exerciser redirected > args.txt`, a one-stage line, instead.
- **A `std` job typed straight after another races the reaper, and the progenitor's retry is kept
  on effort.** A region comes back when `job_undertaker` reaps the job, which can be after the shell
  has prompted. CI hit it on 2026-09-27: `/installed/std-grep needle` right after
  `/installed/std-grep needle docs` answered "could not spawn". `split_job` in
  `crates/system_initializer` yields up to 1,024 times before it gives up. That is a timing guess,
  and whether it covers one reap depends on what else is runnable. We would not choose it if the
  proper fix cost the same; the proper fix is a wire change, proposed in
  `0685-a-job-is-finished-when-its-memory-is-back.md`.
- Each spawn costs a scratch page or two in the progenitor: one to read the shell's frame
  and one to fill the child's copy. They come from `supervision_protocol`'s scratch
  window, which milestone 604 (the builder's scratch cursor is bounded) made wrap, so a reaped
  job's pages are reused.

## Follow-on

- **Done.** Clauses 2 to 5, carried by this block's designation build; the proposal is kept only
  until the integrator retires it, `design/roadmap/0665-designating-a-foreign-programs-words.md`.
- **Milestone 672.** Milestone 672 (the argument page's layout). The page's layout, in `design/roadmap/0672-the-argument-page-layout.md`.
- **Milestone 675.** Milestone 675 (the mark on a foreign program's word). The mark, refused by calef at 2026-09-27T06:35Z and awaiting promotion as a
  refused block, in `design/roadmap/0675-the-mark-on-a-foreign-programs-word.md`.
- **Decision.** Environment variables and exit codes for a foreign program, which §170 leaves open
  in `design/decisions/0170-how-a-foreign-program-is-told-what-to-do.md`.
- **Recorded.** The `--mem` and `xargs` gaps, in this block's `BUGS` above
  (`design/roadmap/0205-foreign-program-arguments.md`).
- **Milestone 685.** Milestone 685 (a job is finished when its memory is back). A job counts as finished only when its memory is back, replacing the retry, in
  `design/roadmap/0685-a-job-is-finished-when-its-memory-is-back.md`.
- **Milestone 599.** A frame per filesystem client channel, whose production pool closes the race
  in the nameset grant recorded in `BUGS` above.
- **Recorded.** `notes/shared-page-audit.md` finding 1 still says the window is unreachable. This
  block's `BUGS` says why it is not corrected there
  (`design/roadmap/0205-foreign-program-arguments.md`). Touching that audit trips the prose ratchet,
  which needs an 82-span rewrite or an exception only calef can grant. Also a `std` pipeline head stranding its region, the 256 KiB image ceiling that
  keeps `rg` from running by path, and the scratch-page cost. All are in this block's `BUGS` above
  (`design/roadmap/0205-foreign-program-arguments.md`).

## Index row

Minted from milestone 121's lane. §170 ruled on 2026-09-26: a byte argv in one page, and
authority from the manifest and the line's directories. The argv is built: a `std` program at the
prompt hears its line through `std::env::args()`, in a pipeline too. A word that names something
in the shell's directory is granted it, read-only for bytes nobody vouched for; a line naming
nothing grants nothing (N1), and there is no mark (calef, 2026-09-27).
