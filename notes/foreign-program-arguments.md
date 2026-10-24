# What one shim costs: the measurement §170 asked for

*(For [§170 (how a foreign program is told what to do)](../design/decisions/0170-how-a-foreign-program-is-told-what-to-do.md),
which gates milestone 205 (how a foreign program is told what to do). Measured 2026-09-25 by lane
`lane/argv-shim-measure`. This page's stem is provisional, per the naming tenet; calef names things.
It measures and does not rule.)*

§170 is decided (calef, 2026-09-26). It took the argv of option A, carried the way this page's
fourth reading describes. The argv is plain bytes in one page. Authority is carried separately, by
the directories the line grants and by what the program's manifest says a resolved word may be. A vouched program's manifest may ask for a named
file read-only, read-write, or as its directory, and may ask to create a name that does not yet
resolve. An unvouched program gets named files read-only unless the word carries a mark. The
per-program tables of option B, and option C, were refused for the reasons measured below.
Milestone 205 built the transport on 2026-09-26 in the layout this page's prototype used, with
slot 8 and `0x1400_0000` for "where"; `crates/program_argument_contract` is the codec and
`design/roadmap/0672-the-argument-page-layout.md` is the proposal that brings it to calef.

§170 set the test in one sentence. If a shim is per-program work, option A (a POSIX argv) wins. If
it is a library written once, options B and C stay open. This page answers that with counts from
the tree, from `ripgrep` 14.1.1 and `gitoxide` 0.59.0 source, and from a host-tested prototype.

## The answer

A shim is two jobs, and they answer the question differently.

1. Transport: getting bytes into `std::env::args()`. This is a library, written once, and every
   option needs it. Nothing else reaches an unmodified program, because both programs measured here
   read their arguments from `std::env::args_os()` and from nowhere else.
2. Designation: deciding which of those bytes name something, so the launcher can grant it as a
   capability. This is per-program work. Only the program knows its grammar, and the grammar is
   large and changes by release.

So option A costs the transport alone. Option B, in the token-by-token form §170 describes, costs
the transport plus a per-program table. Option C cannot carry `ripgrep`'s pattern at all without
first adding the transport.

## Transport, measured by writing it

The prototype is a length-prefixed block and the `std` file that reads it. It was written and
host-tested in a scratch crate and is not in this tree, because its layout is the irreversible part
and belongs to the ruling.

| piece | size | shape it copies |
|---|---|---|
| the layout codec (build and parse) | 39 lines of code, 4 host tests passing | `crates/environment_contract` |
| `sys/args/nife.rs` in the `std` overlay | 11 lines of code | std's own `sys/args/xous.rs`, 19 lines |
| `xtask std-src` dispatch | 2 anchors, 1 generated-module job | the `sys/env/mod.rs` arm in `xtask/src/farm.rs` |
| the spawner's half | one read-only mapping and one `grant_at` | slot 7, the config page, in `system_tests/src/user/std_service.rs` |

The layout: an 8-byte magic, a `u32` count, a `u32` total length, then each argument as a `u32`
length and its bytes. Length-prefixed so an argument may hold any byte, including the `0xff` the
round-trip test puts in a regex. A zeroed or truncated block reads as no arguments, the same
default-honest shape `environment_contract` uses.

The second anchor exists because std compiles `sys/args/common.rs` only for a listed set of
platforms. Adding nife to that list lets the nife file reuse std's `Args` type, as Xous and Motor
do. The alternative is a self-contained `Args`, as zkvm has in 94 lines.

It costs nothing in the syscall surface: no new syscall number and no new method. It rides
`PageFrame` and `MAP_INTO`, which slots 5 and 7 already use. There is a cheaper wire still: std's
`_start` receives three entry registers and ignores all three
(`patches/std-nife/overlay/std/src/sys/pal/nife/mod.rs`), so the block's address and length could
arrive there with no slot at all.

## Designation, measured per program

### `ripgrep` 14.1.1

`ripgrep` parses with its own `lexopt`-based code and reads `std::env::args_os().skip(1)`
(`crates/core/flags/parse.rs:72` and `:103`). Counted from `crates/core/flags/defs.rs`:

