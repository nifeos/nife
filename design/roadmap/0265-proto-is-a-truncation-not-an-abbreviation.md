---
status: BUILT
raised: 2026-09-05
built: 2026-09-14
---
# 265. `_proto` is a truncation, and it collides with the other word it could be short for

calef, 2026-09-05, on being shown `timebase_proto` for ratification: *"I think `_proto` was lazy
on my part. It should have been `_protocol` globally to differentiate from prototype."*

## The measurement

Measured on 2026-09-14 against `origin/main` at `2487a1f7`: 15 crates carrying `_proto`, one more
crate and one program carrying a stem that had to travel with them, referenced across 424 files.
The measurement is
`git grep -lE '\b(<the seventeen names>)\b'`, which is also the enumeration the rename was performed
from.

```
byte_sink_proto   capability_demo_proto  clock_proto      credential_proto  entropy_proto
environment_proto filesystem_proto       graphics_proto   login_proto       mdns_proto
ntp_proto         socket_proto           supervision_proto swap_proto       timebase_proto
```

Plus `mdns_config` (a crate) and `mdns_responder` (a program), which carry no `_proto` and travel
with the stem ruling below.

The 424 splits 252 code and configuration to 172 markdown, and only 108 of the markdown moved:
the status rule (in the appendix named under "How it was performed") decided the other 64.

## Five stems calef ruled on 2026-09-13, which this milestone carried

Working the unratified worklist, calef ruled the `mdns` family, `ntp_proto`'s stem and
`timebase_proto`'s. **The rulings were recorded and the rename was not performed**, deliberately:
doing it then would have meant renaming the same files twice, once for the stem and again for the
suffix. So this milestone grew by five names and the tree grew by none.

| Was | Is | Ruled |
|---|---|---|
| `mdns_proto` | `multicast_dns_protocol` | stem 2026-09-13, suffix by this block |
| `ntp_proto` | `network_time_protocol` | stem 2026-09-13, suffix by this block |
| `timebase_proto` | `counter_frequency_protocol` | stem 2026-09-13, suffix by this block |
| `mdns_config` | `multicast_dns_config` | 2026-09-13 |
| `mdns_responder` | `multicast_dns_responder` | 2026-09-13 |

The last two rows carry no `_proto` suffix and are here because they carry the same stem:
renaming the protocol crate and leaving its config and its responder spelled the short way would
split one protocol across two spellings, which is the state this milestone exists to end.

The third row was missing from this block and is the reason to enumerate rather than trust a
list. It said four stems; there were five. `timebase_proto`'s own provenance records calef ruling
`counter_frequency_proto` on 2026-09-13, the same day and the same worklist as the other four,
and it records `timebase_proto` as refused in the same breath. A lane working from this block's
table alone would have applied the suffix rule to a stem the tree had already said no to, and
produced `timebase_protocol`.

That is a narrower claim than it first looked and is worth stating precisely, because the first
draft of this paragraph overstated it. `timebase_protocol` is not itself on record as refused;
`timebase_proto` is. So `script/names --check`, which catches a name recorded as refused that is also
live, **would not have fired**: it matches the refused string, and the string a suffix sweep produces
is one letter different. The refusal is of the stem and the gate can only see the whole name. Nothing
mechanical stood between this milestone and a crate carrying a refused stem; reading the block did.

The sweep then did the other half of the same failure, and it is the blind-`sed` scar in its purest
form: it rewrote *"Refused `timebase_proto`, above"* into a name refusing itself, twice, and only
enumerating the diff found it.

The crate this milestone was minted over is therefore `counter_frequency_protocol`, not
`timebase_protocol`. The stem is an architect's and the suffix is this block's, applied in one pass
for the same reason the `mdns` and `ntp` rows exist.

`network_time_protocol` is also the answer to a stutter. Expanding the stem alone gives
`network_time_protocol_proto`, which says protocol twice. The suffix change removes the duplication
rather than adding to it, which is an argument for this milestone that its own block did not have.

The external-standard exemption is narrowed by this ruling, and that has to be said out loud.
Both crates' own provenance argued against expanding, and the argument was not weak.
`ntp_proto`'s said NTP is RFC 5905's own name for the protocol, *"the same external-standard
exemption `elf`/`pci`/`dtb`/`gpt` already carry"*. `mdns_proto`'s said the expansion does not stop
cleanly, since DNS is itself an acronym and a consistent spelling runs to
`multicast_domain_name_system_proto`. calef ruled against both on 2026-09-13, twice, having been
shown them.

