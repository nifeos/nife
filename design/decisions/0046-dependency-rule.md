---
status: AMENDED
raised: 2026-07-30
decided: 2026-07-30
ratified_by: calef
---

# 46. Thin primitives or whole subsystems; we write everything in between

(three amendments below: 2026-07-31 distinguishes taking from vendoring, 2026-10-04 makes taking
the default outside the kernel and the crates Kani proves, and 2026-10-25 gives the kernel a bar in
place of a blanket. Rule 1 carries a 2026-10-26 ruling that takes register access off the crates.)

Decided 2026-07-30 (calef), after the calendar crate made the absence of a rule visible. The
practice was already unanimous and written down nowhere, which is the state that produces an
inconsistent decision the first time someone does not share the instinct.

## What the tree actually does

Four external dependencies exist outside `vendor/`:

| Crate | Where | Kind |
|---|---|---|
| `tock-registers` | components | thin: an MMIO register abstraction |
| `spin` | kernel | thin: spinlocks |
| `smoltcp` | user | whole subsystem: a TCP/IP stack |
| `redox_syscall` | fs_server | forced by the vendored engine |

Plus RedoxFS, vendored under `vendor/` with a pin and a divergence patch (§34). Against that,
thirty crates have no external dependencies at all.

The shape is sharp and unaccidental: thin architectural primitives, or entire subsystems we would
never write. Nothing in between. Everything in between is written here.

## The test, in order

1. Is it on the verification path? Then write it. This is the load-bearing reason and it is not
   about pride: you cannot restructure someone else's crate to make a model checker tractable.
   `crates/calendar` is the worked example. The parser gained a byte-level entry point because
   `from_utf8` made CBMC branch on length every step (ten minutes to seventeen seconds, and a better
   API). Monotonicity was rephrased as its induction step over adjacent days (228s to 40s, same
   theorem). `div_euclid` on a symbolic timestamp had to be kept out of the harness entirely. Three
   other lanes hit the same wall the same day (`ntp_proto`'s fixed-point multiply, `gpt`'s
   table-driven CRC) and all three resolved it by changing code. None of that is available in a
   dependency. Register access is the second case. Ruled 2026-10-26 (UTC): the kernel spells
   aarch64's system and MMIO registers in the tree and links no register crate. A proof stubs an
   accessor in `arch/aarch64/instructions.rs`, and it cannot stub another crate's trait method. That replaces §3 (use the crate ecosystem) for register access.
2. Is it a whole subsystem we would never write? Then take it, vendored, under §34's conditions.
   RedoxFS and smoltcp are the model: a filesystem and a TCP/IP stack are each larger than the thesis
   they would serve, and confining them is the thesis.
3. Does it touch the ABI or a capability? Then write it: nothing outside nife implements either.
   Does the kernel link it? Then write it, unless a candidate clears the kernel's bar (below) and an
   architect rules to take it. Until 2026-10-25 (UTC) this clause said "write it" for anything the
   kernel touched; "The kernel's bar, 2026-10-25" says why it changed.
4. Otherwise, prefer writing when the specification is complete and checkable, and prefer
   depending when correctness is won by *exposure* rather than by reading the spec.

## Rule 4 is what stops this becoming "always write", and crypto is the case

The Gregorian calendar is fully specified: every rule is written down, and a proof over all
3,652,425 days in range settles it. Nothing about being widely used tells you more than the quantifier
does.

Cryptography is the opposite: take it, do not write it. Correctness there includes resistance to attacks not yet published and side-channel
behavior no specification states, and that is bought by years of exposure and review. A proof that
our AES matches the spec would not make it safe to use.

So the distinguishing question is not size. It is whether the spec is the whole of correctness.

## Amendment (2026-07-31): taking is not vendoring, and crypto is a *dependency*

An earlier wording of rule 4 said crypto should be vendored, which conflated two decisions. Rule 4
is about write versus take. Whether a taken thing is *vendored* or *depended on* is separate, and
does not follow from it.

The tree's actual trigger for vendoring is "we must patch it." RedoxFS is vendored because it
needed a divergence patch to build `no_std`, and `script/vendor-verify` exists to prove exactly that:
"upstream plus our recorded patches". smoltcp is a whole subsystem and an ordinary
dependency, because nothing needed changing. So "subsystem, therefore vendor" is not what this tree
does, and never was.

RustCrypto's crates are already `no_std`, so no patch is needed and the trigger never fires. With no
divergence, `vendor-verify` has nothing to prove that `Cargo.lock` does not already.

And for crypto in particular, vendoring is actively worse. Advisories are the whole point in that
category, and `cargo-deny` / `cargo-audit` work against registry versions; a vendored copy is
invisible to an advisory until a human notices. Milestone 42 named that gap in general terms: "we
confine code we did not write; an advisory against it is invisible today", and crypto is where it
bites hardest. Vendoring it would take on the maintenance burden and give up the pipeline that
makes the burden survivable.

So: crypto is an ordinary dependency, pinned in `Cargo.lock`, gated by `deny.toml` and
`script/supply-chain`. Vendor only what must be patched.

