# A TLS crypto provider on nife: what builds, what runs, and what is still an architect's to decide

*(Milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets), 2026-09-19.
DECISIONS §196 (nife carries TLS: `rustls` for the protocol, and a crypto provider we make work)
rests on a table this note replaces. §196 was
right to be provisional about it: its own limits paragraph says the probe ran on the stable host
toolchain against the stock bare-metal targets, and that those are not nife's targets.)*

The short answer: `rustls` plus a pure-Rust provider builds and runs on all three nife
architectures, and the two things that stopped it were a `getrandom` major and a handful of
compiler flags. Neither was a wall. One is now closed in this tree. The other is six lines of
build configuration recorded where a reader meets it, and one of those six is a run-time
hazard rather than a build one. That is the part worth carrying past this milestone.

The provider that builds is `rustls-rustcrypto` 0.0.2-alpha, and calef refused it, 2026-09-20:
*"rustls-rustcrypto doesn't seem like a high quality dependency."* The refusal and the numbers
behind it are in [The refusal](#the-refusal-rustls-rustcrypto), and what replaces it is
[the glue path](#pricing-the-glue-path), priced before it was written because it does not avoid
depending on the primitives. The evidence this page has no room for is in
[`cryptography-provider/`](cryptography-provider/README.md).

## What changed about the measurement

§196 measured against `aarch64-unknown-none-softfloat`, `riscv64gc-unknown-none-elf` and
`x86_64-unknown-none`. **nife does not build user programs for those.** It builds them for
`targets/aarch64-unknown-nife.json` and its two siblings, which declare `"os": "nife"` and
`"std": true` and are compiled with `-Zbuild-std` against the patched `std` farm
(notes/std.md). A package client would be one of those programs. So the old table was measuring a
platform this project does not ship. Its `getrandom` rows in particular were measuring the absence
of a thing nife has had since milestone 56 (secrets, credentials, and the entropy to make them
safe): `std::random` over the entropy service.

`script/crypto-probes` is the re-measurement, and it is `script/crate-probes`' sibling: same
`[[bin]]`-that-calls-the-crate discipline, narrowed to providers and primitives, widened to all
three architectures because the answer differs by ISA.

```sh
script/crypto-probes                 # every probe, all three architectures
script/crypto-probes --soft          # the same, with every SIMD path forced off
script/crypto-probes --arch x86_64 sha2
```

## The table

Resolved versions as of 2026-09-19, built for `*-unknown-nife` on `nightly-2026-09-17`, with
`entropy_backend` in every probe. **Two columns per architecture**: as the crate ships, and with
the soft-implementation flags of [the x86_64 finding](#the-x86_64-finding) applied.

| probe | version | aarch64 | riscv64 | x86_64 | x86_64, soft |
|---|---|---|---|---|---|
| `rustls` (tls12, std, no defaults) | 0.23.45 | PASS | PASS | PASS | PASS |
| **`rustls` + `rustls-rustcrypto`** | 0.0.2-alpha | **PASS** | **PASS** | FAIL `polyval` | **PASS** |
| `rustls-rustcrypto` alone | 0.0.2-alpha | PASS | PASS | FAIL `polyval` | PASS |
| `ring` | 0.17.14 | FAIL | FAIL | FAIL | FAIL |
| `aws-lc-rs` | 1.18.1 | FAIL | FAIL | FAIL | FAIL |
| `embedded-tls` | 0.19.0 | PASS | PASS | FAIL `polyval` | PASS |
| `rustls-webpki` | 0.103.15 | PASS | PASS | PASS | PASS |
| `sha2` | 0.10.9 | PASS | PASS | FAIL | PASS |
| `hmac` | 0.12.1 | PASS | PASS | FAIL `sha2` | PASS |
| `hkdf` | 0.12.4 | PASS | PASS | FAIL `sha2` | PASS |
| `aes-gcm` | 0.10.3 | PASS | PASS | FAIL `polyval` | PASS |
| `chacha20poly1305` | 0.10.1 | PASS | PASS | FAIL `poly1305` | PASS |
| `polyval` | 0.6.2 | PASS | PASS | FAIL | PASS |
| `ghash` | 0.5.1 | PASS | PASS | FAIL `polyval` | PASS |
| `p256` | 0.13.2 | PASS | PASS | FAIL `sha2` | PASS |
| `x25519-dalek` | 2.0.1 | PASS | PASS | FAIL `curve25519-dalek` | PASS |
| `ed25519-dalek` | 2.2.0 | PASS | PASS | FAIL `sha2` | PASS |
| `rsa` | 0.9.10 | PASS | PASS | PASS | PASS |

Read it against §196's and the difference is the whole milestone. That table had every
candidate failing on every target. This one has one real failure class left, and it is the one
§196 could already see the shape of: C.

## The two failures, and what each one was

### `getrandom`, which was a major-version gap rather than a missing backend

`rustls-rustcrypto` and `ring` both died at `getrandom`'s `compile_error!`, as §196 recorded.
`entropy_backend` answered 0.3 and 0.4 already, but `rustls-rustcrypto` reaches 0.2 through
`rand_core` 0.6, whose hook is a different symbol with a different signature. The crate now defines
both, and `use entropy_backend as _;` is still load-bearing for each. `ring` then failed one class
along, in its own `cc` build script, and `aws-lc-rs` fails the same way: §46's "no C in the shipping
graph" holding. [`the-getrandom-gap.md`](cryptography-provider/the-getrandom-gap.md) has the detail.

### The x86_64 finding

`x86_64-unknown-nife` sets `+soft-float` and switches off `mmx`, every `sse` level, `avx` and
`avx2`. The RustCrypto crates compile their x86 intrinsic paths anyway, because they select at
run time through `cpufeatures` rather than at compile time through a `cfg` a target specification
could influence. LLVM then cannot legalize a 128-bit vector operation and rustc dies with:

```
rustc-LLVM ERROR: Do not know how to split the result of this operator!
```

which names nothing a reader can act on, and which `cargo` reports only as `could not compile
sha2 (lib)`. §196 called this class "SIMD paths fail on soft-float x86_64" and guessed it might
differ on our target. It does not differ; it is the same failure, and it has an answer.

Each crate has an escape hatch, and nothing discovers it for you:

| crate | how to force the portable implementation |
|---|---|
| `sha2` | `--cfg sha2_backend="soft"` |
| `polyval` | `--cfg polyval_backend="soft"` |
| `poly1305` | `--cfg poly1305_backend="soft"` |
| `aes` (under `aes-gcm`) | `--cfg aes_backend="soft"` |
| `curve25519-dalek` | `--cfg curve25519_dalek_backend="serial"` |
| `chacha20` | `--cfg chacha20_backend="soft"`, and it is not like the others: see below |

Every one is a `--cfg` in `cryptography_exerciser/.cargo/config.toml` and its copy in
`pinned_tls_exerciser`'s. A cfg a crate does not recognise is ignored silently, so a major that
renames one restores the SIMD path: read each crate's `check-cfg` list when its major moves.

### The one that does not fail at build time, and cost most of a day

Every row above except the last fails *loudly*, when rustc dies. `chacha20` compiles perfectly
for `x86_64-unknown-nife` and then executes an AVX2 instruction in ring 3. That target's own
description is "softfloat ring 3, no SSE state", so there is nowhere for the register state to
live, and the program dies:

```
user thread 4 killed: vector 6 (invalid opcode)
```

before printing a single byte, which reads exactly like a program that never started. `chacha20`
selects its backend by **asking the CPU** (`cpufeatures`, and through it `CPUID`) rather than by
asking the compiler, and the CPU under `-cpu max` truthfully has AVX2. The disagreement is between
what the processor can do and what this operating system has set up for ring 3, and no part of the
toolchain models it.

That is a hazard for this target in general, not a fact about crypto. Any crate that
runtime-detects SIMD is in the same position, and the failure it produces is a dead program with no
message rather than a build error. It is written here because this is where it was found; it
belongs to `x86_64-unknown-nife` rather than to any provider.

The cost is real and unmeasured. A `.cargo/config.toml` cannot vary rustflags by target when
the target is a JSON specification path, so the cfgs apply to all three architectures and aarch64
and riscv64 run portable code their vector units could have done faster. Nobody has a number for
what that costs a handshake; milestone 442's block already carries that as a `BUGS` entry.

## What runs, as opposed to what builds

`script/crypto-probes` measures compile and link and nothing else, which is its own `BUGS` entry
and is a weak claim here specifically: on x86_64 every one of these crates is running an
implementation almost nobody runs.

So `cryptography_exerciser` is a `std` program that runs published test vectors against the
primitives directly and against `cryptography_provider`, the provider this tree assembles. Each
vector is transcribed from the specification that publishes it and named beside it. That is the
shape `crates/measured_boot` already uses for its hand-written SHA-256 ("the published FIPS 180-4
vectors, **not** self-consistency checks"). `system_tests/src/user/cryptography_tests.rs` boots it under
QEMU on all three architectures.

| line | vector |
|---|---|
| `sha256 ok`, `sha256 empty ok` | FIPS 180-4 |
| `sha384 ok` | FIPS 180-4 |
| `hmac-sha256 ok` | RFC 4231 test case 1 |
| `hkdf-sha256 ok` | RFC 5869 test case 1 |
| `aes-128-gcm ok` | the GCM specification's test case 2 |
| `poly1305 ok` | RFC 8439 section 2.5.2 |
| `chacha20-poly1305 ok` | RFC 8439 section 2.8.2 |
| `x25519 ok` | RFC 7748 section 5.2 |
| `p256 generator ok` | FIPS 186-4 D.1.2.3 |
| `entropy 0.4 ok` | no vector exists; two draws differ |
| `chain objects.githubusercontent.com ok` | the host's own chain, RSA 2048, captured 2026-09-20 |
| `chain ghcr.io ok` | the host's own chain, RSA 4096 leaf over a 3072-bit intermediate |
| `chain refusals ok` | one flipped signature bit, and a certificate offered for another host's name |
| `provider ok 3 suites 2 groups 3 signature algorithms` | **this tree's own provider** constructs and offers what it claims |

The rule that goes with a transcribed vector, and it is in the program's own docs: if one
fails, do not adjust the vector. Either the transcription is wrong, in which case check it against
the specification rather than against the output, or the implementation is wrong on this target,
which is the whole reason the program exists.

That rule was tested on 2026-09-20 and it held, with a third answer nobody had listed. The
Poly1305 vector failed. The transcription was right, checked against RFC 8439 rather than against
the output, and the crate was right: the call was wrong. `update_padded` zero-fills the last
partial block, which is what the AEAD construction does and is not what section 2.5.2's standalone
34-byte example does; `compute_unpadded` produces the RFC's tag exactly, on this target and on the
host. So the list is: the transcription, the implementation, or the way the test asks, and the
third is the one that bit.

`entropy 0.4 ok` is the line with no vector and the one that is genuinely new. A linker
resolving `__getrandom_v03_custom` says nothing about where bytes come from, so the program draws
twice through `getrandom` 0.4, the path the provider's randomness takes, and asserts the draws differ and are not all zero. That is milestone 56's
claim for `std::random`, made again one layer out, and it is what a silently-stubbed RNG would
fail. A draw with no entropy capability granted panics rather than weakening. That is
milestone 56's deliberate choice and is the behavior this tree wants: a provider that quietly
falls back to a weak source is worse than one that does not build.

## How to run any of it

```sh
script/crypto-probes                              # the build table
script/crypto-probes --soft                       # the same with the portable paths forced
helpers/build-cryptography-exerciser.sh           # build the vector program for all three ISAs
script/test                                       # builds it (the line above) and runs it
```

Since milestone 855 (the TLS graph enters the gated build), `script/test` builds and runs the
program, and runs the provider's host tests.

## The refusal: `rustls-rustcrypto`

calef, 2026-09-20: *"rustls-rustcrypto doesn't seem like a high quality dependency."* Refused.

The numbers that justify it, so the next person to reach for it finds the refusal rather than
re-deriving it. Read from crates.io on 2026-09-20 rather than recalled:

- Version 0.0.2-alpha, published 2024-04-24, which was seventeen months before this
  refusal. Three versions have ever existed and two of them are yanked, so the live release is
  both the newest and an alpha.
- 73 crates in its graph with `rustls` included, against `rustls` alone at 7.
- It is pre-1.0 in the one category DECISIONS §46 (thin primitives or whole subsystems; we write
  everything in between) singles out as bought by exposure rather than by reading a
  specification. An alpha abandoned for seventeen months has had the least exposure of anything in
  the graph it assembles, which is the argument in one sentence: the glue is the weakest link in a
  chain of otherwise well-worn primitives.
- **It pins outdated majors.** It requires `sha2` 0.10, `aes-gcm` 0.10 and `p256` 0.13; the
  current releases of those three are 0.11.0 (2026-03-25), 0.11.1 (2026-08-21) and 0.14.0
  (2026-07-03). Taking it would fix this tree to the dependency choices of an unmaintained crate.

**What it is not refused for.** Its license is Apache-2.0 OR MIT, the pair this tree publishes
under, and it builds and runs correctly on all three architectures, which the table above measured
and the vectors confirmed. The refusal is about who maintains the glue, not about whether it works
today.

## Pricing the glue path

The alternative to the refused alpha is to write the `CryptoProvider` glue in this tree over
primitives chosen deliberately. That does not avoid depending on the primitives, so it was priced
the same way before a line of it was written.
[`pricing-the-glue-path.md`](cryptography-provider/pricing-the-glue-path.md) has the measured graph,
the per-primitive quality table and the full `rsa` argument. What it found:

- Size is no argument. Measured with `cargo tree -e normal`, the provider as built is 67 crates
  against the alpha's 73, and its graph is a strict subset of the alpha's. `rsa` adds fourteen.
- Every license is permissive (MIT, Apache-2.0, BSD-3-Clause, ISC). The two AEADs carry an NCC
  Group audit with no significant findings; `p256` says in its own README it has never been
  independently audited, and is in the path either way.
- `rsa` was a choice. Its README admits the Marvin attack (RUSTSEC-2023-0071), a timing channel in
  PKCS#1 v1.5 decryption. This provider only verifies signatures, holds no RSA private key and is
  TLS 1.3 only, so the advisory does not describe this use. `deny.toml` carries that argument in
  the suppression's `reason` field, with what would end it. Leaving `rsa` out would have cost the
  RSA chains of `objects.githubusercontent.com` and `ghcr.io`, so a client could reach GitHub and
  not download the file. calef, 2026-09-20: "Take rsa."
- §46's line, drawn out loud: the primitives are taken, because their correctness includes
  constant-time behavior a spec does not state. The glue is written, and glue is not crypto: it
  selects, names and plumbs, and computes nothing secret-dependent. Writing RSA verification here is
  refused, because Bleichenbacher's forgery and BERserk are spec-reading failures in exactly that
  code. The random source was always this system's own.

calef ruled on both halves: `rustls-rustcrypto` refused (DECISIONS §198, the glue is ours, the
primitives are not), and then "Take rsa." The provider is built, the two real chains verify on all
three architectures, and the milestone's own list is finished.

## BUGS

- A program that aborts is never heard, and it cost this lane a day and a wrong record.
  `drain_sink` ends only on the sink's end-of-stream marker, which the std runtime sends after
  `main` returns; a panic under `panic = "abort"` traps instead, so every byte the program printed,
  including the panic message, is delivered to the endpoint and thrown away. The transcript
  reads as empty and the reader hangs, which is indistinguishable from a program that never
  started. It was read as exactly that, and a whole finding was written up on that basis before
  `cryptography_exerciser` grew a panic hook that prints and exits cleanly, at which point the
  failure named itself in one run. `design/roadmap/0496-a-dying-programs-last-words-reach-nobody.md`
  has the shapes a real fix could take, and `std_tests::drain_sink` now carries the warning where a
  reader meets it.
- `rsa` is 0.10.0-rc.19, pinned with `=`, and its `encoding` feature brings `pkcs1` 0.8.0-rc.5.
  No released `rsa` shares the graph's `digest` 0.11 traits, and 0.9.10 would double eleven crates.
  Move to 0.10.0 when it is released; re-argue by 2027-01-31.
- **A vector proves the answer, not the manner.** Nothing measures timing, so a portable fallback
  that is correct and not constant-time passes every line. On x86_64 the fallbacks are exactly what
  runs, and constant-time behavior is the property §46 says is bought by exposure and not by a
  specification. This is the gap most worth closing and this note does not close it.
- A cost is measured only under emulation. The block of milestone 501 (a TLS client that speaks
  to one pinned peer) has handshake and transfer times for all three architectures under QEMU's
  TCG, with these soft-implementation cfgs on. The median handshake is 139 to 162 ms, and a 256 KiB
  transfer takes 175 to 441 ms. Nothing on radon or xenon
  yet, and nothing separates what the cfgs cost aarch64 and riscv64.
- `script/crate-probes` may be measuring the wrong toolchain. It builds its probes under
  `target/`, inside this repository, with `RUSTUP_TOOLCHAIN` alone. Milestone 442's lane measured
  that configuration compiling `std` from the unpatched sysroot, which fails in `sys/alloc`'s
  `cfg_select!` and reads exactly like the crate under test failing. `script/crypto-probes` builds
  outside the repository for that reason; nobody has re-run the fifty of milestone 64 (enough `std` to run
  somebody else's crate) since, and the 43/7 split in notes/crates-io-on-nife.md has not been rechecked against this.
- A toolchain FILE is not a fix for that, and the failure it produces is worse. Giving a probe
  its own `rust-toolchain.toml` naming `nife-dev` gets riscv64 and x86_64 right and still gets
  `aarch64-unknown-nife` wrong on an aarch64 host. Two architectures then agree and the third reads
  as a real finding about the crate.
