# The service path: what a userspace server costs

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds `relay_rtt`, `broker_rtt`, `fs_read`,
and two small re-saves from July and August, with the dates, tables and corrections behind them.
Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## The service-path benchmarks: what a userspace-server architecture costs (2026-07-29)

The microkernel bet is that filesystems, network stacks and drivers belong in confined userspace
processes, not the kernel. The skeptic's fair question is the price. A request that a monolith
serves with one syscall now crosses into another process, maybe through a third. Two benches answer
it. The two servers this project runs sit on opposite sides of a measurement line, so they are
measured differently.

### `relay_rtt`: the confined-server tax, isolated and gated

Real services fan out. The FS server CALLs the block server (`client -> fs -> blk -> fs -> client`),
and net_stack CALLs the NIC driver (`client -> net_stack -> driver -> net_stack -> client`).
`relay_rtt` (kernel-side, `bench.rs`) is exactly that two-hop topology: a client through a relay to a
backend and back. It sits on the icount baseline next to the one-hop `ipc_rtt`:

| bench | topology | icount ticks/iter |
|---|---|---|
| `ipc_rtt` | client <-> server (one hop) | ~982 |
| `relay_rtt` | client -> relay -> backend -> relay -> client (two hops) | ~1,961 |

The two-hop path is ~2.0x the one-hop. The difference, ~980 ticks, is what one confined intermediary
that delegates to a backend costs: two extra context switches and two extra rendezvous per request.
That is the architecture's per-request tax over a monolith. It is isolated from any device,
deterministic, and gated by `--check`, so a regression in the IPC or switch path shows up against its
commit.

Adding `relay_rtt` shifted the other kernel-side IPC benches a few percent (`ipc_rtt` +6%) through
whole-crate codegen, all sub-tripwire (the churn in
[the icount drift appendix](icount-drift-and-provenance.md)). The baseline was re-saved to absorb it
in the commit that added the bench.

### `broker_rtt`: what the queue broker costs when both ends are up

Milestone 23 (a capability-routed component OS with live replacement), DECISIONS §41 (the endpoint is the broker). Milestone 23's latency ladder has two rungs built. This number makes
"opt-in per channel, never the default" a rule rather than a preference.

The default rung has no benchmark of its own, because it has no cost of its own. A client holds a
capability to a stable *endpoint*, and whoever waits in `RECEIVE_CAPABILITY` on it answers; a swap changes
who that is. No process stands in the data path, so the steady state is `call_reply` exactly, and the
swap adds nothing to it. The kernel's own sender queue buffers the down window: requests that arrive
while nobody is receiving park there, and the replacement drains them.

The opt-in rung is `broker`, interposed so a producer never blocks on an absent consumer. It is the
same client and the same backend as `call_reply` with a process in between, so the difference is the
whole tax:

