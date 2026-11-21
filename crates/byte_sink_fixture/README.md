# byte_sink_fixture

Test data for the byte-sink contract, in one place so the programs that touch it agree: the bytes
an indifferent writer writes, the name of the file a file sink writes them into, and the sentinels
a sink and a writer report. `no_std`. Its one dependency is `byte_sink_contract`, whose `Sent` it
reports. It stays in nife and is not published.

It is not a wire contract; the framing is in `byte_sink_contract`.

## EXAMPLES

```rust
use byte_sink_contract::Sent;
use byte_sink_fixture::code;

assert_ne!(code(Sent::Gone), code(Sent::NoSink));
```

## What is proven

Nothing is proven. A test checks that the three report codes differ.

## BUGS

- None known.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
