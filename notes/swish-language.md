# `swish` the language: quoting, sequencing, and what an exit status can say

*Milestone 67. `crates/grant_plan/src/word.rs`, `crates/grant_plan/src/line.rs`,
`crates/swish/src/sequence.rs`, `crates/swish/src/status.rs`, `crates/swish/src/words.rs`,
`components/src/swish/`, `system_tests/src/user/language_tests.rs`, `xtask`'s `SWISH_CHECK_SCRIPT`.
Read notes/pipes.md first if you have not: this is the layer above its operators, and it reuses
their vocabulary. The guest test's detail is in [`swish-language/`](swish-language/README.md).*

`swish` had composition, grants, navigation and globbing, and no scripting language at all. This
milestone adds the three things that were missing and settles the one design fork inside them.

- Quoting. `'...'` and `"..."`, which decide what a *word* is, and in this shell a word is often
  the thing you are handing over.
- Sequencing. `;`, `&&` and `||`, outside everything else on the line.
- Exit status. `$?`, and the decision about what a status means when the thing that failed was
  a refusal rather than an error.

Two rows of the roadmap's table were already done when this was written: `>>` and `2>` landed with
milestone 50's later work on 2026-08-03. See notes/pipes.md.

## Quoting is not a convenience, it is an authority gap

**A file called `my notes.txt` could not be named.** In a shell whose whole thesis is that naming a
resource *is* granting it, a resource you cannot name is a resource you cannot grant. So the gap was
in the authority surface, not in the ergonomics, and that is why it is a milestone rather than
polish.

