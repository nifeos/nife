---
status: BUILT
raised: 2026-09-17
built: 2026-09-17
---
# 319. The crate that parses firmware had no proofs, and three of its first ones were false

*(Number provisional until the merge queue lands it.)*

## The premise, measured

`crates/machine_discovery` is 4,072 lines whose entire job is reading bytes this system did not
write: ACPI tables, the PVH boot handoff, device-tree properties, a command-line token another
binary encoded. It carried **zero Kani harnesses** and appeared in `script/verify`'s crate table
not at all.

```console
$ grep -c machine_discovery script/verify
0
$ grep -rc 'kani::proof' crates/machine_discovery/src/*.rs
aarch64.rs:0  acpi.rs:0  cpu_list.rs:0  framebuffer.rs:0  interrupt_id.rs:0
lib.rs:0  plic.rs:0  riscv64.rs:0  x86_64.rs:0
```

`design/fatal-risks/README.md`'s risk 2 quotes milestone 197 on why `user/` deserved the prover: it *"holds
parsers over bytes this system did not produce"*, and the parsers lifted into crates under rule 7
are *"every one already in `script/verify`'s table"*. This is the one that got lifted and then never
proved. Its sibling `dtb` has four harnesses and one of them caught a real defect, `be32`'s
unchecked `at + 4`, reachable from a corrupt device tree on the boot path.

**And the bytes stopped being hypothetical the day before this lane ran.** xenon, a Dell OptiPlex
7050, booted nife on 2026-09-17. This code parsed a real MADT, MCFG and DMAR written by firmware
that had never heard of it (`bench/xenon-2026-09-17/first-light-095500.log`, including an ECAM
window at `0xf0000000` that disagreed with a hardcoded constant). Every table this parser had seen
before that was one QEMU wrote for it.

## Three of the harnesses were false when they were written

All three on arithmetic over a field a test would have had to think to write down, and none of them
reachable from anything QEMU emits. This is the outcome the lane existed for, so it is first.

**`acpi::Dmar::host_address_width` was `body[0] + 1` in a `u8`.** The DMAR stores the machine's
physical address width minus one, so undoing that needs a byte's worth of headroom the byte does not
have. A DMAR whose `HostAddressWidth` field is `0xff` overflows, and
`kernel/src/arch/x86_64/machine.rs`'s `read_dmar` calls `parse_dmar` **directly on firmware bytes
reached through the direct map**, so the panic is on the x86 boot path with nothing above it to
catch one. It is `dtb::be32`'s defect one table over: a field widened by one with no room for the
widening.

Fixed by making the field a `u16`, which is rung one of AGENTS.md's ladder rather than rung two: the
addition cannot overflow at all, instead of being guarded against. The table's byte ranges over
`0..=255` and the width it names ranges over `1..=256`, so the wider type is also the honest one.

**`acpi::McfgEntry::size()` computed `end_bus - start_bus + 1` in `u64`.** Nothing in the MCFG's
encoding forbids a window that ends before it begins, and `mcfg_entry` reads both bytes straight out
of the table without comparing them, so any such window underflowed to roughly 2^64 and then
multiplied by a mebibyte. Fixed by answering zero, which is what "no bus is in this range" means; the
limitation that an inverted window is still *reported* as written is now in the module's `BUGS`,
because the bus numbers are firmware's claim and this module reports claims.

**`framebuffer::Framebuffer::span()` compared a row width against the stride with a
`saturating_mul`.** The saturation was put there to stop `width * 4` overflowing a `u32`, and it
does. It also makes the comparison meaningless: above `2^30 - 1` the product saturates to `u32::MAX`,
so a stride of `u32::MAX` satisfies a guard that one row of pixels genuinely fails, and `span`
returns fewer bytes than a single row needs. That number bounds every write the console makes. Fixed
by comparing in `u64`, where the overflow and the comparison are both right at once.

**And the crate's own test asserted the defective answer.** `a_span_that_cannot_be_computed_is_refused`
expected `Some(u32::MAX * u32::MAX)` for a screen `u32::MAX` pixels wide with a stride of `u32::MAX`,
and called it *"the honest answer"* on a 64-bit host. The test and the code agreed because both read
the same saturating product. That is the clearest answer this lane has to why a crate with 41 tests
wanted a prover, and it is exactly the failure `design/fatal-risks/README.md`'s risk 2 is about.

