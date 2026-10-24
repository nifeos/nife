---
status: IN-PROGRESS
raised: 2026-10-20
branch: milestone/946-granted-file-name-longer-than-sixteen-bytes
promoted_from: a-granted-file-name-longer-than-sixteen-bytes
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 946. A granted file name can be longer than sixteen bytes

**IN-PROGRESS, 2026-10-24 (UTC).** Built on aarch64 and gated there by `script/swish-check`; the
other two legs and the bound itself are what is left ("Needs calef" and "BUGS" below).

*(Title and slug provisional.)*

Raised 2026-10-20 (UTC) by `maintainer/record-2026-10-20`, recording calef's ruling of that day on
milestone 922 (the demos are packaged in basalt). He kept the ratified `least_authority_demo` rather
than shortening it: *"B"*. The fix is to lift the limit that refuses the name. calef asked on
2026-10-24 (UTC) for it to be built.

## What a name may be

- **A file's name is at most `filesystem_contract::grant::MAX_NAME`, 255 bytes**, provisional
  (the question below). `grant_plan::MAX_FILE_NAME` mirrors it, pinned by a host test. It is the
  last component of a path the shell opens, a name a pattern matched, and a name in a grant's set.
- **A name that rides two `START` words is at most `grant::WORDS_NAME`, 16 bytes**: a caretaker's
  directory, `rm`'s operand when it fits, a `login` identity's subtree, an activation message's
  package or vouched name. `grant::pack_name` answers `None` past it, so nothing truncates.
  `grant_plan::WORDS_NAME` mirrors it.
- **A directory the shell stands in, walks through or binds is at most 16 bytes**
  (`grant_plan::nav::MAX_DIRECTORY_NAME`), because a directory grant names its directory in a
  caretaker's two words and the shell carries every position by value. A longer one is refused as
  `Refused::DirectoryNameTooLong`.
- **A name set carries at most 8 names and 512 bytes of them** (`nameset::MAX_NAMES`,
  `nameset::NAME_BYTES`): two of the longest names, or eight of 64 bytes. A record is a length
  byte, a flags byte, then the name. The shell's `NameSet` packs its names into those bytes rather
  than holding eight slots of 255, and over either bound is the same refusal, never a truncation.

## How a long name reaches a program

- **A file the shell opens itself**: `wc quarterly-report.txt`, `<`, `>` and `2>`, and a program
  run by its path. Nothing travels but the shell's own `OPEN`, through the file service's page.
- **A word a program hears** (§170): the name rides the grant's set, on the page
  `fs_nameset_caretaker` maps, as every designated name already did.
- **`rm` with a name over two words**: `dir_grant` tells `rm` `grant::WHOLE_NAMESPACE` and grants the
  set of that one name, which `rm` sweeps.
- **Where the file server binds badges** (milestone 606), a grant carrying a set is bound and then
  served by `fs_nameset_caretaker` over the bound channel, told with an empty name that the bound
  root is its directory (`client_windows::at_bound_root`). The badge alone would hold the whole
  directory, and the set is what narrows it.

Reuse: none exists; this is nife's own grant wiring between the shell, the progenitor and a
caretaker, and no library carries a capability's name by argument words or frame.

## Built 2026-10-24 (UTC)

Lane `milestone/946-granted-file-name-longer-than-sixteen-bytes`.

`script/swish-check`'s aarch64 leg types these, and each answers as shown:

- `wc < quarterly-report.txt` and `wc quarterly-report.txt` both print `1 3 19`, and
  `caps wc quarterly-report.txt` names the 20-byte file as the input.
- `/installed/std-grep needle the-quarterly-report.txt` prints the line from the 24-byte file its
  word named.
- `rm -v rmtree/a-name-longer-than-two-words` removes the 28-byte name, and `ls rmtree | wc` still
  counts the two entries nothing named.
- `installed/least_authority_demo 5` is opened by its 20-byte name, its note read and its bytes
  sent, and the progenitor refuses the line for what the note asks of unvouched bytes. The shell no
  longer refuses the name.

The shell's stack was measured rather than scaled, on the same leg: twelve pages fault on the
boot's first pipeline, sixteen in `installed/jig rollback`, seventeen pass both boots. The shell
gets twenty-one (`system_initializer::SHELL_STACK_PAGES`), the planner is compiled optimized in a
debug build, and a set holds at most 512 bytes of name. notes/glob-grant/xargs.md ("The bytes a
set carries") has the numbers. The progenitor's capability peak stays at the 37 that
`kernel/src/cap.rs` records, on both boots.

## Needs calef

The bound on a granted name, `grant::MAX_NAME`, is an architect's call: the filesystem and the
loader share it. It is one constant, and the build uses 255.

- **B (recommended): 252**, §307 (how long a name may be)'s bound, which is RedoxFS's
  `DIR_ENTRY_MAX_LENGTH` (calef, 2026-10-17: *"252 seems fine for now."*). A name of 253 to 255 bytes
  is grantable under A and unstorable on nife's own disk, which is the "two numbers for one fact"
  §307 refused.
- **A: 255**, Linux's `NAME_MAX` and `xattr::MAX_NAME`. The server refuses a 253-byte create, so
  nothing breaks; a name that long is only refused one step later.

## Exit

On aarch64, riscv64 and x86_64: a file named with more than 16 bytes is granted at the prompt and read
by the program it was granted to, and an installed `least_authority_demo` runs by its package path.

## BUGS

- Only the aarch64 leg has run. riscv64 and x86_64 carry the same code and the same script lines,
  and CI runs them.
- An installed `least_authority_demo` cannot yet be installed to be run: `jig install demos` is
  refused while every image packs the program, until milestone 909 (basalt composes the bootable
  image from its packages). The shell's half of the exit is the `installed/least_authority_demo`
  line above; the run that squares an integer is milestone 922's exit.
- A directory's name is at most 16 bytes, so `cd` refuses a longer one and `mkdir` can make a
  directory the shell cannot then stand in.
- A package or vouched name is at most 16 bytes, because the activation messages carry it in two
  words (the proposal below).
- `fs_file_caretaker` takes its file's name in two words, so it serves a name of at most 16 bytes.
  No prompt line builds one.
- A caretaker on a client window that was once bound gets `EBADF` throughout, since `UNBIND` leaves
  the badge revoked (`client_windows::job_windows`' BUGS). A set grant no longer meets it, because
  it is bound before its caretaker is built.

## Follow-on

- **Proposed.** A package name longer than sixteen bytes installs and is removed:
  `design/roadmap/proposals/a-package-name-longer-than-sixteen-bytes-installs-and-is-removed.md`.
- **Milestone 909.** Taking the demos out of the shipped image is what lets an installed
  `least_authority_demo` run by its package path.
- **Recorded.** A directory name over 16 bytes, in BUGS above.

## Index row

A customer's files are not named for a demonstrator's limit: a name like `quarterly-report.txt`
can be granted at the prompt and read by the program it was granted to, and a package's program
named over sixteen bytes runs by its path.
