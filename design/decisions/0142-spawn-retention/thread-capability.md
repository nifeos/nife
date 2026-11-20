# §142 appendix: the thread capability today, and what R2 would make of it

An appendix to [§142 (what a spawner retains over a child after
`START`)](../0142-spawn-retention.md): the premise check on the thread capability after `START`, the call
sites that delete it, and R2's findings. Moved out of that page on 2026-10-12 (UTC) under [§212 (a
prose budget)](../0212-a-prose-budget-for-every-document.md). The main page is complete without it;
this file exists to verify or challenge it.

## The premise, checked rather than quoted

`kernel/src/system_call.rs:305` says every `ThreadControlBlock` method "is process-spawn machinery",
which reads as a claim that the capability is inert once the thread runs. It is true, and it is
enforced in the scheduler rather than in the dispatcher, which is a stronger place for it to live.
Read at `49db708f`:

| Entry point | Where the check is | The check |
|---|---|---|
| `thread_control_block::CONFIGURE` | `sched::configure_thread_control_block` | `if t.handshake.state != State::Embryo { return Err(WrongObject) }`, twice, before and after taking the address space |
| `thread_control_block::CAP_INSERT` | `sched::thread_control_block_insert_cap` | same, once |
| `thread_control_block::START` | `sched::start_thread_control_block` | same, once, then the whole-or-refuse gate |
| `sched::grant_cycle_counter` | itself | same, and there is **no method number for it**: §139 (who may read the cycle counter, and by what authority) declined to mint one |

There is no fourth method and no fifth entry point. A `ThreadControlBlock` capability held after
`START` authorizes nothing. Every method it names answers `WrongObject` for the life of the thread.
That property is what proposal B would end, and it is worth saying plainly that it is a real property
today and not a comment's optimism.

One consequence of the check's placement, since it bears on cost. The refusal is in `sched`, not
in the `match` arm, so a new verb that works on a running thread does not have to unpick a
dispatcher-level guard. It adds an arm and a scheduler function whose state check is different from
its three neighbors'. That is cheaper than it sounds and it is also the thing that makes the
capability's meaning state-dependent, which the ladder in AGENTS.md says to design out rather than
in.

### What the tree does today, counted

Thirty `START` sites in shipping and test code (`crates/system_initializer`, `user/src/*`; the
`supervision_proto` and `pgrep` matches are the wrapper definition and doc comments, excluded).

| | sites | where |
|---|---|---|
| `cap_delete` on the TCB slot immediately after `START` | **25** | all fifteen in `crates/system_initializer`, `root_supervisor` (2), `swapper` (3), `timetable` (2), `spawner`, `c_confiner`, `login` |
| the capability is kept | **5** | `fixtures/src/hello.rs` only, at 468, 500, 523, 671, 723 |

So the tree already has an unwritten answer, and it is "retain nothing". The five exceptions are
in one file, the kernel's own test program, whose parents `exit()` a few lines later; none of them
records a reason for keeping it, and nothing in the tree would notice if they stopped.

This inverts one of the costs that looks obvious from outside. Proposal B's own account says
"`crates/system_initializer` and every spawn path would need auditing for who ends up holding one
after `START`". The audit has now been run, and the finding is the opposite of a widening: almost
nobody holds one, so B does not hand an existing population of holders a new power. It requires
twenty-five call sites to *stop deleting*, which is an additive convention change rather than a
silent widening, and which has a price measured below.

## R2's findings

Two things learned on 2026-09-03, in conversation with calef, that raise R2's price above what
this entry first stated.

**The state-dependence is this proposal's, not the tree's.** The entry above says R2 makes a
capability's authority a function of the object's state. Re-read, that is backwards about what exists
today: `ThreadControlBlock`'s three methods are all construction verbs, and
`configure_thread_control_block`'s own comment gives the reason its refusal exists, which is a real
precondition rather than a smell:

> Refuses anything but an `Embryo`, so a running thread cannot be reconfigured under itself.

seL4 has no such rule because its TCB methods are *different verbs* (`Suspend`, `ReadRegisters`) that
are meaningful after a thread runs, not the same verbs applied later. So this tree does not have
state-dependent authority to remove; R2 is what would introduce it, by bolting one any-state verb
onto three embryo-only ones. That hybrid is more state-dependent than either model it sits between.

And seL4's verb set follows from a recovery model this tree has already declined. seL4 delivers a
fault to a handler that may read the faulting thread's registers, repair the cause and resume it,
which is why `ReadRegisters` and `Resume` exist and why a lifetime handle is load-bearing there. §26
(the fault endpoint) chose the other answer in as many words: **"restart policy stays in userspace,
and the kernel never relaunches anything"**, and *"restart policy is userspace code and stays there"*.
A supervisor here does not repair a broken thread, it builds a fresh one from the same region, and
`build_child` is that path. §26 reserved a word so a fault-reply/resume protocol could arrive
additively, so this is a deferral rather than an absence.

The consequence for this decision. A faithful R2 pays off only alongside seL4's recovery model,
which would mean adding `Suspend`, `Resume` and register access, new system call surface with no customer.
A partial R2, one lifetime verb on a construction capability, buys the state-dependence without the
uniformity that justifies it. **R2 is therefore not "seL4's answer, available cheaply"; it is half of
seL4's answer without the half that makes it coherent.**