So the exemption now reads: a standard's own name stays whole where it names a format or a piece
of hardware (`elf`, `pci`, `dtb`, `gpt`), and expands where it names a network protocol. That is a
line drawn rather than derived, and a reader is owed the reason: `elf` and `pci` are what the thing
*is* and have no useful longer form in a reader's head, where a protocol's expansion says what it
*does* (network time, multicast DNS) to someone who has not met the acronym. DNS stops because it
is the `pci` case one level down: domain name system teaches nothing a reader did not already
have.

The cost is honest and is this block's to carry: the exemption is a rule with a boundary rather
than one rule, and nothing mechanical can tell a format from a protocol. The next name that
tests it comes to calef.

There is no `ntp` program. Milestone 290 (`components/src/ntp.rs` is three programs wearing one
name) split it into `network_time_client`, `network_time_test_server` and
`unwritable_clock_witness`, all three spelled the way this milestone spells the crate, so nothing
disagrees with `network_time_protocol`.

## Why, and the rule it fails is the tree's own

**`proto` is not an abbreviation, it is a truncation.** `design/naming.md` already refuses the shape:

> Truncating a word you happen to be tired of typing is not abbreviation, it is shorthand, and
> shorthand is what the third principle ("a newcomer must be able to succeed without asking anyone")
> exists to refuse.

And the test it gives: *"would a competent stranger who has never read this tree recognise it?"*
`pci` passes that. `proto` does not, because it is equally short for `prototype`, and this tree
uses that word for a real thing. Milestone 263's spike built a prototype and deleted it on purpose on
2026-09-05, and wrote about doing so in a tree carrying fourteen `_proto` crates. The ambiguity is
live rather than theoretical.

It also fails the acronym rule set the same day, one category over. That rule asks whether an
expansion teaches: `pci` expands to peripheral component interconnect and the reader is no wiser, so
it stays. `proto` expands to protocol, which is exactly what these crates are and is the fact a
reader most needs, so it goes. The rule was written for acronyms and the principle is the same: keep
the short form when it teaches nothing, spell it when it teaches.

calef named it as his own laziness, which is worth recording because the convention was his and
because §75's own line is that the refusals are the valuable half. This one was never refused, only
never examined.

## What this is

A mechanical rename, tree-wide. Directory, package name, every `use`, every `Cargo.toml`
dependency, and the prose that cites them. `_proto` becomes `_protocol`; nothing else about these
crates changes.

Milestone 63 is the precedent and did about twenty names in one pass, including three directories
whose package names matched neither the directory nor the rule.

## Sequencing, which was the whole risk

It collides with almost everything, so it goes when the tree is quiet. Two lanes were named as
the constraint: milestone 264, writing provenance blocks for sixty unrecorded names, and milestone
91, which will touch nearly every documentation file.

**264 was satisfied. 91 was not, and this ran anyway, deliberately.** 264 is `BUILT` and landed; its
branch `milestone/264-name-provenance` is still on the remote and is stale rather than unlanded, the
merge queue having rebased it. 91 is `NOT-STARTED` with no lane on it, and waiting on an unstarted
milestone is waiting indefinitely. The collision with 91 is a merge conflict rather than a
correctness problem, and it is named here so that a future 91 finds it written down:

- Every documentation file 91 touches now spells these crates `_protocol`. The rename is on
  `main`, so 91 rebases onto it and sees no conflict from the spelling itself.
- **What 91 inherits is the 64 markdown files this deliberately did not sweep**, listed by rule
  in that appendix. Those still read `_proto`, correctly, because they are accounts. A documentation sweep that
  normalizes spelling across the tree will read them as rot and must not: the status rule is the
  reason, and `design/naming.md`'s rename procedure is where it is written down.
- And it inherits one thing this milestone could not fix: `design/decisions/` sections are closed
  decisions and keep `_proto` throughout. The cost is recorded in this block's `BUGS`.

