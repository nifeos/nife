# Naming programs, scripts and directories

*An appendix to [`design/naming.md`](../naming.md#programs-scripts-and-directories), which states
the rules. It holds the reasons behind the spelling per domain, the program conventions, `script/`
against `helpers/`, and directories. The directory `design/naming/` and this file's stem are
provisional names; naming is an architect's.*

## Spelling, per domain

| Domain | Form | Because |
|---|---|---|
| Crates, programs, modules | `snake_case` | Rust's own convention |
| `script/` and `helpers/` entry points | `hyphens` | shell commands are hyphenated everywhere (`apt-get`, `pkg-config`), and an underscore in a command reads as a mistake |
| Ordinary markdown (`notes/`, `design/`) | `hyphens` | file names become URL slugs, where a hyphen separates words and an underscore joins them |
| Repo-root markdown | `SCREAMING_SNAKE_CASE` | GitHub recognizes `README.md`, `SECURITY.md` and `CONTRIBUTING.md` by name and links them in its UI |
| A directory holding a Rust package | exactly the package's name | the directory and the package are one thing |
| Any other directory | `hyphens` if it needs two words | a directory is a path element, and paths are hyphenated outside this repository |

The splits fall across domains, on a property that does not change: a file either is a Cargo target
or is an executable in `script/`. There is no second tier within a domain. A name whose shape a
reader already knows keeps it, so `supply-chain` keeps its hyphen.

## Programs

A component is the shippable unit: one binary in `components/src/`, one `[[bin]]` in
`components/Cargo.toml`, one entry in the initial ramdisk archive. A service is what a component
offers, and a contract is the wire protocol it offers it over. "Server" is a fine role word
(`redoxfs_server` serves the filesystem service).

- Name a component for what it is, in words that parse without prior Unix exposure: `spawner`,
  `console`, `painter`, `line_editor`, `fs_subtree_caretaker`.
- Never `-d`. The suffix claims a Unix daemon, which is defined by what it detaches from: no
  controlling terminal, inherited ambient authority, a pid file, a privileged init. A nife program
  has none of those. `script/lint` checks it.
- The binary name, the source file name and the archive entry name are one string. `xtask` pairs
  them positionally, so a mismatch is a runtime "program not found" and not a compile error. The
  program the kernel loads by name is `progenitor`, so the three agree in every row.
- `c_` means written in C. `c_shim`, `c_seam` and `c_confiner` belong to §31 (the
  foreign-language seam); `c_swappable` is the C side of the `rust_swappable` pair. They are not
  one family.
- Fixtures and benchmarks live in `fixtures/` and take program names. A test fixture is named for
  its role in its test, as a noun phrase ([convention-rulings.md](convention-rulings.md), Rule 2).
- `least_authority_demo` is the canonical minimal program.

### Two suffixes carry a category

- An `_exerciser` puts a capability of the system under load and sees whether it holds, with no
  contract probed from outside: `allocator_exerciser`, `std_exerciser`.
- A `_test_client` exercises a service contract from outside, with a server on the other end:
  `fs_test_client`, `socket_test_client`. The plain names (`fs_client`) stay free for the real
  clients, and a test program that took one would squat it.

## Scripts

Two directories, split by audience.

- `script/` is the front door, typed by people:
  [Scripts to Rule Them All](https://github.com/github/scripts-to-rule-them-all) names, no
  extension, lowercase, hyphenated (`qemu-check`, `vendor-verify`, `supply-chain`). The canonical
  set (`setup`, `test`, `server`, `console`) keeps its standard names even where another word would
  describe more, because the value is that the command matches every repository that follows the
  pattern.
- `helpers/` holds what other scripts and `xtask` call, with an extension (`qemu-bounded.sh`). The
  word says what the drawer is for and cannot be confused with `script`, which `scripts/` could.
  The drawer is outside `script/names`' scope ([provenance-limits.md](provenance-limits.md)), so
  this paragraph is its record: `helpers` ratified 2026-09-23 (UTC, calef). Refused `scripts/`
  (one character from `script/`, sorting beside it, and saying nothing about which is which).

A script that does one action takes a verb: `claim`, `bench`, `fmt`, `record-ruling`,
`vendor-verify`, `toolchain-bump`. A standing tool or role takes a noun: `board-boot-watcher`,
`merge-queue-rebaser`, `project-metrics`. A gate ends in `-check`.

Every `script/` entry needs a row in [scripts.md](../../notes/scripts.md). `script/lint` fails
without one, and fails the other way if `README.md` names a script that does not exist.

## Directories

- A directory that holds a Rust package takes exactly the package's name.
- Any other directory is lowercase, hyphenated if it needs two words.
- `target/` is gitignored build output, and `targets/` holds the tracked custom target JSON. Nothing
  enforces the distinction.
- A document keeps its lowercase hyphenated file name when the thing it describes takes a
  `snake_case` name: `notes/line-discipline.md` describes `line_editor`.
