# `machine_discovery`: 77 survivors

The `machine_discovery` part of milestone 326 (nobody has been assigned to turn a mutation score
upward)'s triage, on 2026-09-19, and its re-derivation on 2026-09-20. It verifies the crate's
figures that [notes/mutation-testing.md](../mutation-testing.md) summarizes. The method is set out
in [regressions-2026-09-14-capability-to-device-tree-blob](regressions-2026-09-14-capability-to-device-tree-blob.md).

### `machine_discovery`: 77 survivors, 58 killed, 11 equivalent, 8 recorded gaps

Before: 536 caught, 77 missed, 9 timeouts, 71 unviable (86.2% of viable). After: 594 caught, 19
missed (95.5%). The before column reproduces the 2026-09-19 census row for row. This is the crate
that census blamed for the corpus falling. Every kill below was verified by re-running the sweep.
Every equivalence claim is a mutant the final run still reports.

One sentence explains all 77, and it is not a truncation bug. Every fixture in this crate is a
table or a device tree that is well formed, so a guard was only ever asked about inputs with room
to spare on both sides of it. The crate has an `AcpiError::Truncated` variant, prose about tables
"shorter than it is", and a `MAX_CPU_NODES` the record can overflow. Nothing ever handed a parser
an input at any of those edges.

#### The one-property hypothesis, measured and refused

The crate was taken on the reading that 43 of the 77 were one defect wearing 43 hats: the
short-buffer guards at the top of every parser. On that reading a single prefix property would kill
most of them and take the crate to roughly 96%. It was tested on its own. The prefix property was
written first, alone, and the sweep re-run before anything else was touched.

**It killed 12, and took the crate from 86.2% to 88.1%.** Re-derived, 41 of the 77 are bounds,
length or offset shapes, so the reading of the survivor list was close. But only 12 of those are
guards a prefix of a valid input can reach. The other 29 want inputs a prefix cannot produce:

- The entry-walk guards want a table that is well formed and ends exactly at the buffer, not a
  short one. `at + 2 > body.len()` under `>=` is invisible unless an entry fills the body to its
  last byte. `len >= 8` under `true` needs an entry of a decoded type that is too short for that
  type's own fields, inside a list that is otherwise fine.
- `McfgEntry::size`, `mcfg_entry` and `root_entry` take an index, so no length exists to truncate.
- The `cpu_list` and `plic` guards want a malformed device tree, which is a fixture rather than a
  slice.

The shape the property is written in decides half the result, and that carries to the next crate.
"Feed every prefix and assert it returns an error" kills `<` under `==` and under `>`, because
those read past the end and panic. It does not kill `<` under `<=`: at every length such a loop
feeds, the original refuses too. The mutant that survives a refusal test is the one that refuses a
structure which is exactly long enough. The only input that distinguishes it is the boundary
length. **So the property has to be two-sided: every short prefix refused, and the shortest
sufficient one accepted.** Six of the twelve died only to the second half. `parse_sdt_header`
needed an empty-bodied table for the same reason, so its own `length` field is exactly
`SDT_HEADER_LEN`.

#### The MADT and DMAR entry walks (16)

These two iterators are the same shape twice: `[type, length, ..]` entries, self-describing, walked
until the body runs out. Both read firmware bytes on the x86 boot path, and nothing else checks the
lengths they trust.

`at + 2 > body.len()` was alive under `>=`, and `len < 2` under `<=`. Both were killed by a body of
exactly `MADT_FIXED_LEN + 2` holding the shortest entry the format allows. The DMAR's pair needed
two four-byte structures rather than one. That is what tells a cursor that advanced wrongly
(`at += len` under `*=`) from a walk that stopped early (`at + len > body.len()` under `<`). With
a single entry, both mutants and the original all end the walk in the same place.

The four `len >= N` match guards were alive under `true`. That decodes a processor entry that has
no flags word and reads the bytes after it. Closed by
`an_entry_too_short_for_its_own_kind_is_not_decoded`: four bytes of each decoded type, each
reported by its type code instead. Type 5 was also alive under `false` and under `<`, because
**`LocalApicAddressOverride` had no test at all**. A machine whose local APIC sits above 4 GiB says
so in that entry and nowhere else, since the fixed part's field is 32 bits. A decoder reporting it
as an unrecognized type would use the low address and touch memory that is not the APIC.

#### The MCFG (5)

`McfgEntry::size` was alive under `<=` and `==` on its ends-before-it-begins guard. No test ever
asked about a window of one bus, where equal bus numbers are a mebibyte rather than nothing. It was
also alive under `+` for `-`, because the only window tested starts at bus 0, where a sum and a
difference agree.

Two offset mutants sat beside them: `segment: u16(body, at + 8)` under `-`, and `u16`'s own
`bytes[at + 1]` under `-`. They are the more interesting pair. **The single window this crate
tested is zero in every field a wrong offset would reach**, so the segment could be read out of the
base's low half and still answer 0. Closed by a second window with distinct values in every field.

