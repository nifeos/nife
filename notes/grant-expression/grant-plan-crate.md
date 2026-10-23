# The `grant_plan` crate: how a line becomes a grant

An appendix to [`notes/grant-expression.md`](../grant-expression.md). That note carries the idea;
this one carries the design of `crates/grant_plan`, the crate that holds the logic, where its doc
comments link here. Each section names the module it covers.

## The grammar the parser accepts (`lib.rs`)

```text
<prog> [-abc] [--mem N] [token ...]
caps [command]
time <command>
help
echo <text>
```

`caps` and `time` are prefix words: their operand is a whole command line, operators included,
kept unsplit for the shell to act on. A first word that is not a builtin is a program invocation.
Everything after it is the `--mem N` budget grant plus positional tokens the parser leaves
unclassified. Which token is the argument and which the file is the manifest's job.

Short options are checked against the program's manifest at the prompt, because one of them can
widen a grant: `rm -r logs` hands over the subtree that `rm logs` does not. A bare program name
spawns it, and a bare token in a file position designates the file. Whether it is read or written
lives in the manifest.

The crate performs no IO and makes no syscalls, so all of this is tested on the host (DECISIONS §7
(testing: QEMU harness + host-testable crates)). A program that needs something the command did not
grant fails legibly ("you hold no such capability"), not with a Unix-flavored `EPERM`. `spawnproto`
is the word layout of the shell-to-progenitor spawn protocol. It is a userspace protocol (DECISIONS
§21 (the terminal is a userspace component)): the kernel routes the words and never reads them.

## The operators (`line.rs`)

[`notes/pipes.md`](../pipes.md) has the grammar and the three rules that are not Unix's. Three
details of the splitter are recorded here.

- The `2` of `2>` is the familiar spelling, not a file descriptor. It counts only at a word
  boundary, so `wc2>f` is the word `wc2` and an output redirection, and `least_authority_demo 2 >
  out` keeps its argument.
- A redirection's name may be quoted, and the quotes are off by the time anything downstream sees
  it, because what is designated is the name and not the spelling.
- `2>` binds only to a stream the program's manifest declares (`OutputDeclaration`).

### BUGS

- A line has one diagnostic stream, minted once and given to every stage that declares one. So a
  pipeline carrying `2>` where only some stages declare one is refused.
- `<<` is refused as `Refusal::NoRedirectTarget`, a message about a missing name where the mistake
  was a missing feature.
- Quoting makes an operator an ordinary byte, and nothing puts one back. A quoted redirection name
  is exempt from `Refusal::PatternInRedirect`: `> "*.txt"` writes a file called `*.txt`.
