---
status: BUILT
raised: 2026-08-31
built: 2026-08-31
---
# 202. Every confinement test is a ritual until somebody breaks the confinement and watches it fail

This scopes `design/fatal-risks/README.md`'s risk 7. The enumeration is in
notes/confinement-claims.md: twenty-six confinement claims, where each is stated, which test checks
it, and whether that test has been shown to fail when the claim is broken. Twenty-five Kani
harnesses carry a replayable falsification in milestone 194's shape, and `script/falsifications
--sweep` turns every one of them red in about thirty seconds. The §31 worked example this block
names was run by hand, twice, and the second run is the one that counts.

Say it the narrow way. Nothing here supports "the confinement holds." What it supports is that
*these named claims are tested, and each test has been shown to fail when the claim is broken*, and
the six kernel rows still marked "no" in that table are the honest denominator.

In brief. Risk 7 is *"the confinement claim is false."* The evidence against it is a set of tests
this project wrote about attacks this project chose, and a passing confinement test is consistent
with two very different worlds.

DECISIONS §31's (the foreign-language seam) C component performs a deliberate out-of-bounds write,
and the test asserts its two witness pages are unchanged. That passes if the component reached the
page and the MMU stopped it. It also passes if the component never reached that address at all,
in which case the assertion is decorative. Nothing in this tree distinguishes those cases.

## The mechanism, which DECISIONS §134 already decided

§134 (a harness carries a machine-replayable falsification record) settled that a proof which cannot
be made to fail is not evidence, and chose a recorded, replayable diff as the way to show it can.
The same discipline applies to a confinement test with no change of shape.

Worked example on §31's own setup: grant the C component write access to `WITNESS_RO`, rebuild, run.
The test must go red. If it still passes, the test was never checking what it claims.

## The work

1. Enumerate the claims. What does this system say a confined component cannot do? Reach memory
   outside its grant; make a syscall without a capability; widen its own rights; DMA outside its
   IOMMU domain; name another process's objects. The enumeration is the first deliverable and is
   worth review on its own, because a claim nobody wrote down is a claim nobody is testing.
2. Record, per claim, the change that would break it: remove the check, widen the grant, drop the
   domain.
3. Show the corresponding test goes red under that change.
4. Store it replayably, in whatever shape milestone 194 (build §134) lands, so the two do not
   invent separate conventions for one idea.

## What this is not, and the block exists partly to say so

It cannot find the claim nobody made. Falsifying the claims we enumerated will never reach an
escape route we never thought of, and that is where real escapes live.

So this is a floor. It converts existing confinement tests from rituals into evidence, and it
says nothing about the attacks nobody imagined. A genuine adversarial pass by someone who did not
build this system is a different and better thing; it wants outside eyes. calef's position that
no third party sees nife until there is a package manager and a trivial install gates it behind
milestone 198 (a package manager, and the trivial install that makes a second customer possible).

No result from this milestone may be quoted as "the confinement holds." What it can support is
narrower and worth having: *these named claims are tested, and each test has been shown to fail when
the claim is broken.*

## What breaking them found, which is the part worth reading

§31 (the foreign-language seam)'s headline sentence is not what catches a broken confinement. The worked example above
works: map `WITNESS_RO` read/write into the C component, rebuild, run, and the test goes red. It
does not go red on the assertion anybody would name. The verdict equality, `assert_eq!(v[2],
CONFINED, ...)`, which prints all four bits including `read-only witness intact`, never runs. A
component that is not confined does not fault; a component that does not fault produces no death
report; and `run_seam` collects every report before the test inspects any of them, so the run stalls
at the collection. The witness check, which is the sentence §31 leads with, is reached only by an
escape that faults anyway.

And the first run of it failed for the wrong reason, which is this block's own second `BUGS`
entry firing on the day it was written. `run_seam`'s blocking receive had nothing to take, so the
break surfaced as a watchdog timeout at 234 seconds reading `a livelock, not a lost wakeup`: right
answer, useless diagnostic, and nothing in it says the word confinement. A bounded wait now reports
the stall where it means something, and the second run fails at report 4 of 12 with a sentence about
what a missing death report means.

A proof can be blind to the predicate it is stated in, and it happened twice.
`component_plan::a_plan_never_grants_a_right_the_declaration_did_not_ask_for` asserts equality
against `direction.rights()`, so a `rights()` that adds `GRANT` to everything satisfies it; only the
explicit `& GRANT == 0` beside it catches the defect. That is the shape milestone 194 measured in
`capability_table::derive_never_widens_rights`, one crate over, and it is the argument for keeping an
assertion that looks redundant.

One prediction was measured false and the record says so. The
`the_view_and_the_reap_have_the_same_scope` patch claimed its defect was caught by that harness
alone; it also reaches `reap_is_permitted_only_to_the_supervising_rendezvous`. Writing the
prediction down is what made it checkable.

## BUGS

- Breaking confinement deliberately means writing code that must never ship. The falsification
  diffs are, by construction, patches that disable security checks, and they live in the tree next
  to the checks they disable.
- Six kernel confinement tests are enumerated and cannot be falsified by machine, and this is
  the gap this milestone leaves behind rather than closes. `script/falsifications` walks `crates/`
  and keys on `#[kani::proof]`, so a `#[test_case]` that boots under QEMU is outside it entirely.
  The §31 patch was applied, run and reverted by hand and lives at `kernel/falsifications/`, a
  provisional path nothing sweeps. The blocker underneath is smaller and sharper than it looks:
  there is no way to run one kernel test by name. `kernel/src/testing.rs`'s runner takes no
  filter, `cargo xtask test` parses only `--arch`, `--cpu` and `--hvf`, and anything after `--`
  reaches QEMU rather than the kernel. One falsification costs a whole suite run (about four
  minutes, and 234 seconds of that was one hung test). A filter would make the kernel half of this
  table affordable and is worth a milestone of its own; it is also useful to every lane that has
  ever waited out the full suite to see one test.