| | count |
|---|---|
| flags | 104 |
| switches | 69 |
| flags taking a value | 35 |
| flags with a short letter | 41 |
| value flags that designate a file | 2: `--file`, `--ignore-file` |
| value flags that name a program to run | 2: `--pre`, `--hostname-bin` |

The rest are inert data: numbers, globs, separators, encodings, type names, and the pattern
itself. Positionals follow a rule no table expresses: the first is the pattern unless `-e` or `-f`
was given, and every other positional is a path.

A per-program binder for `ripgrep` therefore needs all 104 arities. Without them it cannot find
the positionals: in `rg -g foo bar`, only the table says `foo` belongs to `-g`. It also needs one
hand-written rule for the pattern. The generic half is getopt's conventions (bundled short flags,
attached values, `--flag=value`, `--`). `lexopt` 0.3.0 is 571 lines of code for exactly that.

### `gitoxide` 0.59.0, the second program

Milestone 99 (`git` on nife) chose gitoxide. Its `gix` binary parses with `clap` derive and reads
`gix::env::args_os()`, which is `std::env::args_os()` with optional Unicode precomposition
(`gix-0.74.1/src/env.rs:25`). So the same transport serves it with no program-specific work.

Its designation table is larger. `src/plumbing/options/` and `src/porcelain/options.rs` declare 260
argument fields across nested subcommands. Of these, 32 are `PathBuf` and 57 are `BString`, which
carry pathspecs and ref names. A binder would need that tree, per release.

`clap` can report part of it. `Arg::get_value_hint` answers `AnyPath` for any `PathBuf` argument
(`clap_builder-4.6.7/src/builder/arg.rs:4372`), so a build step could derive the path flags of a
`clap` program. That covers neither `ripgrep`, which does not use `clap`, nor gitoxide's pathspecs.

### The result for §170's test

The transport is one library and serves both programs unchanged. The designation table is
per-program: 104 flags plus a custom rule for one, 260 fields for the other, and neither is
derivable in general.

## Option C cannot say `rg pattern`

`grant_plan::Endowment` carries `arg: u64` and `flags: u64`. The shell parses the argument with
`parse_u64` (`crates/grant_plan/src/lib.rs:2383`) and refuses anything else as `ProgramArgumentRequired`. A
regex is not a `u64`, and `ripgrep`'s 104 flags do not fit a 64-bit mask. C as written needs a
string channel first, which is the transport above.

C is per-program by construction beyond that. `Program` is a closed enum of 16 programs with their
manifests compiled into `grant_plan`, so each foreign program is a new variant and a new manifest.
None of the 16 is a std program today.

## The three options, priced

| | transport | per-program work | syscall surface | runs unmodified `rg` |
|---|---|---|---|---|
| A: argv | about 50 lines in `std` and a layout crate, plus about 10 in the spawner | none | nothing new | yes |
| B: token-grained | the same | a table per program and release (`rg` 104 flags and a rule, `gix` 260 fields), plus a generic binder of `lexopt`'s size | nothing new | yes, while the table matches the release |
| C: `grant_plan` only | must be added first | a `Program` variant and manifest per program, plus B's table | nothing new | no, until a string field exists |

There is a fourth reading of B that §170 does not list, and every system in the prior art below
uses it. Arguments travel as inert bytes, and authority travels separately as a namespace: the
directories a program was handed. That costs what A costs. A path string can then reach only what
the namespace holds. `ripgrep` already behaves this way here: it resolves names under slot 4, and
the `std` PAL refuses `..` and absolute paths before they reach the wire.

## What A asks two programs to agree on

This is the irreversible list, the part a ruling fixes:

- where the block is: a slot and a virtual address (slot 8 and `0x1400_0000`, one page above the
  config page, would follow its pattern), or the entry registers;
- the layout: magic, count, total, length-prefixed bytes;
- whether `argv[0]` is present. It must be: `ripgrep` skips the first element and `clap` treats it
  as the binary name, so a block without it would lose the pattern;
