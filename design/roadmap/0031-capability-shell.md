---
status: BUILT
raised: 2026-07-27
built: 2026-08-17
---
# 31. A capability shell: designation is authorization

Built. Phase 3's last item is init building a `fs_subtree_caretaker` per grant. The gate is
`script/swish-check` on both ISAs: it types `caps rm rmtree/rm-solo`, `rm -v rmtree/rm-solo` and
`ls rmtree | wc` at the real prompt, so the authority is previewed, the name the command line
designated is removed, and the two names beside it inside the same capability are still there.
`rm gate.txt` beside them is the one grant shape that is still a refusal, and it is a refusal about
what a name *is* rather than about a missing feature; see "The two shapes a grant cannot take"
below, which names the design fork it waits on.

(The Gate paragraph that stood here said `NONE`, and then named the one remaining item; a BUILT
milestone gates nothing, so it is gone rather than stale.)

In brief. The command line becomes a **grant expression**: naming a resource in a command IS the capability grant (`wc report.txt` passes one readable file cap; `wc` alone can read nothing, and the refusal is "no such capability", not EPERM); untyped budgets as first-class grants; a SHILL-style manifest per program checked at spawn; a `caps` command printing a process's whole endowment. Phase 1 built, both ISAs: `grant_plan` (host-tested parse + manifest + spawn protocol), the shell over the existing surface, `--mem N` made real by the `budgeter` program, manifest refusals, `caps`/`caps <command>` introspection; one kernel fix, `Untyped::SPLIT` now grants the child `GRANT` (DECISIONS §16 amendment). Phase 2 built, both ISAs: the FS contract's `CREATE`/`TRUNCATE` (so `std::fs::write` works), and per-file grants as a **caretaker process** (`fs_file_caretaker`) that narrows a directory capability to one file in one direction, proven by a read-only and a writable attacker. Phase 3 built, both ISAs: init builds a `fs_subtree_caretaker` per directory grant, so `rm` runs at the real prompt (2026-08-17). Two scope notes retired on the way: the interactive shell used to refuse a named file because its boot wired no FS service, which milestone 50 fixed, and it then refused a directory grant because init had nothing to build a caretaker from, which this milestone's last lane fixed. One remains, and it is a fork rather than a gap: a grant on the **root** of the shell's namespace cannot be narrowed at all, because a caretaker descends into a name and the root has none. The grammar shown here is milestone 47's, which deleted the `run` verb and the `file:` designator this milestone shipped with; the mechanism did not change, only the spelling. Notes: grant-expression.md, program-manifest.md, fs-server.md

Why it matters. **no-ambient-authority made user-visible**: the inversion of Unix's model at the one interface a human touches. Milestone 23's component contract in embryo, met first at the shell

Phase 1 built (both ISAs). The command line is a grant expression: `grant_plan` (a host-tested crate)
parses it and checks it against a per-program manifest; the shell holds its own untyped budget and
delegates from it. `budgeter --mem N` splits N pages off the shell's budget and delegates the
untyped to init, which endows the child; the budgeter maps them and reports the count (15 of 16, the
rest paid for page tables), proving the grant is real, not parsed-and-ignored. Manifest mismatches
and a named file a program declares but this shell cannot back ("you hold no such capability") are
refused at the prompt; `caps` and `caps <command>` print a process's whole endowment. (The spelling
is milestone 47's: it shipped as `run --mem N budgeter` and `file:PATH`.) One kernel change: `Untyped::SPLIT` grants the
child `GRANT` so an untyped is delegable (DECISIONS §16 amendment), which the headline feature
required and no other object type lacked. Notes: grant-expression.md, program-manifest.md.

Phase 2 built (both ISAs): per-file grants. The FS service's unit of authority is a *directory*
(DECISIONS §27), and `run wc file:report.txt` says less than that, so the narrowing is a
caretaker in Mark Miller's sense: `components/src/fs_file_caretaker.rs` holds the directory
capability, opens the granted name once, and serves the same contract on its own endpoint with a
namespace of exactly one name. Any other name is `ENOENT` (in this scope there is no such name);
`CREATE` is `ENOTDIR` (a file is not a directory); a write without the direction is `EROFS`. Each
refusal is a fact about what the holder has, not a permission that could have said yes.

It is a separate process for two reasons. The FS server receives on one endpoint, so serving a
second narrower one would need a receive over a *set*, which means badging endpoint capabilities
(seL4's answer) and is a design fork, recorded rather than taken. And it makes the claim checkable:
the confined program holds an endpoint to the caretaker and nothing that names the FS server, so "it
cannot reach a second file" is a property of its cspace rather than of a branch it is trusted to
take.

Proven by an attacker, twice, and the second run is what makes the first mean anything. It
reports a bitmap of what got through. Read-only grant: every bit clear, against a neighboring file
that exists and that the caretaker could open. Read/write grant, same shape: the two write bits set
and everything else clear. A caretaker that refused every request passes the first and fails the
second. Phase 2 also landed the contract's `CREATE` and `TRUNCATE` (so `File::create` and
`std::fs::write` work rather than returning `Unsupported`), a name check that does not rely on the
absence of a path walker, and a measured stack for the FS server after a 528-byte
overflow presented as a mystery 900-second test.

This milestone's headline is about *the one interface a human touches*, and it is gated at the real
prompt on both ISAs. `script/swish-check` (`xtask/src/swish_check/`) runs `wc` on `gate.txt` at the
interactive shell, beside a pipeline into `wc`, `caps` printing the endowment, and bare `wc`
refused as the negative control. That is the claim stated as a pair rather than asserted. The
shell's `holdings()` reports a directory, the shell's boot path grants an FS service on both ISAs,
and the interactive runner carries a RedoxFS disk.

