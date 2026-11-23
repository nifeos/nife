# Milestone 47 appendix: `PATH` and completion

An appendix to [milestone 47 (navigation and naming)](../0047-navigation-and-naming.md). The page is
complete without it. It holds why there is no `PATH` search, how a bare name reaches a program, the
questions about the program namespace that stay open, and how Tab completion works.

## `PATH`: there is no search, because there is no ambient namespace to search

The absolute-paths section ([absolute-paths-and-bind.md](absolute-paths-and-bind.md)) takes Plan
9's answer for paths in general. `PATH` is that same question narrowed to programs, and Plan 9
answers it the same way. Plan 9 has no `PATH` variable at all. `/bin` is bound per-process,
union-mounted from whatever that process's namespace assembled, so what you can run is what is
bound.

`PATH` is two bad things at once. It is a *search*, over a namespace you have *ambient access to*.
The search makes the order of a string into a security boundary: a writable directory ahead of a
system one, or `.` anywhere in it, and someone plants an `ls` that you then run. The ambient access
is why the order matters at all. `PATH` never controlled *access* (permissions did), only which of
your already-reachable options wins. The tell is that `which` exists as a whole command whose job is
answering "which one did I actually get?".

So the program namespace is the endowment, and a name binds to exactly one thing in it. The
property that follows is the same class as `rm -rf /` (in [mkdir-and-rm.md](mkdir-and-rm.md)):
`PATH` injection is structurally impossible rather than mitigated. There is no search order to
manipulate, no `.` to include by accident, and no writable directory that can precede a system one,
because there is no search.

A shell may extend its program namespace only with capabilities it already holds, so extending is a
naming convenience and never an authority increase. Unix nominally has this property too and loses
it in practice. Ambient authority means everyone can read `/usr/bin`, so `PATH` order becomes the de
facto security boundary. Here it cannot be, because naming and access are separate things.

Installing a program is granting it into a namespace, per §208 (installing a package is granting
it, and the installed programs are versioned). Milestone 49 (users, login, and attribution) is what hands
a session its program namespace, so "who gets which capabilities at startup" includes which
programs. Milestone 39 (repository structure for a loosely-coupled OS, and the road to a
distribution) owns the packaging consequence.

## How a bare name reaches a program

§229 (how a bare name at the prompt reaches an installed program) is the rule. A bare word at the
prompt resolves to an image program, or through the live activation set
(`activation_set::lookup_name`), which skips owner vouches. Each name has one entry, so there is no
search order. A name both the image and a package carry is refused, naming both. `package install`
refuses a name another package already provides (`NameTaken`) or the image carries (`ImageName`).
A path still runs a program's file directly: a token with a `/` is a file's bytes.

The prompt's both-names refusal is host-tested only, since no gate can now make the pair
(`swish::bare`'s BUGS).

## What stays open about the program namespace

- Unions and shadowing across sources beyond the activation set. A namespace unioned from several
  sources brings first-match-wins back, which is the ambiguity just removed. Plan 9 chose ordered
  union with explicit before/after on `bind`. The alternative is to refuse ambiguity (an error when
  two sources offer `ls`), which is more honest and probably more irritating. §229 takes the refusal
  for the image and the activation set.
- Enumeration. "What can I run?" is enumeration of the namespace. It is the same insight as globbing
  and completion ([touch-and-globbing.md](touch-and-globbing.md)) and bounded the same way:
  completion cannot offer a program no capability reaches.
- Does `$PATH` survive as a string? If the namespace is a capability, `echo $PATH` has no referent,
  and that is a divergence on one of the most-referenced variables in shell scripting. It looks
  earned (the variable's two real uses, inspect and modify, become `caps` and a grant). But the cost
  is real and should be named rather than glossed.

## Completion

§227 (how Tab reaches the shell) is the rule: the shell edits its own line, and the terminal wire
does not change. The shell turns raw mode on at the prompt and runs `line_editor::LineDisc` itself.
Tab is `Event::Tab`, and `swish::complete` finishes the word from what the shell can name. In
command position that is a builtin or an image program. Otherwise it is an entry of the directory
the word leads into, listed with the same `ENUMERATE` `echo *` needs. So the completion menu is a
rendering of your authority and cannot offer a path no capability reaches.

`^C` at the prompt is a byte the shell's editor turns into a discarded line. A supervised job still
takes `^C` through the terminal's count, per §24 (interrupting the foreground process), so the shell
leaves raw mode just before it spawns one. Host tests in `line_editor` and `crates/swish`
cover it, and `script/swish-check` types a Tab twice and a `^C` once at a real prompt on all three
architectures. The binary cost and the per-keystroke cost are measured in
notes/shell-line-editing.md, with its BUGS. fish's extras are proposals (the Follow-on on
[the main page](../0047-navigation-and-naming.md)).
