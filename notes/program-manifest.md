# The program manifest: a component contract in embryo

Milestone 31, phase 1. A manifest is a program's declared endowment: what it expects to be
granted, written down where the shell can check a command against it before spawning anything. It
is the SHILL idea (OSDI 2014: capability contracts for scripts) shrunk to what phase 1 needs, and
it is milestone 23's component contract in its smallest honest form. The type and the checker live
in `grant_plan` (host-tested); this note is the why and the format.

## The problem it solves

Without a manifest, a mismatch between what a command grants and what a program needs surfaces late
and badly. Grant a program too little and it hangs or faults deep inside, on a capability it assumed
was in a slot that is empty. Grant it something it does not understand and the authority leaks
silently. Both are mystery failures at runtime, far from the command that caused them.

The manifest moves the failure to the prompt. The shell checks the command's grants against the
named program's manifest at spawn, before a child exists, so a mismatch is a legible refusal on
the line you typed:

```text
$ memory_grant_depleter
  memory_grant_depleter: needs a memory grant; add --mem <pages>
```

Nothing was built, nothing hung. The contract was checked where you could still read it.

## The format

A manifest declares what a program expects to be handed (`grant_plan::Manifest`):

```rust
struct Manifest {
    arg:           ProgramArgumentDeclaration,     // Required | Forbidden   -- an integer argument?
    mem:           MemoryGrantDeclaration,     // Forbidden | Required { min, max }  -- a memory grant, in pages?
    file:          FileGrantDeclaration,    // Forbidden | Required { writable }  -- one file
    dir:           DirectoryGrantDeclaration,     // a directory capability, and with which rights
    flags:         &'static [u8],  // the option letters the program accepts
    output:        OutputDeclaration,  // where its bytes may go
    input:         InputDeclaration,   // and where they may come from; Required carries
                                // writes_while_reading, which the type will not let you omit
    reports:       bool,        // is it endowed the shared result endpoint?
    interruptible: bool,        // does ^C reach it (DECISIONS §24)?
    clock:         bool,        // may it read the wall clock?
    domain:        bool,        // may it name the processes in its supervision domain?
    config:        bool,        // may it read TZ/LANG/TERM off the inert config page?
    entropy:       bool,        // may it ask the entropy service for random bytes?
    // ... network, machine, share, runtime: see grant_plan::Manifest
    reboot:        bool,        // may it restart the machine (milestone 805, DECISIONS §251)?
    sync:          bool,        // may it ask the file server for SYNC and nothing else?
}
```

**The struct in `crates/grant_plan/src/manifest.rs` is the authority, not this page.** This list said
five fields against the code's ten from some point before 2026-08-14, when milestone 117's first
stranger run found it and reported that following the note produces a struct that does not compile.
It then carried `reports` and `interruptible` twice, and was missing `domain`, `config` and
`entropy`, until 2026-09-05. The program table below is the same hazard at smaller scale: it shows a
few programs and the enum carries more.

**The last four are one family and they are the ones a newcomer misreads.** Every other field is
about something the command line can designate. These four are about authority no token can name, so
there is nothing to type and nothing to refuse. What they do is tell The progenitor which children to
endow, and tell a person reading `caps <program>` that the authority exists at all. `clock` and
`config` are read-only page mappings; `domain` and `entropy` are endpoints placed at named slots
(`grant_plan::DOMAIN_SLOT`, `ENTROPY_SLOT`), narrowed to `ENUMERATE` and `WRITE` respectively.

`entropy` is the one worth pausing on, because randomness is the authority whose use leaves no trace:
a process that draws a key and a process that hardcodes one make the same syscalls and produce
output of the same shape. The declaration is the only thing that tells them apart, which is why
ambient entropy would be ambient authority (DECISIONS §44, notes/entropy.md).

### The file endowment declares a direction, not a name

`FileGrantDeclaration::Required { writable }` is the phase-2 addition, and the interesting part is the split it
makes: the manifest declares the direction, the command line designates the file. So `wc
report.txt` reads and `tee report.txt` writes, and the human never types a mode.

That is SHILL's shape and it is the right one on both halves. Whether a program writes is a property
of what the program does; it is fixed, publishable, and not the caller's to guess. Which file it
should touch is entirely the caller's business and belongs on the line. Authority is still exactly
what the command says, because the program's half is written down where the shell (and the human) can
read it: `caps run wc file:report.txt` prints "read-only, and nothing else on the disk".

One file, not a list. A program that needs two needs a manifest that says so, and that is a later
widening rather than something to leave ambiguous now.

The programs a `Manifest` is written for (the two interrupt demonstrators, `interrupt_heeder` and `interrupt_ignorer`,
declare nothing but `interruptible`):

| program    | arg        | mem                  | file      | reports |
|------------|------------|----------------------|-----------|---------|
| `least_authority_demo`   | Required   | Forbidden            | Forbidden | yes     |
| `memory_grant_depleter` | Forbidden  | Required 1..=64 pages | Forbidden | yes     |
| `date`     | Forbidden  | Forbidden            | Forbidden | yes     |

