# The manual: two BUGS entries about narrowing, in full

*Name: ratified 2026-10-18 (calef, "Approve all"). Refused `narrowing-limitations`. For two of the
parent's `BUGS` entries.*

An appendix to [`notes/documentation.md`](../documentation.md)'s BUGS, which keeps one line for each
under the cap of §212 (a prose budget).

- The caretaker-hop display race is a known, accepted interim, not a bug this decision missed. A
  page's last line and the shell's next `$ ` prompt can interleave under contention. Kernel
  exit-delivery (DECISIONS §26 (the fault endpoint)) tells the shell a child is dead-until-reaped.
  But `terminal_sink_caretaker`'s own trailing `CALL` to `line_editor` is a second, independent call
  with no ordering primitive against the shell's next prompt. It is a display glitch, not a
  confinement or correctness failure, because no capability changes hands and no byte reaches the
  wrong reader. Tracked at milestone 151 (notification objects), the kernel build of DECISIONS §101
  (notification objects), which lets the shell `WAIT` on "the caretaker's queue for this client has
  drained" instead of racing it. See notes/tail-output-narrowing.md.
- A screen-narrowed child is invisible to a concurrent `ps`/`pgrep` for its short life. Its
  DECISIONS §26 fault target is a fresh endpoint this shell minted, not the progenitor's `deaths`
  domain channel, because the shell needs to `RECEIVE` its own child's exit directly rather than
  race `job_undertaker` for the same message. Its memory still returns to the progenitor's job pool
  when this shell reaps it (§26: a corpse's region returns to its *builder*, not its supervisor), so
  nothing leaks. It simply does not appear in a domain survey while it runs. Given `mdr`'s render is
  a handful of milliseconds, this is unlikely to matter in practice. It is recorded here rather than
  fixed because nothing asked for it.
