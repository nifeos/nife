# The fine counter is not ambient, and that took a decision (milestone 229)

*Name: provisional. Named for the grant milestone 229 built.*

An appendix to [notes/abi.md](../abi.md), whose section on the one ambient thing, reading the clock,
summarizes it.

The coarse counter that section describes is ambient. The cycle counter is not, on two of the three
architectures, and the difference is deliberate. It is roughly 160x finer (0.25 ns against 41 ns),
so spending §10's exception a second time on it was not free. DECISIONS 139 (who may read the cycle
counter, and by what authority) decided it in three parts. Milestone 228 made the closed default a
fact rather than an assumption, and milestone 229 (build the cycle-counter grant DECISIONS 139 decided) built the grant.

## The mechanism

It is a per-thread grant the context switch enforces. `sched::grant_cycle_counter` sets a bool on
an embryo and refuses any thread that is not one. So it belongs to the same creation-time set as
`CONFIGURE` and `CAP_INSERT`, and a running program cannot ask for it. The kernel writes the enable
at the switch, beside the address-space root, comparing before it writes. So a machine where nothing
is granted never writes the register at all.

## No syscall method, yet

There is deliberately no syscall method to set it, and that is the part worth knowing. The kernel
half is complete; the surface is deferred. A method number is irreversible, and this one's successor
is already foreseeable. The prior art for a per-thread property as a TCB method is
`seL4_TCB_SetAffinity`, which MCS deleted and replaced with a field of `sched_control_cap`. And
milestone 147 (a profiler that holds exactly the counters it was granted) wants cross-thread
authority with a named target that no such method would provide. 139 also records that 147's
target-naming has no precedent here to price from, so the object is not buildable yet either.

The choice was method-now against not yet, and not-yet costs almost nothing while the grant has no
consumer. Whoever needs it mints it, with a requirement in hand. A field on `CONFIGURE` is not the
cheap way round. `invoke` has three argument registers, and `CONFIGURE` and `START` spend all three
each, so it would mean widening `invoke` itself.

## Per architecture

| ISA | What the grant opens | Closed by default | Cost when nothing is granted |
|---|---|---|---|
| aarch64 | `PMUSERENR_EL0.CR`, so EL0 may read `PMCCNTR_EL0` | yes, `timer::init` writes zero per core | a per-core load and a compare |
| riscv64 | `scounteren.CY`, so U-mode may read `cycle` | yes, `timer::init` writes `TM` alone per hart | a `csrr` and a compare |
| `x86_64` | nothing: `rdtsc` is already ambient | **no**, and that is a stated position | nothing; the call compiles away |

The `x86_64` row is an exception §19 (architectural parity is a tenet) should read as stated
rather than as a gap. `CR4.TSD` would close `rdtsc` to ring 3. And there is no coarse fallback on
that architecture, the way `CNTVCT_EL0` and `rdtime` are fallbacks on the other two.
`user_mode_runtime`'s `now()` there is `rdtsc`. So closing it takes out `Instant`, `thread::sleep`,
the random seed, smoltcp's timestamps and the benchmark harness at once. DECISIONS 139 measured the
alternatives (trap-and-emulate at 1,667 ns, 4.1x the syscall it would be beating) and closed them.

## The grant is only built when something asks for it

That is milestone 237 (the cycle-counter grant costs 136 bytes of IPC fastpath), the cycle-counter grant as a measurement build. Every piece above is behind
`any(test, feature = "cycle_counter_grant")`, so a shipping kernel carries none of it. The field,
the switch read, the register write and `grant_cycle_counter` all leave the binary.
`sched::schedule` is 136 bytes smaller for it (`script/fastpath-footprint`'s aarch64 `ipc_fastpath`
closure, 5852 against 5988). `script/test` compiles and runs milestone 229's proofs on all three
architectures anyway, because `test` is in the predicate. And `script/lint` clippies the feature
without `test` on all three.

Two things that gate does not cover, both deliberately. The default write of milestone 228 (the
cycle counters are closed by assumption) is in every build, granted or not. Closing what we claim is
closed is right whether or not anyone can be granted an exception. So the "closed by default" column
above is true of a production kernel. And nothing here was deleted, because deleting it would make
the cycle figure milestone 25 (cross-OS performance comparison) publishes against seL4's
unreproducible. It would also quietly return DECISIONS 139's answer to "closed for everyone".

## What it is not

None of this is timing confinement, and nothing in the tree should be read as saying it is. Two
threads and a shared word reconstruct a 6.8 ns clock on any of the three architectures; see
notes/confinement-claims.md, which carries that row. What the grant buys is accountable
authority: the cheap accurate instrument is granted rather than ambient, and the kernel knows
which threads hold it.
