# Command-line rulings

*An appendix to [`design/naming.md`](../naming.md), which is the rule. It holds the spellings ruled
under calef's "We abbreviate on the command line." (2026-10-06), whose heading is in
[vocabulary-rulings.md](vocabulary-rulings.md), a document with no room left under the prose
budget. The file and its stem are provisional; naming is an architect's.*

## `jig` takes apt's verbs

calef, 2026-10-10 UTC, on milestone 809 (the package client becomes a program)'s fork 4: *"Lets
use apt's verbs."* Where apt has a verb for the act, `jig` spells it apt's way; where it has none, the spelling is nife's own and says so. The
same day he changed one of nife's own: *"Change add-index to add-repository."*

| `jig` | the act | whose spelling | ratified |
|---|---|---|---|
| `list` | the packages the local copy of the index (today, the image's catalog) offers | apt's | 2026-10-10 |
| `install <name>` | fetch a package by name and install it | apt's | 2026-10-10 |
| `install <path>` | install a package file already on the system; a word with a `/` is a file | apt's verb | 2026-10-10 |
| `remove <program>` | a new generation without the program | apt's | 2026-10-10 |
| `update` | refresh the local copy of the index, installing nothing | apt's | 2026-10-10 |
| `list --upgradable` | what is outdated, against the index copy; not an `outdated` verb | apt's, as ruled | 2026-10-10 |
| `rollback` | the generation below the live one becomes live | nife's: apt has no rollback | 2026-10-10 |
| `add-repository <url>` | add another index | nife's, after Ubuntu's separate `add-apt-repository` tool | 2026-10-10 |

`upgrade`, apt's verb for installing newer versions of what is installed, is reserved, not built:
it is the spelling the "`jig` must update base packages" amendment (calef on #1805, 2026-10-07)
will use.

Read 2026-10-10 UTC, rather than recalled:
[apt(8)](https://manpages.debian.org/stable/apt/apt.8.en.html) ("update is used to download package
information from all configured sources"; no rollback verb).
[add-apt-repository(1)](https://manpages.ubuntu.com/manpages/noble/man1/add-apt-repository.1.html),
"a script which adds an external APT repository", ships in `software-properties-common`, not apt.
FreeBSD's [pkg-update(8)](https://man.freebsd.org/cgi/man.cgi?query=pkg-update) refreshes the
repository's catalog, in its own words:

> pkg update is used for updating the local copy of the repository catalogues

and `pkg version -l '<'` lists what is outdated. The
[Homebrew manpage](https://docs.brew.sh/Manpage)'s `brew update` fetches the newest formulae, and
its `brew outdated` is a verb of its own. None of the three has a
rollback; Nix's `nix-env --rollback`
([manual](https://nix.dev/manual/nix/stable/command-ref/nix-env/rollback)) "switches to the
'previous' generation", which is the shape `jig rollback` has.

Two things the reading found that the ruling's wording did not say. apt(8) spells the flag
`--upgradeable`, with an `e`; the ruling's `--upgradable` is held as written until calef says which
(nothing is built on it yet). And apt(8) documents no `apt install ./file.deb` form: apt-get(8) says
"Each package is a package name, not a fully qualified filename". `jig install <path>` keeps the
verb and takes a path by the prompt's own rule for a word with a `/` (§219 (how the shell names an
installed program to the spawner)), not by apt's.

