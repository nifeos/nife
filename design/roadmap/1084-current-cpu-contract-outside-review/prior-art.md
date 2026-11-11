# Prior art

An appendix to [the milestone](../1084-current-cpu-contract-outside-review.md).

How other systems let a thread learn its own CPU without a crossing, and where this crate differs.

| system | mechanism | sentinel for "not known" | how the reader sizes a per-CPU array | read or recalled |
|---|---|---|---|---|
| Linux `rseq(2)` | a per-thread `struct rseq` the thread registers by syscall; the kernel writes `cpu_id_start`, `cpu_id`, `node_id` and `mm_cid` on every return to user space; 32-byte aligned | `cpu_id` is a `u32`; `RSEQ_CPU_ID_UNINITIALIZED` is -1 and `RSEQ_CPU_ID_REGISTRATION_FAILED` is -2 | at run time, from `sysconf(_SC_NPROCESSORS_CONF)` or `/sys`; no compile-time bound | read (`include/uapi/linux/rseq.h`) |
| Linux x86 vDSO `getcpu` | `RDTSCP` or `RDPID` carry the CPU in a register the kernel loads per CPU; no page | none; always an answer | as above | recalled, unverified |
| FreeBSD `sched_getcpu(3)` | on amd64 a user-mode read of a per-CPU register value, elsewhere a syscall; `MAXCPU` is a compile-time constant and `mp_maxid` the run-time bound | none | `MAXCPU` at compile time in the kernel; user space asks | recalled, unverified |
| Zircon | `zx_object_get_info` with `ZX_INFO_THREAD_STATS`, a selector; `last_scheduled_cpu` is one field | none | not applicable | read, in §204 |
| `counter_frequency_contract` (sibling) | the same shape: magic, then one little-endian `u64`, mapped read-only at a fixed address; written once at boot | a frame without the magic; a zero rate reads as `Some(0)` | not applicable | read |

Where this crate differs: the address stays fixed and the same on three architectures, so no
registration syscall exists and the first instruction of a thread can already read it. The word is
a `u64` and the sentinel is `u64::MAX`, so zero is a real answer. The magic, not the value, refuses a zeroed frame. The bound a consumer sizes arrays by is a compile-time constant
(`CPU_ID_BOUND`, 8). Two binaries must agree on it, where Linux leaves it to a run-time query. The
page also carries the process's core allowance and the scheduler tick, which `rseq` does not. `rseq` carries `node_id` and `mm_cid`, which this page does not. The sibling hard-codes its address and depends on nothing. This crate takes its address from `address_space_map`.

A concrete example of the difference that matters (item 5): an `rseq` reader that gets `cpu_id` 9
on a machine it sized for 8 has made its own mistake, because it chose the size at run time. A
reader of this page that gets 9 has done what the documentation told it to do (`[0u64;
CPU_ID_BOUND]`), and nothing in the page told it the writer's bound was different.
