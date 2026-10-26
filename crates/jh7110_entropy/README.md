# jh7110_entropy

The logic of a driver for the true random number generator (TRNG) in the StarFive JH7110, the
RISC-V system-on-chip of the VisionFive 2. It holds the register layout, the decision of whether a
generation succeeded, failed or is still running, a small pool that serves the 32 bytes of one
generation without serving any byte twice, and what each of the chip's two device trees calls the
block. It never touches a register: you map the window and do the volatile reads, and this crate
decides what they mean. It is `no_std` and tested on the host. It was written for the nife kernel.

## What is proven

Four Kani harnesses exhaust `interpret`, the function that turns one `(STAT, ISTAT)` snapshot and
the eight `RAND` words into an answer:

- `verification::a_lockup_bit_is_never_overridden`: whenever `ISTAT.LFSR_LOCKUP` is set, the answer
  is `Lockup`, whatever else the register says.
- `verification::ready_requires_rand_rdy_and_carries_the_words_untouched`: `Ready` comes only from
  `RAND_RDY` on a seeded core, and its 32 bytes are the eight words, little-endian, word 0 first.
- `verification::neither_bit_set_is_always_not_ready`: a seeded core with neither bit set is
  `NotReady`.
- `verification::an_unseeded_core_never_yields_bytes`: with `STAT.SEEDED` clear, no `ISTAT` value
  yields bytes.

Rerun them with `cargo kani`. Each harness has a falsification record at
`falsifications/<module.path>.<harness>.patch`; applying it with `patch -p3` from the crate root
makes that harness fail at its own assertion, which shows the proof checks something.

Not proven: the quality of the randomness, which is a property of the silicon; `Pool` and
`assemble`, which copy words and are tested; and the device-tree decoders, which are tested.

## The two device trees

Mainline Linux names the block `starfive,jh7110-trng`. The vendor U-Boot's control tree, which a
VisionFive 2 boots with, names the same block `starfive,trng`, at the same window (`0x1600_C000`,
`0x4000` bytes) and interrupt (30), and marks it `status = "disabled"` because U-Boot has no driver
for it. `COMPATIBLES` lists both, mainline's first. Walk your tree for a node carrying one, read
its `reg`, decode `interrupts` with `interrupt` and `status` with `status_okay`, and keep the
result in a `Discovered`. A disabled node is the first suspect when the window reads all zeros.

## Usage

```rust
use jh7110_entropy::{Outcome, interpret, status_okay, ISTAT_RAND_RDY, STAT_SEEDED};

// Firmware that says nothing about `status` means "okay".
assert!(status_okay(None));
assert!(!status_okay(Some(b"disabled\0")));

// A seeded core with RAND_RDY set: the eight words, little-endian, word 0 first.
let rand = [0x0403_0201, 0, 0, 0, 0, 0, 0, 0];
match interpret(STAT_SEEDED, ISTAT_RAND_RDY, rand) {
    Outcome::Ready(bytes) => assert_eq!(&bytes[..4], &[1, 2, 3, 4]),
    other => panic!("expected Ready, got {other:?}"),
}
```

A driver writes `MODE_R256` during bring-up (otherwise only four words carry the answer), clears
`ISTAT` with `ISTAT_ALL`, asks for a reseed with `CTRL_EXEC_RANDRESEED`, then for each request
writes `CTRL_GENE_RANDNUM`, polls `STAT` and `ISTAT`, and hands them to `interpret`. On `Lockup` or
`Unseeded` it reseeds and retries, bounded.

## Limitations

- The TRNG needs its clocks ungated and its reset released first. This crate does not do that;
  the `jh7110_clock_and_reset` crate holds the plan, or your platform must do it some other way.
- None of it has run on silicon with a verified answer.
- The device-tree fixtures it was tested against are excerpts of the mainline and vendor trees.

Minimum Rust version: 1.88.

## License

MIT OR Apache-2.0, at your option: see `LICENSE-MIT` and `LICENSE-APACHE`.
