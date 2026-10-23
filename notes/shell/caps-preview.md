# What `caps` prints, and why each row is there

An appendix to [a shell at EL0](../shell.md). `crates/swish/src/preview.rs` renders `caps <command>`
and `caps <path>`, and its doc comments link here for the reason behind each row. The stem
`caps-preview` is a provisional name.

The preview exists so that the line a person typed, plus the table under it, is the child's
complete authority. Every row below follows from that. Nothing in the preview moves any authority,
which is why it is reachable from a host test at all. The sentences a person reads before deciding
to run something are checked without an emulator, and without a shell that holds anything.

## A whole line, not a stage at a time

`caps <command>` takes a whole line, operators included (milestone 50 (pipes and redirection)).
What a pipeline grants is not the sum of what its stages grant read one at a time, because which
slot holds what depends on where the stage is. Previewing the line the person would run is the only
preview worth having.

`caps time <command>` previews the command, because that is what would run. `time` moves no
authority to it: the shell times with its own clock and the child gets the endowment the tail
names. A preview that stopped at the prefix word would answer "this is not an invocation" about a
line that is one. Nested prefixes are walked in a loop rather than by recursion. `caps time time
time date` is a line a person can type, the function's frame carries a whole split `Line` by value,
and recursion on a shell stack is a bug waiting for a long line (notes/pipes.md has two).

A stage's rows are narrowed by the line's verb (milestone 809 (the package client becomes a
program), option V2). `caps jig list` shows what `jig list` would hold, which is what the
progenitor endows, reading the same word off the argv.

## `caps <path>`: what a file's bytes would be granted, and on whose word

This is §219 (how the shell names an installed program to the spawner), option D and gate D2, and
milestone 198 (a package manager)'s rung 3a. The manifest a file carries in its note is
milestone 597 (a program carries its manifest in an ELF note).

The shell computes every input from the bytes it read and the installed-programs table it can read.
The digest is the file's SHA-256 in its text form, `sha256:<hex>`. The vouch is the live generation
that lists that digest, and whether its entry is the owner's own vouch (§221 (the boot prompt is
the owner's console)). The rows are the endowment the progenitor would build, decided by the same
`grant_plan::image_manifest` it calls. A vouched file gets its own manifest, or
`NO_NOTE_MANIFEST`'s when it carries none. Unvouched bytes get `UNVOUCHED_MANIFEST`'s three slots,
or a refusal.

What the note asks is printed beside what is granted, because for unvouched bytes the two differ,
and that difference is §219's rule made visible. An unvouched `std` image is previewed from its
manifest too (milestone 205 (how a foreign program is told what to do)). Its two pages sit at the
`std` layout's slots and its words at slot 8, so the native rows would name slots it does not get.

It is a preview, and it says so where it could be wrong. The progenitor hashes its own copy when the
line runs, so a file changed in between is judged on what arrives then.

`write_image_capabilities` takes eight arguments. Each is one fact the caller already holds, and a
struct used nowhere else would be more to read, not less.

## The rows, in the order they print

A `std` program's slots are fixed by its runtime, not by the order they are listed in (milestone
595 (the shell runs a `std` program), `crates/std_runtime_contract`). The same grants land somewhere
else, and slot 0 is not even the same kind of object. A preview that printed the native positions
for one would describe a child the progenitor does not build.

Every row is printed from the manifest the progenitor endows from, `m`, never from
`e.program.manifest()`. For an image run from a file, `e.program` is a placeholder row and `m` is
what its note declared. Reading the placeholder under-reported a packaged `free` as holding neither
machine row (the 2026-10-03 security audit's follow-up, item (d)).

- A file reads as one line naming the file and the direction, because that is the whole
  authority: an endpoint a file caretaker serves for this name and no other. The direction comes
  from the manifest, not from anything typed, which is why it is worth printing.
- A directory is the subtree at risk, printed before anything happens. It is the argument for
  `rm` being a program rather than a builtin: a builtin would run with the shell's entire endowment
  and there would be nothing to print. The `-r` line is the load-bearing half, because typing that
  option widens the capability from "may take a name out of this directory" to "may walk everything
  under it".
- The names, all of them. `caps rm *.txt` prints exactly what `echo *.txt` prints, because both
  render the same expansion, so the authority about to move is on the screen before anything moves
  it. For a program that hears words, the row names what its words designated (the names, or the
  whole directory for `.`) and its manifest's word grant. The progenitor makes that read-only for
  bytes nobody vouched for, and the manifest the preview is given says so.
- The clock, which no token on the line designates. It is the progenitor's to endow, and it is
  still part of the child's authority, so a reader who took the command line for the whole story
  would be wrong by exactly one capability. The row says read-only because that is the entire
  reason `date` cannot set the time (§43 (reading the clock is a page)).
- The inert-configuration page, the clock's twin (§111 (inert configuration is a validated
  page)), and its values. §111 asked for them: "print the actual values of declared inert config,
  not just the key names, so a misclassified value is visible to whoever is about to run
  something." They come from the shell's own read-only view of the same frame the child is handed,
  never a copy of the defaults, so the preview cannot drift from what the child reads. A shell with
  no view (a `login` session, a test role) says so rather than guessing.