Phase 3 is built on both ISAs. Init keeps the file service for the life of the boot (two of its
sixteen cspace slots, taking a directory-granted spawn's peak to fifteen). It builds a
`fs_subtree_caretaker` per grant out of the client's own region, which is DECISIONS §92 (a caretaker
is supervised by the client it serves) read through §40 (a supervisor's death is its subtree's
death)'s mechanism. A caretaker's serve loop never returns, so one built in a region of its own
never comes home, and §16 (object revocation)'s LIFO rule then pins the region above it too.

Three behaviors keep that working. `rm` declares the sink contract and sends its end-of-stream, so
`rm -rv logs | wc` is expressible. `fs_subtree_caretaker` refuses a refused descent rather than
panicking, which matters because init is the waiter and has no second thread. And `job_undertaker`
yields and retries a refused reclaim, bounded, and still traps at the end so a real leak stays
loud. A refused reclaim is exactly what the *first* reclaim of a two-process region is by
construction: the endpoint sweep wakes the caretaker so it can be collected, and a thread that can
be scheduled is refused with §16's kill armed.

The two shapes a grant cannot take, both refused at the prompt with nothing spawned, and neither
is a permission:

- A grant on the root of the shell's namespace. A caretaker's whole attenuation is one `OPENDIR`
  *into* the granted directory, and the root has no name to descend into; `fs_proto` has no verb
  meaning "the directory I already hold, with fewer rights". So `rm gate.txt` at the top prompt is a
  refusal and `rm rmtree/rm-solo` works, and the difference is one level of path. This is a design
  fork and belongs to calef, because both answers are permanent: a narrowing verb on the contract
  (small in the server, `Rights::attenuate` with no name resolution, and an addition to something two
  programs agree on) or an interactive boot whose shell is rooted one component below the image root
  (nothing on the wire, and it changes what every other command at that prompt means).
- A directory more than one level down, or a set of more than one name. The first is a *chain* of
  caretakers, which §92 chose supervision partly to make free; the second is
  `fs_nameset_caretaker`, which takes its set in a frame rather than in argument words. Init builds
  one subtree caretaker per grant today, so `caps rm globmany/m-*.txt` still previews an authority
  that cannot yet be delivered.

Deliverable. Invert Unix's authority model at the command line. A Unix child inherits your
entire authority; a nife command line is a **grant expression**: every argument that
designates a resource passes a narrowed capability, and nothing else flows. `run wc report.txt`
grants exactly one readable file capability, because typing the name IS the grant (Miller's
principle: designation is authorization); `run wc` alone spawns a process that can read
nothing, and the failure is "you hold no such capability", legible, not EPERM. Untyped budgets
become first-class grants (`run --mem 16 prog`), the most nife-native piece of the
inversion, with no Unix analog. From SHILL, adapted: a small **manifest** per program declaring
its expected endowment (one readable file, one endpoint, N pages), checked at spawn, so a
mismatch is a refusal at the prompt rather than a mystery hang; this is milestone 23's
component contract in embryo. Introspection is a feature: a `caps` command prints a process's
complete endowment, making §14's "reading one literal tells you a process's whole authority"
interactively true.

Scoping constraint, honest. File capabilities need something to point at; phase one grants
what exists (program spawns, endpoints, frames, untyped, device caps), and per-file grants
arrive with milestone 32's FS server, whose handles must be capability-shaped from birth partly
BECAUSE this milestone will point at them.

Prior art and reuse. Designs only; nothing portable. SHILL (OSDI 2014: capability contracts for
scripts, on Capsicum) is the academic anchor; Mark Miller's object-capability line (E, CapDesk,
Polaris) supplies the organizing principle; Plash is the Linux attempt worth reading as the mistake
catalog. Feeds 23 and 22 (shrinking ambient authority, met at the human layer); sits behind 28's
terminal contract. Effort: built in four lanes. The interactive boot's gate is `script/swish-check`.

## Follow-on

- **Milestone 47.** A grant of more than one name is `components/src/fs_nameset_caretaker.rs`, from
  milestone 47 (navigation and naming)'s globbing, which takes its set in a frame rather than in
  argument words; the reasoning is design/decisions/0052-nameset-glob-grant.md.
- **Recorded.** `notes/dir-capability.md` carries it beside the feature: a grant more than one level
  down is still not delivered, because init builds one caretaker per grant and that shape is a chain
  of them, so removing a file two levels down is a refusal at the prompt.
- **Refused.** Badging endpoint capabilities so the FS server could receive over a set of endpoints
  and serve the narrowed one itself, which is seL4's answer. A separate caretaker process was taken
  instead, because it makes the claim checkable from outside: the confined program holds an endpoint
  to the caretaker and nothing that names the FS server, so "it cannot reach a second file" is a
  property of its cspace rather than of a branch it is trusted to take.
- **Recorded.** `design/decisions/0076-roadmap-status-versus-tree.md` is the standing record of a
  defect class this block met: a milestone's status is maintained by its own lane, and nothing
  maintains it when another lane finishes its work.
- **Milestone 328.** Narrowing a grant on
  the **root** of the shell's namespace, so that `rm gate.txt`
  at the top prompt stops being a refusal when `rm rmtree/rm-solo` works. Both permanent answers are
  calef's: a narrowing verb on the `fs_proto` contract, small in the server and forever on a wire
  two programs agree on, or an interactive boot rooted one component below the image root, which
  puts nothing on the wire and changes what every other command at that prompt means.

## Index row

no-ambient-authority made user-visible, at the one interface a human touches
