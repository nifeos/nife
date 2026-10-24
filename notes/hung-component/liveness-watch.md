# The hung component: the shape of a liveness watcher

*Name: provisional. Named after the provisional program name the note proposes.*

An appendix to [`notes/hung-component.md`](../hung-component.md), whose question 1 draws the
three-party diagram: a watcher holding `ENUMERATE`, a supervisor holding `READ`, and a builder
holding the region. This records why that shape beats a watchdog that can kill, and the provisional
name.

Three properties of that arrangement are worth having on the record, because they are the reason it
is better than a watchdog that can kill:

- The most dangerous program in the system does not exist. A watchdog is the classic candidate
  for one, and this one holds `ENUMERATE` on one endpoint plus `WRITE` on one endpoint. A compromised
  one lies about liveness. It cannot end a process, cannot free memory, cannot answer a request, and
  cannot see a domain it was not handed.
- A lie is refusable. The supervisor is the one that acts, so it can apply policy the watchdog
  cannot express: restart at most N times, never restart the thing holding the only copy, escalate to
  a human. That is §40's "no reaper of last resort" holding rather than being worked around.
- It is the same shape as `caretaker` and `dwarden`, which is what makes it recognizable rather
  than novel: a program whose whole purpose is to hold less than the thing it stands in front of.

**Provisional name, and an architect's call: `liveness_watch`.** `caretaker` and `undertaker` are
spent on capability-narrowing filesystem programs; `watchdog` alone is a hardware term and would
claim a timer this system does not have; the noun is `watch` and the thing watched is liveness.
Nothing was minted in this lane and no such program was built (see below).