None of the three defects is exotic and none was going to be found by a test. Every one of those 41
tests builds the table `q35` produces, because that was the only machine there was. `q35`'s DMAR says
38. Its MCFG says buses 0 through 255. Its screen is 800 by 600.

## What is proved, and why each one could plausibly have been false

Fifteen harnesses, in four `#[cfg(kani)] mod verification` blocks beside the code they prove. Each
carries a `Falsification: replayable` block per DECISIONS §134, and every patch under
`crates/machine_discovery/falsifications/` was checked by `script/falsifications --sweep`: applied,
the harness run, required red, reverted.

| harness | the defect its patch restores |
|---|---|
| `acpi::an_rsdp_is_accepted_only_when_its_bytes_sum_to_zero` | the 20-byte checksum is not checked, so an RSDP is believed on its signature alone |
| `acpi::a_table_header_this_parser_accepts_always_has_a_body_length` | the length guard asks `length == 0` rather than `length < 36`, and `body_len` underflows |
| `acpi::the_madt_walk_terminates_and_stays_inside_the_body` | minimum entry length drops to one, so the cursor advances less than one header per step |
| `acpi::the_dmar_walk_terminates_and_stays_inside_the_body` | the MADT minimum of two is copied onto a table whose own header is four |
| `acpi::no_override_writes_outside_the_sixteen_legacy_irqs` | the `source < 16` bound leaves a four-clause `let`-chain, and a firmware byte indexes a sixteen-entry array |
| `acpi::the_dmar_fixed_part_decodes_without_arithmetic_overflow` | **the shipped defect**, restored |
| `acpi::an_ecam_windows_size_is_total_and_counts_one_mebibyte_per_bus` | **the second shipped defect**, restored |
| `acpi::the_root_tables_entry_count_and_its_entry_reader_agree` | the reader bounds on the entry start instead of its end, so it returns one entry more than the count |
| `framebuffer::an_encoded_token_never_exceeds_the_maximum_it_advertises` | `MAX_LEN` one below the real worst case, which is what its own doc records nearly happening |
| `framebuffer::an_accepted_span_covers_every_pixel_the_geometry_describes` | **the third shipped defect**, restored |
| `framebuffer::every_hex_field_the_loader_writes_is_one_the_kernel_reads_back` | `parse_hex` refuses one digit fewer than `write_hex` can emit |
| `x86_64::no_length_of_handoff_bytes_makes_the_decode_read_past_its_end` | the version-1 length check is gone, and offsets 40 and 48 are read out of a 40-byte handoff |
| `x86_64::a_range_read_out_of_the_memory_map_never_wraps_to_look_empty` | `end()` wraps, so a firmware size near `u64::MAX` makes a range look empty to a frame allocator |
| `riscv64::a_multi_letter_extension_is_never_read_as_a_privilege_letter` | the privilege scan runs past the first `_`, and `_sstc` reads as a supervisor claim |
| `riscv64::a_counter_width_is_widened_before_the_specifications_off_by_one_is_undone` | the six-bit width field is masked with `0xff` |

Three of these deserve their own sentence, because the argument for them is not "it parses bytes".

The RSDP checksum is the crate's one security-shaped claim. The module header already says why:
the RSDP is found by *scanning low memory for an eight-byte string*, so the checksum is the only
thing between a coincidence and a physical address the kernel will follow. The hand-written tests
flip one byte and expect `BadChecksum`; the harness quantifies over every 36-byte pattern, including
the ones where a second error cancels the first.

The RISC-V privilege-letter reading is the one the bench already paid for. The VisionFive 2's
vendor firmware marks its M/U-only S7 monitor core `"okay"` with `mmu-type = "riscv,sv39"`, both
false, and starting it crashed the firmware (2026-08-14, `notes/visionfive2.md`). The hart's own
`riscv,isa` is the only thing that tells the truth, and `supervisor_mode_claim` returning
`Some(false)` is the only thing `Cpu::startable` has to refuse it with. The claim is stated as an
invariance rather than as a restatement of the code: appending a multi-letter extension to an ISA
string never changes what it says about privilege modes. A scan that reaches past the first `_` reads
QEMU's own `_sstc` as a supervisor claim and turns that `Some(false)` into a `Some(true)`.

