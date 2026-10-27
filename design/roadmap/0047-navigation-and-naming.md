---
status: BUILT
raised: 2026-07-31
built: 2026-10-03
---
# 47. Navigation and naming: `cd`, `pwd`, `ls`, `mkdir`, `rm`, paths, and environment

Every item is Done, refused or routed to a numbered milestone, so the block is `BUILT`. The set
grant at the prompt follows §228 (how a set of matched names reaches the progenitor), built by
milestone 205 (how a foreign program is told what to do). The detail of each piece is in
[`0047-navigation-and-naming/`](0047-navigation-and-naming/README.md); this page is complete
without it.

| Piece | Where it stands | Appendix |
|---|---|---|
| `mkdir`, `rm`, `RMDIR` and `rm -r` | built, §49 (removal is a directory operation) | [mkdir-and-rm.md](0047-navigation-and-naming/mkdir-and-rm.md) |
| `ln` | none: hard links declined by §110 (hard links are declined), symlinks replaced by `bind` under §50 (namespace composition) | [links-and-bind.md](0047-navigation-and-naming/links-and-bind.md) |
| `touch`, both halves, and globbing with its set grant | built, §112 (`touch`'s two behaviors need two rights) | [touch-and-globbing.md](0047-navigation-and-naming/touch-and-globbing.md) |
| Absolute paths and `bind` | built; `bind` in a two-grant shell host-tested only (milestone 154 (a process that holds two directory capabilities)) | [absolute-paths-and-bind.md](0047-navigation-and-naming/absolute-paths-and-bind.md) |
| Environment: the config page of §111 (inert configuration is a read-only page), `printenv`, `caps printenv` | built; secrets belong to milestone 65 (a secrets service) | [environment.md](0047-navigation-and-naming/environment.md) |
| `PATH` and completion | built, §229 (how a bare name at the prompt reaches an installed program) B2 and §227 (how Tab reaches the shell) option D | [path-and-completion.md](0047-navigation-and-naming/path-and-completion.md) |
| `file:` and `run` | removed; the function-call syntax refused by §141 (application is grant) | [shell-grammar.md](0047-navigation-and-naming/shell-grammar.md) |

In brief. A navigation model for a system with no global namespace. Keep the Unix command names
and behavior wherever they can work honestly; diverge only where the capability model forces it, and
say why each divergence is earned. The keystone is built: the directory capability and its
rights ladder (DECISIONS §47 (the directory capability's rights), notes/dir-capability.md), six rungs
at the keystone and a seventh added by DECISIONS §112 for `touch -t`. So are the five commands, on
both ISAs. `cd`, `pwd`, `ls`, `mkdir` and `rm` work as shell builtins. `..` is clamped at your root
by popping the stack of capabilities the shell descended through, and `pwd` is relative to that
root. A name on a command line is resolved against the shell's position at the moment the grant is
made, so a child holds a capability to one file and cannot re-resolve anything. The headline is
proven with the real shell binary: two shells rooted in two subtrees, each told nothing about which
it holds, and neither can name the other's files (notes/shell-navigation.md).

Why it matters. calef's framing, and it is the governing constraint: *"I hate Windows/DOS
specifically because they went differently than virtually every other OS I've used."* Gratuitous
divergence taxes every user forever. So the bar is not "is this more capability-pure", it is "does
the model actually force this." Three divergences clear that bar; the rest of Unix's surface should
survive unchanged.

## The reframe: `cd` was never the problem

A working directory, in capability terms, is *a directory capability the shell holds, used as the
default base for resolving names*. Held by the shell that is entirely legitimate, the same as its
untyped budget. The badness in Unix is three specific things, none of which is `cd` itself:

1. Children inherit it silently, so every process gets a starting point nobody granted it.
2. Relative paths resolve implicitly, so a program's reach depends on invisible state.
3. `..` walks out, so the cwd bounds nothing.

Fix those three and the command is fine.

## `cd`, `pwd`, `ls` are shell builtins, not programs

The same category as `caps`, which already prints the shell's whole endowment: they spawn nothing,
need no grant, and confer no new authority, because the shell is reading and rebinding what it already
holds. So no listing program is over-granted with the power to read everything it lists: `ls` is
not a program.

The cwd stops at the process boundary. `wc report.txt` resolves the name against the
shell's current directory *at the moment the grant is made*, and the child receives a capability to
that one file. The child has no cwd, inherits nothing, and cannot re-resolve anything. The convenience
is the shell's; the authority is explicit.

## The earned divergences

- `..` stops at your root. You descend from what you hold and never ascend past it. This is
  chroot's shape arrived at from the other direction.
- `pwd` is relative to your root, because naming anything above it implies a namespace that does
  not exist.

Absolute paths are not a divergence: `/` is the root of your own namespace, Plan 9's answer
([absolute-paths-and-bind.md](0047-navigation-and-naming/absolute-paths-and-bind.md)). The
`InvalidFilename` refusal survives for the two cases that still name nothing, `..` past the root
and a Windows-shaped prefix.

What that buys, and Unix cannot: every shell has its own root. Two shells can hold different
subtrees and neither can name the other's files, not by policy but because no capability reaching them
exists.

## The primitive underneath

`cd`, `mkdir`, and per-process namespaces each converge on the same primitive: a verb that returns
a directory capability rather than bytes. It is the first place this contract hands back authority
instead of data. It deserves the care `Endpoint::REAP` got under §32 (a supervisor may collect a
corpse without being able to build one). What rights does the child directory carry, can they ever
exceed the parent's, and who may call it?

## Follow-on

- **Done.** The shell's heap: fallible, capped at 32 KiB, and no substituted value re-split
  (notes/heap.md).
- **Done.** Nothing here waits on milestone 122 (`OPENDIR` reaches the PAL), which is BUILT (pull
  request #320) and proven on both ISAs by `std_exerciser`.
- **Done.** `caps` prints a bound name's own row: `crates/swish/src/lib.rs`, with a test asserting
  `bind recent -> /logs/2026`.
- **Done.** Tab completion under §227 option D: see
  [path-and-completion.md](0047-navigation-and-naming/path-and-completion.md) and
  notes/shell-line-editing.md.
- **Milestone 657.** Milestone 657 (a live history search at the prompt). fish's extras, recorded rather than built: suggestions from history
  (`design/roadmap/0679-the-prompt-suggests-from-history.md`), coloring the first word by
  whether it can run (`design/roadmap/0678-the-prompt-colours-what-it-can-name.md`), a live
  `^R` search (`design/roadmap/0657-a-live-history-search-at-the-prompt.md`), and argument
  completion from the manifest (`design/roadmap/0662-argument-completion-reads-the-manifest.md`).
- **Done.** `PATH` as §229 B2. A bare word resolves through the live activation set
  (`activation_set::lookup_name`, which skips owner vouches). A name both the image and a package
  have is refused naming both, and install refuses a name another package provides (`NameTaken`)
  or the image carries (`ImageName`). An owner's vouch sits beside a package's entry. Proven by
  host tests and by `script/swish-check` (`greeting` and `noteless` by bare name, `package install
  downloads/uptime.nifepkg` refused, `unvouched` reaching nothing). The prompt's both-names refusal
  is host-tested only, since no gate can make the pair (`swish::bare`'s BUGS).
- **Milestone 65.** Environment's secrets third. A secret here is an endpoint (§41 (the endpoint
  is the broker)), the service that holds one is milestone 65's (BUILT), and where a stored secret
  comes from is §165 (where a stored secret comes from). §220 (signed builds) recommends building no signer on nife until a
  build made on nife must run elsewhere. There is nothing for a navigation milestone to build here.
- **Done.** `caps printenv` prints the inert-config values. The progenitor places the frame it
  endows children with in the boot shell, `READ` only, at `grant_plan::SHELL_CONFIG_SLOT` and maps
  it at `SHELL_CONFIG_VA` (both provisional). Host-tested in `crates/swish`, proven at a real
  prompt by `script/swish-check`. A shell choosing a *different* set per child is not built and has
  no customer; it and a `login` session's blind preview are in notes/env-config.md's BUGS.
- **Milestone 154.** `bind` in a two-grant shell stays host-tested only. Both real entry points
  still pass no second directory (`components/src/swish.rs`, `user/src/system_initializer.rs`), and
  `crates/system_initializer` calls the path unverified against a real boot.
- **Done.** The set grant at the prompt, per §228 option 2b with option A
  (notes/a-set-grant-at-the-prompt.md), built by milestone 205. A pattern matching two or more
  names reaches the progenitor as one read-only frame (`spawnproto::NAMESET_BIT`) and the
  progenitor builds `fs_nameset_caretaker` over a copy of it.
- **Refused.** What a stored `..` means to a shallower holder, and the `rm -r link/` trailing
  slash, are moot: `design/decisions/0050-namespace-composition.md` chose composition over stored
  paths, so there is no stored path to clamp or to slash.
- **Decision.** Hard links are declined, in `design/decisions/0110-hard-links-declined.md`: no
  customer, `RENAME` already covers the atomic-replace idiom, and a DAG would cost an audit of
  every subtree argument in the tree.
- **Refused.** Function-call shell syntax: DECISIONS §141 records the model and refuses each
  notation with a measured reason.

## Index row

divergence from Unix must be earned, never stylistic. Keep the commands; change only what the
capability model actually forces, and get one missing primitive right
