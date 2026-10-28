---
status: DECIDED
raised: 2026-09-05
decided: 2026-10-28
ratified_by: calef
---

# 145. Compartmentalization at process cost: is Qubes' mission the reason the world needs this OS?

Ruled 2026-10-28 (UTC): A, Qubes as a benchmark, not a product target: a claim-for-claim comparison
beside Qubes' published architecture and security-critical list, plus real disposable-domain cost
measured properly rather than stood in for by a spawn benchmark. D, becoming a Qubes competitor, is
refused in writing; C, the bounded customer, is held for its own section once this comparison has
produced numbers.

Raised by calef, 2026-09-05, in one sentence: *"Qubes seems useful as an OS.
It seems like we could do their mission well."* *(Section number provisional until the merge queue
lands it.)*

What is being decided is not whether to build a Qubes competitor. It is whether
compartmentalization at process cost is the answer to a question this tree has had open and
unanswered since it was written down.

`design/competitor-question.md` parks all competitor-shaped work behind two triggers. The first
fired on 2026-08-26 (a verified core running a real workload, milestone 19). The second is still
open, and it reads:

> a reason the world needs another OS that the demonstrator has by then proved

Nothing has ever been proposed as a candidate answer to that sentence. This is the first one.

## What Qubes actually claims, read rather than recalled

Read 2026-09-05 at the URLs given, because this tree carried a fabricated block quote for twelve
days through every gate and a claim from memory is a claim to mark as such.

From `doc.qubes-os.org/en/latest/developer/system/architecture.html`:

> Qubes implements a security-by-compartmentalization approach.

> No networking code in the privileged domain (dom0)

From `doc.qubes-os.org/en/latest/developer/system/security-critical-code.html`, and this is the
page that matters:

> The size of the current TCB is on the order of hundreds of thousands of lines of C code, which is
> several orders of magnitude less than other OSes.

> A successful attack against any of these components could compromise the system's security.

The components it then lists include the Xen hypervisor, Xen's xenstore backend in dom0, Xen's
block backend in dom0's kernel, the dom0 side of the GUI, sound and `qrexec` code, and the VM memory
manager.

Read that as the compliment it is. Qubes chose TCB size as its own headline metric, states it
plainly, enumerates what is in it, and says what a break costs. That is the same posture this tree
takes, and it is why the comparison is worth making at all rather than being a cheap shot.

## The argument, and it is one sentence

Qubes' architecture is a workaround for the operating system underneath it. It runs a hypervisor
and a virtual machine per activity because Linux processes are not a security boundary. The
disposable VMs, the driver domains, the per-domain window borders are all downstream of that single
concession.

**A capability system does not need the concession.** Isolation at process granularity is what
capabilities are. So the claim available to this project is not "we could build Qubes"; it is that
Qubes buys with virtual machines what a capability kernel gives at process cost, which is a
[§14](0014-project-direction.md) demonstrator claim rather than a product claim.

## Two places it is measurable rather than arguable

TCB, on their own chosen metric. `kernel/src` is 36,494 non-comment, non-blank lines of
Rust (measured 2026-09-05; 71,036 lines with comments, which is why the code figure is the one to
quote), of which `kernel/src/arch/` is 7,607. Against "hundreds of thousands of lines of C". With
151 Kani proof harnesses, and since [milestone 193](../roadmap/0193-kernel-kani-reachable.md) the
prover reaches `kernel/src` itself rather than only the pure crates.

**The caveat is mandatory and it is not small.** Qubes' TCB is doing work ours does not: a GUI
virtualization stack, sound, an RPC broker, a memory manager, and a package-signature path. A
smaller number for a system that does less is not a win, it is arithmetic. The honest comparison
is TCB per capability delivered, and constructing it fairly is most of the work in option B below.
The tree's standard for this is milestone 25's: state what each number means and where it is not
apples-to-apples.

Cost per domain, which is what actually bounds how Qubes gets used. People run five to ten
qubes rather than five hundred because each is hundreds of megabytes and seconds to start. radon
measured `spawn_el0` at 66 microseconds on 2026-09-04, on a 1.5 GHz in-order U74. The caveat is
the same one [§14](0014-project-direction.md)'s spawn comparison already carries, in stronger form: a
qube is an entire Linux virtual machine and an EL0 process is not.

**The point survives the caveat, and that is why it is interesting.** If a confined domain costs
what a process costs, the model changes in kind rather than in degree: one per download, one per
document, one per tab. Qubes' disposable VM is the feature its users praise most and ration most.

## What would be a mistake, stated first because it is the likelier outcome

**Qubes' value is the software it isolates, not the isolation.** People run it to use Firefox,
Thunderbird, LibreOffice and Windows VMs. Risk 1 went green on 2026-08-31 in a narrow and honest
sense, an unmodified `ripgrep` reaching its own argument parsing, and a browser is not in that
universe. **Qubes without applications is not Qubes.**