## The honest costs of writing, recorded so they are not rediscovered as complaints

- `crates/calendar` is 1,538 lines and adds ~7 minutes to `script/verify`, where it is now the
  largest single entry in a 95-harness suite. If that ratio repeats the gate becomes something people
  skip, and a skipped gate is worse than none, which is the same lesson `script/fmt` taught on
  2026-07-30 from the other direction.
- Battle-tested libraries have already found the subtle bugs. Our harnesses were validated by
  falsification (reducing the leap rule to `% 4 == 0` fails in 8s): that proves they would catch an
  error, not that they caught one we had written. The gain is a quantifier and a better API, not
  bugs found.

## The process failure that produced this entry

There was no decision to write `crates/calendar` rather than depend on `time` or `chrono`, both of
which support `no_std` and would have covered most of it. The lane brief said "build the crate", and
the choice was made by omission, in a prompt, rather than on the record. The outcome was
consistent with what this tree does everywhere; it was consistent with nothing anyone could point at.
That is the gap this entry closes.

## What is already in place and does not change

`deny.toml`, `script/supply-chain` and `script/vendor-verify` from milestone 44, and `vendor/`'s pin
plus divergence-patch discipline. A dependency taken under rule 2 goes through all of them.

## The amendment, 2026-10-04

Ruled 2026-10-04 (UTC) by calef, who answered "Yes, amend §46" to the recommendation below. His
reason, in his words: *"We seem to be building everything. To be successful, it seems like this
project needs to reuse software that already exists."*

### What changed, and what did not

Rule 1 still stands, and so does the reason behind it: you cannot restructure someone else's crate to
make a model checker tractable. What changed is the economics around it. Writing is now cheap for
agents, so "we can write it" no longer argues for anything. What is expensive is trust, and code
written here has had no exposure outside this tree. Fatal risks 3 (the tests do not test anything)
and 7 (the confinement claim is false) both come down to the same gap: everything that vouches for
our code was written by the people who wrote the code. Third-party code that many systems have run
carries evidence of a kind we cannot produce for ourselves. Rule 4 already said this about crypto.
This amendment applies it more widely.

Writing stays the default for:

- anything that touches the ABI or a capability (rule 3);
- the kernel (`kernel/`) and what it links, unless a candidate clears the kernel's bar below;
- a crate whose Kani harnesses are listed in `script/verify` because of a claim the system rests on.
  Adding harnesses to code that could have been taken does not put it on the verification path
  after the fact. The block has to argue the claim.

Taking or adapting comes first for everything else: drivers, protocol parsers, file formats,
userland programs and tools. A milestone in these layers names what it considered before writing.
Writing our own needs a recorded reason, and the burden of proof is on the reason.

### What a recorded reason to write looks like

It goes in the milestone block's `Reuse:` line (or `## Reuse` section), which `script/roadmap
--check` requires of every block raised on or after 2026-10-05 (UTC). The line names at least one
candidate, or says `none exists` and what was searched. Reasons that hold:

- It is on the verification path, in the sense above.
- No candidate builds `no_std`, and making one do so is a fork we would carry forever.
- The license is incompatible. GPL code in the shipping graph is the question of §135
  (running GPL software is aggregation), and §135 answers it with aggregation across a capability
  boundary, not linking.
- Adapting would cost more than writing, with numbers: lines to change against lines to write,
  or a measured performance gap. An adjective is not a number.

"Ours would be cleaner" is not on the list. If the honest reason is effort, the block says so in
those words (CLAUDE.md, *Elegance and performance beat implementation convenience*).

### Who rules

The default changes. The authority does not. Taking a dependency is still a decision (CLAUDE.md's
codebase rule 6): a lane proposes a dependency in its block and pull request, the pull request
carries `needs-architect`, and an architect rules. The conditions of §34 (RedoxFS is the primary
filesystem, on three conditions) and the 2026-07-31 amendment (vendor only what must be patched)
apply unchanged to anything taken. `deny.toml` and `script/supply-chain` gate it like every other
dependency.

Redox is a source we consume and never contribute to. Its drivers are MIT-licensed, and its
CONTRIBUTING refuses LLM-generated contributions. So we take its code under the license and send
nothing back. A patch we need is carried here (§34, `script/vendor-verify`), never offered upstream.

*2026-10-07 (UTC): calef ruled on #1815 that bug reports to Redox are allowed (disclosed, each approved, security findings privately first); code and patches still go nowhere. See §176 (offering the RedoxFS patches upstream).*

### The cases that prompted it

- DNS: milestone 384 (in a capability system the resolver is a grant), pull request #1634. It wrote
  `crates/domain_name_system` (name provisional) with three Kani harnesses and a fuzz target. On
  crates.io, `hickory-proto` (MIT or Apache-2.0), `simple-dns` (MIT) and NLnet Labs' `domain`
  (BSD-3-Clause) all parse DNS replies; whether each builds `no_std` was not checked when this was
  written. A parser that takes untrusted input from the network can be a verification-path case.
  Under this amendment that has to be argued in the block, not assumed. The lane is adding a
  `## Reuse` section voluntarily, since 384 was raised before the cutoff.