- bytes, not UTF-8, because `args_os` is `OsString` and a path need not be text;
- a size ceiling. One page leaves 4,080 bytes for arguments;
- whether the environment rides in the same block, as Fuchsia and Genode do, or stays on the
  validated page of §111 (inert configuration is a read-only page).

## The page as built

`crates/program_argument_contract` is that layout, built. Its rules beyond the table:

- Records are length-prefixed, not NUL-terminated, so an argument may hold any byte, including NUL
  and the `0xff` a regex can carry.
- `argv[0]` is present. The builder does not insist, because an empty argv is a representable thing
  to hand a program, and the shell always pushes the name it ran.
- Everything past the records is unspecified. A reader never looks there, so a page reused for a
  shorter line needs no clearing.
- One page holds 4,080 bytes of records, the four-byte prefixes included. A line that does not fit is
  refused at assembly and never truncated.

## Prior art, read from source

- Fuchsia, `zircon/system/public/zircon/processargs.h`. The bootstrap message carries `args_off`
  and `args_num` (NUL-terminated UTF-8 strings), `environ_off` and `environ_num`, and a name table
  for `PA_NS_DIR` handles. Strings and authority travel in one message but in different fields: a
  path in argv names something in the namespace handles. The ELF runner's manifest `args` is a
  vector of strings passed in order. Structured configuration (`PA_VMO_COMPONENT_CONFIG`) is the
  B-shaped channel, and it sits beside argv rather than replacing it.
- Genode, `repos/libports/include/libc/args.h`. `populate_args_and_env` reads `<arg>` and `<env>`
  nodes from the component's config ROM and builds `argv` and `envp` System V style, once, in libc.
  `construct.cc` then calls `main(argc, argv, envp)`. What a component may touch is routed
  separately as sessions (from memory, not re-read for this page).
- seL4. The kernel has no argv. `sel4runtime`'s entry finds `argc`, `argv`, `envp` and `auxv` on
  the stack, and `libsel4utils/src/process.c` writes them there when a parent spawns a child
  (`sel4utils_stack_copy_args`). Its own `sel4utils_create_word_args` formats integers as decimal
  strings to send them through argv. Microkit passes no argv; `setvar` rewrites symbols in the image
  at build time.
- Xous, in std itself (`library/std/src/sys/args/xous.rs`, `sys/pal/xous/params.rs`). A Rust
  microkernel's loader passes a pointer to a parameter block as the second entry argument. Tagged
  blocks (`AppP`, `EnvB`, an argument list) carry arguments and environment. This is the nearest
  neighbor to what A would build here.

None of the four delivers arguments as capabilities. All four deliver bytes and keep authority in a
separate channel.

## BUGS

- `ripgrep` has not searched here yet. The prototype became `crates/program_argument_contract`, and
  milestone 205 boots it at the prompt (`std_exerciser` prints its argv on all three
  architectures). `rg` still has nothing to search: a `std` program at the prompt holds no
  directory until the designation half is built
  (`design/roadmap/0665-designating-a-foreign-programs-words.md`).
- The shell cannot spawn a std program at all. All 16 `grant_plan::Program` entries are native, and
  `ripgrep` runs only from the kernel test harness (`fs_service::start_std_full`). Every option
  inherits this gap. The demonstration at the prompt that milestone 121 (`ripgrep` on nife) wants
  needs it closed. Proposed to the maintainer as a milestone of its own. *Partly closed
  2026-09-26 by milestone 595 (provisional), in which the shell runs a `std` program: the
  progenitor builds `std`'s layout and `std_exerciser` runs from the prompt. `rg` now hears its
  arguments (milestone 205) and still holds no directory to search.*
- gitoxide has not been built for a nife target. Its argument path was read from source, not run.
- The counts are source counts from one release of each program. `ripgrep`'s flag count was taken
  with a script over `impl Flag for` blocks, and gitoxide's by matching field declarations. Either
  could be off by a few; neither could be off by enough to make a table small.
- A byte argv can carry a secret. §111 refused free-form strings on the config page for exactly
  that reason, and validation cannot help here, because a regex is arbitrary bytes. Neither
  program measured takes a secret on its command line. The risk is real for others and would need
  a policy answer, not a wire one. §170 records it as a known cost of the ruling.
