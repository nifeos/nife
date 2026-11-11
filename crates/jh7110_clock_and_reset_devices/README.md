# jh7110_clock_and_reset_devices

Which StarFive JH7110 clocks and resets each device needs, keyed by the physical address its own
registers start at, and the walk that brings one device up. `no_std`, no unsafe code, no pointer:
every read and store goes through a `Windows` the caller implements.

A clock and reset server holds the chip's shared windows and answers each client for the device its
badge names. `Device::at` turns that badge into the device, and the device's methods touch its own
clocks and resets alone.

## EXAMPLES

```rust
use jh7110_clock_and_reset_devices::{Device, DEVICES, WindowName};

// The TRNG's register base, as the device tree's `reg` gives it.
let trng = Device::at(0x1600_c000).expect("the TRNG is in the table");
assert_eq!(trng.parts[0].window, WindowName::SystemTopGroup);
assert!(trng.rated_clock.is_none());

// A base the table does not hold is no device, and an index past the device's own is no clock.
assert!(Device::at(0x1000_0000).is_none());
assert!(trng.clock(u8::MAX).is_none());
assert_eq!(DEVICES.len(), 5);
```

## BUGS

Each device's plan is `jh7110_clock_and_reset`'s constant, never the device tree's `clocks` and
`resets`.

## License

MIT OR Apache-2.0.
