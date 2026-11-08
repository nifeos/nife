# jh7110_clock_and_reset

The logic of the clock and reset generator in the StarFive JH7110, the RISC-V system-on-chip of the
VisionFive 2. It holds the register offsets and bit fields of the STG (System-Top-Group), AON
(always-on) and SYS (system) domains, the bring-up plans the TRNG (true random number generator),
the Ethernet controller, both SD/MMC (MultiMediaCard) controllers and the I2C bus of the PMIC (power
management integrated circuit) need, the I2C input-clock arithmetic, and what each of the chip's two
device trees calls the controllers. It never touches a register: you map the window and do the
volatile stores, and this crate says which word, which bit and in what order. It is `no_std`,
depends on nothing, and is tested on the host.

## EXAMPLES

```rust
use jh7110_clock_and_reset::{
    is_deasserted, Step, WindowLocation, ENTROPY_BRING_UP, SYSTEM_TOP_GROUP,
    SYSTEM_TOP_GROUP_WINDOW,
};

// The vendor clkgen node: three windows, found by name.
let regions = [(0x1302_0000, 0x1_0000), (0x1023_0000, 0x1_0000), (0x1700_0000, 0x1_0000)];
let names = Some(&b"sys\0stg\0aon\0"[..]);
let system_top_group = WindowLocation::from_regions(&SYSTEM_TOP_GROUP_WINDOW.spellings[1], &regions, names)
    .unwrap_or(SYSTEM_TOP_GROUP_WINDOW.fallback());
assert_eq!(system_top_group.base, 0x1023_0000);

// The TRNG's plan: two clocks, then one reset.
assert_eq!(ENTROPY_BRING_UP[2], Step::DeassertReset(3));
assert_eq!(SYSTEM_TOP_GROUP.clock_offset(15), Some(0x3c));
let reset = SYSTEM_TOP_GROUP.reset_bit(3).unwrap();
assert!(is_deasserted(0b1000, reset.mask));
```

Walk a plan in order: for `EnableClock(i)` set `CLOCK_ENABLE` in the word at
`domain.clock_offset(i)`, for `SelectParent` write `with_parent`, and for `DeassertReset(id)` clear
the bit at `reset_bit(id).assert_offset`, then poll `status_offset` until `is_deasserted`. Keep the
words before and after in a `BringUpReport`, which tells "already up" apart from "this run started
it".

## What is proven

Nine Kani harnesses (`cargo kani`, stock Kani) prove, each with a falsification patch under
`falsifications/` that makes it fail at its own assertion:

- `with_parent` changes only the clock parent field: every other bit of the word is unchanged, and an
  over-wide parent never reaches the enable bit.
- For every domain and every index, `clock_offset` and `reset_bit` answer `None` past the domain's
  counts, and otherwise a word-aligned offset inside the domain, a reset mask of exactly one bit,
  and assert and status words the same distance from their first words.
- Every fixed plan (the TRNG, both halves of the Ethernet controller, the PMIC bus, both SD/MMC
  slots) holds only ids in its domain, enables every clock before it releases any reset, and picks a
  clock's parent before enabling that clock.
- A `DevicePlan` or `PowerManagementChipBus` built from device-tree bytes holds only in-bounds ids,
  never more steps than its bound, and every clock step ahead of every reset release. The bus plan
  is never empty.
- `is_deasserted`, `is_clock_enabled` and `clock_divider` read exactly the bit or field their
  definition names, for every word.

The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
checker, so it needs network access. CI uses version 0.68.0, and another version may verify
differently.

Not proved: that a plan's order is the one the silicon needs, which only a board shows; the
specifier decoders when the lookup answers differently (another provider, another cell count, an
error), or for property lengths that are not a whole number of specifiers; plans of more than three
specifiers, so the truncation flag; and the I2C timing and SD divider arithmetic. The host tests
cover those cases by example. Its host tests also pin every offset against both the mainline Linux
and vendor U-Boot numbering, the order of each plan, the rate chain and the I2C timing formula.

## The two device trees

Mainline Linux names one controller node per domain (`starfive,jh7110-stgcrg`, `-aoncrg`,
`-syscrg`). The vendor U-Boot tree a VisionFive 2 boots with names one clock node
(`starfive,jh7110-clkgen`) and one reset node (`starfive,jh7110-reset`), each carrying several
windows told apart by `reg-names`. `SYSTEM_TOP_GROUP_WINDOW`, `ALWAYS_ON_WINDOW` and `SYSTEM_WINDOW`
list those spellings in the order to try them. Find the first node carrying one, and hand its `reg`
entries and `reg-names` to `WindowLocation::from_regions`, which picks the window by name, never by
position. A tree that names none gets `RegisterWindow::fallback`, the address both trees agree on,
with `from_tree: false`. Do not store through a fallback on a machine your tree does not name as a
JH7110.

The PMIC is `x-powers,axp15060` in mainline and `stf,axp15060-regulator` in the vendor tree
(`POWER_MANAGEMENT_CHIP_COMPATIBLES`). `PowerManagementChipBus::from_nodes` and
`DevicePlan::from_specifiers` turn a node's `clocks` and `resets` into steps, calling back for each
phandle's `#clock-cells`, `#reset-cells` and `compatible`. A specifier for another provider, or an
id past the domain's bounds (the vendor tree's virtual clock 298), is counted in `skipped` and never
becomes a store.

## BUGS

- None of the bring-up has been confirmed on silicon end to end. In particular, the PMIC reset path
  (`standard_mode_100_kilohertz` at the input clock `inter_integrated_circuit_5_clock_frequency`
  computes, and `POWER_MANAGEMENT_CHIP_RESET_REGISTER` with its reset bit) has been checked on the
  host only; no board has run the timing this release computes, which is 198 high counts and 297 low
  counts at 49.5 MHz. The crate does not claim it is proven on a board.
- `Step`, `BringUpReport`, `PowerManagementChipBus`, `DevicePlan` and `WindowLocation` are
  `#[non_exhaustive]`, so a later release can add a step kind or a field without a breaking change.
  Build a `BringUpReport` with `BringUpReport::default()` and assign its fields, and match a `Step`
  with a wildcard arm.
- The reset `SYSTEM_TOP_GROUP_RESET_SECURITY_ADVANCED_HIGH_PERFORMANCE_BUS` is shared with the
  direct memory access (DMA) engine, so the crate offers no assert.
- Some items describe one board and not the chip: the PMIC reset register and bit, the PMIC bus
  plan and the I2C5 clock chain follow the VisionFive 2's AXP15060 wiring.
- Only the STG, AON and SYS domains are described, each only as far as a device needed it.

## Minimum Rust version

Rust 1.88.

## License

MIT OR Apache-2.0, at your option: see `LICENSE-MIT` and `LICENSE-APACHE`.
