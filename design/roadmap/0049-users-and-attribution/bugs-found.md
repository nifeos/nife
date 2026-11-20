# Milestone 49: bugs found on the way, and two limits raised

This appendix belongs to [milestone 49 (users, login, and
attribution)](../0049-users-and-attribution.md). It was moved out of that page on 2026-10-12 (UTC)
under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main
page is complete without it. Each bug here is worth reading past this milestone, because none of
them is specific to `login`.

## Two resource leaks from the channel-per-client front door

The channel-per-client piece ([build.md](build.md#the-channel-per-client-front-door)) introduced
two resource leaks. Both were found and fixed before it landed, and both have the same shape.

### `MemoryRegion::DESTROY` does not free the destroyer's own capability-table slot

`DESTROY` tears down the objects retyped from a region and returns its pages. `revoke_region`
deletes every `PageFrame` capability naming a freed page. It touches no `Rendezvous` capability,
and never the `MemoryRegion` capability *naming the region being destroyed*.

So a server that destroys a region per request runs out of its sixteen-slot capability table while
its memory budget still looks healthy. The failure arrives as whatever that server says when it
cannot serve. Here that was `login_contract::DENIED` on a correct password, on the second login after
start-up, for two days.

The kernel's `Error::OutOfMemory` made it expensive to find. It collapses "your budget is empty"
and "your table is full" into one code. That is milestone 153 (`OutOfMemory` collapses three distinct causes into one error code). Four separate memory
hypotheses were measured and ruled out first.

### A region destroyed out of LIFO order strands its pages

The pages stay stranded until the region's parent dies. That is `crates/memory_regions`'
`return_to_parent`, DECISIONS §16 (object revocation)'s documented half-answer.

A channel is minted before the login it carries and destroyed after it. Suppose its region is carved
from the same budget as that login's caretaker and client budget. Then the channel region is never
the top when it goes. One suite run showed 368 pages of holes.

The fix is a budget with exactly one spender. That makes the channel region always its only live
child. The general form: a short-lived region wants a parent nothing long-lived is carved from.

## Two boot hangs found while wiring the interactive boot

Both were found by `script/swish-check`, not reasoned to in advance.

### Waiting for a readiness message that could not exist yet

`credentialer.rs`'s readiness message (`RPT_READY`) fires only *after* its provision endpoint is
sealed, not at startup. `credentialer.rs`'s own "Two phases" doc says so. An earlier version of the
wiring read it *before* provisioning. That is not "wait for the service to come up", the way the
entropy block's own handshake is. It is a wait for a message that cannot exist yet. The boot hung
rather than faulted.

### Asking for rights init cannot hold

`login`'s delegation of the file service's shared page asked for `READ | WRITE`.
`crates/system_initializer::boot` cannot hold that: the kernel's own grant to init is
`WRITE | GRANT` only. A writable mapping never needed it anyway.
`kernel::system_call::page_frame_map`'s own comment says a read/write mapping checks only `WRITE`.
`components/src/login.rs`'s comment on that delegation has the full account.

### The build order and a full capability table

`credentialer` and `login` sit after the shell's own build, not after the sink adapter. An earlier
version of the lane placed them after the sink adapter, and `script/swish-check` was trapped in
total silence.

By the sink adapter, `boot`'s capability table already rests at eleven capabilities.
`credentialer`'s own six retypes on top of that make seventeen, against sixteen usable slots. Right
after `term_in` goes back, and before `term_sink` and the undertaker's own supervision endpoint are
retyped, the table rests at eight instead. That position was found by bisecting, the same way the
entropy-ordering fault was.

## Two limits raised

`kernel::user::spawn_init`'s and `riscv_shell_boot`'s construction budget was raised from 2048 to
12288 pages. `kernel::cap::CAPABILITY_TABLE_SLOTS` was raised from 16 to 17 for the same reason;
that constant's own comment carries the account. Four more permanent components is a real, measured
cost.

`kernel::cap` calls this "a one-number change here, paid in TCB size". It is this tree's established
answer to a real feature needing more of a cheap resource. `MAX_REGIONS` and `nifefs::NAME_LEN` were
each raised once already in the same shape.

Neither number is tuned to a minimum. Both carry real margin, found empirically rather than
derived. A later lane that wants either number tighter has real bisection work ahead of it, not a
guess to correct.
