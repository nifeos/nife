# How other systems admit heavy work

An appendix to [the milestone](../1044-heavy-jobs-wait-in-order-for-measured-memory.md). Each entry says
how the system solves admission, where nife differs, and whether this lane read the source on
2026-11-05 (UTC) or recalled it.

## GNU make jobserver

Read in part, and tested. Make hands out job tokens through a pipe (every version) or a named fifo
(4.4 and later, `--jobserver-style=fifo`). A client reads a token before it starts a job and writes
it back when the job ends. Every client also owns one implicit token it never reads. Cargo is a
client: under `make -j2` it ran two build scripts at once, under `make -j2` with `cargo -j8` still
two, and on a bare fifo with two tokens, three. Two cargo runs on one two-token fifo ran four.

Where nife differs. The jobserver bounds CPU and knows nothing of memory, which is what kills this
host. A client killed with `SIGKILL` keeps its tokens for ever: the probe lost both of two. nife's
`flock` liveness exists precisely so that a killed job frees what it held.

## Bazel

Read. `--local_resources` sets the resources Bazel may use, as `name=value` pairs such as
`memory=HOST_RAM*.5` or `cpu=HOST_CPUS`, and "Bazel will limit concurrently running actions based
on the available resources and the resources required." A test declares its need with a
`resources:<name>:<amount>` tag. An experimental flag,
`--experimental_memory_load_scheduling`, schedules on the machine's memory load instead of per-action
estimates.

Where nife differs. Bazel is one server process that sees every action, so its ledger lives in
memory. nife has many independent processes from many worktrees and no server, so the ledger lives
in files and the kernel's `flock` keeps it honest. Rule authors declare Bazel's estimates;
nife's come from measured peaks.

## Nix and Hydra

Read for Nix. `max-jobs` is the "maximum number of jobs that Nix will try to build locally in
parallel", a count. `cores` sets `NIX_BUILD_CORES` for each builder and enforces nothing. Nothing in
the configuration admits by memory or by load. Recalled for Hydra: each build machine has a job
count and a speed factor, and Hydra fills machines up to the count.

Where nife differs. Nix's answer is today's slot count, and it shares today's weakness: a count
treats a 100 MiB build and a 50 GiB build as equal.

## Slurm

Read for cgroups, recalled for scheduling. A job requests memory and CPUs up front. With
`ConstrainRAMSpace=yes` in `cgroup.conf`, Slurm sets the job's cgroup soft limit to its allocation
and its hard limit to the allocation times `AllowedRAMSpace` percent. Past that the kernel kills
processes in the job and Slurm marks the step OOM. The scheduler (recalled) runs jobs in priority
order and backfills a smaller job ahead only when its time limit shows that it ends before the
blocked job could start.

Where nife differs. This proposal is closest to Slurm: an up-front request, a cgroup cap at the
request times a factor, a queue. nife has no time limits, so it backfills by age (ten minutes) and
not by estimated end time. Slurm asks the user for the memory figure; nife measures it.

## systemd-oomd

Read. A user-space daemon that watches pressure stall information on cgroups whose units set
`ManagedOOMSwap=` or `ManagedOOMMemoryPressure=kill`, and kills a whole descendant cgroup before the
kernel's killer acts. The manual gives 60% as the pressure limit for `system.slice`-style units.

Where nife differs. It is a backstop, not admission, and it is inactive on this host and needs root
to turn on. The proposal's parent slice with `MemoryMax` gives heavy jobs a hard ceiling without it.

## ninja pools

Read. A pool has a fixed `depth` and an edge in the pool waits for a free place; `console` is a
predefined pool of depth 1. Ninja follows the GNU make jobserver protocol, fifo style only on POSIX,
and still enforces `-l` load limits in that mode.

Where nife differs. A pool is a slot count per kind of work, which is today's Kani pool in spirit.
It does not weigh jobs.

## GitHub Actions concurrency groups

Read. At most one running and one pending run per group; a new pending run cancels and replaces the
older pending one (`queue: max` raises pending to 100).

Where nife differs. A concurrency group serializes and discards, which suits a deployment where
only the newest run matters. One lane's lint never makes another lane's pointless, so nife must
queue, not cancel.

## What nife takes from each

From Bazel and Slurm, the up-front request in real units. From Slurm, the cgroup cap at a factor of
the request. From make, the lesson that a token must return itself when its holder dies, which
`flock` already gives. From Nix and ninja, the warning that a count is where this started.
