# §142 amendment, 2026-10-27 (UTC): a requester may hold a job's region

The architect ruled on `design/roadmap/0933-every-job-is-built-the-way-a-supervised-job-is.md` on this date.
Retention stays a declared field on `ChildEndowment`. A requester may now hold the region the
builder builds its job from, and tell the builder to use it. Then:

- The requester supervises the job on an endpoint it holds, reaps it, and tears it down with
  `DESTROY` on the region. The builder retains nothing over such a job.
- The builder learns that the job's file-service window is free from a message the requester sends,
  not from the undertaker's.
- The requester's memory budget grows by what the builder's pool shrinks.

## The wire, as built

- `Wiring::held` is a request bit (`HELD_BIT`). Three capabilities follow the interrupt pair: the
  region (every right), the requester's endpoint (`READ`, `WRITE`, `GRANT` and `ENUMERATE`), and,
  for a program that declares a share view, the requester's budget narrowed to `ENUMERATE`.
- The builder badges the endpoint with a label it mints, places it as the job's supervision
  capability, and answers once with `(SPAWN_OK, label, 0)`. Retention is `Requester`, so the
  job's `ThreadControlBlock` follows as the one message after a good answer (§309 (a supervised job's `ThreadControlBlock` goes to the shell that asked for it)).
- A foreground job's output goes through a second copy of the labeled endpoint. A background job's
  goes to the terminal's sink (`SCREEN_BIT` with `HELD_BIT`).
- The kernel stamps the label on the sixth word of the job's death message. The requester tells a
  death from the job's own output by that word.
- The requester sends `release(label)` after its `DESTROY` has returned the region. The builder
  frees the job's window, resolver grants and installer reply, as it does for the undertaker's
  message, and answers nothing.

The builder refuses a held request whose program it takes from the image pool (an installer
holder, or a file run by its path). The pool and the image pool do not shrink until the shell
holds those jobs as well. That block states the numbers and what remains.