**And `timebase_proto` was not ratified before this landed.** It was on `script/names --unratified`
as provisional, and ratifying it would have settled a name into a form this milestone was about to
change. calef held it back on 2026-09-05 for exactly that reason, and its successor
`counter_frequency_protocol` is still `provisional` for the same one: he ruled the stem and has not
been shown the whole name.

## `login_protocol` is final

`login_proto` became `login_protocol`, suffix only. calef ruled that the `login` stem stays for the
whole family (2026-09-15), so `login_protocol` is final. The argument for keeping the stem is in
`design/naming/vocabulary-rulings.md`, "The `login` stem stays".

## How it was performed, and what the sweep took

[`how-it-was-performed.md`](0265-proto-is-a-truncation-not-an-abbreviation/how-it-was-performed.md)
holds the pattern and the two habits it relied on, the five records the
sweep took and how they were restored, the `script/lint` check 3 glob, and the status rule that
decided which markdown moved.

## BUGS

- This is a rename with no functional change, which makes it the kind of diff nobody reads
  carefully. The pattern used and what it was checked against are written out above, which is what
  the original wording of this bullet asked for.
- `design/decisions/` keeps `_proto` throughout, and that leaves paths a reader cannot follow.
  Twenty-two closed decisions cite `crates/filesystem_proto` and its siblings; those directories no
  longer exist. Keeping the old spelling is correct (a closed decision is what was decided, in the
  words used then) and the cost is real, so it is named here rather than hidden. Nothing is red,
  because `script/citations` checks glosses and attributed block quotes, not backticked spans.
- It does not touch `_rt`, `_cli` or any other suffix, and nobody has checked whether the tree
  carries other truncations of the same kind. That sweep is a different milestone and this block does
  not claim it. `_rt` is already gone, by milestone 285 and for the same reason.
- Fifteen crate names get five characters longer, and `nifefs` caps archive names at 32 bytes.
  Crates are not in the archive, so nothing here is bounded by it, but a program taking one of these
  names later would be. The longest name this milestone creates is
  `counter_frequency_protocol` at 26.
- **Five of the renamed crates are still `provisional` and this milestone did not change that.**
  `multicast_dns_protocol`, `multicast_dns_config`, `multicast_dns_responder`,
  `network_time_protocol` and `counter_frequency_protocol` carry stems calef ruled and whole names he
  has not been shown. They stay on `script/names --unratified` and that is the correct place for
  them.

## Follow-on

- **Recorded.** `design/decisions/` keeps the old spelling, in this block's `BUGS` and in
  `design/naming.md`'s rename procedure, where somebody performing the next rename meets it.
- **Done.** `login_protocol`'s stem was ruled on 2026-09-15, as the correction above says. Recorded
  in the crate's provenance block and in `design/naming.md`.
- **Done.** The five stems calef ruled on 2026-09-13 were performed in this pass, including
  `counter_frequency_protocol`, which this block's own table had omitted. Carried by this lane's pull
  request.
- **Done.** `script/lint` check 3's glob, which this rename would have left selecting nothing. Fixed
  in the same pull request, with the hazard written into the check's comment.
- **Milestone 401.** The class that glob belongs to. One instance is a fix; the question of how
  many other gates pick their own subject with a pattern that is allowed to stop matching is a lane,
  and this milestone measured only what it tripped over.
- **Recorded.** The other truncations nobody has swept for, in this block's `BUGS`. It is the same
  shape as the acronym question, and milestone 388 (an acronym sweep the tree can do at once) is the list that already exists for acronyms,
  with its `BUGS` carrying the names still waiting on an architect.

## Index row

calef, 2026-09-05: it was lazy and should have been `_protocol` globally. `proto` is equally short
for `prototype`, which this tree uses for a real thing, and design/naming.md already refuses
truncation. 15 crates carried `_proto`, across 424 files (measured 2026-09-14 at `2487a1f7`). Five
stems calef ruled on 2026-09-13 travelled in the same pass rather than renaming the same files
twice: `counter_frequency_proto` was ruled and `timebase_proto` refused, and `mdns_config` and
`mdns_responder` moved with their protocol's stem. `login_protocol` took the suffix and is final,
since calef ruled that the `login` stem stays (2026-09-15). The appendix records the five records
the sweep took and restored, and why `script/lint`'s `crates/*proto` glob had to move with it.
