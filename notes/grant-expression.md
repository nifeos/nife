# The command line as a grant expression

Milestone 31, phase 1. This is the note on the one idea the capability shell exists to make
visible: on a nife command line, naming a resource is how you grant it. Mark Miller's
principle, "designation is authorization," applied at the one interface a human touches. The pure
logic lives in the `grant_plan` crate (host-tested); the wiring is `swish.rs` and, on the
progenitor's side of the channel, `crates/system_initializer`'s spawn service. The two first
processes this note was written against were `hello.rs`'s `init_boot` role on aarch64 and
`components/src/progenitor.rs` on riscv. They became slot tables in milestone 96 (one init) and one
program in milestone 266 (one progenitor), so
the service that decodes a grant expression is written once. The manifest half is in
[program-manifest.md](program-manifest.md). The evidence behind this note is in
[grant-expression/](grant-expression/README.md).

## What Unix does, and why it is the opposite

A Unix child inherits every one of your file descriptors and runs under your uid, so it may
`open()` anything your uid allows. Authority comes from who you are, and it flows to a child
whether or not the command mentioned it. `grep secret public.txt` hands grep the authority to read every file you own; it touching only
`public.txt` is good manners. This is ambient authority ([capabilities.md](capabilities.md)), and it is what
makes the confused deputy constructible.

The inversion: a nife command grants exactly what it names, and nothing else. There is no ambient pool to draw from, so the question "may I?"
is never asked; there is nothing in the program's hands it was not given. `least_authority_demo 9` grants a
report channel and an argument. `memory_grant_depleter --mem 16` grants a report channel and a 16-page memory
budget. `memory_grant_depleter` alone grants a report channel and is refused, because memory_grant_depleter's manifest says it
needs memory and the command named none.

## The grammar

```text
<prog> [--mem N] [token ...]
caps [command]
help
echo <text>
```

The command line itself is the grant expression. Its parts are designators:

- `<prog>` names the program to spawn (a closed set today: `least_authority_demo`, `memory_grant_depleter`, `interrupt_heeder`,
  `interrupt_ignorer`, `date`).
- `--mem N` designates N pages of untyped, carved from the shell's own budget.
- a bare token designates whatever the program's manifest declares in that position: the integer
  argument, then a file (one name, at most 252 bytes, no path). See the per-file grant section
  below for what the shell narrows it from and what it can back today.

`caps` is introspection: with no argument it prints the shell's whole endowment; with a tail it
previews exactly what that command would grant, so reading the command is reading the child's
authority. That is DECISIONS §14's claim made interactive.

## Two words came out of this grammar, and why (milestone 47)

Phase 1 spelled the same thing `run [--mem N] <prog> [arg] [file:PATH ...]`. Both extra words went.
`run` failed on consistency: builtins like `ls` are bare words, and nobody would type `run ls`.
`file:` announced the wrong half of the grant, which file rather than which direction, and the
manifest was doing all the work while the prefix took credit. A token shaped like a flag still never
falls into the file position. The parser now keeps positional tokens in order and lets
`plan_against` place them into the slots the manifest declares, so `wc 2026` designates a file named
`2026`. A file whose name begins with `-` cannot be designated. The argument in full is in
[grant-expression/two-words.md](grant-expression/two-words.md).

## Where the authority actually comes from, and how it moves

The shell holds four capabilities (the progenitor grants them at boot, in this order): the terminal endpoint
(slot 0), a spawn endpoint to the progenitor (slot 1), a result endpoint (slot 2), and its own untyped
budget (slot 3). The budget is the piece milestone 31 (capability shell) added: the progenitor splits it off its own untyped
and `CAP_INSERT`s it into the shell, so the shell has memory that is genuinely its to give.

The shell does not build children itself; the progenitor holds the initrd and stays the ELF loader (the
parser lives in one place, out of the shell). So the shell directs The progenitor and delegates the
capabilities it grants, over the spawn endpoint. The protocol (`grant_plan::spawnproto`, a userspace
protocol like the terminal contract, DECISIONS §21):

