# Spelled-out rulings

*An appendix to [`design/naming.md`](../naming.md), which is the rule. It holds rulings that spell
out a contraction nothing outside the tree owns, after the `receive` ruling in
[vocabulary-rulings.md](vocabulary-rulings.md), which sat in a document with no room left under the
prose budget. The file and its stem are provisional; naming is an architect's.*

## The tree spells `operation`, never `op`

calef, 2026-10-04 UTC, after the `receive` ruling: *"Seems like OP should be OPERATION and RECV
should be RECEIVE."*

> `operation` is spelled out in every identifier this tree owns (`OPERATION_READLINE`,
> `request_operation`, `operation(w0)`). `op` is not a ratified abbreviation for it. An identifier
> that mirrors an external API, or where `op` means something else, keeps its spelling.

The rule is the one the `receive` ruling applied: spell out a contraction nothing outside the tree
owns. The `OP_` prefix on request-code constants became `OPERATION_`. The decoder `op(w0)` became
`operation(w0)`. The names `req_op`, `fs_op`, `op_code`, `op_in`, `OPS`, `PROOF_OPS` and `ops`
followed. No constant value, opcode or wire layout changed.

Refused: keeping `OP_` because the prefix is short and every constant carries it, since length is
the cost the tree already pays for `RECEIVE_CAP`. Also refused: `opcode` or `OPCODE_`, which names
the number rather than the thing requested and would be a second ruling.

Kept, because someone else owns the spelling or the word means something else:

- `core::ops` and `std::ops` (`Deref`, `Range`), and every `no-op` or `no_op`, an English idiom.
- `unsafe_op_in_unsafe_fn`, a rustc lint.
- `op` as an instruction mnemonic in `script/stack-depth-check` and `script/fastpath-footprint`,
  and as the TFTP opcode field in `script/board-netboot`.
- DHCP's `op` field (`op = BOOTREQUEST` in `crates/virtio`).
- `is_op` and `is_diag_op` in `crates/grant_plan/src/line.rs`, and the `op` loop in a swish help
  test. Both mean a shell *operator* (`|`, `>`, `2>`).
- `vendor/redoxfs`, a permission-bit `op` that is not ours to edit.

`OPERATION_SENDTO` keeps the POSIX `SENDTO` spelling in its suffix, pending a separate ruling on
whether the tree spells that `SEND_TO`. This ruling does not touch it.

Closed records (`BUILT` roadmap blocks, `design/decisions/`, dated audit reports) keep the name they
used, so a grep for `OP_` finds them.

Live notes and roadmap documents spell the names out.

## The tree spells `capability`, never `cap`

calef, 2026-10-06 UTC: *"I don't think we should abbreviate capability as cap."* When the
maintainer proposed how to apply it, he answered: *"Record the ruling. We've almost got a quiet
tree so we can run that shortly."*

> New code spells `capability` out in every identifier. `cap` is not a ratified abbreviation for
> it, as a whole word or as a component (`cap`, `Cap`, `CAP`, `caps`, `cap_slot`, `SEND_CAP`,
> `irq_cap`). The existing names are renamed in one sweep when the tree is quiet.

The rule is the `operation` ruling's. Its exceptions are carried over as proposals, not yet ruled:
an identifier that mirrors a name a specification coined keeps it, and so does `cap` where it is
the English word for a ceiling. Comments and prose are outside the rule, since "a cap on lane
count" abbreviates nothing.

`script/lint` holds the line until the sweep, through `helpers/cap_abbreviation.py`: a ratchet
over Rust identifiers that fails when the count rises, 2,890 on the ruling's day. The census, the
kept names and a proposed spelling for each public name are in
[capability-spelling-worklist.md](capability-spelling-worklist.md). The proposals are not ruled. The ABI names
(`SEND_CAP`, `RECEIVE_CAP`, `CAP_INSERT`, `SYS_CAP_DELETE`) and the `Cap` type go last, as one batch.

## The tree spells `receive` and `transmit`, never `rx` and `tx`