#### The device trees (12)

These are four shapes no dump can produce. Three fixtures are new (`many-harts`, `plic-shapes`,
`partial-psci`) and `interrupt-shapes` grew a node. Names provisional.

- `#address-cells` two bytes wide, and a `reg` short of its declared width at both widths. The CPU
  bindings require the property, so two bytes of it is a malformed tree with no correct reading.
  What there is, is a decoder that must not read the two bytes after it. All three guards were
  alive under `true`.
- Eighteen cores. `CpuList::truncated` (now `is_truncated`) was alive as a constant `false` and
  under `<` for `>`. No fixture in the tree overflows sixteen slots. So a predicate no caller could
  rely on was passing a test that asserted it on a seven-core machine.
- A `/psci` node stating only `method`. `method.is_none() && cpu_on.is_none() &&
  compatible.is_none()` was alive under `||` at both positions. `can_start_a_core` was alive as a
  constant `true` and under `||`. All four are the gap between "no PSCI node" (which
  `no-psci.dtb` covers) and "a complete one" (every other fixture). Firmware that published a
  conduit and no function id makes starting a core impossible, for a reason a boot line can name. A
  decoder demanding all three properties would report it as having no PSCI at all.
- The context map's two walks falling out of step. `plic-shapes` carries three cases. The first is
  the PLIC named like its harts' own controllers and declared before `/cpus`. Both JH7110 fixtures
  declare theirs after, so a walk that failed to filter it by `compatible` still got the right
  answer there. The second is a hart controller with no `phandle`, which still consumes its hart's
  slot. The third is a hart whose id is exactly `MAX_CONTEXT_HARTS`: the tree names an S context
  for it and the record has no slot for it. That last one is a write past a sixteen-element array
  under `<=`.
- PPI 16. `interrupt-shapes` had `badppi@` at 99, which any reading of the bound turns away. 16 is
  the first number that is not a PPI. Folded into the bank it becomes INTID 32, which is SPI 0: a
  line a different device owns.

#### The rest (10)

`Conduit::name`, `EnableMethod::name` and `MemoryKind::name` were each alive under `""` and
`"xyzzy"`. They are three enums whose only consumer is the boot print, and nothing asserted a word.
Then a revision-2 RSDP with a null XSDT. That is the only input where the second half of
`root_table`'s condition is asked, and following the zero sends the kernel to physical address 0.
Then `parse_hex` given `0x` with no digits, and `parse_decimal` given the character after `'9'`
read as a tenth digit. And blue, the only one of the three colors that travels up the word, so red
alone could not tell a sixteen-bit shift from either direction.

#### The eleven equivalents, by group

- `1 << 0` under `>>` (2). `MADT_PCAT_COMPAT` and `SBI_TIME`. Both sides are 1. This is the
  degenerate case [the recurring patterns](baseline-2026-08-03.md#patterns-that-recur-named-once)
  name, recorded rather than excluded so it stays visible if either constant moves off zero.
- Disjoint operands under `^` for `|` (5). `PixelOrder::store`'s two (bits 8..16, 16..24 and 0..8,
  built from masks that cannot overlap). `parse_hex`'s `(value << 4) | digit`, where the shift
  clears exactly the four bits a hex digit occupies. `eid`'s `(acc << 8) | b[i]`, since a byte
  cannot reach above bit 7. And `pmu_event`'s `(type << 16) | code`. That last is the weakest of
  the five. Its disjointness is documented rather than enforced: `event_idx[19:16] = type` and
  `[15:0] = code`, with no mask in the code. So it holds for every input the encoding admits, not
  for every `usize`. It is checkable because the existing `pmu_event(1, 0xffff)` assertion already
  exercises the widest code the field has room for.
- An assignment that is a no-op at equality (2). `Isa::from_device_tree` narrows twice, `base <
  out.base` and `t < out.mmu`, each guarding `out.base = base` and `out.mmu = t`. Under `<=` the
  extra branch fires only where the two compare equal, and the assignment then writes the value
  already there. It is the same value, not just an equal one, because neither ordering is coarser
  than its equality. `MmuType` derives `Ord` over its variants. `Base`'s hand-written `cmp` maps
  its four variants to four distinct widths (0, 32, 64, 128), so `Equal` means the same variant in
  both. An ordering that put two variants at one rank would break this argument rather than the
  code.
- A bound the other ceiling makes unreachable (1). `plic`'s `n < MAX_CONTEXT_HARTS` under `<=`. The
  conjunct after it is `cpus.cpus().get(n)`, and that slice is at most `MAX_CPU_NODES` long. Both
  constants are sixteen, so `get(16)` is `None` for every tree. The mutant's extra iteration never
  reaches the array it would index. **Re-check this one if either constant ever moves**: the
  equivalence is a relationship between two numbers, not a property of the line.
