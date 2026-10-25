# jh7110_clock_and_reset

The logic of the clock and reset generator in the StarFive JH7110, the RISC-V system-on-chip of
the VisionFive 2. It holds the register offsets and bit fields of the STG, AON and SYS domains,
the bring-up plans the TRNG, the Ethernet controller, both SD/MMC controllers and the PMIC's I2C
bus need, the I2C input-clock arithmetic, and what each of the chip's two device trees calls the
controllers. It never touches a register: you map the window and do the volatile stores, and this
crate says which word, which bit and in what order. It is `no_std`, depends on nothing, and is
tested on the host. It was written for the nife kernel.

## What is proven

Nothing yet. The crate has no Kani harnesses, so `cargo kani` has nothing to prove. Its host tests
pin every offset against both the mainline Linux and vendor U-Boot numbering, the order of each
plan (every clock before any reset, a parent chosen while its clock is gated), the rate chain and
the I2C timing formula, and the decoders on hand-built property values.

## The two device trees

Mainline Linux names one controller node per domain (`starfive,jh7110-stgcrg`, `-aoncrg`,
`-syscrg`). The vendor U-Boot tree a VisionFive 2 boots with names one clock node
(`starfive,jh7110-clkgen`) and one reset node (`starfive,jh7110-reset`), each carrying several
windows told apart by `reg-names`. `STG_WINDOW`, `AON_WINDOW` and `SYS_WINDOW` list those
spellings in the order to try them. Find the first node carrying one, and hand its `reg` entries
and `reg-names` to `Found::from_regions`, which picks the window by name, never by position. A tree
that names none gets `Window::fallback`, the address both trees agree on, with `from_tree: false`.
Do not store through a fallback on a machine your tree does not name as a JH7110.

The PMIC is `x-powers,axp15060` in mainline and `stf,axp15060-regulator` in the vendor tree
(`PMIC_COMPATIBLES`). `PmicBus::from_bus` and `DevicePlan::from_specifiers` turn a node's `clocks`
and `resets` into steps, calling back for each phandle's `#clock-cells`, `#reset-cells` and
`compatible`. A specifier for another provider, or an id past the domain's bounds (the vendor
tree's virtual clock 298), is counted in `skipped` and never becomes a store.

## Usage

```rust
use jh7110_clock_and_reset::{STG, STG_WINDOW, Found, Step, TRNG_BRING_UP, is_deasserted};

// The vendor clkgen node: three windows, found by name.
let regions = [(0x1302_0000, 0x1_0000), (0x1023_0000, 0x1_0000), (0x1700_0000, 0x1_0000)];
let stg = Found::from_regions(&STG_WINDOW.spellings[1], &regions, Some(b"sys\0stg\0aon\0"))
    .unwrap_or(STG_WINDOW.fallback());
assert_eq!(stg.base, 0x1023_0000);

// The TRNG's plan: two clocks, then one reset.
assert_eq!(TRNG_BRING_UP[2], Step::DeassertReset(3));
assert_eq!(STG.clock_offset(15), Some(0x3c));
let reset = STG.reset_bit(3).unwrap();
assert!(is_deasserted(0b1000, reset.mask));
```

Walk a plan in order: for `EnableClock(i)` set `CLOCK_ENABLE` in the word at
`domain.clock_offset(i)`, for `SelectParent` write `with_parent`, and for `DeassertReset(id)` clear
the bit at `reset_bit(id).assert_offset`, then poll `status_offset` until `is_deasserted`. Keep the
words before and after in a `Report`, which tells "already up" apart from "this run started it".

## Limitations

- None of the bring-up has been confirmed on silicon end to end.
- The reset `STGRST_SEC_AHB` is shared with the DMA engine, so the crate offers no assert.
- Only the STG, AON and SYS domains are described, each only as far as a device needed it.

Minimum Rust version: 1.88.

## License

MIT OR Apache-2.0, at your option: see `LICENSE-MIT` and `LICENSE-APACHE`.