Ruled 2026-10-26 (UTC), tree-wide: every name this tree authors spells `rx` and `tx` out
(`receive_*`, `transmit_*`, `ReceiveVerdict`, `TransmitError`). Register and bit names transcribed
from a datasheet or another project (`RDT`, `TXDCTL`, `CHAN_*`, `DD`, `EOP`) keep their spelling.
The ruling turns the 2026-09-24 refusal of `rx_waiting` (now `is_byte_waiting` in
`kernel/src/console.rs`) into the rule, and it follows the `receive` ruling in
[vocabulary-rulings.md](vocabulary-rulings.md). Where the word is a side rather than a direction,
the name says what it holds: the pipe ends of c_library's `End` are `read_pipe` and `write_pipe`,
after `read`, `write`, `readers` and `writers` (Decision 43, ruled 2026-10-26 (UTC)).

## The tree spells `physical_layer_device`, never `phy`

Ruled 2026-10-26 (UTC), tree-wide: where `phy` names the chip, every name this tree authors spells
it `physical_layer_device`, IEEE 802.3's own phrase (`PhysicalLayerDeviceResetBlocked`,
`physical_layer_device_id`). Refused `physical_layer`, which would blur the chip with the layer:
smoltcp's `phy` module is a layer abstraction. Documentation gives "PHY" on first use, so a search
for it lands. Foreign names keep their spelling: register names, and smoltcp's `phy` module and its
types.

## The tree spells `initialize`, never `init`

Ruled 2026-10-26 (UTC), tree-wide: where `init` means initialize, every name this tree authors
spells it `initialize` (`initialize_hardware`, `initialize_physical_layer_device_workarounds`). The
ruling follows `config` and `arg`, and it ends the split with `system_initializer`. Foreign names
keep their spelling: Intel's `STATUS.LAN_INIT_DONE` and the e1000e `LanInitTimeout` that names it,
and Linux's `e1000_init_hw_ich8lan` where documentation quotes it.

## The tree spells `irq` out, by what the name holds

Ruled 2026-10-26 (UTC), tree-wide and ABI included: every name this tree authors spells `irq` out,
and a renamed item carries `#[doc(alias = "irq")]`. The word follows what the identifier holds:

- `interrupt` for an interrupt itself (`mask_interrupts`, `unmask_interrupts`);
- `interrupt_number` for an IRQ number;
- `interrupt_line` for a wired source.

The ABI capability takes `Interrupt` or `InterruptLine` by what it names, and its calls follow
(`bind_interrupt`, `interrupt_wait`, or the `_line` form). Refused `interrupt_request`: `ipi` and
`nmi` expanded to "... interrupt" on 2026-10-18 (UTC), and `irq` matches them.

Ruled 2026-10-27 (UTC), the names the irq sweep left open:

- The ABI capability is `Interrupt` (`Object::Interrupt`), not `InterruptLine`.
- `arch::<arch>::irq` is `arch::<arch>::interrupt_controller` on aarch64, riscv64 and x86_64.
- The self-test `deviceirq` prints `device_interrupt`. It is a clean break: no table maps the old
  name to the new one.
- Boot transcript labels spell it out: `uart interrupt`, `device interrupt`, `isa interrupt` and
  `interrupt<id>`.
- RISC-V cause 9 is `SUPERVISOR_EXTERNAL_INTERRUPT`, with `#[doc(alias = "IRQ_S_EXT")]` for the name
  Linux gives it.
- The Arm exception vector constants are `VECTOR_INTERRUPT_CURRENT` and `VECTOR_INTERRUPT_LOWER`.
  Arm's own `IRQ` and `FIQ` vector kinds keep their spelling in the table strings.
- Doc comments on register accessors name the manual that defines the register and give no section
  numbers.

## The tree spells `queue`, never `fifo`

Ruled 2026-10-26 (UTC), tree-wide: where a name this tree authors says `fifo`, it says `queue`
(`receive_queue_bytes`, `transmit_queue_bytes` and `queue_bytes`, after the databook's RQS and
TQS), and the renamed item carries `#[doc(alias = "fifo")]`. Register names copied from a
datasheet (`FIFOTH`) keep their spelling. The ruling reaches the crate ratified on 2026-08-23
(Decision 41, ruled 2026-10-26 (UTC)): `intrusive_fifo` becomes `intrusive_queue` and its `Fifo`
becomes `Queue`, with `#[doc(alias = "fifo")]`.


