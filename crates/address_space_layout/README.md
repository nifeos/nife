# address_space_layout

Which band of a process's low half holds what: eight bands that tile the first 2 GiB, the same on
aarch64, riscv64 and x86_64, with `const fn` checks that stop the build when a page or a window is
placed outside its band.

Several programs must agree on where things go in a process: a linker script, two loaders, a
runtime, a kernel and every fixture that shares a page. This crate is the one table they read, so a
new page is placed by the table and not by choosing a number beside the others. The crate is
`no_std`, has no dependencies and is dual licensed `MIT OR Apache-2.0`. It is proven: every property
it claims is a Kani harness in the tree, and every harness carries a falsification patch that shows
what turns it red.

## EXAMPLES

Place a pair page and a whole window in a `const`, so a wrong address stops the build:

```rust
use address_space_layout::{pair_page, window, SERVICE_WINDOWS};

// One page of a builder's shared window, checked against PAIR_PAGES.
const CONTROL_PAGE: u64 = pair_page(0x60_0000);
// An 8 MiB block channel: all of it, not only its first page, lies in SERVICE_WINDOWS.
const BLOCK_CHANNEL: u64 = window(SERVICE_WINDOWS, 0x5000_0000, 8 << 20);

assert_eq!(CONTROL_PAGE, 0x60_0000);
assert_eq!(BLOCK_CHANNEL, 0x5000_0000);
```

Ask whether a program image fits, and print why it does not:

```rust
use address_space_layout::{check_image, IMAGE};

assert!(check_image(IMAGE.start, IMAGE.start + 0x29_c000).is_ok());
let refusal = check_image(IMAGE.start, IMAGE.end + 0x1000).unwrap_err();
println!("{refusal}");
// the program image is too large: it ends at 0x7f001000, past the stack's base at 0x7f000000;
// an image may be at most 496 MiB
```

## What is proven

Eleven Kani harnesses state the layout per address and per window:

- `every_address_below_the_ceiling_lies_in_exactly_one_band`: every address below 2 GiB is in one
  band and no address above it is in any, each band is page aligned, and the whole map lies in the
  user half of all three architectures.
- `the_bands_are_listed_in_address_order`: `BANDS` is in address order.
- `pair_page_accepts_every_page_of_its_band`, `runtime_window_accepts_every_page_of_its_band` and
  `service_window_accepts_every_page_of_its_band`: each band function accepts every page of its
  band, returns it unchanged, and the page lies in no other band.
- `pair_page_refuses_every_address_its_doc_forbids`, `runtime_window_refuses_every_address_its_doc_forbids`
  and `service_window_refuses_every_address_its_doc_forbids`: each refuses an unaligned address or
  one outside its band, so the build stops.
- `window_accepts_every_whole_window_its_band_holds` and
  `window_refuses_every_malformed_or_overrunning_window`: `window` accepts a page aligned, non-empty window
  that lies wholly inside its band, and refuses an unaligned, empty, wrapping or overrunning one.
- `check_image_places_exactly_what_the_image_band_holds`: for every pair of bounds, `check_image`
  places the image, calls it too large, or calls it outside the band, as its documentation says.

Each harness has a falsification record at
`falsifications/<module>.<harness>.patch`; applying it with `git apply -p1` from the crate root
makes that harness fail at its own assertion, which shows the proof checks something. The patches
need a git checkout, and you revert each one with `git checkout -- src` before the next.

Not proven: that two windows in one band do not overlap. The crate that places them asserts that.

## The bands

| band | start | end | size | what goes in it |
|---|---|---|---|---|
| `NULL_GUARD` | `0x0000_0000` | `0x0020_0000` | 2 MiB | nothing, ever |
| `PAIR_PAGES` | `0x0020_0000` | `0x1000_0000` | 254 MiB | pages one builder agrees with the one program it builds, and a program's private windows |
| `RUNTIME_WINDOWS` | `0x1000_0000` | `0x4000_0000` | 768 MiB | windows a loader or runtime places for a program |
| `HEAP` | `0x4000_0000` | `0x5000_0000` | 256 MiB | the heap and nothing else |
| `SERVICE_WINDOWS` | `0x5000_0000` | `0x6000_0000` | 256 MiB | large windows a service contract names |
| `IMAGE` | `0x6000_0000` | `0x7F00_0000` | 496 MiB | the program's code and data |
| `STACK` | `0x7F00_0000` | `0x7FFF_0000` | 16 MiB less 64 KiB | the stack, growing down; its lowest page is never mapped |
| `PROCESS_PAGES` | `0x7FFF_0000` | `0x8000_0000` | 64 KiB | pages the kernel maps into every process unasked |

The crate's root documentation, which `cargo doc --open` shows, holds the same table, the order in
which to ask where a new page goes, and the reasons for the numbers.

## Build, test and prove

You need a Rust toolchain at or above the minimum version at the foot of this page (`rustup` is the
usual way to get one) and a git checkout. From the crate root:

- `cargo build` and `cargo test` run on the host and need nothing else. The tests are unit tests
  and two doctests.
- The crate also builds for bare metal. Add the three targets with `rustup target add
  riscv64imac-unknown-none-elf aarch64-unknown-none-softfloat x86_64-unknown-none`. Then run
  `cargo build --lib --target <target>` for each.
- The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
  kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
  checker, so it needs network access. After that, `cargo kani` runs all eleven harnesses in about
  fifteen seconds on one development machine, most of it compiling. A passing run prints a long
  list of checks, some marked `UNREACHABLE` or `SUCCESS` for standard library code, and one summary
  line per harness; only the final `VERIFICATION:- SUCCESSFUL` and the line `Complete - 11
  successfully verified harnesses` matter.

## BUGS

- Pair pages are checked for membership, not for collisions within one process; a program with
  several windows asserts their disjointness itself.
- `pair_page`, `runtime_window` and `service_window` check one page. A window of many pages is
  written with `window`; one written with a band function alone can run into the next band unseen.
- `window` checks a window against its band, not against another window in the same band.
- **The layout is fixed.** Every band and every fixed window sits at one address in every process
  on every architecture, so there is no address-space layout randomization. The design buys a pair
  page checked at compile time and one linker script; it gives up the unpredictability that a
  randomizing system offers an attacker's guess.
- Other crates restate some of these numbers, pinned by their own tests.
- A loader that reports a too-large image as "could not spawn" hides `check_image`'s reason.
- `window` is a provisional name, and so are the address and length constants (`PAGE`,
  `IMAGE_BASE`, `STACK_TOP` and the rest), which do not yet say whether they hold an address or a
  length.

## Minimum Rust version

Rust 1.88.

## License

MIT OR Apache-2.0, at your option: see `LICENSE-MIT` and `LICENSE-APACHE`.
