---
status: BUILT
raised: 2026-08-04
built: 2026-09-05
---
# 111. A shell that can endow a child with entropy

Built 2026-09-05. Raised 2026-08-04 from `notes/entropy.md`, which called it "future
work with no design problem in it". That was right: no new mechanism, no new right, nothing on the
wire. The smallest entry in that sweep, and the block said so rather than dressing it up.

*(The gate this block carried was answered by calef on 2026-09-05: give it a lane now, against
this block's own recommendation, which was to fold it into milestone 65 or milestone 31's phase two.
It is written as prose rather than as a live gate line because nothing gates finished work.)*

**The recommendation was not wrong when it was written; its premise acquired an expiry date.** The
gate asked whether anything typed at the prompt needs entropy before either of those runs, and said
nothing does. That is still literally true: `disk_partitioner` is spawned only by the test harness
(`kernel/src/user/disk_tests.rs`), never by `init` and never as a shell command. But it is the
program a person types to partition a disk. On the same day a milestone was minted to wipe xenon's
NVMe so nife can drive it. (That is the EL0 NVMe driver §86 (whether an NVMe driver can leave the kernel) decided, whose number the integrator
mints at merge. It is deliberately not cited by number here, because a number this block guessed
would be one the roadmap gate could not resolve.) A GPT gives every partition a random globally
unique id and `crates/globally_unique_identifier_partition_table` refuses to invent one. So the
moment that disk is partitioned from a prompt this is on
the path.

The ranking argument against a lane was that this has no consumer today, and calef's answer is
sequencing. The work is an afternoon whichever way it is done, and doing it now means it lands
before 261 needs it rather than after. Folding it would have made it arrive late for the one
consumer that can be named.

The finding. Milestone 56 built the entropy service and the grant that reaches it. Nothing at
the prompt can pass that grant on. The note, under BUGS:

> **`init` does not endow the shell with entropy.** The std wiring and the milestone-56 tests do.
> Ambient entropy would be ambient authority, and the point of the grant is that a program's
> dependence on randomness is visible in what it holds. A shell that needs to hand entropy to a
> child is future work with no design problem in it.

So a program that needs randomness works when the system spawns it and cannot be run by a person.

Why the design is already settled. The manifest already expresses per-program endowments and the
shell already plans grants against what it holds (`crates/grant_plan`). Entropy is one more
endowment of the same kind: a program declares it needs the service, the shell hands over the
endpoint it holds, and a program that did not declare it gets nothing. There is no new mechanism,
no new right, and no question about what "ambient" would mean, because the answer is the same answer
the rest of the shell already gives.

What it costs. Init endows the shell, the manifest grows an entropy endowment, and the shell's
planner learns one more capability to place. The interesting part is not the wiring: it is that
`caps <program>` then prints "entropy" for a program that draws random numbers, which is the visible
form of the property milestone 56 built the service to have.

## What was built

`grant_plan::Manifest::entropy` joins `clock`, `domain` and `config` as a declared endowment no
token on the command line can designate. Init reads the declaration at spawn and places a `WRITE`
view of the entropy service's request endpoint at `grant_plan::ENTROPY_SLOT` (nine, one past
`DIAGNOSTICS_SLOT`, so the named slots stay one contiguous block). A program that did not declare it
holds an empty slot there. `caps <program>` prints the row. `components/src/uuid.rs` is the consumer.

`WRITE` alone, and the narrowing is the grant. On a rendezvous that is the right to `CALL`. A
declaring child may ask the service for bytes and may not `RECV` another client's request out from
under the service. It holds no `GRANT`, so it cannot hand a random source to anything it spawns.

Three departures from what this block predicted, each smaller than it sounds and each recorded
because a reader comparing the two paragraphs would otherwise find a discrepancy with no reason
beside it.

- **The shell does not hold an entropy endpoint.** This block's cost line said "init endows the
  shell", by analogy with the clock. The clock is the exception rather than the pattern: the shell
  holds one because `time` measures with it. Nothing the shell does as a builtin is unpredictable,
  so a shell holding randomness would hold an authority nothing uses, which is exactly the call
  `config` and `domain` already make. Init places the capability into the declaring child directly,
  which is `domain`'s shape and needs no bit on the spawn wire.