1. The shell resolves the command into an endowment (program id, argument, page count), checking it
   against the program's manifest first. A mismatch is refused at the prompt; nothing is sent.
2. The shell `SEND`s the request (program id, argument, page count).
3. If a memory grant was named, the shell `SPLIT`s N pages off its slot-3 budget and `SEND_CAP`s
   the resulting untyped to the progenitor, narrowed to `WRITE|GRANT`.
4. The progenitor loads the named ELF, endows the child with the result endpoint (slot 0) and, when one was
   delegated, the untyped (slot 1) narrowed to `WRITE`, and starts it with the argument.
5. The child runs and reports its answer on the result endpoint; the shell reads the one word.

Nothing the command did not name reaches the child. The progenitor inserts only the report channel every
spawn carries and whatever the shell delegated. The child's authority is the command line, read
literally.

## The memory grant is real

Making `--mem N` real needed a kernel fix, an amendment to DECISIONS §16 (object revocation): a `SPLIT` child now
inherits the invoking capability's rights, so untyped can be delegated without any process
manufacturing a `GRANT` it was denied. `memory_grant_depleter --mem 16` spends what it was granted and
reports 15 pages mapped on both ISAs, the sixteenth paying for the page table. The rights chain and
the proof are in [grant-expression/memory-grant.md](grant-expression/memory-grant.md).

## The refusals read like the model, not like errno

A refusal is a fact about what the shell holds, phrased in the capability model's voice:

- `frobnicate 1` → "frobnicate: no such program (try 'help' for the builtins)." There is nothing to
  name. A mistyped builtin lands here too, now that the first word is either a builtin or a program,
  which is why the line points at both halves of what the prompt understands.
- `memory_grant_depleter` → "memory_grant_depleter: needs a memory grant; add --mem <pages>." The manifest caught it.
- `least_authority_demo 3 --mem 8` → "least_authority_demo: takes no memory grant; drop the --mem."
- `least_authority_demo 5 extra` → "least_authority_demo: takes no file; drop the name." The token could only have been a file,
  and least_authority_demo declares none, so it is refused rather than granted-and-dropped. The answer is the
  same in a shell that holds a directory, which is the point: the manifest decides, not the
  endowment.
- `least_authority_demo eight` → "least_authority_demo: needs an integer argument." Not a file, because least_authority_demo has no file slot
  for the word to fall into.
- `wc report.txt`, at a program that *does* declare a file, in a shell that was granted no directory
  → "wc: you hold no such capability: this shell was granted no directory to narrow." Since
  milestone 50 the *interactive* shell is not that shell (it holds the image root); the wiring with
  no disk attached still is, and it is the same binary in both.
- `wc sub/report.txt` → "wc: that is not a name this shell can grant: one component, at most 16
  bytes." There is no namespace here to walk, so a path is refused where it was typed rather than
  becoming an `ENOENT` from a server asked something meaningless.
- `wc --secret report.txt` → "wc: unexpected argument (this shell will not grant what it cannot
  place)." An unknown flag never reaches the file position.

The `no such capability` line is the headline refusal, and it is a statement about the shell's own
capability table: "there is nothing I hold that could grant this," never a Unix-flavored EPERM.

One ordering changed with the designator, deliberately: a program's declaration is checked before
the shell's holdings, so the durable fact is the one printed. Why is in
[grant-expression/two-words.md](grant-expression/two-words.md).

## Per-file grants: one file, one direction, and a caretaker in between

*(Phase 2. The designator grammar was designed in phase 1 so this would slot in without a grammar
change, and it did. Milestone 47 (navigation and naming) then removed the `file:` prefix from it, which changed the spelling
and not one line of the mechanism below.)*

The filesystem's unit of authority is a directory: the endpoint a client holds IS the directory
capability, and every name in an `OPEN` resolves under it (DECISIONS §27). `wc report.txt` says
less than that. It names one file, so it must grant one file.

