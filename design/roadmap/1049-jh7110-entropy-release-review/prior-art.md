# How other systems handle a hardware entropy source

An appendix to [the milestone](../1049-jh7110-entropy-release-review.md).

The review read the cited tree for rows marked *read*. Rows marked *recalled* come from memory.

| system | what it does that this crate should know | how |
|---|---|---|
| Linux `jh7110-trng.c` (*read*) | Interrupt-driven. `wait_idle` polls `STAT` for `RAND_GENERATING` and `RAND_SEEDING` clear with a 100 ms timeout before any command; a generate waits up to 1 ms (40 µs when the caller will not wait); the interrupt handler acknowledges each `ISTAT` bit by writing it back and issues a reseed on `LFSR_LOCKUP`; init writes `AUTO_AGE` and `AUTO_RQSTS` (default 0), clears `ISTAT`, enables all interrupts, sets `MODE.R256`, and reseeds. The read path clamps to 16 bytes in 128-bit mode and 32 in 256-bit mode. It declares no `quality` to the hwrng core. | items 6, 7; the bench timing checks |
| NetBSD `jh7110_trng.c` (*read*) | Polling. Reads `RAND_RDY` only when `STAT.SEEDED` is set and reseeds otherwise; acknowledges `RAND_RDY` before reading the eight words; `explicit_memset`s the words after handing them to the entropy pool; init masks interrupts and writes `~0` to `ISTAT`; names the `FEATURES` and `BCONF` field bits and the `RQST_LOCKUP` interrupt bit the crate calls the request alarm. Feeds the words to `rnd_add_data_sync` claiming full entropy. | items 8, 16 |
| Linux hwrng core and rng-tools (*recalled*) | The kernel mixes hardware output into its pool through a conditioning function and trusts a per-mille `quality` the driver declares; the FIPS 140-2 continuous tests run in userspace `rngd`, not in the driver. | the health-test question stays open, as the crate says |
| NIST SP 800-90B (*recalled*) | The repetition count test and the adaptive proportion test are the two continuous health tests a noise source is expected to carry; neither the manual nor any cited driver claims one for this block. | the open question in the crate's doc comment on health tests |
| rand_core 0.10.1 (*read*) | `TryRng` with `type Error: core::error::Error`, `try_next_u32`, `try_next_u64`, `try_fill_bytes`; `Rng` for the infallible case; `CryptoRng` and `TryCryptoRng` as marker traits; `RngCore` and `TryRngCore` kept for compatibility. An embedded driver crate commonly implements `TryRng` behind a feature. | item 17 (declined) |
| getrandom (*read*, README) | A bare-metal system routes `getrandom` to its own source through the opt-in `custom` backend (`getrandom_backend` configuration flag); a library cannot do this for its users. | nothing for the crate to do; the README can say so |
| esp-hal, embassy-stm32, atsamd-hal, nrf-hal (*recalled*) | Each SoC HAL exposes its TRNG as a type implementing rand_core's trait, blocking or fallible; embassy-stm32's RNG recovers from the hardware's seed and clock error bits and does not serve bytes past them. | items 6, 7, 17 |
| embedded-hal 1.0 (*recalled*) | Carries no RNG trait; the ecosystem converged on rand_core. | item 17 |
| Tock `Entropy32` and Zephyr `entropy` (*recalled*) | Both define an OS-side entropy interface that a driver fills; neither is reachable from a Rust crate without the OS. | none |
| `jh71xx-pac`, `jh71xx-hal` (Codeberg, GPL-3.0-only; found by milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own)) | Register maps for the JH7110 family with no TRNG logic, under a license this crate cannot take. | none; this crate stays the only Rust TRNG logic for the chip found |

A user would miss three things today. One is a `TryRng` implementation (item 17, declined). Another is a decoded answer to "what width is this block in" (items 6, 16). The last is a measured poll bound (the bench).

Three things are unique. Kani exhausts the decision function with replayable falsifications. Proofs rather than comments state the seeded gate and the lockup precedence. Both device-tree spellings travel with the register map.

