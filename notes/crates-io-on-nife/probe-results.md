# The fifty probes: the recipe, the passes and the four failure classes

*Name: provisional.*

An appendix to [notes/crates-io-on-nife.md](../crates-io-on-nife.md), moved there on 2026-10-11
(UTC) under §212 (a prose budget). It holds the measurement itself: how the headline split was corrected twice, the
recipe the harness automates, all fifty results, and each failure class in full.

## How the headline split was corrected

The split is 43/7 today, and it was 39/11 before `entropy_backend` landed (re-derived
2026-08-18, milestone 64 (enough `std` to run somebody else's crate)'s third pass, by `script/crate-probes`, which is now the measurement
rather than the recipe below). Both numbers are one command apart and both are real: bare
`script/crate-probes` gives 43 built and 7 failed, and `script/crate-probes --no-backend` runs
the same fifty without this tree's `getrandom` answer.

The four crates between them are `rand`, `uuid`, `gix-object` and `gix-actor`, which is exactly
class A's remainder, and the second run was checked against those four rather than assumed: all
four still stop at `getrandom`'s `compile_error!` without the backend. So 39 "built with no
change at all" and 43 "built against this tree as it ships", and the old headline is the first
of those, still true and no longer the interesting one. `entropy_backend` is eleven lines and two
lines of consuming config; calling that "a change" and 43 crates "not building" was the reading
that kept the number down.

The number before that was 35/15, and it was wrong for thirteen days (corrected 2026-08-17).
The headline double-counted the four crates that appear in two failure classes at once:
`zip` and `ring` are in class A *and* class C; `gix-config` and `gix` are in class A *and* class
B. Summing the class headings gives 8 + 3 + 3 + 1 = 15, and the distinct crates behind them are
eleven. The tables below were right all along; only the sentence over them was wrong, which is why
nothing caught it. Three hand re-derivations of one measurement produced two wrong headlines,
which is why `script/crate-probes` exists: a measurement a reader cannot re-run is a claim.

## What the harness is doing, which was this recipe

One throwaway crate per probe, built against this checkout's patched std farm by path:

```sh
cargo xtask std-src                      # build/refresh the patched-std farm
mkdir -p /tmp/probe/src && cd /tmp/probe
cat > Cargo.toml <<'EOF'
[package]
name = "probe"
version = "0.1.0"
edition = "2021"
[workspace]
[dependencies]
regex = "1"
[profile.release]
panic = "abort"
EOF
echo 'fn main() { println!("{}", regex::Regex::new("^a+$").unwrap().is_match("aaa")); }' > src/main.rs

RUSTUP_TOOLCHAIN=/path/to/nife/target/nife-farm cargo build --release \
    -Zjson-target-spec \
    -Zbuild-std=core,alloc,std,panic_abort \
    -Zbuild-std-features=compiler-builtins-mem \
    --target /path/to/nife/targets/aarch64-unknown-nife.json
```

Build a `[[bin]]`, not a `[lib]`, and this is not a detail. A library target is never linked, so
a crate whose only blocker is a missing C library passes. `diesel` with the `sqlite` feature compiles
clean and then fails at `rust-lld: error: unable to find library -lsqlite3`. The first pass of this
measurement used libraries and recorded `diesel` as a pass.

And the `main` must CALL the crate, which is the same rule one notch further in and which the
2026-08-17 re-derivation found by tripping over it. A `[[bin]]` whose body is `fn main() {}` declares
the dependency, compiles it, and still does not link it, so `diesel` passed again until the probe was
given `SqliteConnection::establish(":memory:")` to call, at which point `-lsqlite3` came back exactly
as recorded. The rule that covers both: a probe proves nothing the linker was not asked to do.

Set `CARGO_TARGET_DIR` to one shared directory across probes so build-std compiles the patched `std`
once (about 10 seconds) instead of once per crate.

## What happened, by crate

Fifty crates, resolved versions as of 2026-08-04, built for `aarch64-unknown-nife`.

## Built with no change (39)

| crate | version | why it was probed |
|---|---|---|
| `itertools` | 0.14.0 | the floor: pure computation |
| `bitflags` | 2.13.1 | the floor |
| `memchr` | 2.8.3 | the floor |
| `byteorder` | 1.5.0 | the floor |
| `smallvec` | 1.15.2 | the floor |
| `hashbrown` | 0.15.5 | the floor |
| `nom` | 8.0.0 | parser |
| `regex` | 1.13.1 | parser, heavy generics |
| `httparse` | 1.10.1 | wire format |
| `semver` | 1.0.28 | parser |
| `url` | 2.5.8 | parser, 36-crate closure |
| `serde` | 1.0.229 | serialization, derive macros |
| `serde_json` | 1.0.151 | serialization |
| `toml` | 0.9.12 | serialization |
| `csv` | 1.4.0 | serialization over `io::Read` |
| `base64` | 0.22.1 | format |
| `sha2` | 0.10.9 | format |
| `hex` | 0.4.3 | format |
| `flate2` | 1.1.9 | compression (`rust_backend`) |
| `miniz_oxide` | 0.8.9 | compression |
| `log` | 0.4.33 | ubiquitous facade |
| `anyhow` | 1.0.104 | ubiquitous errors |
| `thiserror` | 2.0.19 | ubiquitous errors |
| `once_cell` | 1.21.4 | sync primitive |
| `bytes` | 1.12.1 | buffers |
| `chrono` | 0.4.45 | wall-clock time |
| `time` | 0.3.55 | wall-clock time |
| `tempfile` | 3.27.0 | fs, and see the sting below |
| `walkdir` | 2.5.0 | fs traversal |
| `fs-err` | 3.x | fs, names every call it makes |
| `ignore` | 0.4.33 | fs plus threads |
| `crossbeam-channel` | 0.5.16 | threads |
| `rayon` | 1.12.0 | threads |
| `num_cpus` | 1.17.0 | reports parallelism |
| `mio` | 1.2.2 | (with no features; see `rocket`) |
| `tokio` | 1.53.1 | async runtime, `rt` feature |
| `clap` | 4.6.5 | ubiquitous CLI |
| `tracing` | 0.1.44 | ubiquitous observability |
| `gix-hash` | 0.20.1 | milestone 99 leaf |

Twelve of these were rebuilt as executables that call the crate for real (`serde_json` round
trip, `regex` match, `flate2` gzip round trip, `walkdir` over `.`, `chrono::Utc::now`, `clap` parse,
a `tokio` current-thread `block_on`, `csv` records, `tempfile::NamedTempFile::new`,
`fs_err::read_to_string`, `rayon` parallel sum). All twelve linked. Whether they work is a
different question, answered below and not by this measurement.

## Failed, and why (11 crates, in four classes)

Every failure is one of four classes. The class matters much more than the crate, which is why the
classes are the headings; four crates are in two classes at once, and adding the class headings
up is what produced the wrong 15 above.

| crate | classes |
|---|---|
| `rand`, `uuid`, `gix-object`, `gix-actor` | A only |
| `zip`, `ring` | A, then C |
| `gix-config`, `gix` | A, then B |
| `tar` | B |
| `diesel` | C |
| `rocket` | D |

### Class A: `getrandom` has no `nife` backend (8 of 11), closed 2026-08-17

`rand`, `uuid`, `zip`, `gix-object`, `gix-actor`, `gix-config`, `gix`, `ring` all die on the same
`compile_error!`:

```
error: target is not supported. You may need to define a custom backend see:
       https://docs.rs/getrandom/0.3.4/#custom-backend
```

`getrandom` selects a backend on `target_os`, and there is no `nife` arm. This is not a `std` gap
at all: `std::random::SystemRng` works (milestone 56, slot 6). It is one crate in the ecosystem that
predates us.

It is also the cheapest thing on this list to fix, and that was measured rather than assumed.
`getrandom` 0.3 and 0.4 both document a custom backend: build with
`RUSTFLAGS='--cfg getrandom_backend="custom"'` and define one function. With a stub backend that
fills from `std::random::SystemRng`, six of the eight went from failing to building:

| crate | round 1 | with a custom `getrandom` backend |
|---|---|---|
| `rand` 0.9.5 | FAIL | PASS |
| `uuid` 1.24.0 | FAIL | PASS |
| `gix-object` 0.51.1 | FAIL | PASS |
| `gix-actor` 0.36.1 | FAIL | PASS |
| `gix-features` 0.45 | FAIL | PASS |
| `gix-hash` 0.20.1 | (passed) | PASS |
| `zip` 5.1.1 | FAIL | FAIL (class C: `zstd-sys`) |
| `gix-config` 0.48.0 | FAIL | FAIL (class B: `gix-sec`) |
| `gix` 0.74.1 | FAIL | FAIL (class B: `gix-sec`) |
| `ring` 0.17.14 | FAIL | FAIL (class C: C sources) |

The backend used for the probe:

```rust
#![feature(random)]
use std::random::{Rng, SystemRng};

#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    dest: *mut u8,
    len: usize,
) -> Result<(), getrandom::Error> {
    let buf = unsafe { core::slice::from_raw_parts_mut(dest, len) };
    SystemRng.fill_bytes(buf);
    Ok(())
}
```

Note the symbol is `__getrandom_v03_custom` for both 0.3 and 0.4; `getrandom` 0.4.3's
`backends/custom.rs` still declares the v03 name. `getrandom` 0.2, which `ring` pulls, uses a
different mechanism (the `register_custom_getrandom!` macro), so a fix has to cover two shapes.

The right fix is probably not the custom hook, because the hook is a `RUSTFLAGS` setting that
every consumer has to remember and that a workspace cannot express per-dependency. `getrandom` 0.4
carries a `hermit.rs` backend selected by `target_os = "hermit"`, which is exactly the shape this
project's `std` took from Hermit. An upstream arm, or a patch under `patches/`, is a
decision for whoever picks this up.

#### What was decided, and where the paragraph above was wrong

`entropy_backend` is the answer (milestone 64, 2026-08-17), and it is the custom hook
after all. The objection above is half right: the flag is a `RUSTFLAGS` setting, and a consumer that
forgets it gets the same `compile_error!`. But a workspace states it once, in its own
`.cargo/config.toml`. Per-workspace is the correct granularity anyway, because whether an
entropy source exists is a property of the target rather than of any one dependency. "Cannot express
per-dependency" was true and was not the requirement.

What tipped it was the other side of the ledger. A `[patch.crates-io]` fork needs no flag at the call
site, which is genuinely nicer, and costs a maintained fork of a crate this very note recorded as
mid-transition across 0.2, 0.3 and 0.4 inside one dependency graph. §46 (thin)'s rule is that we vendor
where correctness is won by exposure. A hook the upstream crate designed for this case is neither
that nor code we would otherwise write, and it is eleven lines that upstream cannot break without
also breaking Hermit's.

The upstream arm is still the right long-term fix and is a smaller diff than the hook. It is a
pull request against `getrandom`, not a change to this tree.

Three things about the recipe, and the third cost an hour:

1. Depend on `entropy_backend`.
2. `rustflags = ["--cfg", "getrandom_backend=\"custom\""]` in the consuming workspace's
   `.cargo/config.toml`.
3. `use entropy_backend as _;` in the binary. An rlib nothing references is not linked, so
   without this the build reaches `rust-lld: error: undefined symbol: __getrandom_v03_custom`. Two of
   the eight probes linked without it because they happened to call `getrandom` on a path the linker
   kept; six did not. Same shape as a `no_std` panic handler: a crate that exists to define a symbol
   has to be pulled in on purpose.

Re-measured with it, on 2026-08-17:

| crate | with `entropy_backend` |
|---|---|
| `rand` 0.9 | PASS |
| `uuid` 1 | PASS |
| `gix-object` 0.51 | PASS |
| `gix-actor` 0.36 | PASS |
| `gix-config` 0.48 | FAIL, class B (`gix-sec`) |
| `gix` 0.74 | FAIL, class B (the `errno` crate now, not `gix-sec`) |
| `zip` 5 | FAIL, class C (`zstd-sys`) |
| `ring` 0.17 | FAIL, `getrandom` 0.2, whose hook is the `register_custom_getrandom!` macro |

Four of eight, which is exactly what the 2026-08-04 stub reached, and the point is the other
four: not one of them still fails on `getrandom`'s dispatch. Class A is closed and what is left
behind it is classes B and C, which are different work.

### Class B: the crate falls through to `unix` (3 of 11)

`tar` (via `filetime` 0.2.29) and `gix-config`/`gix` (via `gix-sec` 0.12.2) both do a
`cfg_if`-shaped dispatch whose last arm assumes unix:

```
error[E0433]: cannot find `unix` in `os`
  --> gix-sec-0.12.2/src/identity.rs:36:26
   |
36 |             use std::os::unix::fs::MetadataExt;
error[E0425]: cannot find function `geteuid` in crate `libc`
```

This is the class that says something about us rather than about them. There is no
`std::os::nife`, `libc` has no `nife` module, and a crate whose platform ladder ends in "else
it is unix" cannot compile here. The three crates that hit it are asking for a uid and a
file mtime set, neither of which this system has in the form they want.

Contrast `tempfile`, which has an explicit `other.rs` arm and therefore compiles. The distinction
between the 35 and this class is entirely whether the crate author wrote a fallback, and nothing
about how hard the crate is.

### Class C: a C library or C sources (3 of 11)

- `ring` 0.17.14: builds C and assembly, and `cc` cannot find `assert.h` for this target.
  `fatal error: 'assert.h' file not found`.
- `zip` 5.1.1 (via `zstd-sys`): same shape, a C build script.
- `diesel` 2.3.11 with `sqlite`: compiles and fails at link,
  `rust-lld: error: unable to find library -lsqlite3`.

These are jobs for the C seam (notes/c-seam.md), not for the `std` PAL. They are also the only class
where "make it build" and "make it work" are the same task.

### Class D: a socket/fd model that does not exist (1 of 11)

`rocket` 0.5.1 fails in `socket2` 0.6.5 and then `mio` 1.2.2:

```
error: Socket2 doesn't support the compile target
error[E0432]: unresolved import `crate::sys::IoSourceState`
```

`mio` with no features compiles (its `sys` module is empty), which is why it appears in the pass
list; the moment anything asks for the reactor it does not. This is the same shape as class B, one
level up: a readiness-based IO model wants file descriptors and a poller, and neither exists here.
