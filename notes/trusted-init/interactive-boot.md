# The interactive boot, migrated

An appendix to [notes/trusted-init.md](../trusted-init.md), which summarizes it.

The tree in [supervision-tree.md](supervision-tree.md) proves the pattern with programs written for it. This is the same discipline applied
to the boot path a person actually types at: `system_initializer` (riscv) and `hello`'s `init_boot`
role (aarch64), which until now held the kernel's whole construction budget for life.

## The shape, and why it is not the one that was predicted

The prediction recorded here was: *the spawn service becomes a sub-server holding the archive and a
budget, the progenitor wires the shell to it, and the progenitor drops what it no longer needs.* That was not built, and
the reason is worth keeping rather than quietly diverging from.

The spawn service is the ELF loader, and the ELF loader is the archive. Moving it out means the
sub-server holds the initrd, which is every program in the system. So "it can build exactly one
program" (the property that makes `spawner` worth having) does not survive the move. The sub-server
would hold strictly more than the progenitor does today, and the progenitor would hold nothing but a pipe. That is a
relocation of the authority, not a reduction of it, and it costs an IPC hop on every capability the
shell delegates, because `spawnproto` moves capabilities and not just words.

So the interactive progenitor keeps the loader and gives up three other things instead.

1. The root construction budget. It carves `PROGENITOR_RESERVED_PAGES` (128, for its own scratch page
   tables) and `JOBS_BUDGET_PAGES` (240, the job pool) off the root untyped and deletes the root.
   After that it can spend at most those two, and it cannot reach the rest of what the kernel gave it
   or delegate the root to anything it builds.
2. The device authority. The UART device capability and the UART receive interrupt go back as
   soon as the console and input drivers are built, along with aarch64's test SGI and the kernel's
   report endpoint, which were never part of the interactive system. A progenitor that kept the device
   could hand the UART to anything it later builds.
3. Anything that reaches a live job's memory. Each job is built in a region split off the pool,
   and the progenitor deletes that region capability as soon as the job starts. Since §32 the reap does not
   need it.

## The job pool is bounded *because* it is renewable

Bounding the progenitor's budget would be a bad trade on its own: a prompt that runs out of memory after thirty
commands is worse than a prompt whose progenitor holds too much. What makes it cheap is that the pages come
back. Every job is born supervised (§26's spawn-slot convention: a `READ` view of one endpoint in the
reserved fault slot, which `START` reads and clears), and `job_undertaker` collects the corpse through
`Endpoint::REAP`. Its entire authority is that one endpoint capability: no untyped, no frame, no TCB,
nothing it could build with. The reclaimed region returns to *the progenitor's* pool, because §13 says a region
belongs to whoever owns it and the progenitor is the one who split it. A process that can free a job's memory
and can never spend it is exactly what §32 was decided for, and this is its first non-test consumer.

Why a second process rather than the progenitor collecting its own children. There is no non-blocking
receive, and the progenitor is parked in `RECEIVE` on the shell's spawn channel for its whole life. Multiplexing
deaths onto that same endpoint was considered and rejected: the shell holds `WRITE` on it, so a
compromised shell could forge death messages, and `EVENT_FAULT`/`EVENT_EXIT` (1 and 2) collide with
the program ids `spawnproto` already sends in word 0.

## Proven, both ISAs

- `system_tests/src/user/job_undertaker_tests.rs`, a control and a claim in that order.
  `without_a_collector_a_bounded_job_pool_runs_out` builds three jobs in a three-job pool, lets all
  three *finish*, and shows the fourth refused: a finished job's region is not free memory. Then
  `job_undertaker_returns_every_finished_job_to_the_pool` puts twelve jobs through the same pool with
  the real `job_undertaker` binary running, and asserts after each one that the pool came all the way
  back and at the end that it carves again in one piece. The assertion is which budget the pages are
  in, never how long anything took.
- `script/swish-check`, which is the only thing that boots the real interactive progenitor. It reads a
  sentence the progenitor prints from inside itself, after deleting the root untyped and before starting the
  shell. The progenitor retypes a page and retypes a kernel object on that slot. It prints "construction budget
  dropped; retype answers NoSuchSlot" only when both answered `NoSuchSlot` (-1) rather than
  `NotPermitted` (-3). Gone, not narrowed; the other branch prints "NOT dropped" so a boot that kept
  its budget fails loudly. The script then runs thirteen jobs through the six-job pool, so a boot
  where nothing was collected answers "could not spawn (the progenitor is out of memory)" partway down instead
  of the arithmetic.

## BUGS

- Recovery is LIFO (§16, `crates/memory_regions`). A job region reclaimed while it is not at the top of
  the pool's watermark returns nothing and leaves a hole until the pool's owner dies, which the progenitor never
  does. Sequential commands at a prompt are exactly LIFO and recover fully; two jobs alive at once (a
  pipeline stage outliving its producer) permanently costs one region, so a long enough session of
  concurrent pipelines still ends at "could not spawn". Six job slots is generous for a prompt and
  small enough to keep the gate honest, which is the trade.
- Init held a writable mapping of everything it ever built, until 2026-10-05 (UTC). `build_child`'s
  scratch window is never unmapped (it cannot be: nothing in the ABI unmaps a page), so the progenitor can read and write any
  page it laid down for a child. That included every boot server's. *Closed 2026-10-05; the
  parenthesis stays because §162 (whether a holder can give up a mapping) quotes it, and the
  sentence it quotes is also kept on the parent page, which is the file §162 names.* §249 (a
  running address space stays nameable) let the kernel grant the progenitor its own space at slot
  28, and the loader now `UNMAP`s each page once it is in the child. The shell's output page and
  the two DMA pages the progenitor peeks at go the same way. `system_tests`' `running_space_tests`
  has the negative control: a builder writes to a page it filled for a child and faults.
