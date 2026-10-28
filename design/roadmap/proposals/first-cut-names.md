---
status: PROPOSED
raised: 2026-10-28
milestone_dependencies: 756
decision_dependencies: 316
machine_requirements: none
specific_machine: none
needs_person: no
---
# The first cut's names

*(Title and slug provisional.)*

Raised 2026-10-28 (UTC) by the lane of milestone 756 (process-tools, core-tools and disk-tools are
the first code to leave this repository), as that block's naming half. §316 (one repository per
package leaves the tree) rules the grain the cut leaves at; this brief holds the names for calef's
review. Nothing here carries a ruling. calef ratifies names, and every name below stays provisional
until they rule.

Reuse: the six surfaces each name must survive come from the tree's own records (§305, §250, §307)
rather than a new list, and the freezing evidence is
[notes/the-first-split.md](../../../notes/the-first-split.md)'s measurements. This brief writes no
code; the deliverable is the review.

## The six names

Three package names, calef's rulings of 2026-10-06 (UTC), each in place of a distribution's term of
art (`procps`, `coreutils`, `util-linux`), and the three repository names they become under §316's
rule. Each `packages/<name>.package.toml` already points `home` at the same string, with
`status = "provisional"`; the naming review ratifies the names, and a home status flips when each
repository exists.

| string | names today | becomes, under the ruling |
|---|---|---|
| `process-tools` | the package of Debian's procps programs (milestone 126 (the `procps` package)) | the repository `nifeos/process-tools` |
| `core-tools` | the package holding `date`, `printenv`, `rm`, `uuid` and `wc` | the repository `nifeos/core-tools` |
| `disk-tools` | the package holding `disk_surveyor` and `disk_partitioner` | the repository `nifeos/disk-tools` |

Each name must survive six surfaces at once:

1. A GitHub repository name under `nifeos`.
2. A git dependency URL fragment, `https://github.com/nifeos/<repository>`, pinned by `rev` to a
   landed commit (§305 (git dependencies on `nifeos/*` only)).
3. A basalt pin target, `nifeos/<repo>@<sha>`, cited in the pull request that bumps it.
4. A package name in the image's package index (§250 (an image names its distribution's package
   index)), and the head of the package file name
   `<name>-<version>-<architecture>.jig.tar.zst` (§307 (how long a name may be)).
5. A citation site in prose and records: "the `process-tools` repository" in a milestone's gate, a
   moved-things entry, a bump's `nifeos/<repo>@<sha>` line.
6. A row in the workspace's front page, the table of checkouts.

A rename after anything exists touches every surface on that list. A rename today touches three
manifests, the generated package table, and prose.

## The fifteen binaries

| package | programs |
|---|---|
| `process-tools` | `ps`, `pgrep`, `pmap`, `top`, `uptime`, `free`, `vmstat`, `slabtop` |
| `core-tools` | `date`, `printenv`, `rm`, `uuid`, `wc` |
| `disk-tools` | `disk_surveyor`, `disk_partitioner` |

`number_writer` rides with the first group as a crate rather than a program: it is the
right-aligned number writer `free`, `vmstat` and `slabtop` share.

Why this review runs now, and why most of these names are close to frozen.
[notes/the-first-split.md](../../../notes/the-first-split.md) measures the cut's outside references
as 24 by program name and 0 by path, and one of the 24 is a gate: `swish-check` types `ps` at the
real prompt and asserts its output header
(`xtask/src/swish_check/script/process_tools.rs`). No one can rename a program that basalt images
ship without breaking the user scripts that type it. After the first image ships, the
compatibility surface closes. Today it is still open, which makes this the cheap moment for every
rename anyone wants.

### The coined names, read closest

- `disk_surveyor`, which stands in for `lsblk`, and `disk_partitioner`, which stands in for
  `fdisk`. Agent nouns in `snake_case`, no abbreviation in either, and each parses without prior
  Unix exposure.
- `number_writer`, the crate. Its provenance block records what lost: `padnum` (abbreviates
  `num`, and reads as a verb phrase), `decimal_writer` (names a fixed-point type),
  `padded_number` (names the output, not the crate), and `number_column` (reads as an ordinal).

All three pass every check below outright.

### The inherited names, read as a compatibility surface

The thirteen Unix names fail no user's memory, and renaming them buys nothing. Each arrives across
Unix's interface and keeps its owner's spelling; typed commands keep their latitude under §154 (the
acronym test is whether the phrase is spoken). Unix convention abbreviates six of them: `ps`,
`pgrep`, `pmap`, `wc`, `rm` and `uuid`. What each would have to become, if calef wants nife's own
names; the sketches say what the name would have to say, and propose nothing:

