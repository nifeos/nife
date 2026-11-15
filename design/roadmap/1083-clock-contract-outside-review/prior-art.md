# Prior art

An appendix to [the milestone](../1083-clock-contract-outside-review.md).

How the crate sits against what a reader already knows. The Linux vDSO and FreeBSD rows were checked against `include/vdso/helpers.h` and `sys/kern/kern_tc.c`. The other rows come from the reviewer's knowledge of these systems, not from a re-read; treat their mechanism column as unverified until a lane confirms it against the source named.

| system | how readers get wall time | what a stalled writer does to readers | how a step is bounded | what a reader coming from it will miss here |
|---|---|---|---|---|
| Linux vDSO (`vdso_data`, `vdso_read_begin`/`vdso_read_retry`) | a seqlock over a page the kernel writes; realtime is a base plus the cycle counter's delta | readers spin while the sequence is odd; the writer is the kernel, which does not die mid-update | not at this layer; `adjtimex` and the daemon decide | a kernel-owned writer; here the writer is a user process |
| FreeBSD `timehands` (`tc_windup`, `th_generation`, `binuptime`) | a ring of records, two by default and up to sixteen (`kern.timecounter.timehands_count`); the writer, under a spin mutex, zeroes the next record's generation, fills it, stores a new generation with a release and swaps the `timehands` pointer | a reader retries while its record's generation is zero or has moved, so it waits only on a writer rewriting the very record it holds | not at this layer | nothing: the page takes the same shape (Decision 1) |
| Windows `KUSER_SHARED_DATA` (`KSYSTEM_TIME`: `High1Time`, `LowPart`, `High2Time`) | three words; the writer stores `High2`, `Low`, `High1`; a reader retries while `High1 != High2` | readers never wait; a retry needs the writer to have moved | not at this layer | wait-free readers for one 64-bit value |
| Fuchsia clock objects (`zx_clock_read`, `zx_clock_update`, backstop time) | a kernel-maintained transformation from the monotonic clock, read through the vDSO | the kernel owns the record | a clock's backstop is the floor on what it may ever report; the maintainer applies its own step and slew limits | a floor as a property of the clock rather than of the policy, and a first-class "has not yet started" state |
| ntpd and chrony | the daemon sets the kernel clock | not applicable | ntpd refuses steps past a panic threshold unless started with its bootstrap option; chrony's `makestep` allows steps only in the first updates | the bootstrap-accepts-any-plausible-time rule, stated once as a constant |
| systemd-timesyncd | the daemon sets the kernel clock | not applicable | a floor at the binary's own build time, applied before the network is up | the build-era floor, which this crate also carries |
| `seqlock` and `crossbeam-utils`'s internal `SeqLock` | a seqlock over a `Copy` value inside one process | readers spin while the sequence is odd | not applicable | one process, one writer, std only |
| `embedded-hal`, `rtcc` | an RTC trait a driver implements | not applicable | not applicable | the RTC as a trait; here the RTC binding is three constants the service is told |

Where this crate differs: the three authorities are three capabilities over one page and one
endpoint, with no syscall on the read path. A zeroed frame reads as "the machine does not know". The policy is a pure function in the contract, so a well-behaved proposer can predict the answer. Forward and backward bounds differ by three orders of magnitude on purpose. The algorithm loom checks is the algorithm that ships, with both fence halves in one file. What it costs: a writer stopped while it holds the claim blocks later writers, though never a reader (Decision 1). States and opcodes are `u64` constants rather than enums, and the reply has no type.
