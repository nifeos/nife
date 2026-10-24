# `top`: its slots and a run

[`notes/process-view.md`](../process-view.md) is the page to read. This appendix holds what
`components/src/top.rs` is handed and what a run prints.

## The slots

| slot | what | why |
|---|---|---|
| 0 | the output sink, `WRITE` | where the summary and the table go |
| 7 | the process domain, `ENUMERATE` | the supervision endpoint whose members it may name |
| 8 | the diagnostics sink, `WRITE` | where a refusal goes, so `>` cannot swallow it |
| 11 | the machine statistics page, `READ`, mapped read-only | the machine line under the summary |

That is `ps`'s authority plus one read-only page. Nothing in `top` can name a process outside the
domain it was handed. The slot 11 line is what became of `tload` under §225 (`free` sees the machine
and your share).

CPU time is continuous, so two runs show how much work another thread did between them. §150 (how
does a thread's CPU time reach userspace?) accepted that. `ENUMERATE` is already the right to learn
what exists, and the leak stays inside the domain.

## A run

```text
$ top
up 00:01:12, 3 threads: 1 running, 0 ready, 1 blocked, 1 dead
machine: 2 runnable on 4 cores, 7% busy since boot
         TID  STATE     TIME(ms)
           9  running        310
4294967302  dead             150
           5  blocked         20
$ top        top: this process holds no process-domain capability
```

The second run was granted no process domain, so it prints a refusal and no summary.