The narrowing is a caretaker, Mark Miller's pattern: a process that holds the wider capability,
exports a narrower one, and is the only path between them. `components/src/fs_file_caretaker.rs` opens the
granted name once at startup and then serves the *same* `filesystem_contract::fs` contract on its own endpoint:

```text
  FS server ──file IPC──► fs_file_caretaker ──narrowed file IPC──► the confined program
              (a directory)          (one file, one direction)
```

Three rules, and each is phrased as a fact about what the holder *has* rather than as a permission
refusal, because there is no policy here to consult:

| the holder asks | answer | why that answer |
|---|---|---|
| `OPEN` any other name | `ENOENT` | in this scope there is no such name. It cannot enumerate, and it cannot learn what else exists |
| `CREATE` | `ENOTDIR` | a file capability is not a directory; "make a name in it" is not a request that means anything |
| `WRITE` / `TRUNCATE` without the direction | `EROFS` | the capability carries one direction. `EACCES` was rejected: it implies a policy that could have said yes |
| `SETXATTR` / `REMOVEXATTR` without the direction | `EROFS` | the same rule, and derived rather than listed: milestone 61's verb table answers "does this verb mutate", so the third way to change a file is refused by the branch that refuses the other two |

### The verb surface is part of what the capability is (milestone 61)

A file capability is a different protocol from a directory, so which verbs exist is what the
capability is, written down in `filesystem_contract::verb::file_grant::POLICY`, one row per opcode.
Each row is `Local`, `Forward` or `Refused(errno)`, and the errno says which kind of "no" it is.
Milestone 61 (caretakers) made the attribute verbs reachable through it ([xattr.md](xattr.md)).
#### BUGS

- **The directory verbs other than `CREATE` answer `EBADF`, not `ENOTDIR`, and that is inherited
  rather than argued.** Before the table, `OPENDIR`, `READDIR`, `MKDIR`, `RENAME`, `UNLINK` and
  `RMDIR` fell through one `_ =>` arm shared with "you named a handle I never minted", so two
  different statements came out as one word. Writing the rows down is what made the conflation
  visible. `ENOTDIR` is likely right for all seven, by exactly the argument `CREATE` already
  makes, but changing it changes what a client observes on the wire, so it is a contract decision
  rather than a table's to take. Named here, and in `POLICY`'s own doc comment, so the reader meets
  it where they meet the feature.

### Why it is a process, and how it is proven

The caretaker is a process rather than a check inside the FS server, because the server receives on
one endpoint and a second would need a receive over a set. So "it cannot reach a second file" is a
property of the confined program's capability table, not of a branch it is trusted to take. The
grant costs no memory. An attacker reports a bitmap of what got through under a read-only grant and
a read/write grant, on both ISAs, and the second run is what makes the first mean anything. Both
arguments are in [grant-expression/per-file-grants.md](grant-expression/per-file-grants.md).
### The manifest declares the direction; the command line designates the file

`wc report.txt` reads and `tee report.txt` writes, with no flag either way. The split is
SHILL's and it is deliberate: whether a program writes is a property of what it does and belongs in
its published manifest, while *which* file is the human's business and belongs on the line. The
authority is still exactly what the line says, because the program's half is fixed and readable.
`caps wc report.txt` prints it:

```text
  wc would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 2  endpoint  file     report.txt  (read-only, and nothing else on the disk)
```

### At the interactive prompt, and what it cost

Since milestone 50 (pipes and redirection) the interactive shell holds the image root, and since 2026-08-17 the progenitor
builds a `fs_subtree_caretaker` per directory grant, so `rm` runs; `script/swish-check` proves it on
both ISAs. `FileGrantDeclaration::Required` still has no consumer: no shipped program declares a file. Of the
four obstacles this note predicted, three landed as written. Depth is the one still open: every
caretaker descends exactly one name, so `rm gate.txt` at the top prompt is refused. The fourth, the
lifetime, cost the most care, because the first reclaim of a caretaker's region is refused by
construction. Two shipped programs also disagreed with their own declarations until they ran.