**`date`'s row is all `Forbidden`, and that is the interesting one.** Its authority is a read-only
mapping of the clock page, which the progenitor endows and the command line cannot name: there is nothing to
type, so there is nothing to get wrong. What the manifest still does is refuse everything else, so a
memory grant or a file aimed at a clock reader stops at the prompt rather than being handed over and
ignored. A manifest is as much about what a program will *not* accept as what it needs.

No shipped program declares a file yet, because the shell it would be spawned from holds no directory
to narrow (notes/grant-expression.md says why, and why that refusal is true rather than pending). The
`FileGrantDeclaration::Required` logic is not therefore untested: `plan_against` takes an explicit manifest
rather than reading the static table, so the host tests check a manifest shape no program declares.
That split was worth making anyway, because milestone 23 needs exactly it: a manifest that travels
with a component, checked by a composer that did not write the program.

`least_authority_demo` needs its `n` and no memory; granting `--mem` to it is a refusal. `memory_grant_depleter` exists to
spend a budget, so it *requires* `--mem` (the lower bound of 1 makes "memory_grant_depleter with no grant" a
refusal), with an upper bound the shell's own budget can actually back.

## The check, and its order

`grant_plan::plan` resolves a parsed invocation against the manifest and yields either an `Endowment`
(exactly what to grant) or a typed `Refusal`. The order starts with the program name. A name that
resolves to nothing is a fact about the system, and everything after it is a fact about a program
that exists. Then a flag nothing knows, then the positional tokens placed into the slots the manifest
declares (the integer argument, then the file), then the memory rules.

Placing the tokens is what milestone 47 (navigation and naming) moved out of the parser. The parser knows a token's shape.
Only the manifest knows what a token *is*, which is why `wc 2026` designates a file named `2026`
rather than an argument nobody declared. Two refusals fall out of the same rule:

- a token past the last declared slot cannot be placed, so `least_authority_demo 5 extra` is refused. That is the
  safety property the `file:` prefix was credited with, and it was always the manifest's.
- inside a file slot, what the *shell holds* decides whether the designation can be backed at all:
  "you hold no such capability" beats "and that name is too long", because it is the bigger fact.

Each refusal carries a fixed message (`Refusal::message`), host-tested so the wording cannot drift,
and the shell prefixes the program name. The strings are part of the deliverable: a refusal must
read like the capability model, not like errno. See [grant-expression.md](grant-expression.md) for
the full refusal catalog.

## Why it lives in the shell, not the kernel

The manifest is a userspace contract, checked by the party doing the granting. The kernel does not
read it, does not enforce it, and does not need to. Even if the shell skipped the check and granted
a program too little, the program would fault on an empty slot and die, harming only itself. That is
because there is no ambient authority to fall back on. The manifest is not a security boundary; the
capability model is. The manifest is a usability boundary, turning a deep mystery hang into a
one-line refusal at the prompt. That is exactly the altitude SHILL's contracts sit at, and exactly
what the components of milestone 23 (a capability-routed component OS) will formalize. A component
declares the capabilities it needs, checked by whoever wires it up, so a bad wiring is caught at
composition, not at runtime.

## What the line cannot name: authorities a manifest declares

Most of `grant_plan::Manifest` is about what a command line designates. The rest is about what it
cannot: the clock, the process domain, the configuration page, entropy, the network, the machine
page, the job budget's view, the reboot object, the sync capability, and the package manager's
endpoints and directories. None of these is a name a person types, so there is no token to place and
no refusal to write. Such a field does two things. It tells the progenitor which children to endow,
and it tells a person reading `caps date` or `caps uuid` that the authority exists at all.

Ambient entropy would be ambient authority. A shell that handed every child a random source would
make a program's dependence on randomness invisible, which is the one property the entropy service
was built to have (DECISIONS §44 (entropy is a capability)). The same holds for each of the others.

The rights are the narrowest that work:

- A page is mapped read-only. A program declaring `clock` can read the time and has no object
  through which it could set or propose one, which is the split of DECISIONS §43 (reading the clock
  is a page). One declaring `configuration` can read the three inert values and cannot change what a
  shell hands its children.
- An endpoint carries `WRITE` alone, the right to `CALL`. Without `READ` a client cannot take
  another client's request off the endpoint, and without `GRANT` it cannot hand the service to
  anything it spawns. The network endpoint reaches no other program's sockets, since each socket is a
  capability the stack hands back when it is opened (§255 (each socket is its own capability)). The
  stack the progenitor builds holds `socket_contract::NO_LISTEN_GRANT`, so a declaring program can
  connect out and cannot listen.
- The process domain arrives with `READ`, which is what `abi::rendezvous::SURVEY` takes. The job
  budget's view is narrowed to `ENUMERATE`: `abi::memory_region::USAGE` can ask what was spent, and
  the holder cannot spend, split or destroy it.