And building toward one would be the milestone 55 mistake in a larger costume. AGENTS.md's own
correction, written after the family backup server: *"A first customer should be something nife can
plausibly be adequate at within a milestone or two."* A Qubes competitor needs a browser, USB
passthrough, suspend and resume, a display stack good enough to live in, and a hardware
compatibility story. That is larger than the workload this project has already failed to reach once.

It also collides with [§131](0131-hold-at-rung-two.md), decided 2026-08-26: hold at rung two and
prove text-mode usefulness first. Qubes is a graphical product, and its per-domain window borders
are not decoration, they are the interface. Any reading of this section that ends in a desktop is
reopening §131 without saying so.

## Where the tree already leans this way

`design/driver-domains.md` cites the model by name, in its option 3:

> This is the Xen "driver domain" / stub domain model, and the QubesOS "sys-net / sys-usb" model:
> the most dangerous, most bug-prone code (drivers) runs in disposable, DMA-confined boxes.

That option is parked as "most isolation, most infrastructure". Two things have changed since. Drivers moved into EL0 processes rather than virtual machines,
which is the cheaper half of the same idea and is built; [§86](0086-el0-nvme-driver.md) is the worked
case, decided 2026-09-03. And milestone 159 ran a confined EL0 driver against real silicon on
2026-09-04, which is `sys-net`'s shape with no hypervisor under it.

[Milestone 202](../roadmap/0202-confinement-claims-falsified.md) enumerated 26 confinement claims, each
with where it is stated, which test checks it, and whether that test has been shown to fail when the
claim is broken, with 25 carrying a replayable falsification. **Qubes does not publish its claims in
that form.** That asymmetry is the most defensible thing this project owns in this comparison and it
already exists.

## The options

A. Refuse it, and record the refusal. Compartmentalization is not the answer to
`competitor-question.md`'s open half, and the question stays open. Cost: nothing. Risk: the second
trigger has now gone unanswered for long enough that nobody is looking for candidates, and a parked
question with no candidates is a question that has quietly become a no.

**B. Take Qubes as a benchmark, not a product target.** Two deliverables, neither of which requires
a desktop, a browser, or reopening §131:

- A claim-for-claim comparison. Milestone 202's 26 claims beside Qubes' published architecture
  and security-critical list: what each system claims, what enforces it, what is in each TCB, and
  what a break costs. This is milestone 25's shape applied to security instead of performance, and
  it needs no new kernel code.
- The disposable-domain cost, measured properly. The argument above rests on a spawn benchmark
  standing in for a confined domain, which is not the same object. Measuring the real thing means
  defining what a nife equivalent of a disposable VM is, which is itself the useful part.

C. Take the bounded customer that falls out of it. Qubes' most valuable *bounded* feature is the
disposable VM that sanitizes untrusted input: hand it bytes, it processes them somewhere that can
reach nothing, it returns output. That is a service rather than a desktop, it is plausibly
something nife could be adequate at within a milestone or two, and AGENTS.md's ranking function has
been vacant since 2026-08-30. It is also risk 7's adversarial half wearing a shape somebody would
use.

D. Become a Qubes competitor. Named so that it is refused explicitly rather than by omission.
It reopens §131, contradicts AGENTS.md's first-customer correction, and needs an application
ecosystem this project does not have.

## Recommendation

B now, C as the thing to decide next, A's refusal written into D.

B is cheap, is entirely on-thesis, and produces exactly the artifact §14 says this project exists to
produce: a measured comparison against a system built for the property we claim. It also fails
usefully. If the claim-for-claim comparison shows Qubes enforcing things we cannot, that is worth
more than the flattering version.

C is the one that could answer the open question, and it is deliberately not recommended yet,
because it is a customer-path decision and this tree has made one of those too fast before. It wants
its own section once B has produced numbers.

D should be refused in writing today, not because it is a bad idea in the abstract but because
leaving it unrefused is how a demonstrator slides into a second unfinished Linux. That is the
failure `design/competitor-question.md` was written to prevent.

## What is blocked until this is answered

Nothing is blocked, and that should be said plainly rather than manufacturing urgency. No lane
is waiting, no milestone is gated on it, and the roadmap is unaffected either way. What is at stake
is whether `competitor-question.md`'s second trigger acquires its first candidate answer or stays
open with none.

## BUGS

- The TCB comparison is not yet fair and this section says so twice on purpose. 36,494 against
  "hundreds of thousands" is two systems doing different amounts of work. Anyone quoting the raw
  ratio before option B constructs the per-capability version is doing the thing this tree refuses
  to do to other projects.
- The 66 microsecond figure is a spawn benchmark, not a disposable domain. It is the closest
  measurement that exists and it is not the measurement the argument needs.
- Nobody here has run Qubes. Every claim about it in this section comes from its own
  documentation, read on one day, and documentation is what a project says about itself. The parts
  most likely to be wrong are the ones about how it feels to use, which is exactly where a claim
  about what its users ration would live.
- This section argues from Qubes' strongest published position, its own TCB page. A comparison
  drawn against a project's own best statement of itself is the honest kind and is also the kind
  most likely to be missing the parts it does not advertise.