| name | reads as today | a nife-owned name would have to say, for example |
|---|---|---|
| `ps` | a process listing where `/proc` does not exist | `process_lister` |
| `pgrep` | the filter over a domain's members | `process_finder` |
| `pmap` | one process's mappings | `process_map` |
| `top` | that listing, ranked by what it costs | `cost_table` |
| `uptime` | how long the machine has been counting | plain English already; a paraphrase such as `boot_age` buys nothing |
| `free` | the machine's memory, and this prompt's share | `memory_report` (today it is a generic word) |
| `vmstat` | the machine's counters since boot | `virtual_memory_statistics`, spelled out, at 29 bytes |
| `slabtop` | where a prompt's job budget went, by kind of kernel object | `budget_table`; this kernel has no slab, so the name would stop pointing at one |
| `date` | the date and time | a word the `time` package's `clock` does not hold, such as `calendar_date` |
| `printenv` | the environment, printed | `environment_printer` |
| `rm` | removal | `remove`, which reopens the shell rule that a builtin and a program cannot share a first word |
| `uuid` | the spoken term of art | `universally_unique_identifier`, spelled out, at 31 bytes |
| `wc` | lines, words and bytes counted | `word_counter`, which narrows what the program counts |

## The mechanical checks, and the conventions applied by hand

`helpers/name_checks.py`, the check `script/lint` runs as "names meet the current naming rules",
covers every crate and program source in the tree. Run over the whole tree on 2026-10-28, it puts
zero failing rows on the nine `process-tools` crates and on each of the fifteen program sources
under `components/src/`. Nothing else this brief names fails either. The checker reaches Rust
names, so the three hyphenated package strings sit outside it; the conventions below cover them.

| name | name_checks.py | acronym test (§154) | nouns over verbs | §307 (how long a name may be) |
|---|---|---|---|---|
| `process-tools` | outside its reach (not a Rust name) | passes: no abbreviation | passes: noun | passes: 13 bytes, under the 16-byte install cap and the 252-byte file name |
| `core-tools` | outside its reach (not a Rust name) | passes: no abbreviation | passes: noun | passes: 10 bytes |
| `disk-tools` | outside its reach (not a Rust name) | passes: no abbreviation | passes: noun | passes: 10 bytes |
| `ps` | passes | passes as an inherited name | passes | passes |
| `pgrep` | passes | passes as an inherited name | passes | passes |
| `pmap` | passes | passes as an inherited name | passes | passes |
| `top` | passes | passes as an inherited name | passes | passes |
| `uptime` | passes | passes | passes | passes |
| `free` | passes | passes as an inherited name | passes | passes |
| `vmstat` | passes | passes as an inherited name | passes | passes |
| `slabtop` | passes | passes as an inherited name | passes | passes |
| `date` | passes | passes | passes | passes |
| `printenv` | passes | passes as an inherited name | passes | passes |
| `rm` | passes | passes as an inherited name | fails alone (a verb), passes as Unix's word | passes |
| `uuid` | passes | passes as an inherited name | passes | passes |
| `wc` | passes | passes as an inherited name | passes | passes |
| `disk_surveyor` | passes | passes | passes | passes |
| `disk_partitioner` | passes | passes | passes | passes: 16 bytes, the longest program name here, far under the bound |
| `number_writer` | passes | passes | passes | passes |

None fails. A pass is not a claim to be right: design/naming/rules-a-script-can-check.md holds the
rules no script can apply, and applying them to people's memories is what this review is for.

## The pattern the first cut sets

These three names are the template the next leaver copies. milestone 691 (packages move out of
this repository, one per pull request)'s queue holds `login`, `init`, `terminal`, `network`,
`display` and the rest, and each takes its repository name from its package name the way these
three do. A rename today edits three manifests and prose. A rename after a second package leaves
also edits that repository's name on GitHub, the pins that point at it, and every citation written
in the meantime. The cheap moment is before the second package exists, and the first cut is what
sets the pattern the second one copies.

## Questions for calef

Each question below is one decision. If you do nothing, the default happens: the provisional name
stands, and stays flagged provisional in its provenance block.

1. `process-tools`, as the package name and the repository name. Ratify, or rename. The 2026-10-06
   ruling refused `procps` (the GNU/Linux term of art, given up so package names follow one
   spelled-out rule). `process-utils` loses to the same rule (it abbreviates "utilities"). The
   `-tools` suffix is what makes the three names read as one family. Default: `process-tools`
   stands.
2. `core-tools`, as the package name and the repository name. Ratify, or rename. The 2026-10-06
   ruling refused `coreutils` for the same reason. `core-utilities` spells the word out and
   lengthens the name, and reads as two nouns with no role named. Default: `core-tools` stands.
3. `disk-tools`, as the package name and the repository name. Ratify, or rename. The 2026-10-06
   ruling refused `util-linux` as "a horrible package name for a nife package"
   ([notes/package-boundaries.md](../../../notes/package-boundaries.md)). `disk-utils` loses to the
   same spelled-out rule. Default: `disk-tools` stands.
4. The coined names `disk_surveyor` and `disk_partitioner`, and the crate `number_writer`. Ratify,
   or rename; its provenance block or the table above records what each replaced. Nothing
   ships yet, so a rename today breaks no user script; after the first basalt image it would.
   Default: each stands.