An undeclared authority is an empty slot, and that difference is the feature. A program without the
declaration finds the slot empty and the kernel refuses its call with `abi::Error::NoSuchSlot`. `ps`
prints that refusal apart from an empty domain. `entropy_contract::delivered` reads it as `None`
rather than as a short reply, which `disk_partitioner` turns into `R_NO_ENTROPY` instead of writing
a GPT full of counter-derived ids. §225 (`free` sees the machine and your share) grants the machine page
to every login by default. A session the owner withheld it from spawns programs that say they cannot
see the machine, rather than show a machine of zero bytes.

Four of these are the owner's alone:

- `reboot`. The kernel grants the progenitor the one reboot object there is, and exactly one program
  declares it. It syncs nothing, and neither does the kernel (§251 (restarting the machine is a
  kernel object the progenitor hands out)), so `reboot` also declares `sync`. That is a capability
  badged and bound by `filesystem_contract::fs::BIND_SYNC`, which answers `fs::SYNC` and refuses
  every other verb. A sync stalls every client's queue, so it goes only to the program about to
  restart the machine. A boot with no writable filesystem leaves its slot empty. A boot whose file
  server could not make one refuses the spawn rather than start `reboot` unable to sync.
- `installer`. It is the progenitor's spawn endpoint, badged for one job and narrowed to `WRITE`. On
  it the progenitor serves the activation request for install, remove and rollback, and refuses
  every other request, `vouch` and a spawn included. So the holder can change what is installed and
  cannot run anything. The progenitor places it only for a request that arrived on the unbadged spawn
  endpoint, which the boot prompt alone holds (DECISIONS §221 (the boot prompt is the owner's
  console)). Its answers come back on an endpoint of the holder's own, because the shell is reading
  its result endpoint while the program runs.
- `indexes`. The progenitor installs from the index copy a package manager writes, trusting it
  because it sits in a directory only the owner's console grants (ruling I2 of 2026-10-06, calef: "I
  would think jig"). The `std` layout has one directory slot, so a line that designates a directory
  of its own is given that one instead.
- `activation_directory` and `console_input`. The first is read only, knowledge rather than
  authority, but what is installed is the owner's to show. The second is the console's input, which
  only the owner's console hands out.

No image may declare any of the four (`grant_plan::image_can_carry`), so "only one program can
restart the machine" is a fact about the table rather than about what somebody installed.

### The named slots

These authorities land at fixed slots rather than the next free one. A child's ordinary grants fill
from zero upward, and how many there are depends on the wiring: `date` gets a clock from the
progenitor and none from the guest test harness. A low number would move under the program, and it
would probe the wrong slot. `abi::thread_control_block::CAP_INSERT`'s explicit target places them,
as it places the fault endpoint at `abi::fault::FAULT_EP_SLOT`, the table's last slot.

The slots run as one block from 7 (`DOMAIN_SLOT`) through 18 (`INDEX_SOURCES_SLOT`). They also clear
the shell's own table, which the progenitor fills with up to seven positional capabilities, slots 0
through 6. A lower number would collide with the shell's clock on a boot with a filesystem and a
second directory grant.

`DIAGNOSTICS_SLOT` (8) is one constant every declarer shares, and that is not a retreat to an ambient
convention. What the manifest declares is whether there is a second stream. A program that declares
none has no slot and no stream, which is where the model differs from Unix's fd 2.

`SHELL_CONFIGURATION_SLOT` (21) is where the boot shell holds the same configuration frame the
progenitor endows a child with, so `caps` prints the values a child would read. A copy of the
defaults would agree with the child by coincidence and drift the first time either changed. It is
probed at `_start`, before the shell allocates anything, as `spawnproto::RUN_UNVOUCHED_SLOT` is. A
shell built any other way finds it empty and says it cannot see the values.

## What grows from here

- **milestone 23** turns this from a static table keyed by program name into a contract a component
  *ships with*, so the shell (or any composer) checks a program it did not write against the
  program's own declaration. The shape is the same; the manifest just travels with the binary.
- **milestone 32** adds file and directory grants to the endowment vocabulary, so a manifest can
  declare "one readable file" and the checker can match a designated name against it. The
  `ProgramArgumentDeclaration`/`MemoryGrantDeclaration` pattern extends directly to a `FileGrantDeclaration`.
- **milestone 47** will have to grow `ProgramArgumentDeclaration` into something with position and arity the first
  time a program wants both an argument and a file (`grep pattern file.txt`), or wants two of
  either. Today's rule (the argument takes the first positional, the file the next) is unambiguous
  only because at most one bare token can be a file. `date` is already pressing on this: it reads
  three registers (format, UTC offset, provenance) and declares `ProgramArgumentDeclaration::Forbidden`, so the shell
  spawns it with the defaults and its selectors are unreachable from the prompt.