- The boot servers are not supervised. Endowing them a supervision endpoint with nobody to
  restart them would make their corpses persist forever instead of being reaped by the kernel, which
  is strictly worse. A dead console is still a halt, which is §26's fail-closed floor and correct for
  a tier-one server nothing can rebuild.
- `job_undertaker` cannot say anything. Init holds no channel it could report on, so a refused reap
  traps rather than being ignored. The visible symptom of the collector dying is that the pool stops
  coming back and the prompt eventually answers "could not spawn".
- A hung job is not collected, on purpose: `REAP` refuses a live thread (§32), and that is the
  watchdog case, which belongs to milestone 23. At the prompt the case a person can see is already
  covered by §24's forcible tier, and those jobs are built from the shell's own untyped
  rather than the progenitor's, so they never reach the collector at all.

## One progenitor, and one loader (milestone 96)

This increment left two duplications behind on purpose, and both are gone now.

The larger one was the system itself. `user::initial_ramdisk()` loaded an archive entry called `init`. That
was `fixtures/src/hello.rs`'s `init_boot` role on aarch64 and a separate program on riscv64
(milestone 266 (one progenitor, on all three architectures, and `init` stops being a role) later collapsed both the alias and the two programs into one `progenitor`).
Everything on this page (the six boot components, the giveaway, the negative control, the spawn service)
was written once in each. That was about three hundred near-identical lines, and the failure mode is
the reason it mattered rather than the line count. A fix that lands in one and not the other is a
boot that reaches userspace and prints nothing at all, with no fault and no message. That cost three
separate lanes an evening each. It is now `crates/system_initializer`, and the boot entry is the table of slot numbers
its own kernel granted plus a call into it. Those tables are the one thing the boards genuinely
disagree about. aarch64's boot path is shared with the test roles of milestone 19d (granular verbs and userspace init). So it grants a report
endpoint and a test SGI the interactive system never uses, and numbers everything after them
differently. That is data the crate takes (`BootEndowment::unused`), not code it repeats.

The smaller one was the loader. `crates/supervision_protocol`'s child builder was a generalization of
the two inits' `build_child`, so the logic existed three times. This increment added the fault slot
to all three rather than unifying them. Unifying loaders and migrating the boot path in one pass
would have made a boot failure ambiguous. Milestone 96 (one init: the spawn service written twice) did both, in that order, with
`script/swish-check` between them. `supervision_protocol`'s is the tree's only loader now; what it grew
is what the inits needed, a capability placed at a named slot (§67's diagnostic stream) and a
stack size the caller states. The stack is a field rather than a constant because the two callers
honestly differ: four pages is enough for the supervision tree, and a child at the prompt gets twelve
because the shell's redirection path found the wall twice.

## What the boot keeps, and what it says before the prompt

`system_initializer::boot` ends by holding a few capabilities for the life of the boot and printing
a few sentences. Each has a reason the code cannot show, and they are here.

The file service stays. A directory grant has one delivery mechanism, a caretaker, and the caretaker
must hold the file service to attenuate it. The shell's copy carries no `GRANT`, so only the
progenitor can build one. A program spawned without the capability its command line named would be
the worst failure this model has. The two slots sit on the progenitor's capability peak for the
whole boot, and `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` keeps that account.

The boot terminal's endpoint stays too, and its output page does not. A `graphical_terminal` session
with no virtio keyboard takes its keystrokes from the boot's own line discipline, in raw mode. Only
the progenitor can grant a session that endpoint. The shell's copy carries no `GRANT`, so nothing
typed at the prompt can hand the terminal, with its `OPERATION_READLINE`, to any program it likes.
That widening is what milestone 50 (pipes and redirection)'s sink adapter exists to avoid. So the
spawn service holds `WRITE | GRANT` on an endpoint it never receives on, the same shape as the file
service pair.

The sentences are printed through the shell's output page, mapped into the progenitor only once the
first one is said. So the entropy, network and login outcomes are decided early in `boot` and said
together at the end. The measurement sentence is the load-bearing one. A measured boot's natural bug
is for the check to vanish when the build step does not run, and a boot that says nothing looks like
one that measured everything. `script/swish-check` reads the sentence, so a boot that stopped
measuring fails the gate. The sentence about how directory grants are delivered is there for the
same gate.

The login sentence names its evidence. It reports that `identity_provisioner` answered `IDP_RPT_OK`:
a credential store exists and holds this identity and secret. It does not claim that `login` is
alive. Knowing that would need `login` to say so, which is a message, an endpoint and a wait the
progenitor does not have. A boot that blocks on a child's readiness hangs when that child is the
broken thing. The survival claim lives in `script/swish-check` instead, which fails a boot whose
kernel reported killing a user thread. The password is printed once, because the progenitor's own
copy is wiped once `identity_provisioner` has used it.

The undertaker gets three capabilities and nothing else. `READ` on the supervision endpoint lets it
free a job's memory and never spend it. `WRITE` on the result endpoint lets it say the one word a
killed job cannot say for itself, `JOB_FAULTED`. `WRITE` on a copy of the spawn endpoint, badged
`UNDERTAKER_BADGE`, lets it say which job it reaped. Neither `WRITE` carries `GRANT`.
