# clock_contract

The wall clock as a contract between a clock service and everything that reads or sets the time:
the layout of a shared clock page, the small protocol for proposing a new time, and the policy a
proposal must meet. `no_std`, no dependencies outside an optional model-checking build. It was
written for the nife kernel's clock service, its readers and its standard library, which share this
one definition so they cannot drift.

Reading the clock is a read-only mapping of the page. Setting it is a read/write mapping of the same
page. Proposing a time is a message to an endpoint, which the service answers by applying the policy
here. Wall-clock time is the monotonic counter plus an offset, and only the offset is ever written,
so stepping the clock cannot disturb a monotonic `Instant`. Times are nanoseconds since the Unix
epoch in a `u64`, which runs out in 2554.

## The contract

- The page: four 64-bit words (`WORDS`), the rest of the frame reserved and zero. Word 0 is `MAGIC`
  (ASCII `CLOCKv01`), word 1 a sequence number, word 2 the state, word 3 the offset in nanoseconds.
  A frame without the magic reads as the unknown state. `VERSION` (1) is the digits of the magic: a
  reader built for one version reads another's page as no clock at all. A layout break bumps both.
- States (`state`): `UNKNOWN`, `RTC`, `SET`, `SYNCED`. `UNKNOWN` is a real answer. A machine with no
  usable clock must say so; reporting 1970 plus uptime is the failure this contract exists to
  prevent.
- `ClockPage` is the page as seen through one mapping. It is a seqlock, because there are many
  readers, several writers and no lock can cross an address space. `read` never blocks a writer and
  returns one consistent `Reading` (state, offset, generation, where generation counts publishes).
  `publish` claims the sequence with a compare-exchange, so writers exclude each other.
  `new` is `unsafe`: the address must be a mapped, 8-byte-aligned frame that stays mapped.
  A writer killed between its claim and its release leaves every reader spinning.
- The conversion: `wall_ns(offset, monotonic)` saturates rather than wraps, and `offset_for` is its
  inverse.
- Proposals (`propose`, `status`): a request is `req(PROPOSE)` with the proposed time as the second
  word, or `req(STATE)` to ask what the clock knows. The opcode sits in bits 63:56. The reply's first
  word is a `status` (`ACCEPTED`, `REFUSED_IMPLAUSIBLE`, `REFUSED_TOO_FAR_FORWARD`,
  `REFUSED_TOO_FAR_BACKWARD`, `BAD_REQUEST`) and its second the wall-clock nanoseconds in force.
- The policy (`policy::decide`): a proposal outside the window from 2026-01-01 (`NOT_BEFORE_NS`) to
  2100-01-01 (`NOT_AFTER_NS`) is refused as implausible, and the window also judges what a hardware
  clock reports. Once the clock knows the time, one accepted proposal may move it forward by at most
  an hour (`MAXIMUM_STEP_FORWARD_NS`) and backward by at most a second (`MAXIMUM_STEP_BACKWARD_NS`).
  Moving backward makes instants happen twice, which breaks log ordering and expiry. A clock that
  is `UNKNOWN` accepts any plausible proposal. The window is a floor on plausibility, not a claim of
  accuracy.
- `rtc` names the kinds of hardware clock the service can be told about: `NONE`, `PL031`, `GOLDFISH`
  and `CMOS`. The first three name a register layout the service polls; `CMOS` means the value has
  already been read and arrives as data.

## Usage

```rust
use clock_contract::{NS_PER_SECOND, offset_for, policy, state, status, wall_ns};

// The machine has been up for 5 seconds and the service learns the wall time.
let monotonic = 5 * NS_PER_SECOND;
let now = 1_800_000_000 * NS_PER_SECOND;
assert_eq!(policy::decide(state::UNKNOWN, 0, now), status::ACCEPTED);
let offset = offset_for(now, monotonic);
assert_eq!(wall_ns(offset, monotonic), now);

// A proposal a day ahead of a synchronized clock is refused.
assert_eq!(
    policy::decide(state::SYNCED, now, now + 86_400 * NS_PER_SECOND),
    status::REFUSED_TOO_FAR_FORWARD,
);
```

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
RUSTFLAGS="--cfg loom" cargo test --release      # the seqlock under every interleaving loom permits
```

The minimum supported Rust version is 1.88. The loom run checks the seqlock's two memory-ordering
fences against the C11 model, not against ARM's or RISC-V's own models. There are no Kani harnesses.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