The screen token is a wire format, which is what earns it a prover rather than a test. The
loader writes it and the kernel reads it, so they are separate binaries. A description one can
write that the other cannot read is a black screen with nothing to say why (`parse` is documented to
read a malformed token as no screen at all). `MAX_LEN`'s own doc records the near miss: the token was
`fb=` when 64 was chosen, `screen=` is four characters longer, and the worst case went to 63 of 64
with nothing noticing.

## What was deliberately not proved

Risk 2's retrospective is specific about how this goes wrong, so the refusals are listed rather than
implied.

- `interrupt_id`, `cpu_list` and `plic`'s real logic needs a symbolic `Dtb`, which is the wall
  the `device_tree_blob` package already records for its own structure-block token loop and `crates/elf` records for the
  loader. Their leaf decoders (`be32_word`, `hwid_from_reg`, `cells_to_u64`, `is_okay`) are written
  with `get`, `>=` guards and `checked_mul` already, so a totality harness over any of them is
  `capability_table::subset_is_reflexive`'s cousin: true of every plausible implementation, and evidence of
  nothing. They are not proved and this is why.
- `Cpu::startable` is three booleans and a `matches!`. A harness over it would restate its own
  body. The property that matters about it is about `supervisor_mode_claim`, which is proved.
- `aarch64::Isa` is bitfield extraction from architected ID registers, which are read from the
  part in front of you rather than parsed from anything. It is the crate's one module that is not a
  parser, and it is out of scope for that reason.
- The whole-table walk from the RSDP through the XSDT to each table needs a symbolic pointer into
  memory this crate never holds. The leaves and the two self-describing entry walks are what bounded
  model checking reaches, so they are what is proved. The volatile half of that walk,
  `kernel/src/arch/x86_64/machine.rs`'s 836 lines reading raw pointers into the direct map, was
  flagged by milestone 304's lane as wanting a design decision and is untouched here. See the
  follow-on below for what this lane learned about its shape.
- **`mcfg_entry`'s index arithmetic was checked and found not to need a harness.** The suspicion was
  that `MCFG_FIXED_LEN + index.checked_mul(MCFG_ENTRY_LEN)?` overflows on the *outer*, unchecked
  addition. It cannot: `checked_mul` by 16 returns at most `usize::MAX - 15`, and the eight it adds
  fits. Recorded because the cost of checking was two minutes and the cost of proving a tautology is
  permanent.

## What was tried and abandoned, with the number

Milestone 197's abandoned properties, with their costs, were worth more than the ones it proved, so
this one is reported the same way.

The whole-token round trip, `parse(encode(x)) == Some(x)` for every `Framebuffer` the encoder
accepts, was still running after 18 minutes on the dev Mac at `--unwind 18`, and was killed rather
than waited out. For scale: the whole suite's serial time is 30.3 minutes, its sharding floor is
`glob::the_dot_rule_only_touches_names_that_start_with_a_dot` at 10.8, and the other two framebuffer
harnesses in the same run finished in 4.8 and 1.6 seconds. One harness at 18-plus minutes would have
become the new floor and roughly doubled the crate's own row.

The cost is not the round trip's logic, it is `parse`'s front end: `split_ascii_whitespace` and
`find_map` over a `str` whose length is symbolic, so every byte position is a branch before any of
the four converters is reached.

What replaced it is the half that carries the risk, which is the converters rather than the
tokenizer: `every_hex_field_the_loader_writes_is_one_the_kernel_reads_back`. The disagreement this
property exists to catch is `write_hex` emitting a sixteenth digit that `parse_hex` refuses, and that
is now proved for every `u64` with no tokenizer in the formula. The whole-token round trip stays a
hand-written test over six realistic geometries, which is what it was before this lane and is honest
about what it covers.

The decimal twin was written, measured, and dropped: 911 seconds. `write_decimal` divides a
symbolic `u32` by ten, ten times, and symbolic division is the one operation bit-blasting does
badly; `parse_decimal` multiplies it back. 15.2 minutes for one harness would have become the
suite's atomic floor, above `glob` at 15.0, which is the number the whole sharding argument in
`script/verify` is built on. The hex twin is shifts and masks and costs 135 seconds, which is
still most of this crate's row and is why the row reads 180 rather than 60.

