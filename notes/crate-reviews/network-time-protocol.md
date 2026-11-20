# Review package: `network_time_protocol`

*Name: provisional, minted by lane 1102-renames-package. The `network_time_protocol` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to ratify the crate's names and to clear it for its move to its own
repository, as part of [milestone 1102 (`network_time_protocol`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1102-network-time-protocol-outside-review.md).
It describes the crate as `main` plus this branch leaves it. It ratifies nothing and clears nothing.

## In one paragraph

The NTPv4 wire format of RFC 5905 as pure computation. It holds the 48-byte packet, the 1900-epoch
fixed-point timestamp with its era pivot, the offset and delay arithmetic, and the response checks
that are the whole of plain NTP's spoofing resistance. It owns no socket, no clock and no policy.
A `Query` carries a 64-bit nonce in the transmit field and `accept` refuses any reply whose origin
field differs; the crate has no constructor that sends a guessable timestamp. It is `no_std`, has
`#![forbid(unsafe_code)]` and has no dependency of any kind. Nine Kani harnesses, each with a
falsification patch, prove the round trip, the era pivot, totality of decoding and of the interval
accessors, and the nonce check.
Nothing in its source points back into nife except the manifest's header comment and the crate-root
milestone and section citations, which the move rewrites.

## Identity

| item | value |
|---|---|
| crate | `network_time_protocol`, ratified 2026-10-18 ("the standard's own full name"; the one exception to the `_contract` suffix rule) |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024; the workspace sets 1.88 and the move writes it into the manifest |
| license | `license.workspace = true` today (`MIT OR Apache-2.0`); the move inlines it and writes both texts |
| `publish` | `false`; the repository keeps it so until the release milestone |
| dependencies | none. `clock_contract` is gone (Decision 1, below) |
| size | `src/lib.rs` of 1,238 lines (library and host tests) and `src/verification.rs` of 187 lines (Kani, under `cfg(kani)`) |

Decision 1 removed the crate's only dependency, so the pin that lane PR #3781 adds for
`nifeos/clock_contract` is not needed by this crate. That pin serves other dependents of
`clock_contract` and can proceed without this crate.

## Who in nife depends on it

Each by a path dependency with `version = "0.1.0"`:

- `components`, for `components/src/network_time_client.rs` (the client program);
- `fixtures`, for `fixtures/src/network_time_test_server.rs` (the test server);
- `system_tests` (development only), for `system_tests/src/user/ntp_tests.rs`.

`kernel/src/user/ntp_service.rs` names `Reject` in a doc comment and reaches the crate through
`components`. `packages/time.package.toml` lists the crate. The workspace `Cargo.toml` sets
`[profile.dev.package.network_time_protocol] opt-level = 2`, which the Kani and exhaustive tests
need. Four rows of `helpers/template_report_baseline.txt`, one row of `.cargo/mutants-baseline.txt`
and the lists in `helpers/` that name crates (the move lane's list from `script/crate-move`) refer to it.

What the crate references back into nife:

- Its manifest header comment cites milestone 51 (Wall-clock time, the `date` command, and an NTP
  service) and `notes/ntp.md`.
- Its sources cite milestones and sections. `script/crate-move --dry-run` counts four milestone
  citations, nine nife paths and four section citations.
- Its manifest inherits `license` and `[lints]` from the workspace.

## What was run

All on the branch tip, on the lane's Linux host.

| check | result |
|---|---|
| `cargo test -p network_time_protocol` | 27 unit tests and 2 doctests pass |
| `cargo clippy -p network_time_protocol --all-targets -- -D warnings` | no warning |
| `cargo doc -p network_time_protocol --no-deps` with warnings denied | no warning |
| `cargo build --target <t> -p components` for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none` | all three build, with the kernel's callers on the new names |
| MSRV 1.88: a copy of the crate outside the tree, manifest inlined | 27 unit tests and 2 doctests pass |
| `cargo miri test -p network_time_protocol` | passes, doctests included |
| `cargo build --manifest-path fuzz/Cargo.toml` (nife's fuzz workspace) | builds |
| `script/crate-move --dry-run --into <dir>` | stages at template `v8`; the Name blocks strip, the stripped crate root passes `rustfmt --check`, 9 patches regenerate for `-p1` and apply with no offset and no fuzz. Seven findings, all the move lane's (below) |
| `script/lint --quick` | all sections pass |
| `script/verify --only network_time_protocol` (Kani 0.68.0) | all nine harnesses verify, in 26 s of proving |
| the new harness's falsification patch, applied alone | both sign assertions of `the_interval_accessors_are_total` fail |
| `cargo llvm-cov -p network_time_protocol` at the workspace's `opt-level = 2` | 485 of 495 lines of `src/lib.rs`, 97 per cent, over the template floor of 85 |
| `script/crate-standalone --no-kani network_time_protocol` | ten failures, each an item below: 54 names not yet ratified (the first is `PACKET_BYTES`), the manifest's inherited `license` and `lints` and its missing `rust-version`, no README and no license texts, and nife citations in the manifest and `src/lib.rs`. The build passes on stable for the host and the three bare-metal targets |
| `script/falsifications` patch replay of `verification.accepting_is_total_and_a_sample_is_coherent.patch` | applies with `patch -p3 --dry-run -F0` from the crate directory; the harness-and-patch pairing section of `script/lint` passes |

Not run, and so not claimed: the stranger test, the template's workflows, the first weekly
mutation run and `cargo-semver-checks` (a first release has no baseline). Mutation last ran in the 2026-10 census; the baseline row records 5 missed of 166 caught,
and the renames change no behavior.

## Public API

| kind | items |
|---|---|
| constants | `PORT` (123), `PACKET_BYTES` (48), `VERSION` (4), `UNIX_EPOCH_DELTA_SECONDS`, `MINIMUM_UNIX_SECONDS`, `MAXIMUM_UNIX_SECONDS` (inclusive), `MAXIMUM_STRATUM` (15), `MAXIMUM_ROOT_DISTANCE`, `DEFAULT_POLL_EXPONENT` (6) |
| `mode` | `SYMMETRIC_ACTIVE`, `SYMMETRIC_PASSIVE`, `CLIENT`, `SERVER`, `BROADCAST` |
| `leap` | `NONE`, `INSERT`, `DELETE`, `UNSYNCHRONIZED` |
| `Timestamp` | `ZERO`, `from_bits`, `bits`, `seconds`, `fraction`, `is_zero`, `from_unix(seconds, nanoseconds) -> Option`, `to_unix`, `since -> Interval` |
| `ShortFormat` | private field; `from_bits`, `bits`, `nanoseconds`, `seconds` |
| `Interval` | private field; `ZERO`, `from_ticks`, `ticks`, `nanoseconds`, `microseconds`, `is_negative` |
| `Packet` | public fields `leap`, `version`, `mode`, `stratum`, `poll`, `precision`, `root_delay`, `root_dispersion`, `reference_id`, `reference`, `origin`, `receive`, `transmit`; `parse`, `to_bytes` |
| `Reject` | `Length(usize)`, `Version(u8)`, `Mode(u8)`, `KissOfDeath([u8; 4])`, `Stratum(u8)`, `Unsynchronized`, `OriginMismatch`, `TransmitZero`, `ServerOrderReversed`, `ClientOrderReversed`, `NegativeDelay`, `RootDistance`, `ReferenceAfterTransmit` |
| `Sample` | `offset`, `delay`, `response` |
| `Query` | `with_nonce`, `with_poll`, `nonce`, `sent`, `request`, `accept` |

No nife type, capability or handoff appears in a public signature. `Query::new`, the low-bit
randomizer (Decision 3) and `Interval::abs_ticks` (Decision 2) no longer exist.

What a stranger may find surprising:

- Mode, leap indicator and stratum are `u8` constants in modules, not enums, so a value outside the
  RFC's list parses and a caller can inspect it.
- `Query::accept` checks the packet but does not remember the nonce: a caller that sends twice must
  not reuse a nonce, and the documentation says so.
- `Timestamp::from_unix` returns `None` outside 1970 to 2104 and never an aliased answer.

## Proofs

Nine Kani harnesses, each with a falsification patch under `falsifications/`:

- The era pivot is exact over every Unix second in the window.
- Nanoseconds survive the fixed point.
- Decoding any wire value is total.
- `from_unix` refuses exactly the out-of-range inputs.
- `parse` then `to_bytes` is the identity over all 2^384 packets.
- An unmatched origin is always rejected.
- `accept` is total, and a sample is coherent.
- `accept` takes a consistent exchange.
- `Interval`'s nanosecond and microsecond accessors never overflow over every `i128` tick count,
  and keep the interval's sign.

The standalone run's result is in the checks table. `notes/ntp/what-is-proved.md` states each
claim, and the one place where a solver was the wrong tool.

## Names

Every public name with its status and a proposal. "Provisional" means the source carries a
`Name: provisional.` block. No name reads ratified; the renamed ones stay provisional until the
architect ratifies them in one pass. The move strips every `Name:` block, so each name must read
ratified first.

| name | kind | status | proposal |
|---|---|---|---|
| `network_time_protocol` | crate | ratified 2026-10-18 | keep |
| `PORT` | constant | no block | ratify: the registered port; a spelled-out `UDP_PORT` is a possible alternative |
| `PACKET_BYTES` | constant | provisional, replaces `PACKET_LEN` (Decision 2) | ratify: the unit is in the name |
| `VERSION` | constant | no block | ratify |
| `UNIX_EPOCH_DELTA_SECONDS` | constant | provisional, replaces `UNIX_DELTA` | ratify |
| `MINIMUM_UNIX_SECONDS` | constant | provisional, replaces `UNIX_MINIMUM`; the ratified name carried no unit | ratify |
| `MAXIMUM_UNIX_SECONDS` | constant | provisional, replaces `UNIX_LIMIT`; inclusive | ratify |
| `MAXIMUM_STRATUM` | constant | recorded (RFC 5905 `MAXSTRAT`) | ratify |
| `MAXIMUM_ROOT_DISTANCE` | constant | recorded (RFC 5905 `MAXDISP`); now a `ShortFormat` built with `from_bits` | ratify |
| `DEFAULT_POLL_EXPONENT` | constant | provisional, replaces `DEFAULT_POLL`; the poll is a base-2 exponent | ratify |
| `mode`, `mode::*` | module, constants | no block | ratify: the RFC's mode names |
| `leap`, `leap::NONE`, `INSERT`, `DELETE` | module, constants | no block | ratify |
| `leap::UNSYNCHRONIZED` | constant | recorded (RFC 5905 leap indicator 3), American spelling, replaces `UNSYNCHRONISED` | ratify |
| `Reject::Unsynchronized` | variant | replaces `Unsynchronised` | ratify |
| `Reject` (other variants) | enum | no block | ratify as built |
| `Timestamp` and its methods | type | no block, except `fraction` (provisional) | ratify; `fraction` as built |
| `ShortFormat` | type | provisional, replaces `Short`; private field (Decision 2) | ratify: the RFC's term is "NTP short format" |
| `ShortFormat::from_bits`, `bits` | methods | provisional, new; they match `Timestamp` and `Interval` | ratify |
| `Interval` and its methods | type | no block | ratify; `abs_ticks` leaves |
| `Packet` and its fields | type | no block | ratify |
| `Sample` | type | no block | ratify |
| `Query`, `Query::with_nonce`, `with_poll`, `nonce`, `sent`, `request`, `accept` | type, methods | no block | ratify; `Query::new` leaves |

No alias carries an old spelling (the tree's rule before a first release). The `Packet` field
`leap`, `version`, `mode`, `stratum`, `poll`, `precision`, `reference_id` and the `Timestamp`
fields `origin`, `receive` and `transmit` use the RFC's words. `poll` and `precision` are base-2
exponents rather than seconds; the documentation says so and the unit is not part of the name.

## License

`MIT OR Apache-2.0`, the workspace's. The crate adapts no source. It follows RFC 5905 as a
specification; the RFC's reference implementation supplies two checks, and the crate copies no text.

## What still blocks the move

| item | owner |
|---|---|
| P0-1: inline `license`, `rust-version` and `[lints]` in the manifest; drop the nife publish-allowlist comment | move lane |
| P0-2: write the README, both license texts, the changelog and a stranger record | move lane |
| P1-8: remove the milestone, section and path citations from `src/`, the manifest and `notes/ntp.md` references | move lane |
| P1-9: the fuzz opt-in and target (the crate is safe Rust, so the opt-in applies) | move lane |
| P1-10 and P1-11: the exhaustive-sweep profile in the new manifest and the mutation exemptions file | move lane |
| Convert the three path dependencies to `git` and `rev` with `version = "0.1.0"`; drop `crates/network_time_protocol` from the workspace; delete the baseline rows that name it | move lane |
| Rewrite the provisional `Name:` blocks to ratified after the architect's pass | a later lane |

Narrowing `Interval::from_ticks` to `i64` is a two-way door the maintainer may rule before the
first publish; the accessors are already proved total without it. The milestone's later items
stay open.

## Open questions

1. Whether `PORT` should read `UDP_PORT`. Recommendation: keep `PORT`; the module is the NTP
   crate and the RFC names it "port 123". The maintainer can decide it.

## Recommendation

Ready for review.
