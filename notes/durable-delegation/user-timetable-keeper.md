# The user timetable keeper, step by step

An appendix to [notes/durable-delegation.md](../durable-delegation.md). The doc comments in
`components/src/user_timetable_keeper.rs` state its capabilities and its BUGS. This page holds what
the program does, in order, from the moment `login` starts it.

## What it does

1. It builds `timetable` from the image `login` copied in, out of its budget slot. It hands the
   timetable no programs, because a scheduled job runs what the live activation generation names.
   The timetable gets the two read-only store caretakers (`activation/` and `packages/`) and the
   page they stage through, which makes it a store-mode timetable (`timetable::contract`). The
   registration page is mapped at the keeper's `TIMETABLE_PAGE_VA`, so the timetable starts empty
   and silent and waits for a `REPLACE`.
2. It says it is ready on its readiness slot, once. `login` is blocked waiting for exactly that
   word.
3. It blocks on the timetable's supervision endpoint for the rest of its life. Only the timetable's
   death arrives there, because a scheduled job holds no report endpoint.
4. When the timetable is gone, it gives its budget's contents back and exits. The registration page
   stays, in `login`'s region, so `login` can read why the timetable stopped.
