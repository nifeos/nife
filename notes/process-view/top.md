# `top`

*Name: provisional. For the program it describes.*

An appendix to [the process view](../process-view.md). `crates/top` is the program's logic and
`components/src/top.rs` is its syscalls and output streams.

## What it ranks by

`top` means the top N by consumption, so the program waited for something to rank by:
`abi::survey::record::CPU_TIME`, a measurement. §150 (how does a thread's CPU time reach
userspace?) refused wall-clock age and userspace sampling as numbers that would not mean what the
name says. The walk, the CPU-time join, the ranking and the table are all `ps`'s. They are `ps::collect`,
`ps::Survey::join_cpu_time`, `ps::Survey::rank_by_cpu_time` and `ps::Survey::write_report`. `top`
adds the summary line, which a ranked view needs because the head of a distribution says nothing
about its size.

```text
$ top
up 00:01:12, 4 threads: 1 running, 1 ready, 1 blocked, 1 dead
         TID  STATE     TIME(ms)
           9  running        310
4294967302  dead             150
           5  blocked         20
           7  ready            0
$ top        top: this process holds no process-domain capability
```

## The summary line

The summary goes to the output stream rather than to diagnostics: it is part of the answer, not a
complaint about it. A caller that was refused prints neither the summary nor the table
(`ps::Survey::write_report` already declines), so `top > out.txt` is empty on a refusal exactly as
`ps > out.txt` is.

Its uptime is `user_mode_runtime::monotonic_nanos()`, the ambient counter every process holds
unconditionally. That counter is `kernel/src/arch/*/timer.rs`'s documented exception to DECISIONS
§10 (process model: capability-based, microkernel)'s no-ambient-authority rule. `write_summary`
takes it as a parameter rather than calling it, so its output is a value a host test can assert on.

The state counts are in the order a reader scans for: what is on a CPU, what wants one, what is
waiting, what is over. A zero count is printed rather than elided, because a row missing from a
status line reads as a status line that did not check.

## The machine line

Upstream `tload` draws the load average as a graph. This kernel keeps no decaying load figure, and
§225 (`free` sees the machine and your share) ruled that the question belongs in `top`'s summary
rather than in a program of its own. So the line says what the machine statistics page can say
truthfully. It gives how many threads were runnable at each core's last tick, on how many cores, and
what share of all ticks since boot went to something other than the idle thread. It widens `top`'s
authority past `ps`'s by one read-only page, which also settles the question milestone 281 (`watch`
holds exactly what `ps` holds)'s rule left open: `top` no longer holds exactly what `ps` holds.

## What it does not do

- No live refresh. An interval spin would be charged to this program by the very counter the table
  ranks on, so `top` would report itself as the busiest thread.
- No `%CPU`. A percentage needs two samples, and a cumulative one against uptime is the wall-clock
  age error §150 refused. `TIME(ms)` is the cumulative figure, as Unix's `TIME+` is.
- No top N: `ProgramArgumentDeclaration` cannot declare an optional integer.
- The summary counts this domain, not the machine, because nothing here is ambient.
- Ties break by tid, so a quiet machine ranks in slot order.