- A test can go red for the wrong reason. Widening a grant may break a test through an unrelated
  assertion, which looks like success and proves nothing, so each falsification has to name which
  assertion it expects to fail. This fired twice on the day it was written, which is why it is
  worth keeping rather than a caution: once as the watchdog timeout above, and once as a claim about
  which harnesses a defect discriminates that measurement contradicted. Both are recorded in the
  patches themselves.
- It duplicates milestone 194's machinery if the two are built separately, which is why item 4
  points at whatever 194 lands rather than specifying a format here. It did not: the Kani half is
  194's convention unchanged, and the kernel half invented no second one, it recorded that 194's
  does not reach there.
- `script/falsifications --check` rejected the block its own header documents. An `unfalsified`
  state followed by prose parsed as a state token with a full stop attached, so the reference
  example in that script's own header failed its own gate. Fixed where the token is read, since
  `attested` already tolerated its trailing prose, and noted here because a convention whose
  documented example does not work teaches the wrong thing to whoever meets it first.
- The enumeration in step 1 is the hard part and is not a lane's to finish alone. What this
  system claims about confinement is spread across DECISIONS §31, §14, the DMA validator and
  `notes/untrusted-input-audit.md`. Assembling it did surface claims nobody had stated plainly:
  that a confined device's *values* are not confined but only its reach, that a `SURVEY` cursor
  counts threads the viewer cannot name, and that init's bytes are unsigned. The first two are in
  the table as claims this system deliberately does not make, so that nobody reads the DMA rows
  as covering them.
- The table inherits the blind spots of the tests it enumerates, because every row in it was
  found by reading what this project already wrote. That is the same limit as the section above and
  it is why nothing here retires risk 7.

## Follow-on

- **Milestone 210.** No kernel test can be run by name, which is the blocker under the six kernel
  rows this milestone could not falsify by machine. This block called it worth a milestone of its
  own and it was minted from this lane.
- **Milestone 212.** `script/falsifications` walked `crates/` only, so `kernel/falsifications/` was
  swept by nothing and the ratio printed as the tree's was one directory's. The walk now comes from
  `cargo metadata` and this milestone's record is reported by name.
- **Milestone 211.** The shape found twice here, a harness whose property is stated through the
  function under test so that function cannot be seen to break. 211 swept the tree for it: 146
  harnesses read, 11 measured blind, all 11 rewritten.
- **Milestone 198.** The adversarial pass by someone who did not build this system, which is the
  better thing this milestone is explicitly not. It waits on a package manager and a trivial
  install, because no third party sees nife before those exist.
- **Recorded.** `design/roadmap/0202-confinement-claims-falsified.md`: breaking confinement
  deliberately means the falsification diffs are, by construction, patches that disable security
  checks, living in the tree beside the checks they disable.
- **Recorded.** `design/roadmap/0202-confinement-claims-falsified.md`: a test can go red for the
  wrong reason, which fired twice on the day this was written, so each falsification has to name the
  assertion it expects to fail.
- **Recorded.** `design/fatal-risks/README.md` keeps risk 7 open. The enumeration inherits the blind spots
  of the tests it was read out of, and it cannot reach the claim nobody made, which is where real
  escapes live.
- **Done.** The claim §219 (how the shell names an installed program to the spawner) owed this
  milestone: an unvouched child holds no capability the caller did not delegate, beyond the clock
  and configuration pages. Built 2026-09-26 with §219's
  gate D2. `script/swish-check` runs `installed/unvouched`, a stripped `unreachable_network_witness`,
  which probes the process domain, entropy and the network and lists every slot it holds. Granting
  each of the three turned the line red, once each, by hand on aarch64
  (notes/packages/running-unvouched.md). The census counts capabilities, not mappings.
- **Milestone 673.** Done 2026-10-03. Milestone 673 (the confinement table lists the unvouched child). The claim above as row 31 of notes/confinement-claims.md's table
  (`design/roadmap/0673-the-confinement-table-lists-the-unvouched-child.md`).

## Index row

Scopes risk 7. A passing confinement test is consistent with two worlds: the component was
stopped, or it never reached the address and the assertion is decorative. Nothing distinguishes
them today. Applies §134's falsification discipline to security claims: enumerate what a confined
component cannot do, record the change that breaks each, show the test goes red. A floor,
explicitly: it cannot find the claim nobody made, and no result may be quoted as "the confinement
holds". Landed as notes/confinement-claims.md (26 claims enumerated) plus 25 replayable
falsifications, up from 6. Found that §31's headline witness assertion is not what catches a
broken confinement, and that six kernel confinement tests cannot be falsified by machine because
no kernel test can be run by name.