- **The consumer is `uuid`, not `disk_partitioner`.** The partitioner needs a disk capability the
  shell does not hold and cannot attenuate, so making it typeable is a different milestone. `uuid`
  is its draw with the disk taken away: the same sixteen bytes from the same service through the
  same `gpt::guid::Guid::v4_from_random`, which is the half a prompt can reach today. The name is
  **provisional**.
- Init keeps a capability it used to release. The entropy service's request endpoint was
  `cap_delete`d once `credentialer` held its own copy; it is now held for the life of the boot,
  because init is the only process that can make this grant. That is one slot on init's peak, and
  it moved `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` from 21 to 22 of 24.

## What was demonstrated

The endowment, at the real prompt, through the real init. `script/swish-check` is the only gate that
runs `crates/system_initializer` at all. It is the only thing in this tree that can put a
capability at a slot a manifest names (the kernel's own `Spawn` fills a capability table from slot 0
upward). On aarch64 and riscv64:

```text
$ uuid > id.txt
$ wc < id.txt
  1 1 37
$ uuid 2> ent.txt
  AB7AEBA1-5C53-4E79-8BAE-183013D1DBF6
$ wc < ent.txt
  0 0 0
$ caps uuid
  uuid would grant the new process, and nothing else:
    cap 0  endpoint  result   report its answer back
    cap 9  endpoint  entropy  WRITE. it may ask the entropy service for random
                              bytes, and nothing else: it cannot reach the device,
                              cannot receive another client's request, and cannot
                              hand a random source to anything it spawns
                              a program without this row draws no randomness at all
```

No byte of the identifier is asserted, and that is the point: a version-4 UUID a gate could predict
would be one drawn from nothing. What is asserted is the framing (36 characters and a newline) and
the **silence of the second stream**, which this program breaks in exactly one case.

The refusal, deterministically, on all three architectures.
`kernel::user::uuid_tests::a_process_granted_no_entropy_prints_no_identifier` spawns the real `uuid`
binary holding nothing but somewhere to print, so `ENTROPY_SLOT` is empty. It asserts silence: the
whole stream is the one refusal sentence, not one hyphen of identifier-shaped output reaches the
sink, and the process ends normally rather than faulting on a `CALL` that cannot succeed.

That direction is the one carrying the claim, and randomness is the authority where it is hardest to
check by looking. A process that draws a key and a process that hardcodes one make the same system calls
and produce output of the same shape. Only taking the capability away tells them apart, which is why
`crates/globally_unique_identifier_partition_table` refuses to invent a GUID and `disk_partitioner` reports `R_NO_ENTROPY` instead of
falling back to a counter.

`crates/swish` also gates the visibility half on the host: `caps date` and `caps wc` carry no entropy
row, and exactly one program in the whole manifest table declares the field.

## BUGS

- The endowed direction has no kernel-harness proof, on any ISA. `Spawn::grants` fills a child's
  capability table from slot 0 upward and cannot place a capability at the slot a manifest names, so
  nothing under `script/test` can spawn a `uuid` that actually holds entropy. It is proven only by
  `script/swish-check`, which does run in `script/gates` and in CI's `script/ci-build` since
  milestone 230, but which is one gate rather than the suite. This is the same gap `xtask`'s own
  swish-check list already records for `date`'s declared second stream; see the Follow-on.
- Two slots of headroom left in init. The measured peak is 22 of 24 and this milestone spent one
  of the three milestone 230 left. The next thing that holds a capability across init's login block
  should expect to spend another and should read `kernel::cap::CAPABILITY_TABLE_SLOTS`'s own
  arithmetic before assuming there is a third. The ceiling was deliberately not raised.
- One identifier per invocation, version 4 only. `uuidgen -n 10` has no spelling here because
  `ArgSpec` carries no positional argument yet, which is the same gap `date`'s and `printenv`'s
  module docs name. No v7, so nothing sortable; nothing wants one.