- A guard that is vacuous on the host (1). `Framebuffer::span`'s `bytes <= usize::MAX as u64` under
  `true`. On a 64-bit host `usize::MAX as u64` is `u64::MAX`, so the guard is always true and no
  host test can distinguish them. It is not dead code. It is what makes the span honest on a 32-bit
  target, which this crate is built for.

#### The eight recorded gaps, which are two instrument holes rather than two omissions

Six are compile-time `const _: () = { .. }` blocks: `IMPLEMENTERS`' duplicate-code check in
`aarch64.rs` and `IMPLEMENTATIONS`' sequential-id check in `riscv64.rs`. Their loop bounds survive
under `==`, `>` and `<=`. Each makes the loop body never run, so the block compiles and checks
nothing. No `cargo test` can see this, because the checker is `rustc` and the evidence of success
is that the build happened. It is the same object as `filesystem_contract`'s `verb::TABLE` walk in
[regressions-2026-09-14-clock-contract-swish-filesystem-contract](regressions-2026-09-14-clock-contract-swish-filesystem-contract.md).
It is recorded for that entry's reason: an exclusion would have to be by line, and would hide
anything else that lands there.

Two are a `const` inside a `#[cfg(kani)] mod verification` block: `x86_64.rs`'s
`const N: usize = V1_LEN + 1;` under `-` and `*`. `cargo test` never compiles that module, so a
mutant there always survives. `.cargo/mutants.toml`'s `verification::` regex cannot reach it.
cargo-mutants names a mutant by its path within the parsed file, and **a `const` gets no module
path in its name at all**. So there is nothing for a module-path regex to match. This is the third
member of a family, after `timetable`'s `proofs.rs` (a module that is its own file) and the two
globs added beside it, both in [regressions-2026-09-14-capability-to-device-tree-blob](regressions-2026-09-14-capability-to-device-tree-blob.md).
The concurrent census-delta lane of the same day measured the tree-wide extent: three mutants, of
which these are two. It filed a proposal for the general fix, whose number is minted at merge. No
test is owed here, because it is code no host build compiles.

The 9 timeouts are hangs and are unchanged by any of this. Four are `+=` under `*=` on a walker's
cursor (`Isa::implementer_name`, `Sbi::impl_name`, `lookup`). Five are the MADT and DMAR entry
walks' `len < 2` and `len < 4` under `==` and `>`, and their `||` under `&&`. A length of zero that
the walk accepts never advances. That is the tests noticing rather than missing, which is how
[the main note](../mutation-testing.md) reads a timeout whose mutant can hang.

#### 2026-09-20 re-derivation: two more pull requests touched this crate, and neither left a survivor

A lane assigned this crate for triage found it already at the state above. The accounting above is
what `milestone/326-machine-discovery-truncation` produced, merged to `main` on 2026-09-19
(`fdc51e8a` and neighbors, via the `maintainer/drain-the-proposal-pile` integration). The
2026-09-19 census row this crate was assigned under (622 viable, 77 missed, 86.2% caught) is that
lane's own `before` column. It was already closed by the time a second lane was briefed on it.
Briefing from a week-old census without checking the tree first is the trap milestone 326's own
`BUGS` section warns about. This is that trap sprung a third time.

Two pull requests landed after the triage and before this re-check. Pull request #976 (`b5fe8c5a`, `TGran4
0b0001` is a 4 KiB granule) widened the aarch64 granule decode. Pull request #977 (`b69e0ffa`) was milestone
227 (a GICv3 driver)'s driver, which added the whole of `src/gic.rs`: 242 lines with its own
`discover` and `confirm`. Both grew the crate. Their own tests were not checked against a mutation
sweep before they landed. So re-running `script/mutation -p machine_discovery` was the only way to
know whether they had left survivors.

Re-run: 714 mutants tested (up from 693), 612 caught (up from 594), 19 missed, 9 timeouts, 74
unviable (up from 71). That is **95.6% of viable (612 of 640)**. Every one of the 19 missed and 9 timeout
mutants is at the same file, line and operator argued above: the eleven equivalents, the eight
recorded gaps and the nine noticing timeouts. None is new. The 21 new mutants (18 caught, 3
unviable) all belong to the TGran4 widening and to `gic.rs`, and every one was caught. `gic.rs`
shipped with `tests/gic_versions.rs` covering both bindings, both refusal shapes, the
redistributor-region gap and the hardware cross-check. Cargo-mutants found nothing there to
survive on.

No untriaged survivor is left in this crate. The milestone 326 roadmap block's follow-on entry for
this crate is current and needs no correction beyond noting this re-check. There is no code change
to land. (Checked 2026-09-24: the 2026-09-21 census, run 35589550926, agrees; it scores
`machine_discovery` 97.0% with the same 19 missed.)