| bench | topology | icount ticks/iter (aarch64) | HVF ns/iter |
|---|---|---|---|
| `call_reply` | client <-> server, one endpoint (**and the swap's steady state**) | ~1,007 | ~1,172 |
| `broker_rtt` | client -> broker -> backend -> broker -> client | ~2,010 | ~2,368 |

That is 1.99x, and about 1.2 microseconds of real time per request on this laptop. RISC-V agrees to
within a percent (169,282 vs 338,000 ticks/1000, 2.00x). It is the price of decoupling two
components' lifecycles. Paying it on every IPC would trade the measured round-trip advantage for a
feature used during swaps, so the broker is wired per channel. It sits on the icount baseline for
both ISAs, like `relay_rtt`.

Two notes. First, `broker_rtt` and `relay_rtt` measure the same shape (one confined intermediary) in
the two idioms the codebase uses, `CALL`/`Reply` and `SEND`/`RECEIVE` pairs. They land within 2.5% of
each other, a small cross-check that the Reply-capability path costs about what a pre-wired reply
endpoint does. Second, what the broker bench measures is the broker's pass-through. During a down
window it does strictly less work per request (one rendezvous, an enqueue, an immediate answer).
That is not the number to quote, because the steady state is where a channel spends its life.

Adding this bench shifted the other kernel-side numbers by a couple of percent through whole-crate
codegen (`spawn_reap` -2.3%, `ipc_rtt_el0` +0.3%), all sub-tripwire. Both baselines were re-saved in
the commit that added it.

### `fs_read`: the real RedoxFS read, whole path, and why it cannot be the isolated number

A client opens a file through a granted directory capability and reads a block over the real
confined stack. That stack is a block server driving the RedoxFS disk by DMA, and the vendored
RedoxFS engine mounting it over blk IPC on its own heap. It runs on the `--real --smp` boot, where the
redoxfs_server test proves the whole stack, and it reports:

```
fs_read   ~9.8M ticks / 2000 reads   ~204 us/read   (HVF, --release --smp, stable across runs)
```

The 204 microseconds is device latency. A read is not served warm from a cache. It goes to the block
server, which does a DMA transfer and waits on the disk's completion interrupt, ~200 us per block
under HVF. That swamps the FS server's own IPC-contract tax (the extra `client -> fs` hop and the
engine's dispatch), which `relay_rtt` puts at a few hundred *nanoseconds*.

*Correction, 2026-09-24: "not served warm from a cache" was true of the build measured here. Milestone
138's step 2 (2026-08-19) added a 64-block metadata cache, and `fs_read` then measured 9,474 ns warm,
22.2x faster. See [milestone 138 (close the read gap), steps 4 and 2](read-path-block-contract-and-metadata-cache.md). The ~204 us stays
the cold, uncached figure and the quiet-machine control the filesystem appendices use.*

So `fs_read` is the whole-path cost of a userspace file read, not an isolated server tax. Milestone
21's rule names this case: when device latency swamps the isolation, measure the whole path and say
so. Isolating the file server's own cost was attempted and abandoned. A warm cache read and a raw
blk-IPC read differ by a few hundred nanoseconds on top of a ~200 us block read with its own
run-to-run spread, so the delta is in the device noise. The isolated per-hop tax lives in `relay_rtt`,
where it is measurable.

`fs_read` is `--real`-only and never gated. The mount and every read are interrupt-driven, not
deterministic under `-icount`, so gating on it would enshrine the non-determinism the 2026-07-28
lesson warns against ([the icount drift appendix](icount-drift-and-provenance.md)). It self-skips
(the `online_count() > 1` gate) everywhere but `--real --smp`, so `bench/baseline-aarch64.txt` never
sees it.

### net_stack's socket round trip: measured, but not as a third icount bench

The net path has the same shape as the FS path: a confined server the client reaches only through a
granted `Stack` capability. Its per-request IPC tax is the same `relay_rtt` topology. But a net_stack
socket round trip is even less gate-able than `fs_read`. net_stack only reaches its serve loop after
a DHCP handshake, and its RECEIVE path drives smoltcp's own retransmit and delay-ACK timers
(notes/net/the-outbound-gates.md). So the path is deterministic under neither `-icount` nor, at the socket level, a warm
HVF loop.

The existing net tests prove and time the socket contract end to end instead
(`a_client_resolves_dns_through_the_socket_contract`, `a_client_echoes_over_tcp_...`, both ISAs,
both transports). A bench could only report device-and-timer latency. The bare EL0 round trip those
build on, `ipc_rtt_el0` in [the cross-OS appendix](cross-os-primitives.md), is the raw baseline. The
`relay_rtt` delta is the confined-server tax net_stack pays on top of it, the same as the FS server.

## 2026-07-29: both baselines re-saved for one instruction on the exception-return path

Milestone 22 (trusted init) phase B.2 fixed a real race in the exception-return path. Staging
`SPSR_EL1`/`ELR_EL1`, or `sepc`/`sstatus` on RISC-V, is not atomic with respect to a nested
exception (notes/exceptions.md). The fix is one instruction at the top of the restore, masking
interrupts. It lands on every return from an exception, the hottest path either instrument measures.

The movement is well under 1%, and both baselines were re-saved in the commit that caused it, per the
milestone-21 discipline. aarch64 `null_syscall` went 457503 -> 458753 ticks over 20000 iterations
(+0.3%), `ipc_rtt_el0` +0.2%, `ctx_switch` +0.1%. RISC-V shows the same order, mixed in sign
(`null_syscall` +0.3%, `ipc_rtt` -2.2%). That is the build-to-build icount drift
[the icount drift appendix](icount-drift-and-provenance.md) documents, not anything the change did.

This is not a regression traded for correctness. One masked-interrupt instruction per exception
return is what the correct version of that path always cost. The previous numbers measured a version
that could return a new process to its entry point at the wrong exception level.

Measured-boot (phase B.1) moved nothing on either ISA. The bench boot enters no boot program, so the
SHA-256 over the progenitor never runs there.

## 2026-08-04: the RISC-V baseline re-saved for a win this instrument cannot see

Milestone 58 (RISC-V TLB shootdown) removed the unconditional `sfence.vma` from every RISC-V context switch: a full TLB
flush per switch, gone. The numbers went up:

| benchmark | before | after | delta |
|---|---|---|---|
| ctx_switch | 471,827 | 477,635 | +1.2% |
| ipc_rtt_el0 | 1,738,256 | 1,766,199 | +1.6% |
| yield_switch | 179,097 | 179,217 | +0.07% |

This is the clearest case of icount measuring the wrong thing. icount counts guest instructions
retired. A TLB flush is one instruction; its entire cost is the misses that follow. QEMU's TCG refills
its softmmu TLB with host-side work that retires no guest instructions at all. So the instrument
charged for what was added and credited nothing for what was removed.

What was added, per switch: an atomic load and a branch for the ASID-width gate, and a `csrr satp`
plus a compare for the "already installed?" early return `switch_user_root` gained. The early return has been
on aarch64 since milestone 15 (tagged address spaces). It fires on every switch between two kernel threads, which is
most of them on an idle machine. Three or four instructions, traded for not throwing the TLB away.

The baseline was re-saved in the commit that caused it, per the milestone-21 discipline. aarch64 was
left alone. The only aarch64 change in that milestone is a `dsb ishst` at the top of `flush_asid`,
which runs at address-space teardown and not on any measured path. Its numbers moved by less than
the documented run-to-run drift.

The number that would settle it needs hardware with a real TLB. `--real` runs under
Hypervisor.framework, which executes the host's own ISA, so there is no accelerated RISC-V leg. The
VisionFive 2 is where this gets measured.
