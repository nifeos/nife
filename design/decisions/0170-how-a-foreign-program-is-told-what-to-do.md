---
status: AMENDED
raised: 2026-09-19
decided: 2026-09-26
ratified_by: calef
---

# 170. How a foreign program is told what to do

Ruled by calef on 2026-09-26 (UTC), built up over one conversation with the maintainer after the
shim was priced in [notes/foreign-program-arguments.md](../../notes/foreign-program-arguments.md)
(#1314).

## The ruling

A program written by somebody else hears its arguments as bytes. What those bytes may touch is
decided separately, by the program's manifest and by the directories the line grants.

1. Arguments arrive as plain bytes: a POSIX-shaped argv in one page, carrying no authority. The
   measurement priced this at about 50 lines, written once, with no new syscall.
2. A command-line word that resolves to an existing file is granted that file, in the way the
   program's own manifest declares: read-only, read-write, or the file's directory. The manifest
   travels in the ELF, per §197 (a package is one archive file), M2, ruled the same day.
3. Any program's manifest may also declare "may create the named path" for a word that does not
   yet resolve, as in `cc -o main` or `tar cf x.tar`. This holds for vouched and unvouched programs
   alike. In practice it is write authority for that one name in its directory.
4. For an unvouched program, the default is read-only. It has no vouch, so under §219 (how the
   shell names an installed program to the spawner) its own note grants nothing. A named file is
   granted read-only, and read-write or create each need an explicit mark on the word. This
   section calls it the read-only default.
5. The directories granted on the line bound everything else. `..` and absolute paths are already
   refused, by the `std` layer before they reach the wire.

calef's framing of the read-only default: unvouched programs are copied binaries or fresh builds,
and locking them down by default means they can do little damage.

The maintainer's correction to that framing, recorded as such. This is confinement, not
antivirus. It does not recognize bad code; it bounds all unvouched code alike. An unvouched program
can still:

- read what was named on its line;
- write where `>` points;
- use the processor time and memory it was granted;
- print misleading output;
- damage the one file whose word carried a mark.

## Still open

Environment variables and exit codes for a foreign program. They are the same family, and this
ruling is silent on both. §111 (inert configuration is a validated page) covers the native case
only.

The block's layout. Two programs agree on it, so milestone 205 proposes it and a ruling fixes it.
The note lists what is in it:

- where the page sits, a slot or `std`'s ignored entry registers;
- whether `argv[0]` is present, which the note shows it must be;
- bytes rather than UTF-8;
- the one-page ceiling of 4,080 bytes.

## Refused, with reasons

| option | why it lost |
|---|---|
| B. Designation per token, from a table per program | Only the program knows its own grammar. The table is 104 flags for `ripgrep` and 260 fields for `gitoxide`, redone per release, and neither is derivable in general. |
| C. `grant_plan` as the only channel | It carries one `u64` and a 64-bit flag mask. It cannot carry a regex, so `rg pattern` cannot be said at all. |
| Asking y/n at the prompt before granting | It breaks scripts and pipelines, which have nobody to answer. |

## Recorded as a later refinement, not ruled

Honoring an unvouched binary's note up to a ceiling set per session. That ties to §220 (signed
builds: a vendor signs, a developer self-signs), which is PROPOSED. Until something rules it, an
unvouched note grants nothing, as clause 4 says.

## Known cost

A secret passed on a command line is plain bytes, handed to the program with no protection. §111 refused free-form strings on the configuration page for this reason.
No wire can help, because a regex is also arbitrary bytes. Programs that take a secret this way
need a policy answer, which is §41 (the endpoint is the broker)'s shape.

## The analysis the ruling was made against

The rest of this section is the record as it stood before 2026-09-26, shortened.

### The premise, measured 2026-09-19

The block said *"The nife ABI has no argument vector"*, and that was right. It was not true that
programs here take no arguments. `grant_plan::Endowment` carries `arg: u64` and `flags: u64`, where
bit `i` is set when the manifest's `flags[i]` was typed. A program declares what it takes in a
`Manifest`, and the shell binds tokens into those slots or prints a typed refusal. What unmodified
`ripgrep` reaches for is `std::env::args()`, which compiles std's `unsupported` backend and yields
nothing.

### What the tree already did in the analogous case