## The tree spells `minimum`, `maximum` and `operating_system`, with three limits kept

[precedents-set-by-example.md](precedents-set-by-example.md) holds the expansion, ruled 2026-10-26
(UTC): `min` expands to `minimum`, `max` to `maximum` and `os` to `operating_system`. Ruled
2026-10-27 (UTC), on the sweep that applied it:

- A limit this tree defines under the Unix names `PATH_MAX`, `LINE_MAX` and `NAME_MAX` expands to
  `PATH_MAXIMUM`, `LINE_MAXIMUM` and `NAME_MAXIMUM`. Prose that cites Unix itself keeps the original
  spelling, since it names the Unix limit and not ours.
- The GPT crate's constants are `OPERATING_SYSTEM_TYPE` and `PROTECTIVE_OPERATING_SYSTEM_TYPE`. Their
  documentation cites UEFI's `OSType` field, and each carries `#[doc(alias = "OSType")]`.
- `os_primitives_benchmarker` keeps its name until the variable-width step of Decision 307 lands.
  That step renames it; the sweep does not.

## The `intrusive_queue` order property says `first_in_first_out`

Ruled 2026-10-27 (UTC): the `intrusive_queue` property test that checks arrival order is
`any_push_pop_interleaving_is_first_in_first_out_and_lossless`. It replaces
`any_push_pop_interleaving_is_queue_ordered_and_lossless`, whose name did not say which order it
checks. `senders_queue_in_arrival_order` keeps its name.

## The min, max and irq sweeps: rows ratified, and the names they changed