`wc report.txt` is `wc < report.txt` with the operator left out: the shell opens the file and
streams it, and the child holds an endpoint, not a file. That is narrower than a file capability,
and `caps` says so. The prompt, the cost and the input operand are in
[grant-expression/per-file-grants.md](grant-expression/per-file-grants.md).
## `date` at the prompt, and the authority the command line cannot name

`date` became reachable from the shell with the grammar change, because with `run` gone `date` is
exactly what a person types. Its manifest is all `Forbidden`: no argument, no memory, no file. It is
the first program in the table whose whole authority is something the command line cannot
designate, and that is worth being explicit about rather than letting it read as an oversight.

What `date` holds is a read-only mapping of the clock page (DECISIONS §43, notes/clock.md). Read,
set and propose are three different objects there, and the reason `date -s` cannot exist is that the
read authority is a page permission rather than a check the program could skip. None of that is
expressible on a command line, so there is nothing to type and nothing to get wrong.

**The interactive boot now starts a clock service and hands the page to the progenitor**, on both ISAs, so
`date` at the prompt prints a time. The shell is not on that path and holds no clock: `Manifest`
grew a `clock: bool` the way it has `reports: bool` (a fixed fact about the program, not a
designation), and *The progenitor* reads it and endows the child. `caps date` prints the row anyway, because a
preview that showed only what the line designates would be off by exactly one capability:

```text
$ caps date
  date would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 1  frame     clock    read-only. it can read the time and not set it,
                              and no token on the line could have asked for more
    arg    (none)
  reading the command is reading its whole authority.
$ date
  Tue 2026-08-04 00:35:59 UTC
```

Three of the four links this note once said were missing were right. The wrong one was that the
spawn protocol needed a clock position: the progenitor reads the program's manifest itself, so the
wire did not change. A flag word carries what the sender chose, and an authority the sender could
not choose does not belong on it. Nothing lets the shell delegate a clock, because no program asks
for it. The four links and the delegation question are in
[grant-expression/clock-grant.md](grant-expression/clock-grant.md).

## What phase 1 deliberately does not do

- **No live capability table introspection.** `caps` prints the shell's own endowment (which it knows by the
  boot convention) and previews a command's grant (from the manifest). Reading *another running
  process's* capability table would need a new kernel method (a debug/reflection capability), which is a
  design fork, not built here. The manifest is the userspace stand-in, and for the shell's purpose
  (what would this command grant?) it is the right answer anyway: the authority is the command, and
  the command is on the screen.
## The interrupt grant (DECISIONS §24)

A foreground job the user can `^C` is another grant the command line expresses, and it flows the
same way. The manifest marks a program `interruptible`, and the shell endows a supervised job with
what the two-tier interrupt (DECISIONS §24) needs, and nothing more.

- **A shared job frame** the shell mints per job (`grant_plan::jobframe`) and maps into the child. The
  cooperative signal is a word in it: on the first `^C` the shell writes the interrupt flag, and a
  cooperative program reads it between work units and exits cleanly. Shared memory, not an endpoint,
  because a running computation cannot poll an endpoint (no non-blocking receive); this is the one
  place control rides in memory rather than a message, and the note says why.
- **A job untyped** the shell splits from its own budget and delegates for the progenitor to build the *whole*
  child from, so the child's region is one the shell holds. That is what makes the forcible tier a
  capability the shell already has: a second `^C` tears the job down with `MemoryRegion::DESTROY` on that
  region (which force-kills the resident thread, §16 amendment). Even a runaway that ignores the
  cooperative flag ends and the prompt returns.

A program the command did not run as a supervised job holds no job frame and no reclaimable region,
so it cannot be signaled or torn down through this path; the authority is exactly the endowment, as
everywhere else. The escalation policy (how many `^C`, the grace timeout) is the shell's, host-tested
in `grant_plan::Escalation`. The two demonstrators are `interrupt_heeder` (heeds the cooperative `^C`) and `interrupt_ignorer`
(a bare loop only the forcible tier ends). See DECISIONS §24's implementation amendment and
notes/terminal-contract.md's `OPERATION_INTRCOUNT`.