- "Unpredictable" is still a claim about the boot. The bytes are whatever the entropy service
  delivers, which on QEMU is a virtio-rng backed by the host (DECISIONS §120's stopgap). Endowing a
  grant does not change what is behind it, and `notes/entropy.md`'s health-test, rate-limit and
  hardware-TRNG entries are all still open. In particular "nife has a cryptographic random source"
  remains a claim about QEMU until the JH7110's TRNG is verified on radon.
- No rate limit reaches the prompt either. A `uuid` a person types can `CALL` the service as
  fast as it likes, the same as any other client. `notes/entropy.md` already records that the
  service has no quota. Putting a client at the prompt makes it easier to reach rather than
  worse in kind.
- `design/roadmap/README.md` and `231`'s own block still quote 21 of 24. They are accurate as
  history (that is what 231 measured on the day) and a lane does not edit another milestone's
  block; `notes/frames.md` and `kernel/src/cap.rs` carry the current number.

## Follow-on

- **Milestone 390.** Give the kernel's `Spawn` a `placed` list the way
  `supervision_protocol::ChildEndowment` already has one, so the guest suite can spawn a program
  holding a capability at the slot its manifest names. It would close this block's first `BUGS`
  entry, `date`'s second-stream gap, and every future named slot's in one change. Numbered on
  2026-09-19 by milestone 433's drain of the pile; the crate was `supervision_proto` when this
  bullet was written and is `supervision_protocol` since milestone 265.
- **Recorded.** `disk_partitioner` still cannot be typed at a prompt, and this milestone does not
  claim it can. The EL0 NVMe driver whose
  milestone calef minted on 2026-09-05 (the block above names it without a number, because the
  number was the integrator's to mint) is the sequencing reason this was built now. The partitioner
  at the prompt is that work's business. The entropy half of its endowment is built and
  waiting, and what is missing is a disk capability the shell can attenuate. 
- **Recorded.** The shell holding no entropy of its own is a decision, not an omission; the reason
  is in `Channels::entropy`'s own doc where a reader meets the wiring.
- **Recorded.** `uuid` is a provisional name, flagged in `components/src/uuid.rs`'s provenance block with
  the two refusals (`uuidgen`, `guid`) and their reasons. `Manifest::entropy` and `ENTROPY_SLOT`
  are provisional in the same sense. `script/names --unratified` is the worklist.
- **Done.** Milestone 65 (a secrets service) and milestone 31 phase two are unaffected by this
  landing first, which is what the scope note worried about. Both wanted the same planner change
  with a different capability in it. It is now a field on `Manifest` and three lines in init
  rather than a mechanism either has to build.

## Scope note

*(Written before the gate was answered, and kept because it is the argument calef overruled.)*

**This may not deserve its own lane, and the honest options are two.**

- Fold into milestone 65 (a secrets service). 65 holds keys and exposes operations, so it will
  need the shell to endow a child with a *service* endpoint under exactly this pattern. If 65 is
  scheduled, this is one endowment inside it and should not be a separate lane.
- Fold into milestone 31 (a capability shell), phase two. 31 owns per-file grants pointing at FS
  server directory capabilities, which is the same planner change with a different capability in it.

What would decide it: whether anything at the prompt needs entropy before 65 or 31 runs. Today
nothing does, which is why this has sat unbuilt without hurting. The moment a typed command needs
randomness (a `uuid`, a key generator, anything in 65's family), it stops being foldable and becomes
a prerequisite. Until then, folding is the better answer and this block exists so the work is not
lost in a BUGS list while the fold is being decided.

**Not a health test, a rate limit, or a hardware TRNG.** `notes/entropy.md` records all three as open
and they are the service's business, not the shell's. In particular "nife has a cryptographic
random source" is still a claim about QEMU until the JH7110's TRNG is verified on the VisionFive 2,
and endowing a shell with a grant does not change what is behind it.

## Index row

`Manifest::entropy` joins `clock`, `domain` and `config`: init places a `WRITE` view of the
entropy service at `ENTROPY_SLOT` for a child that declares it, a child that does not holds an
empty slot, and `caps uuid` prints the row. `uuid` is the first program a person can type that
needs randomness