Ruled 2026-10-27 (UTC), on [#2797](https://github.com/nifeos/nife/pull/2797): the architect ratified
every `Name: provisional` block the irq sweep and the min, max and os sweep added, under the names
below. Each ratified block now reads `Name: ratified 2026-10-27 (#2797).` Five rows stay
provisional for their own sweeps: `interrupt_cap` and `interrupt_cap_rights` (the `cap` ruling
below), `MAXIMUM_CAPS`, and `MAXIMUM_STEP_FORWARD_NANOS` with `MAXIMUM_STEP_BACKWARD_NANOS` (the
unit ruling below).

Renamed, then ratified under the new name:

- A length constant carries its unit. `MAXIMUM_NAME_LEN` is `MAXIMUM_NAME_BYTES` and
  `MAXIMUM_IDENTITY_LEN` is `MAXIMUM_IDENTITY_BYTES`. A bare `MAXIMUM_LEN` is `MAXIMUM_BYTES` in
  `uptime`, `uefi_loader`'s `device_path.rs`, `machine_discovery`'s `framebuffer.rs` and both
  `boot_slot` modules. `filesystem_contract`'s `MAXIMUM_LEN` is `MAXIMUM_LENGTH_OR_OFFSET`, because
  it bounds a packed field that holds either.
- `MAXIMUM_COLS` is `MAXIMUM_COLUMNS` in `video_terminal` and in the `rmle` component.
- Argon2 cost: the fields, parameters and accessors `m_kib`, `t` and `p` are `memory_kib`, `passes`
  and `lanes`, each with `#[doc(alias = "m")]`, `"t"` and `"p"` and RFC 9106's terms in the docs.
  `MAXIMUM_M_KIB` is `MAXIMUM_MEMORY_KIB` and `MINIMUM_M_COST` is `MINIMUM_MEMORY_KIB`.
- The IOMMU limits share one noun for both vendors. `MAXIMUM_DRHDS` and `MAXIMUM_IVHDS` are
  `MAXIMUM_TRANSLATION_UNITS`. `MAXIMUM_RMRRS` and `MAXIMUM_IVMDS` are `MAXIMUM_RESERVED_REGIONS`.
  `MAXIMUM_RMRR_SCOPES` is `MAXIMUM_RESERVED_REGION_DEVICES`. `MAXIMUM_IVHD_RANGES` is
  `MAXIMUM_TRANSLATION_UNIT_DEVICE_RANGES`. Each doc names the specification's entry and its full
  name (Intel VT-d: DMA Remapping Hardware unit Definition, Reserved Memory Region Reporting; AMD
  IOMMU: I/O Virtualization Hardware Definition, I/O Virtualization Memory Definition) and carries
  `#[doc(alias = "DRHD")]`, `"RMRR"`, `"IVHD"` or `"IVMD"`.
- `boot_slot`'s `MAXIMUM_NIBBLE` is `MAXIMUM_FIELD_VALUE`. The doc says both fields are four bits
  wide, and the item carries `#[doc(alias = "nibble")]`.
- `environment_contract`: `TZ_MAXIMUM`, `LANG_MAXIMUM` and `TERM_MAXIMUM` are
  `MAXIMUM_TIME_ZONE_BYTES`, `MAXIMUM_LOCALE_BYTES` and `MAXIMUM_TERMINAL_TYPE_BYTES`. `KNOWN_TZ`,
  `KNOWN_LANG` and `KNOWN_TERM` are `KNOWN_TIME_ZONES`, `KNOWN_LOCALES` and `KNOWN_TERMINAL_TYPES`.
  They carry `#[doc(alias = "TZ")]`, `"LANG"` and `"TERM"`. The variable-name strings `"TZ"`,
  `"LANG"` and `"TERM"` do not change.
- `documentation`'s search-term limit `TERM_MAXIMUM` is `MAXIMUM_TERM_BYTES`.
- `domain_name_system`'s `UDP_MESSAGE_MAXIMUM` is `MAXIMUM_UDP_MESSAGE_BYTES`, and its doc spells out
  User Datagram Protocol.

## `capability` reaches every name, PCI capability structures included

Ruled 2026-10-27 (UTC), recorded here and applied by its own sweep: every name this tree authors
spells `capability` out, and that includes PCI capability structures. This extends the
[`capability` ruling above](#the-tree-spells-capability-never-cap) to the names it left open.
`interrupt_cap`, `interrupt_cap_rights` and `MAXIMUM_CAPS` stay provisional until that sweep
renames them.

## Unit symbols in names

Ruled 2026-10-27 (UTC), recorded here and applied by its own sweep:

- A name uses a unit symbol only from the allowed list: `ns` and `ms`; `kib`, `mib` and `gib`; `hz`,
  `khz` and `mhz`. A symbol that reads two ways is spelled out: `microseconds`, not `us`; `seconds`,
  not `s`; `minutes`; `meters`. `bytes` stays a word.
- The list grows by ruling.
- Rust standard-library names such as `as_nanos` keep their spelling.
- `NANOS_PER_SEC` gets one definition, `clock_contract::NS_PER_SECOND`. `network_time_protocol`,
  `timetable`, `uptime` and `user_mode_runtime` import it.
- `MAXIMUM_STEP_FORWARD_NS` and `MAXIMUM_STEP_BACKWARD_NS` carry ratified Name blocks (2026-10-27 UTC).
- `helpers/unit_symbols.py` counts the identifiers that break this ruling and holds the count to a
  ceiling that only falls; `script/lint` runs it. It leaves `nsec`, `msec`, `kb`, `mb`, `gb`, `tb`,
  `bps` and `ppm` uncounted, since the list above does not settle them.

## Abbreviation sweeps owed under §154

calef ruled on 2026-10-18 (UTC) that `config`, `spec`, `arg`, `mem`, `dir`, `va`, `pa`, `ipi`, `nmi`, `ist` and `msr` expand under §154 (the acronym test is whether the phrase is spoken); the table there holds his words. The rulings are in force; the identifiers are not yet spelled out. Measured on 2026-10-18 in tracked Rust outside `vendor/` and `patches/`, by a broad pattern that over-counts:

| Family | Hits | Files (summed per pattern) |
|---|---|---|
| `config` | 659 | 109 |
| `_VA`, `_PA` | 1,366 | 192 |
| `ipi`, `nmi`, `ist`, `msr` | 355 | 75 |
| `spec`, `arg`, `mem`, `dir` | 4,206 | 556 |
| `ipc`, ruled by §154 on 2026-09-18 | 1,668 | 285 |

Each family lands as its own pull request, serially. `ipc_receive_cap` also carries `cap`, which the section above already rules. A site where the owner's spelling is unclear goes to the architect as a list.