- e1000e: milestone 494 (a driver for the network card a PC actually has). Redirected on
  2026-10-04 to assess Redox's MIT-licensed `e1000d` before writing anything. This is the first lane
  to work under the new default.
- USB xHCI: milestone 242 (USB host and HID), pull request #1629. It was built before this
  amendment. rust-osdev's `xhci` crate (0.9.2, MIT or Apache-2.0, register and ring definitions) and
  Redox's `xhcid` could have covered part of it. That is recorded as a follow-up survey
  (`design/roadmap/0785-survey-what-milestone-242-could-have-taken.md`), not a rewrite: the
  survey says what taking would have bought, and a rewrite waits on its numbers.
- Userland: milestone 756 (procps, coreutils and util-linux leave first). Ports are this
  amendment applied to programs. A port is the default, and a rewrite needs a reason.

### Why the gate dates from 2026-10-05

A check must not fail the tree it lands in, so blocks raised before 2026-10-05 (UTC) are exempt.
That exempts 242, 384 and 494. It is an exception, and it is marked in the check's comment: an
older block that is reopened does not owe a `Reuse:` line, and the check cannot tell.

## Amended 2026-10-06: milestone 756's packages are renamed

The userland bullet above cites milestone 756 by its title of 2026-10-04. calef renamed its three
packages on 2026-10-06 (UTC), and it is now milestone 756 (`process-tools`, `core-tools` and
`disk-tools` are the first code to leave this repository). The rule this section applies to them is
unchanged. The reasons for the names are in
[notes/package-boundaries.md](../../notes/package-boundaries.md).

## The kernel's bar, 2026-10-25

Ruled 2026-10-25 (UTC). It replaces rule 3's blanket for the kernel with a bar that a candidate can
pass or fail. The question that raised it was why the tree writes `device_tree_blob` when a crate
does the job, and the only answer was rule 3.

### Why the kernel clause changed

Rule 3 never said why the kernel writes its own. Its ABI and capability clauses need no reason,
since nothing outside nife implements either. The kernel clause is different.

- It overlaps rule 1 wherever rule 1 is strong. `paging` and `capability` are ours because claims
  rest on them, and rule 1 already says so.
- Where it adds anything, it covers kernel code off the verification path. That is mostly the
  parsers of what firmware and boot media hand the kernel: the device tree, ACPI tables, ELF images.
- Those parsers are where the 2026-10-04 reason is strongest. Exposure buys trust, and the
  bugs that hurt a parser are the inputs nobody thought of.
- The kernel already links crates a blanket "write it" cannot explain: `spin`, and on x86_64
  `uuid`, which arrives through `boot_slot` and `globally_unique_identifier_partition_table`.
- "Touches the kernel" was never defined. Milestone 784 read it as anything the kernel uses for
  more than shared constants, which is wider than the rule's words.

The concern the clause was reaching for is real. Every crate the kernel links runs with full
privilege and joins its trusted code. So the bar below is higher than the userland default, and
every take is still an architect's ruling.

### The bar

The kernel may take a crate off the verification path when the crate meets all of these. The block
or pull request that proposes it shows each one.

1. It builds `no_std` on all three kernel targets, and it needs no allocator or uses the kernel's.
2. If it parses input, it survives ours: the tree's fixtures and fuzz corpus for that input, with no
   panic, hang or out-of-range read.
3. Its dependency graph is small: it adds at most three new crates compiled into the kernel, its
   own crate included, as `cargo tree -e normal` counts them for each kernel target. Build-time
   crates (procedural macros, build scripts and what they pull in) have no cap, and the proposal
   lists each one. `Cargo.lock` pins every crate in the graph, build-time ones included, and
   `deny.toml` and `script/supply-chain` gate each one like any other dependency.
4. Its proposal lists the crate's `unsafe`: the count and location of its hand-written blocks, and
   the unsafe code its macros generate, which counts as the crate's own. For each, it names what
   checks it, upstream (Kani, Miri, fuzzing, a published soundness policy) or here. No cap applies.
   Any `unsafe` that a claim in nife rests on, we prove ourselves: either we can write the Kani
   harnesses its claims need against its public API, or no claim rests on it. If a claim rests on
   it and its structure defeats Kani, rule 1 decides and we write.
5. It stays upgradable, under §81 (a dependency stays upgradable).

The two crates the kernel links meet conditions 1, 2, 3 and 5 on the record, and neither adds more
than three crates on any target. `spin` is a thin architectural primitive with no parser. `uuid`
builds `no_std` with default features off and depends on nothing.
The kernel only builds identifiers from bytes the partition table crate has already reordered, and
calls its text parser nowhere outside tests. Neither has the listing of `unsafe` that
condition 4 asks for, since they predate it; a proposal that touches one of them writes it. Nothing
here changes the ABI, capabilities or the verification path.

The first case under the bar is `device_tree_blob` against Google's `dtoolkit`, measured in
milestone 784 (reuse what the tree wrote).

