# jh7110_clock_and_reset_contract

The words a client and a StarFive JH7110 clock and reset server exchange to bring one device's
clocks and resets up. `no_std`, no dependencies, no system calls of its own: the caller supplies the
`CALL`.

The server holds the shared clock and reset windows. A client holds an endpoint capability whose
badge is its device's register base physical address, and the server answers for that device alone.
No request word names a device, and no operation turns one off.

## The contract

A `CALL` carries two request words and returns two reply words.

- Request word 0: the operation in bits 63:56 (`OPERATION_SHIFT` is 56), and for the reads the
  device's own clock or reset index in bits 7:0. Build it with `request`, read it with `operation`
  and `index`. Request word 1: `SET_RATE`'s most hertz.
- The operations: `VERSION_QUERY` (0, reserved), `BRING_UP` (1), `READ_CLOCK` (2), `READ_RESET`
  (3) and `SET_RATE` (4).
- Reply word 0: a `Status`, 0 to 7. A kernel failure of the `CALL` is a small negative number,
  enormous as a `u64`, so `Status::from_word` answers `None` for it.
- Reply word 1: the bring-up `Summary`, the raw clock or reset word, or the hertz the rated clock
  now runs at.

## EXAMPLES

```rust
use jh7110_clock_and_reset_contract::{BRING_UP, Status, Summary, operation, request};

// What a server does with a bring-up whose clocks all came up and whose one reset stayed held.
fn server(w0: u64) -> (u64, u64) {
    assert_eq!(operation(w0), BRING_UP);
    let summary = Summary {
        clocks_in_plan: 2,
        clocks_running: 2,
        resets_in_plan: 1,
        resets_released: 0,
        already_up: false,
        refused: 0,
    };
    (summary.status().to_word(), summary.pack())
}

let (status, word) = server(request(BRING_UP, 0));
assert_eq!(Status::from_word(status), Some(Status::ResetHeld));
assert_eq!(Summary::unpack(word).map(|s| s.resets_released), Some(0));
```

## What is proven

Host tests pin every number the wire carries: the interface id and version, the operation numbers
and their bit position, each status's word, and each summary field's place, with a stray bit
refused.

## BUGS

The server answers `VERSION_QUERY` with `UnknownOperation` until the version query exists.

## Minimum Rust version

Rust 1.88.

## License

MIT OR Apache-2.0.