- The argv, which is bytes and not authority (§170 (how a foreign program is told what to do)).
  It is a capability the child holds, worded so nobody reads it as more: a path among these words
  reaches only what a directory row already granted.
- Entropy (milestone 111 (a shell that can endow a child with entropy)). Randomness is the one
  authority whose use leaves no trace. A process that draws a key and one that hardcodes a key look
  identical from outside. No observation of a running system says whether a program depends on
  unpredictable bytes, and this row says it before anything is spawned. `WRITE` is the whole grant,
  and on an endpoint that is the right to `CALL`. The child may ask for bytes. It may not receive
  another client's request (that is `READ`, which only the service holds), and without `GRANT` it
  cannot pass randomness on.
- The network (milestone 590 (the booted system starts its network stack)), in the entropy
  row's shape and for its reason: a person reads it before anything runs rather than learning it
  from a firewall afterward. Each socket is its own capability (§255 (each socket is its own
  capability), milestone 649 (every client of a network stack shares its socket numbers)), so
  one declaring program cannot reach another's sockets. A `std`
  program holds the stack and, beside it, the region it is built in, which the net PAL retypes each
  socket's shared page from.
- The supervision domain, `ps`'s row. On Linux `ps` reads `/proc` and the answer is every
  process on the machine, which no command line chose. Here the scope is a capability, so a wider
  grant would be a different line rather than an invisible one. The right is `ENUMERATE`, which
  names the domain's members and does not receive their deaths or collect them; printing `READ`
  would describe a wider grant than the one being made. The row says the program cannot learn
  anything about a process outside the domain but that it exists. `SURVEY` returns a cursor that is
  a machine-wide thread-table slot index, so a viewer with two members can count the threads created
  between them, though it cannot name one. Scoping the cursor is a milestone, and
  notes/process-view.md's BUGS carries it.
- The name resolver, beside the network (milestone 892 (a std program at the prompt holds the
  network and a resolver)). The zone is the program table's, so a person reads which names the
  program may look up, where on Unix every process may read `/etc/resolv.conf` and ask about
  anything.
- The machine, and this prompt's share of it (milestone 126 (the `procps` package), §225
  (`free` sees the machine and your share)). Two rows, because one can be held without the other:
  `vmstat` sees the machine and not the budget, `slabtop` the budget and not the machine, `free`
  both. The first says the owner's switch exists, so a person reading `caps free` on a machine
  whose owner withheld the page learns why the program cannot see it.
- The reboot object (milestone 805 (`reboot` at the prompt), §251 (restarting the machine is a
  kernel object the progenitor hands out)), the row its exit criterion 3 asks `caps reboot` to
  print. The reboot and sync rows come together, because the sync is what makes the restart safe,
  and neither shows anywhere else before the machine stops.
- The installer endpoint (milestone 809, §270 (a package manager holds an installer endpoint,
  not the spawn endpoint)). Three lines say what it cannot do, because the endpoint it is a copy of
  can do all three.
- Where the output goes, the demonstration milestone 50 owed. The destination is a capability
  rather than an integer with a convention attached, so `caps` can name it; on Unix fd 1 is
  whatever the shell's fd 1 happened to be. For a `>` the row names the file, and the parenthesis
  names what the program holds, which is not the file. Its slot 0 is the shell's result endpoint
  either way, so a redirected program still cannot seek, truncate, re-read or stat. `>` and `>>`
  hand the child the same endpoint, so the one line that differs is about the shell: whether it
  empties the file first.
- The second stream, for a program that declares one (§67 (a program's second stream is a
  declaration, not a number)). It is printed even with no `2>` on the line. A reader who took `date
  > when.txt` for "everything `date` says goes into `when.txt`" would be wrong, and this row shows
  it. A program that declares none gets no row, because it has no second stream to hide.
- The `arg` line reads the manifest rather than a second list of which programs take an
  argument. A preview that under-reports authority is one a reader would trust, so this line must
  never print `(none)` for a program the shell then hands an argument.

## How the tests hold it

`crates/swish/src/preview/tests.rs` sweeps the whole program table where a row depends on the
manifest. `the_arg_line_follows_the_manifest_for_every_program` types, for each program, a line
that supplies every operand its manifest asks for. That is a `--mem` at the declared minimum and a
file name for a declared input or file, so a manifest shape nothing has used yet is still previewed.

The counts are gates on purpose. Exactly three programs declare entropy: `uuid`, `std_exerciser`
(whose two draws from `std::random::SystemRng` must differ) and `jig`, whose TLS handshake draws
its key share and nonces. Only `jig`'s verbs that reach the index hold it (`update`, `install`),
by `grant_plan::verbs`. A `std` program that does not declare entropy still runs: its `SystemRng`
panics rather than inventing bytes, and its `HashMap` seeds best-effort. Exactly two reach the
network: `network_echo_client` and `jig`, which fetches packages itself so the progenitor parses no
network input. Only `jig` resolves names (calef's Q5 on #1884). A new program declaring any of
these is a decision, and the count is where it has to be made.
