# designware_inter_integrated_circuit

A polled Synopsys DesignWare I2C master for one transaction class: write bytes to a 7-bit target,
optionally read bytes back, one transfer, every wait bounded. `no_std`, no dependencies, no unsafe.
The caller supplies the register file through the `Registers` trait and the standard-mode `Timing`
it computed from the controller's input clock.

## EXAMPLES

```rust
use designware_inter_integrated_circuit::{Controller, Registers, Timing, with_bit};

// A register file over a mapped controller page would use volatile accesses; this one answers
// nothing, so the controller is not a DesignWare and nothing is written.
struct Absent;
impl Registers for Absent {
    fn read(&mut self, _offset: u32) -> u32 {
        0
    }
    fn write(&mut self, _offset: u32, _value: u32) {}
}

let timing = Timing { control: 0x63, high_count: 0xd8, low_count: 0xfd, data_hold: 0x0f };
let mut controller = Controller::new(Absent, 1_000_000);
assert!(controller.read_register(&timing, 0x36, 0x32).is_err());
assert_eq!(with_bit(0x24, 6), 0x64);
```

## What is proven

Host tests run the sequence against a simulated controller and target: the timing and the target
programmed while the controller is disabled, the stop on the last command of a read and of a write,
a read-modify-write that keeps every other bit, and each stuck wait ending at its bound with the
controller disabled.

## BUGS

- Standard mode only; there is no fast mode.
- Every bound is a count of register reads, not a duration.

## Minimum Rust version

Rust 1.88.

## License

MIT OR Apache-2.0.
