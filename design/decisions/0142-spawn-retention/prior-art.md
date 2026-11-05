# §142 appendix: prior art

An appendix to [§142 (what a spawner retains over a child after
`START`)](../0142-spawn-retention.md): what seL4, Genode, Zircon, L4Re and QNX let a spawner keep,
quoted from their own documentation. The main page carries the survey table and is complete
without this file, which exists to verify or challenge it.

## Prior art, fetched rather than recalled

Every quote below was fetched on 2026-09-03. Where a source would not yield a verbatim
sentence it says so rather than paraphrasing into quotation marks; this tree carried a fabricated
block quote for twelve days through every gate.

### seL4: there is no parent, only whoever kept the capability

`seL4_TCB_Suspend`, `seL4_TCB_Resume`, `seL4_TCB_Configure` and `seL4_TCB_SetPriority` all state the
same requirement, word for word:

> Capability to the TCB which is being operated on.
>
> Source: [seL4 API Reference](https://docs.sel4.systems/projects/sel4/api-doc.html) (fetched
> 2026-09-03)

Twenty TCB methods share one authority and one shape. There is no spawner in the picture at all: a
TCB is retyped from untyped, the capability lands in the retyper's cspace, and whether the retyper
keeps it is a fact about that program rather than about the kernel. **seL4's answer to "what does a
spawner retain" is "exactly what it did not delete", and the retention handle and the construction
handle are the same object.** That is the shape nife has today, minus the running-thread verbs.

The relevant caution is already in this tree: §139 records `seL4_TCB_SetAffinity` as the worked
example of a per-thread property expressed as a TCB method that MCS deleted outright.

### Genode: the parent keeps a session capability, and the power that comes with it is total

> Since the parent retains the PD-session capabilities of all its children, it can issue further
> quota transfers back and forth between the children's PD sessions and its own PD session, which
> represents the reference account for all children.
>
> Source: [Resource trading, Genode OS Framework
> Foundations](https://genode.org/documentation/genode-foundations/25.05/architecture/Resource_trading.html),
> "Trading memory between clients and servers" (fetched 2026-09-03)

> This includes the decision to destruct a child at any time in order to regain the resources that
> were assigned to the child.
>
> Source: [Recursive system structure, Genode OS Framework
> Foundations](https://genode.org/documentation/genode-foundations/25.05/architecture/Recursive_system_structure.html),
> "Component ownership" (fetched 2026-09-03)

And in the other direction, the child gets one thing:

> At creation time, the parent installs a single capability called _parent capability_ into the new
> protection domain.
>
> Source: same page (fetched 2026-09-03)

Genode is the system that answers this question hardest, and its answer is a per-child session
capability held for life. Note what it buys with it: not a kill verb, but *resources*. Retention in
Genode is about the quota account, and destruction falls out of owning what the child is made of.
That is §40's argument, reached by a different tradition, with a handle attached.

### Zircon: handles, and a deliberate refusal at the thread

> This asynchronously kills the given process or job and its children recursively, until the entire
> task tree rooted at handle is dead. Killing a thread is not supported.
>
> _handle_ must have `ZX_RIGHT_DESTROY`.
>
> Source: [`zx_task_kill`, Fuchsia](https://fuchsia.dev/reference/syscalls/task_kill) (fetched
> 2026-09-03)

The authority is a handle with a right, held by whoever holds it, and the *granularity* is the
process or the job. Zircon retains a handle and refuses to point it at a thread.

### L4Re: the retained thing is the capability, and destruction is deleting it

> If obj has the delete permission, initiates the deletion of the object. This implies that all
> capabilities for that object are gone afterwards.
>
> Source: [`l4_task_delete_obj`, L4Re](https://l4re.org/doc/group__l4__task__api.html) (fetched
> 2026-09-03)

> If the reference counter of a kernel object referenced in fpage goes down to zero (as a result of
> deleting capabilities), the deletion of the object is initiated.
>
> Source: [`l4_task_unmap`, same page](https://l4re.org/doc/group__l4__task__api.html) (fetched
> 2026-09-03)

### QNX: the counterexample, and it is the ambient one

> To signal another process, either the user IDs must match, or the calling process must have the
> `PROCMGR_AID_SIGNAL` ability enabled.
>
> Source: [`kill()`, QNX Neutrino Library
> Reference](https://www.qnx.com/developers/docs/8.0/com.qnx.doc.neutrino.lib_ref/topic/k/kill.html)
> (fetched 2026-09-03)