Milestone 47 (navigation and naming) split Unix's environment map in three. Inert configuration
became a validated read-only page (§111). Names were answered by the namespace, because designation
is authorization. Secrets went behind an endpoint (§41). "Arguments" is the same kind of word: a
path is a designation, `--jobs 8` is data, and `--password` is a secret. The ruling keeps that
split. Bytes carry data, the manifest and the line's directories carry designation, and secrets stay
a known cost. §15 (the native ABI) had chosen capability slots over a self-describing environment,
and the ruling keeps authority out of the bytes for the same reason.

### The options as first written

| | shape | cost as first written |
|---|---|---|
| A | An argv, as POSIX has it | What ported programs expect. It puts designation, data and secrets back in one string vector. |
| B | A nife-shaped equivalent: designations as capabilities, inert values as data | Coherent with §15 and milestone 47. Every foreign program needs a shim, then uncosted. |
| C | `grant_plan` is the only answer, and a shim is policy | The most honest about this system and the least welcoming to the corpus of milestone 123 (the demonstration: somebody else's software, running narrow). |

The measurement found a fourth reading of B, used by Fuchsia, Genode, seL4's `sel4utils` and Xous:
bytes travel as an argv and authority travels separately, as a namespace. That is what clauses 1
and 5 rule, with clauses 2 to 4 adding what a manifest may say about a single word.

## How reversible it is

Not at all, in the way that matters. Every future program and every port is written against it.
Nothing outside this tree had acted on it at the ruling.

## What the ruling unblocks

Milestone 205, which builds the transport and the designation rules above. Behind it, everything
milestone 121 (`ripgrep` on nife) still owes: the confined `rg`, the loud `ENUMERATE` refusal and
the walk benchmark. Milestone 595 (the shell runs a `std` program)'s `rg pattern` step. Scripts
that take a parameter (§219). Milestone 123's corpus still waits on §171 (where a program image
starts) as well.

## Amendment (2026-09-27, 06:27Z): a word that names nothing grants nothing (N1)

Clause 2 says what a resolving word is granted. It left open what a word that resolves to nothing
grants, and calef ruled it: **a command line whose words name no existing file grants nothing.**
There is no directory implied by absence. To search here, you type `rg pattern .`; the `.` is the
word that does the granting, not a default filled in when nothing was typed.

The alternative, N2, was to grant the shell's current directory read-only whenever the line names
none. It was refused. Naming is what authorizes everywhere else in the shell: `rm`, `wc` and
globbing all require a word that names what they touch, and none of them falls back to "wherever
you happen to be" when a word is missing. N2 would have been the first grant in this system that
nobody typed. It remains possible to add later, because widening a grant breaks no script that
already runs without it; refusing it now costs nothing that narrowing later would not also have to
pay.

## Amendment (2026-09-27, 06:35Z): there is no mark

This resolves the mark's spelling, which the clauses above leave open. calef's
ruling is that there is no mark to spell.

- An unvouched program is granted read-only on each word on its command line that names an
  existing file, plus `>` redirection for its own output. That is the whole of what it gets.
- To give a program more than that, you install it: vouching, and its manifest then applies, per
  §208 (installing is granting).
- Clause 4's "read-write or create each need an explicit mark on the word" is withdrawn. There is
  no mark, so an unvouched program cannot be handed read-write or create authority on the line at
  all.
- Clause 3's "This holds for vouched and unvouched programs alike" no longer covers the unvouched
  case: an unvouched program's own note grants nothing, so it cannot declare its own create-path
  clause. Only a vouched program's manifest can still say "may create the named path".

calef's question that prompted this: "Why do we need this? Why isn't it contextual?" It already is
contextual, for a vouched program: its manifest says what it may touch, through the mechanism
clause 2 and §208 give it. A mark on the command line only ever mattered for the unvouched case,
and that case is now fixed at read-only, so there is nothing left for a mark to spell.

The reasoning, in calef's framing: a mark exists only to widen an untrusted program past the
read-only default. Choosing its spelling is the expensive kind of decision, because a script would
carry whatever got picked, and every later user of that script inherits the choice. Having no mark
at all is the reversible one instead: a mark can still be added later without breaking any script
that predates it, since no script yet depends on one.

Considered and refused: M1, a prefix (`+file`); M2, a separate word; M3, a flag (`--rw`/`--new`);
M4, a suffix (`file:rw`). A runtime prompt (a powerbox) was refused for the same reason the y/n
prompt above was: it breaks scripts and pipelines, with nobody there to answer it. So was
honoring the unvouched program's own note up to a ceiling set per session, which was already
deferred to §220 (signed builds: a vendor signs, a developer self-signs), itself PROPOSED. Until
§220 is ruled, an unvouched note still grants nothing.
