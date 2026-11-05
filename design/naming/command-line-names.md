# Command-line names

*An appendix to [`design/naming.md`](../naming.md#command-line-names), which states the rules. It
holds `jig`'s verbs and their prior art, and the reasons behind the builtin rules.*

## The command line abbreviates, and the code behind it does not

A person types a command many times, so a command-line spelling may abbreviate. The short form stops
at the option: `--mem` is the option, and the identifiers behind it say `memory_grant`. Where both
appear, the code records the difference as deliberate, so a reader does not take it for drift. Each
abbreviation is still a name to ratify.

A letter or word after a command is an option, as POSIX says, never a flag. The words that follow
the options are operands.

## `jig` takes apt's verbs

Where apt has a verb for the act, `jig` spells it apt's way. Where apt has none, the spelling is
nife's own and says so.

| `jig` | the act | whose spelling |
|---|---|---|
| `list` | the packages the local copy of the index offers | apt's |
| `install <name>` | fetch a package by name and install it | apt's |
| `install <path>` | install a package file already on the system; a word with a `/` is a path, by the prompt's own rule ([§219 (how the shell names an installed program to the spawner)](../decisions/0219-naming-an-installed-program-to-the-spawner.md)) | apt's verb |
| `remove <package>` | a new generation without every row the package installed; the object is a package, never a program | apt's |
| `update` | refresh the local copy of the index, installing nothing | apt's |
| `list -u` | what is outdated, against the index copy; `--upgradable` and `--upgradeable` fail with a message naming `-u` | apt's short option |
| `rollback` | the generation below the live one becomes live | nife's: apt has none, and Nix's `nix-env --rollback` has this shape |
| `add-repository <url>` | add another index | nife's, after Ubuntu's `add-apt-repository` |
| `upgrade` | reserved for installing newer versions of what is installed, not built | apt's |

Prior art, read from the manuals: [apt(8)](https://manpages.debian.org/stable/apt/apt.8.en.html),
[add-apt-repository(1)](https://manpages.ubuntu.com/manpages/noble/man1/add-apt-repository.1.html),
FreeBSD's [pkg-update(8)](https://man.freebsd.org/cgi/man.cgi?query=pkg-update), the
[Homebrew manpage](https://docs.brew.sh/Manpage) and Nix's
[`nix-env --rollback`](https://nix.dev/manual/nix/stable/command-ref/nix-env/rollback). apt(8)
documents no `apt install ./file.deb` form, which is why `jig install <path>` follows the prompt's
rule and not apt's.

## Shell builtins

A builtin is a word the shell answers itself, and after a program's name it is the most
reader-facing name in the tree. `script/lint` cannot check one, because a builtin is a match arm in
`grant_plan::parse`, not a file.

- A builtin takes Unix's word where one exists: `cd`, `pwd`, `ls`, `mkdir`, `echo`, `time`,
  `xargs`, `apropos`. A reader arriving from anywhere else knows what it does before running it.
- A builtin never shares its first word with a program, since the shell matches builtins first.
  `doc search <term>` would shadow the `doc` viewer for every line that begins with `doc`, so the
  search builtin is `apropos`.
- A generic word that could name almost anything (`search`) is refused, and so is a Unix word for a
  different job (`find` walks a directory tree, which this system cannot do).
- `caps` is ours: it prints the process's endowment. It is a command name, not an identifier, so
  the `capability` rule does not reach it; whether it becomes `capabilities` is an open naming
  question, and its `caps_*` test names follow the answer.

The shell itself is `swish`, because shell names are identities (`bash`, `zsh`, `fish`), not
descriptions. `capsh` is Linux libcap's capability wrapper, so a reader from Linux would assume ours
is that tool.

## BUGS

- The boot mode is `shell` (`cargo xtask shell`, the kernel's `--features shell`) while the program
  is `swish`. The word names a configuration (boot straight to a prompt), not the binary, so a grep
  for `shell` meets a word that names no program.
