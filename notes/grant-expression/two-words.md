# Two words came out of the grammar, and why (milestone 47)

An appendix to [notes/grant-expression.md](../grant-expression.md), moved there on 2026-10-11
(UTC) under §212 (a prose budget). It is the argument for dropping `run` and `file:`, and what
the parser stopped doing.

Phase 1 spelled the same thing `run [--mem N] <prog> [arg] [file:PATH ...]`. calef asked to be
convinced the two extra words earned their keep; they did not.

`run` failed on consistency. Milestone 47 (navigation naming) adds `ls`, `cd`, `pwd`, `mkdir` and `rm` as shell
builtins, and nobody would type `run ls`. Keeping the verb would mean builtins are bare words while
programs need a prefix, so a user has to know *which class a command is in* before knowing how to
type it. That is the gratuitous divergence the milestone exists to refuse, and milestone 50 finishes
the argument: `run a | run b` is indefensible. What replaces it already existed, since resolving a
name to a program is `Prog::from_name` either way. The cost is three reserved words: `help`, `echo`
and `caps` win over a program of the same name, so the program namespace must not contain them.

`file:` failed because it announced the wrong half of the grant. `wc report.txt` reads and `tee
report.txt` writes: identical syntax, opposite authority, because the direction lives in the manifest
by design. The prefix decorated *which file*, which was already on the screen, and was silent about
read-versus-write, which is the part that decides what the child can do.

Its safety argument failed on inspection too. `least_authority_demo 5 extra` is refused as unplaceable because
least_authority_demo's manifest says `FileGrantDeclaration::Forbidden`, not because of any prefix: the manifest was doing all
the work and the prefix was taking credit. The one thing the prefix genuinely bought is kept, in
the place where it applies. A token shaped like a flag (`--secret`) never falls into the file
position, because that is the one way a typo could become a capability transfer.

The deeper reason a prefix could never carry the thesis: the capability claim is about absence.
That a filename grants access to that file surprises nobody. What `wc report.txt` proves is that wc
got that file *and nothing else*, and that claim lives in the tokens which are not on the line. A
prefix decorating a token that is present cannot express it. `caps <command>` can, which is what
makes `caps` the visibility surface now that the designator is gone.

## The parser stopped classifying, and that is the load-bearing part

`parse` keeps the positional tokens in the order typed and refuses to say which is which;
`plan_against` places them into the slots the manifest declares. So `wc 2026` designates a file named
`2026`, which a shape-based rule ("a number is the argument") would have got wrong. It means
"which token is the file" and "may this program have a file at all" are answered by the same
declaration, which is the honest version of what the prefix pretended to do.

The window is why it happened now. No program today takes both an argument and a file, so
positional resolution is at most one bare token. The first program that wants both (`grep pattern
file.txt`) forces `ProgramArgumentDeclaration` to grow position and arity, and this change would have been a redesign
instead of an edit.

Two limitations, named where a reader meets them. A file whose name begins with `-` cannot be
designated (it reads as a flag), which is Unix's problem too and Unix's answer (`--`) is available
when something needs it. And with the prefix gone, the "you hold no such capability" refusal is only
reachable through a manifest that declares a file. So no *shipped* program can produce it at the
prompt today: `least_authority_demo report.txt` now answers "least_authority_demo: takes no file; drop the name", which is the
durable fact about least_authority_demo rather than an accident of this shell's endowment. That reordering is
deliberate; see the refusal catalog below.

## One refusal ordering changed with the designator

**One ordering changed with the designator, deliberately.** Phase 1 reported "you hold no such
capability" before any manifest quibble, so `least_authority_demo file:x` produced it even though least_authority_demo takes no
file. That was the prefix taking credit again: with the designator gone, a program's declaration is
checked first, and the holdings decide only whether a file the program *does* declare can be backed.
The reason is that "least_authority_demo takes no file" stays true whatever this shell holds, while "no directory
to narrow" is an accident of this boot; the durable fact is the more useful one to print. The visible
consequence is that no shipped program can reach the headline refusal from the prompt today, because
none declares `FileGrantDeclaration::Required`; it is exercised by the host tests through `plan_against`, the
same door `FileGrantDeclaration::Required` has always come through.