So the decimal converters are **not proved**, and the boundary that matters about them (`4294967295`
accepted, `4294967296` refused) is covered by a hand-written test rather than for every `u32`. That
is a worse guarantee and it is the one the measurement bought.

And the span property was rewritten twice for the same reason, which is the pattern worth taking
away. The natural spelling, `span >= width * 4 * height`, reaches `2^64` at the type extremes and
so has to be done in `u128`: 20 minutes, killed. Restating it as a conjunction did not help,
because the second half (`span == stride * height`) asks the solver to equate two 64-by-64-bit
multiplies: 9 minutes, killed. What ships is the half that carries the defect,
`stride >= width * 4`, at 0.04 seconds.

That is a 30,000-fold spread over three spellings of one claim, and the cheapest one is the one that
found the bug. The lesson is not that wide arithmetic is slow, which everyone knows. It is that
`span == stride * height` restates the function's own definition, so proving it was never going
to catch anything, and it was costing all of the time. Ask what a clause could falsify before paying
for it.

## What this cost and what it bought

| | this crate | tree-wide |
|---|---|---|
| harnesses before | 0 | 153 |
| harnesses after | 15 | 168 |
| `script/verify` row | absent | `machine_discovery 180` |

The 180 seconds is a dev-Mac measurement rather than a CI-log one, which is the wrong machine for
that column, the same caveat `jh7110_entropy` and `kernel` carry. 135 of the 190 solver seconds are one harness,
`every_hex_field_the_loader_writes_is_one_the_kernel_reads_back`; the other fourteen together are
about fifty-five, and the two slowest of those are 17.6 and 14.5.

`script/lint`'s "every crate with proof harnesses is in the verify table" gate (milestone 193's,
written after `jh7110_entropy` and `multicast_dns_protocol` each carried harnesses that ran nowhere)
fails until the row exists. That is exactly what it is for, and it is how this lane knew the row was the
second half of the work rather than an afterthought.

## Names

Every harness name here is **provisional**, along with the four `verification` module names. calef
names things. The module name follows the `device_tree_blob` package's and `crates/capability_table`'s existing spelling so
the `<module.path>.<harness>.patch` convention in DECISIONS §134 needs no special case.

## Follow-on

Reported as proposals rather than as prose, per AGENTS.md's rule that identified work leaves the lane
in a tracked form:

- **Milestone 423.** The volatile half's seam is narrower than it looked. `kernel/src/arch/x86_64/machine.rs`'s ACPI
   walk is 836 lines, but with this crate proved, what is left on the kernel side of the seam is
   *reading N bytes at a physical address through the direct map* and handing them here. Every
   decision about what those bytes mean is now in a proved crate. That suggests the design decision
   milestone 304 (only ever compiled one architecture)'s lane wanted is a smaller one than "how do we prove a volatile walk". It is whether
   the direct-map read itself gets a checked accessor with a bound, which is the same question
   `dtb::Dtb::from_ptr` already answered on the other two architectures. Wants a lane, and wants
   calef on the accessor's name.
- **Milestone 426.** The DMAR defect's family should be swept for. `field + 1` where the field is a byte and the
   value is a byte is a shape, not an incident. This lane found it twice in two crates (broken in
   `acpi::parse_dmar`, correct in `riscv64::CounterInfo::bits`) with nothing at either site saying
   which it was. `cargo mutants --list` will not find it, because the mutation is in the type rather
   than in the expression. A `git grep` for specification fields documented as "one less than" is
   cheap and is not this lane's.
- **Milestone 323.** Milestone 323 (the falsification record is incomplete in five ways) found the four harnesses of the `device_tree_blob` package all `unfalsified`. They are the sibling this lane was argued
   from, and the argument cuts both ways: a harness with no falsification record is counted as
   `unfalsified` and says so. Four patches is an afternoon.

## Index row

4,072 lines that parse firmware the system did not write had no proofs at all and no row in
`script/verify`'s table, the day after xenon made those bytes a real Dell's rather than QEMU's.
Three of the first fifteen harnesses were false, one of them a panic on the x86 boot path from a
single `0xff` byte in a DMAR. One of them is a guard whose `saturating_mul` made the comparison
vacuous in a way the crate's own test had written down as the expected answer