The transcript under [EXAMPLES](#examples) writes `"my notes.txt"`, then reads it twice, as
`wc < "my notes.txt"` and as `wc "my notes.txt"`, and both answer `1 2 12`. The second read is what
makes the pair a claim rather than an assertion. `wc "my notes.txt"` is the
same designation with the operator left out (milestone 31's headline), so if the two disagreed, one
of them opened something else. Before this milestone neither line existed: the `>` would have
written to a file called `"my`, and `wc` would have been handed two unplaceable tokens.

### Quoting delimits a word. It never rewrites one.

Every token in this shell is a slice of the line you typed. Nothing is copied and nothing is
reassembled. That is what lets a shell with no allocator hand a name straight to the grant planner,
and it is why `line::split` can keep a stage's command text as a slice rather than rebuilding it. A
backslash escape would have to *remove* a byte from the middle of a word, and a word with a byte
removed is not a slice of anything. So:

| | |
|---|---|
| `'...'` and `"..."` | wrap a **whole** word; the word is the span between them |
| `\` | an ordinary byte in a name. There is no escape |
| `a"b c"d` | `Refusal::PartlyQuoted`. Two pieces are never joined |
| `'it''s'` | the same refusal. Write it `"it's"` |
| `'unclosed` | `Refusal::UnclosedQuote`, and the whole line is refused |

What that buys is worth the corners it costs: what is between the quotes is exactly what is
designated, byte for byte. There is no rewriting step between what you typed and what moves, so a
preview and a grant cannot come to disagree about what a name is.

### What quoting does and does not change about authority

It changes what is designated, and that is the whole list:

- It changes a word's boundaries, so a name with a space, a `>` or a `|` in it can be named at all.
  That is new authority only in the sense that a resource you could not name was a resource you
  could not grant.
- It suppresses pattern expansion, which is a *narrowing*. `rm "*.txt"` designates one name
  spelled `*.txt`; `rm *.txt` designates the set.
- It stops a token being an option, which is the sharpest edge here. `rm "-r"` names a file
  called `-r`. Reading it as the flag that widens a directory grant from "may take a name out of
  this directory" to "may walk everything under it" would be the loudest possible version of a typo
  becoming a capability transfer.

It changes nothing else. There is no quoting form that widens a grant, none that reaches a
capability the same word unquoted could not, and none that skips a manifest check: `"secret.txt"` is
planned against exactly what `secret.txt` is planned against. Quoting is a fact about the *line*,
and authority here is a fact about what the shell *holds*.

The narrowing is visible before anything moves, which is the pairing `echo` and `caps` exist for:
`echo "*.txt"` prints `*.txt`, and `caps wc "my notes.txt"` previews the grant. Both are in the
transcript under [EXAMPLES](#examples).

### `'` and `"` are the same today, and saying which will differ is not this milestone's call

Both forms are literal. The difference POSIX draws between them is `$` expansion, and this shell has
no variables (milestone 47 owns them, and studies them as "the same question wearing a string
costume"). Inventing the distinction now would be inventing it rather than discovering it, which is
the argument milestone 50 (pipes and redirection) made about `InputDeclaration` and got right by waiting.

What they do differ in today is which byte they let you write: `echo "it's"` and `echo 'say "hi"'`
both work and neither would with one form.

### Where the quote state machine lives, and why there is one of it

One, `grant_plan::word::Cursor`, shared by the three scanners that split on whitespace, on
redirections and pipes, and on connectors; `word::read` takes the quotes off a whole token, which
is what lets one place refuse `a"b"`. [`quote-state-machine.md`](swish-language/quote-state-machine.md)
has why none of them can share a tokenizer.

## Sequencing splits outermost, and that is a decision

`caps`, `time` and `xargs` are prefix words whose operand is a whole command line, which is why
`swish::route` answers them before the line is split on `|`. Sequencing has the opposite need:
`time a && b` must time `a` and then run `b`, not hand `time` the string `a && b`.

So the split is outside everything: `dispatch_line` cuts the line into segments and hands each to
`dispatch`, which is the function every path in the shell already went through. A segment is a
whole command line and nothing under it learned a new grammar. It is also bash's binding, where a
connector joins pipelines and `time` applies to one pipeline.

```text
line      := segment (connector segment)*
connector := ';' | '&&' | '||'
segment   := <a whole command line: stages, operators, prefix words>
```

### A connector carries one bit and no capability

This is where a shell usually leaks. On Unix a `&&` chain runs inside one process holding one
ambient authority, so "what the second command may touch" is a question the connector never has to
answer. Here it does, and the answer is that each segment is planned from scratch against what the
shell holds, exactly as if it had been typed alone.

The concrete thing that could have gone wrong is the pipeline region. Each line splits a region off
the shell's budget, mints its endpoints in it, and `DESTROY`s it when the line is over, and that
destroy is what turns a stalled writer's next `SEND` into `abi::Error::Gone` (notes/pipes.md). A
chain that reused one region across its segments would keep every earlier segment's endpoints alive
for the whole chain, and a writer parked on the first would never be told.

It does not, and the reason is where the split is rather than anything anybody wrote: teardown
happens at the end of a *segment*, because `dispatch_line` sits above `pipeline` rather than
inside it.

### Two things that are deliberately not connectors

A single `|` is the pipe. Only the doubled form is a connector, and getting that wrong would
turn every pipeline in the system into two commands.

A single `&` ending a line backgrounds its one command (milestone 48 (job control)), and only a
supervised job may go there. Anywhere else `&` is an ordinary byte, so `echo a & b` prints it. A
chain or pipeline ending in `&` is refused.

## Exit status: a refusal is not an error

This is the fork the milestone was raised to settle.

Unix cannot draw the line. `127` (no such command) and a program's own `exit(1)` are the same
kind of integer there, and `&&` cannot tell them apart because the shell has nothing better to say.
Here the two are genuinely different events and the shell knows which:

| `$?` | | |
|---|---|---|
| `0` | **Ran** | the line ran and the shell has nothing to report |
| `1` | **Failed** | something was attempted and did not work: the filesystem answered with an errno, the progenitor had no memory to spawn with, a job was interrupted or torn down |
| `2` | **Refused** | the shell declined, decided at the prompt from what it *holds* and what a manifest says, with **nothing spawned, nothing opened and no authority moved** |

Separating the last two is the answer, and it is worth a number because they answer different
questions. *"Did my command fail?"* and *"was I able to ask?"* are not the same question, and a shell
that refuses constantly and by design should be able to say which one happened. A refusal is also
reproducible in a way a failure is not: the same line refuses again. The transcript under
[EXAMPLES](#examples) shows `least_authority_demo` with no argument refused, and `$?` reading `2`.

`&&` and `||` read one bit out of it, because they ask one question and both non-zero answers
are "no". A third connector that distinguished them would be inventing a control-flow word nobody
has asked for; the distinction stays where a person can see it, in `$?`.

### What the status is *not*, stated because the gap is real

No program in this system reports an exit status, and `$?` does not pretend one did. A spawned
program answers with a *value* (`least_authority_demo 7` answers 49), with bytes, or through a job frame, and none
of those is a status. So `$?` is the shell's own reading of what happened to the line, which
today is all there is.

Inventing a per-program status would mean a `spawnproto` bit, a delegation position, and an edit to
every program, which is a milestone and not a field.

### And what it cannot carry

One small integer, which designates nothing: no capability, no name, no handle. A `&&` chain hands
the next segment a *bit*. That is the whole of what passes between two commands here, and it is
the difference between this and a shell where the second command inherits the first's world.

### `$?` is a word the expander knows, not a variable

There is no variable mechanism in this shell at all, so `$?` is one word `echo` recognizes, in the
same category as a pattern. It is spelled `$?` because that is the spelling every shell user already
arrives with, and this project does not respell a name a reader already knows.

It is expressible at all only because the status has three values: `Status::digits` returns a
`&'static [u8]`, and a `'static` slice unifies with the line's lifetime. A status with an unbounded
range would need a buffer, and in a shell with no allocator there is nowhere to put one. That is the
same constraint quoting met, met again from the other side.

### `$?` is the previous *command*, not the previous line

`echo $?` straight after `least_authority_demo || echo no` prints `0`, and that is the shell being
right: the last thing that ran was the `echo`. A skipped segment leaves `$?`
alone, because nothing happened, which is bash's rule and now this shell's.

The mechanism is two cells rather than one (`CURRENT` and `LAST` in `components/src/swish/main.rs`), because a
segment has to read the previous segment's answer *while* accumulating its own: `least_authority_demo || echo $?`
is exactly the case one cell could not serve.

`CURRENT` was an `AtomicBool` called `TROUBLE` until this milestone, set by whichever printer had bad
news and read by `xargs` to stop a sweep. Widening it from a bit to a status is the whole of what
`$?` needed: the shell already knew that something had gone wrong, and what it did not record was
which kind. `xargs` now reads it the same way `&&` does.

## EXAMPLES

At a real prompt on the RedoxFS fixture. The transcript below is `NIFE_SHOW_TRANSCRIPT=1
script/swish-check --arch aarch64`, which boots the kernel and types at the prompt through
the real `components/src/progenitor.rs`.

```text
$ echo hello world > "my notes.txt"
$ wc < "my notes.txt"
  1 2 12
$ wc "my notes.txt"
  1 2 12
$ echo "*.txt"
*.txt
$ caps wc "my notes.txt"
  wc would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    output   this shell's result endpoint (it reads the bytes and prints them)
    input    my notes.txt  (this shell reads it and streams it in; the program
             holds an endpoint, not a file)
    arg    (none)
  reading the command is reading its whole authority.
$ least_authority_demo 3 && echo yes
  a process at EL0 computed 3*3 = 9
yes
$ least_authority_demo || echo no
  least_authority_demo: needs an integer argument
no
$ least_authority_demo
  least_authority_demo: needs an integer argument
$ echo $?
2
```

Read `caps wc "my notes.txt"` rather than the counts. The `input` row names a file whose name has a
space in it, which is a sentence the preview could not have been about before this milestone. The
parenthesis after it is unchanged: what the child holds is still an endpoint and not a
capability naming the disk.

The riscv64 leg is the same session with the same answers, which is the parity gate met by running
the same script rather than by a second implementation.

## What the guest test proves, on both ISAs

`system_tests::user::language_tests` reads the tail of the same script `redirection_tests` runs,
against the real shell and a real RedoxFS subtree, because a quoted name is only worth something if
it reaches a filesystem. It shares that witness rather than adding a role: a seventh scripted shell
put another test over the frame pool intermittently. A scripted-shell witness is not free, and the
price is paid by whatever test runs last. The assertions are pairs, quoted against bare, refused
against run, so a shell that ignored the feature fails both halves.
[`the-guest-test.md`](swish-language/the-guest-test.md) has the four pairs and the flake in full.
## BUGS, named where the reader meets them

- No backslash escape, and none is planned while tokens are slices. `my\ notes.txt` is a file
  whose name contains a backslash, and `nav::component_fits` refuses that byte, so the line is
  refused rather than misread. The quoted spelling is the one that works.
- **Adjacent pieces are not joined.** `a"b"` is `Refusal::PartlyQuoted` where POSIX reads `ab`, and
  `'it''s'` is the same where POSIX joins three pieces. Both are refused by name rather than
  silently misread, and `"it's"` is the spelling that works.
- `"$?"` prints `$?`, because both quote forms are literal today. When variables arrive the two
  forms have to stop being the same thing, and that decision belongs with them.
- `$?` is readable only in `echo`. `least_authority_demo $?` treats the two characters as an argument and is
  refused for not being an integer. Substituting a word anywhere else needs the machinery milestone
  47's variables need anyway, and building half of it here would be building it twice.
- There is no grouping. `a && b || c` is left to right with no precedence between `&&` and `||`,
  which is bash's rule, and there is no `{ }` or `( )` to override it. Subshells are milestone 52's
  and grouping should arrive with them.
- `time a && b` times only `a`. That is the outermost-split binding and it matches bash, but a
  person who wanted the chain timed has no spelling for it until grouping exists.
- `&` backgrounds only a line of one supervised job; bash's `a & b` has no spelling.
- A sequence is at most eight commands and a pipeline at most four stages. Past either the line
  is refused rather than truncated, which is the same posture `line::MAX_STAGES` already took.
- `xargs <program>` still stops after planning batch one, unchanged by this milestone: the shell
  cannot yet ask the progenitor to mint a per-batch caretaker, which is milestone 47's delegation chain. A
  sweep that stops is a segment that did not succeed, so `xargs rm *.txt && echo done` will not print
  `done`, which is the right answer for the wrong reason.
- **A `Failed` and a `Refused` are indistinguishable to `&&`.** That is deliberate (one question, one
  bit) but it means a chain cannot branch on *why* it stopped without reading `$?`, and reading `$?`
  needs `echo`, which cannot condition anything. Until there is a conditional, the distinction is for
  a person and not for a script.
- **A pipeline's status is the first thing that went wrong, not the last stage's.** bash reports the
  last stage by default; here the first non-`Ran` wins, because the first thing that went wrong is
  what stopped the line and a later printer describing a consequence should not overwrite the cause.
  Nothing in this system reports a per-stage status anyway, so the two rules cannot yet disagree
  about anything a person could observe.
- Scripting is still nowhere. `if`, `while`, `for`, functions, and reading a script file are not
  here and were never in scope: this project has no story yet for what a script *is* when a program
  namespace is an endowment. Doing quoting and sequencing first is what makes that question
  answerable rather than theoretical.
