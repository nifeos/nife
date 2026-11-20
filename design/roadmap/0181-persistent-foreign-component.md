---
status: NOT-STARTED
raised: 2026-08-27
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 181. The foreign-language seam, extended to a process the shell calls more than once

Milestone 169 (`kilo`: the smallest real text editor) is built in Rust, so DECISIONS §31 (the
foreign-language seam) still has no real, load-bearing C program. calef, 2026-08-27: *"My only
reservation is we need an answer to running open source programs we don't build."*

The seam's per-call ABI (`(u8*, usize) -> u32`, DECISIONS §31 rule 2) does not need
to change. What has never been built is calling into an already-running C component's exported
function more than once, with its own state persisting between calls the ordinary way a process's
own memory persists between two function calls. That is an extension to an existing, working
mechanism, not a new design fork.

That is the sharper question, and this milestone is the answer to it, not milestone 169 (`kilo`)'s.
[DECISIONS §84](../decisions/0084-how-we-port.md) already answers a related but different question
well: how to *port* software into Rust while narrowing its authority. Nothing in this tree yet
answers the question §31 was raised for: how to run real, unmodified foreign-language code at all,
confined, without rewriting its own logic. §31's only evidence is `c_seam.c`, a 150-line throwaway
spike that calls into C exactly once and exits. If the cheapest realistic real-world program
(milestone 169 called `kilo` exactly that) does not fit the seam as built. Nothing else realistic
will either. "Rewrite it in Rust instead" quietly becomes the only path every time. That is
not an answer to the question. It is the failure mode §84 (port) already names as the last resort: a
demonstrator with no community is a demonstrator nobody continues.

## What is actually missing, checked against the code rather than assumed

`fixtures/src/c_shim.rs`'s `_start` does one thing and exits. `kernel/src/user::c_seam_tests` proves
this directly by running three separate process instances in sequence, one call each. Not one
process called three times. The C ABI itself (rule 2) was never the limiting factor. The
*driving loop* was always one-shot, because nothing has needed more than one shot yet.

What a persistent, interactive foreign component needs, concretely:

1. A shim loop, not a shim call. `_start` currently: map the grant, call the C function once,
   report the result, exit. The extension: map the grant once, then loop. Perform whatever system call
   the interactive behavior needs (for a terminal program, `OPERATION_READRAW`/`OPERATION_READLINE` per iteration,
   see milestone 169). Call into the C function with that iteration's input, and act on its return value
   (write output, or a return code meaning "the program asked to exit"). Tear down the
   process only when the C side signals it is done. The C function's own static/global state persists
   across these calls exactly the way any long-running process's memory already does.
   Nothing about
   rule 2's scalars-and-buffers shape prevents that, because rule 2 constrains one call's crossing,
   not what survives between two calls into the same live process.
2. **A real exit signal.** Today's spike reports one outcome and the shim exits unconditionally. A
   persistent component needs the C side to say "call me again" versus "I am done." That is one
   more bit in the existing `u32` return value, not a new type.
3. **Whatever the specific program's own I/O shape needs**, decided per program rather than
   speculatively here. A terminal program needs the raw-keystroke primitive milestone 169 already
   built (language-agnostic; it lives in `line_editor`'s own contract, reusable regardless of which
   language calls it). A line-oriented program needs only the `OPERATION_READLINE` contract that already
   exists.

None of this is believed to require a new system call, a new capability type, or a change to rule 2's
ABI. It is real work, sized like a milestone rather than a patch, but it is engineering rather than
an open design fork, which is why the gate is NONE rather than DECISION.

## The proof program, and why it should not be `kilo`

**Recommendation: prove the mechanism with something smaller and more isolated first.** `kilo`
bundles two separable things: raw-keystroke terminal I/O, already built, in Rust, by milestone
169, and reusable regardless of which language eventually drives it. And the seam's own
call-repeatedly extension (this milestone's actual subject). Proving them together risks
attributing a failure to the wrong half.

**`dc`, the POSIX arbitrary-precision RPN calculator, is the better first proof.** It is one of the
oldest real Unix programs (predates C itself; the original was written in B). It is still shipped by
every major Unix and BSD today. Minimal implementations exist at a few hundred to roughly 2,000
lines depending on feature completeness. Its dependency surface is close to nil: no threads, no
sockets, no dynamic linking, no subprocess, `stdin`/`stdout` only. Its shape is exactly what this
milestone needs to prove and nothing more. Read a line, evaluate against state the program keeps
itself (the numeric stack), print a result, loop until an explicit quit. That is a real, repeated,
stateful call into the same C component, using only the `OPERATION_READLINE` contract that already exists
today. No raw-keystroke dependency at all. A minimal single-file implementation
is a smaller, more isolated test than `kilo`'s roughly-1,000-line editor with its own additional
terminal dependency. Several exist under BSD or public-domain-equivalent licences. Busybox's `dc`
applet is one candidate, GPL and therefore requiring a license check against this tree's own posture
before use.

Once `dc` proves the mechanism, `kilo` becomes a second, harder proof of the same primitive
(one that also needs the raw-keystroke contract). The real antirez `kilo.c` can then be ported through
it as the direct answer to milestone 169's own original, unmet goal. At that point there is a real
choice between the Rust `kilo` already built and a real C port, informed by both actually existing.

## What this unblocks

The actual, general answer to "can nife run open source software it did not write," which is
broader than any one editor. Any real, long-running, interactively-driven C program with a modest
dependency surface (tier one or two of milestone 36 (a foreign-language component, seam first)'s roadmap, DECISIONS §31 rule 3) becomes
portable in the "adapt the system call layer, keep the program's own logic" sense §31 was built for.
Not only in the "rewrite it" sense §84 already answers well. Also unblocks a real C port of `kilo.c`
through the seam, if calef wants that as this milestone's own capstone once `dc` proves the
mechanism.

## What this does not decide

Whether the eventual `kilo.c` port replaces or coexists with the Rust `kilo` milestone 169 already
built. That is an architect's call once both exist to compare, not a decision this milestone forces
in either direction. Also does not decide license questions for whichever `dc` implementation gets
used as source material. That gets checked at that point, not assumed here.

## BUGS

Not started; nothing built yet to carry its own BUGS section.

## Index row

Milestone 169 quietly answered its own C-seam goal by building `kilo` in Rust instead. calef's
reservation named the sharper question this milestone actually answers: nife has no proven way to
run real, unmodified foreign-language code at all, only a 150-line one-shot spike (DECISIONS §31).
Extends the seam's driving loop to call an already-running C component repeatedly rather than
once. Recommends proving it with `dc` (small, real, stateful, no raw-keystroke dependency) before
attempting a real `kilo.c` port.
