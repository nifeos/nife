# §142 amendment, 2026-10-27 (UTC): a job may be built from a region its requester holds

Milestone 933 (every job is built the way a supervised job is) is ruled. Retention stays a declared
field on `ChildEndowment`. A job's builder may now be told to build from a region the requester
holds, and then:

- The requester supervises the job on an endpoint it holds, reaps it, and tears it down with
  `DESTROY` on the region. The builder retains nothing over such a job.
- The builder learns that the job's file-service window is free from a message the requester sends,
  not from the undertaker's.
- The requester's memory budget grows by what the builder's pool shrinks.

The wire change and its numbers are milestone 933's to record when it is built.
