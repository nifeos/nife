# A TLS crypto provider on nife: what builds, what runs, and what is still an architect's to decide

*(Milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets), 2026-09-19.
DECISIONS §196 (nife carries TLS: `rustls` for the protocol, and a crypto provider we make work)
rests on a table this note replaces. §196 was
right to be provisional about it: its own limits paragraph says the probe ran on the stable host
toolchain against the stock bare-metal targets, and that those are not nife's targets.)*

The short answer: `rustls` plus a pure-Rust provider builds and runs on all three nife
architectures, and the two things that stopped it were a `getrandom` major and a handful of
compiler flags. Neither was a wall. One is now closed in this tree; the other is six lines of
build configuration recorded where a reader meets it, and one of those six is a run-time
hazard rather than a build one, which is the part worth carrying past this milestone.

**The provider that builds is `rustls-rustcrypto` 0.0.2-alpha, and calef refused it**, 2026-09-20:
*"rustls-rustcrypto doesn't seem like a high quality dependency."* The refusal and the numbers
behind it are in [The refusal](#the-refusal-rustls-rustcrypto), and what replaces it is
[the glue path](#pricing-the-glue-path), priced before it was written because it does not avoid
depending on the primitives.

## What changed about the measurement

§196 measured against `aarch64-unknown-none-softfloat`, `riscv64gc-unknown-none-elf` and
`x86_64-unknown-none`. **nife does not build user programs for those.** It builds them for
`targets/aarch64-unknown-nife.json` and its two siblings, which declare `"os": "nife"` and
`"std": true` and are compiled with `-Zbuild-std` against the patched `std` farm
(notes/std.md). A package client would be one of those programs. So the old table was measuring a
platform this project does not ship, and its `getrandom` rows in particular were measuring the
absence of a thing nife has had since milestone 56 (secrets, credentials, and the entropy to make them safe):
`std::random` over the entropy service.

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

`rustls-rustcrypto` and `ring` both died at `getrandom`'s `compile_error!`, exactly as §196
recorded, and `entropy_backend` was already the answer to that class. It answered 0.3 and 0.4,
whose hook is a bare `__getrandom_v03_custom` symbol. `rustls-rustcrypto` reaches `getrandom`
0.2 through `rand_core` 0.6, and 0.2's hook is a different symbol with a different signature,
emitted by the `register_custom_getrandom!` macro.

`entropy_backend`'s own `BUGS` section had named this and forecast it: *"nothing currently needs
the second shape. Something will."* It did. The crate now defines both symbols. Two details are
worth carrying:

- 0.2's hook returns a bare `u32` (zero for success), not a `Result`, so defining it needs no
  type from `getrandom`. What it needs is 0.2's `custom` feature, and only a manifest can turn
  a feature on, which is why `entropy_backend` now depends on 0.2 as well. The cost is one extra
  crate in the graph of a consumer that only ever needed 0.4.
- `use entropy_backend as _;` is still load-bearing, for the 0.2 symbol exactly as for the
  other. Upstream's documentation says a registration "can only be registered in the root binary
  crate", which is not quite the rule; the rule is that an rlib nothing references is not linked.

`ring` moved one class along as a result: it no longer fails at `getrandom`, it fails in its own
`cc` build script, which is what notes/crates-io-on-nife.md's class C predicted and what §196
already recorded from a different angle. `aws-lc-rs`, which §196 never probed, fails the same way
in `aws-lc-sys`. Neither is available to this project at any price a lane can pay, and that is
a finding rather than a disappointment: it is §46's "no C in the shipping graph" holding.

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

Each crate has an escape hatch, and they are not spelled alike, which is the part worth
writing down because nothing discovers it for you:

| crate | how to force the portable implementation |
|---|---|
| `sha2` | a cargo **feature**, `force-soft` |
| `polyval` | `--cfg polyval_force_soft` |
| `poly1305` | `--cfg poly1305_force_soft` |
| `aes` (under `aes-gcm`) | `--cfg aes_force_soft` |
| `curve25519-dalek` | `--cfg curve25519_dalek_backend="serial"` |
| `chacha20` | `--cfg chacha20_force_soft`, and it is not like the others: see below |

The feature is the awkward one. A consumer that never names `sha2` still has to name it, because
cargo unifies features across the graph and the crate that actually depends on it
(`rustls-rustcrypto`, `p256`, `hmac`) has no way to ask. `cryptography_exerciser/Cargo.toml` names
it for that reason and says so.

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
primitives directly and against `cryptography_provider`, the provider this tree assembles, transcribed
from the specification that publishes each one and named beside it, in the shape
`crates/measured_boot` already uses for its hand-written SHA-256 ("the published FIPS 180-4
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
| `entropy 0.2 ok` | no vector exists; two draws differ |
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

`entropy 0.2 ok` is the line with no vector and the one that is genuinely new. A linker
resolving `__getrandom_custom` says nothing about where bytes come from, so the program draws twice
through `rand_core` 0.6 and asserts the draws differ and are not all zero. That is milestone 56's
claim for `std::random`, made again one layer out, and it is what a silently-stubbed RNG would
fail. A draw with no entropy capability granted panics rather than weakening, which is
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
- **It pins superseded majors.** It requires `sha2` 0.10, `aes-gcm` 0.10 and `p256` 0.13; the
  current releases of those three are 0.11.0 (2026-03-25), 0.11.1 (2026-08-21) and 0.14.0
  (2026-07-03). Taking it would fix this tree to the dependency choices of an unmaintained crate.

**What it is not refused for.** Its license is Apache-2.0 OR MIT, the pair this tree publishes
under, and it builds and runs correctly on all three architectures, which the table above measured
and the vectors confirmed. The refusal is about who maintains the glue, not about whether it works
today.

## Pricing the glue path

The alternative named in this milestone is to write the `CryptoProvider` glue in this tree over
primitives chosen deliberately. **That does not avoid depending on the primitives**, so it was
priced the same way before a line of it was written.

### The graph, measured the same way for both

`cargo tree -e normal`, normal edges only, so build scripts and proc-macro crates that run on the
host are excluded from every column. The earlier figure of 107 in this milestone's pull request was
`Cargo.lock` lines, which counts build dependencies too; it is superseded here by a like-for-like
count, and the honest headline is that the glue path is barely smaller.

| | crates | `rsa` in the graph | alpha crate in the trust path |
|---|---|---|---|
| `rustls` + `rustls-rustcrypto` | **73** | yes | yes |
| **`cryptography_provider` as built** | **67** | yes | **no** |
| the same without `rsa`, which was built first and then ruled against | 53 | no | no |

Re-priced on 2026-09-20, after calef ruled "Take rsa". The provider is 67 crates, which is
exactly the figure this note predicted for "the glue at equal algorithm coverage" before the code
existed, and it is **six fewer than the refused alpha**. The whole of that six is
`rustls-rustcrypto` itself plus `base64ct`, `paste`, `pem-rfc7468`, `pkcs5` and a second, older
`rustls-webpki` it pinned beside the one `rustls` already wants.

So the size argument is now as weak as it can be, and the record says so rather than quietly
dropping the comparison. 67 against 73 is not why the glue was written. It was written because
the glue is the piece with the least exposure in the chain, which is DECISIONS §198 (the glue is
ours, the primitives are not).

**`rsa` adds fourteen crates**: `rsa`, `pkcs1`, `num-bigint-dig`, `num-integer`, `num-iter`,
`num-traits`, `libm`, `lazy_static`, `rand`, `rand_chacha`, `ppv-lite86`, `zerocopy`, `smallvec`
and `spin`.

**Two of them runtime-detect SIMD, and neither is the ring-3 hazard**, which was checked rather
than assumed. `ppv-lite86` gates its x86_64 module on `target_feature = "sse2"`, and `libm` gates
its x86 arch module the same way; `x86_64-unknown-nife` switches SSE2 off, so both compile to their
portable paths with no flag and no opportunity to execute an instruction the target has no state
for. That is the opposite of `chacha20`, which selects by asking the CPU and therefore needed
`--cfg chacha20_force_soft`. Confirmed by the x86_64 leg running both real certificate chains.

73 against 67 is not an argument, and saying so is the point. The full glue graph is a strict
subset of `rustls-rustcrypto`'s: there is nothing in it that crate does not also pull. Writing
the glue removes exactly six crates, and one of the six is `rustls-rustcrypto` itself. The others
are `base64ct`, `paste`, `pem-rfc7468`, `pkcs5`, and a second, older copy of `rustls-webpki`
(0.102.8) that it pins beside the 0.103 `rustls` already wants.

The third row is where the real difference is, and it is not size. Writing the glue means
choosing which primitives are in the trust path at all, and the one worth choosing about is `rsa`.

### Quality, per primitive, read rather than recalled

Latest-release dates from crates.io on 2026-09-20; audit and warning text quoted from the README
that ships inside each crate, so any reader can check it without leaving their disk.

| crate | version here | license | latest release | audit, in its own words | runtime SIMD here |
|---|---|---|---|---|---|
| `sha2` | 0.10.9 | MIT OR Apache-2.0 | 0.11.0, 2026-03-25 | no audit statement | yes; needs the `force-soft` **feature** |
| `hmac`, `hkdf` | 0.12.1, 0.12.4 | MIT OR Apache-2.0 | current line | no audit statement | no |
| `aes-gcm` (with `aes`, `ghash`, `polyval`) | 0.10.3 | Apache-2.0 OR MIT | 0.11.1, 2026-08-21 | *"one security audit by NCC Group, with no significant findings"*, funded by MobileCoin | yes; needs `aes_force_soft` and `polyval_force_soft` |
| `chacha20poly1305` (with `chacha20`, `poly1305`) | 0.10.1 | Apache-2.0 OR MIT | 0.11.0, 2026-06-28 | the same NCC Group audit, same wording | yes; needs `chacha20_force_soft` and `poly1305_force_soft` |
| `p256` | 0.13.2 | Apache-2.0 OR MIT | 0.14.0, 2026-07-03 | *"The elliptic curve arithmetic contained in this crate has never been independently audited!"* | no |
| `x25519-dalek`, `curve25519-dalek` | 2.0.1, 4.1.3 | BSD-3-Clause | current line | no audit statement | yes; needs `curve25519_dalek_backend="serial"` |
| `ed25519-dalek` | 2.2.0 | BSD-3-Clause | current line | no audit statement | no |
| `rsa` | 0.9.10 | MIT OR Apache-2.0 | current line | one audit by Include Security, one minor finding addressed, and see below | no |
| `rustls-webpki` | 0.103.15 | ISC | current line | part of the rustls project | no |

**Every license is permissive** (MIT, Apache-2.0, BSD-3-Clause, ISC), so nothing here touches
DECISIONS §135 (running GPL software is aggregation, the capability boundary is what makes it so,
and packages are how it arrives).

The two audited crates are the two AEADs, and they are the ones handling every byte of every
record. The NCC Group engagement was commissioned by MobileCoin, reported publicly in February
2020, and both crates state its outcome in their own README. That is the strongest evidence in the
table and it covers the record layer.

`p256` says in its own README that it has never been independently audited. It is in the path
either way, because TLS 1.3 negotiates it and because GitHub's own certificate chain is ECDSA
P-256, so this is a fact to record rather than a choice to make.

### The one that was a choice: `rsa`, and how it was answered

calef, 2026-09-20: "Take rsa." It is in, and this section keeps the argument because the entry
it produced in `deny.toml` is the tree's first suppression and the next person will read it as
precedent.

`rsa` 0.9.10's own README says it plainly:

> This crate is vulnerable to the [Marvin Attack] which could enable private key recovery by a
> network attacker (see [RUSTSEC-2023-0071]).

**Why the advisory does not describe this use.** Marvin is a timing side channel in RSA PKCS#1 v1.5
**decryption**: it recovers a private key from an oracle that decrypts attacker-chosen
ciphertexts. `cryptography_provider` calls `rsa` in exactly one file, `src/verify.rs`, and only for
signature verification, which is a public-key operation. It holds no RSA private key, decrypts
nothing, and offers no oracle. The provider being TLS 1.3 only is part of what holds that up, since
TLS 1.3 removed RSA key transport altogether.

What would end the claim, which is the half that makes it bounded rather than a dismissal: any
use of `rsa` here to decrypt, to sign, or for key transport. That sentence is in `deny.toml` beside
the entry, in the `reason` field rather than only in a comment, so `cargo-deny` prints it with the
finding.

And the suppression is now scanned rather than asserted. `script/supply-chain` ran over four
manifests and neither of these packages was among them, so an `ignore` entry would have been a
claim about a graph no gate looked at. Both are on that list now, which also brought three
licenses onto the allow-list with their own reasons (ISC for `rustls-webpki` and `untrusted`,
Unicode-3.0 for a proc-macro dependency that ships nothing) and made `publish = false` necessary,
because `cargo-deny` forgives a path dependency only in a package that could not be published.

What it cost to leave out, measured rather than argued, and the reason the ruling went this
way:

| host | chain |
|---|---|
| `github.com` | ECDSA P-256 throughout, `ecdsa-with-SHA256` and `ecdsa-with-SHA384` |
| `objects.githubusercontent.com`, where a Release asset is served | **RSA 2048**, `sha256WithRSAEncryption`, Let's Encrypt |
| `ghcr.io` | **RSA 4096** leaf over a 3072-bit intermediate, Sectigo |

A client without RSA verification reaches GitHub and cannot download the file.

### §46's test, said out loud

§46 says this tree writes what is on the verification path and takes what is won by exposure
rather than by reading a spec, and it says in so many words that cryptography is the second kind:
*"take it, do not write it."* So the line has to be drawn deliberately rather than assumed:

- The primitives are taken. AES-GCM, ChaCha20-Poly1305, SHA-2, HMAC, HKDF, X25519, P-256 and
  RSA verification are all the second kind. Their correctness includes constant-time behavior and
  resistance to attacks no specification states, which is exactly what years of use and the NCC
  Group audit above buy and what a proof against a spec would not.
- **The glue is written, and glue is not crypto.** A `CryptoProvider` is five fields: a cipher
  suite table, a key exchange group table, a signature verification algorithm table, a random
  source, and a private key loader. It selects, names and plumbs; it computes nothing. Nothing in
  it is secret-dependent in a way a timing attack could read, because every secret-dependent
  operation is inside a primitive.
- **The one place the line could be crossed, and is not.** The temptation with `rsa` gone is to
  write RSA PKCS#1 v1.5 verification here, which is genuinely only public-key arithmetic and a
  padding check. It is refused: the classic failures there (Bleichenbacher's signature forgery, and
  BERserk after it) are spec-reading failures in exactly this code, which is §46's argument for
  taking rather than writing, restated. `crypto-bigint` being already in the graph is a
  convenience argument and this tree's tenets say to distrust that one.
- The random source is ours and always was. `SecureRandom` reaches `entropy_backend` and the
  entropy service. That is not a §46 question: it is this system's own capability.

### What the pricing concluded, and what happened next

The glue path is barely smaller and meaningfully better in the one dimension calef named. At
equal coverage it is 67 crates against 73, which is nothing, and it takes an abandoned alpha out of
the trust path and replaces it with code this tree can read, over primitives that are individually
maintained, widely used, and in two cases audited. It also unpins the superseded majors. If the
argument for it were crate count it would be a bad argument, and that is recorded here rather
than smoothed over.

calef ruled on both halves: `rustls-rustcrypto` refused (DECISIONS §198, the glue is ours, the
primitives are not), and then, on 2026-09-20, "Take rsa." The provider is built, the two real
chains verify on all three architectures, and the milestone's own list is finished.

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
- **A vector proves the answer, not the manner.** Nothing measures timing, so a portable fallback
  that is correct and not constant-time passes every line. On x86_64 the fallbacks are exactly what
  runs, and constant-time behavior is the property §46 says is bought by exposure and not by a
  specification. This is the gap most worth closing and this note does not close it.
- A cost is measured only under emulation. Milestone 501 (a TLS client that speaks to one
  pinned peer)'s block has handshake and transfer
  times for all three architectures under QEMU's TCG, with these soft-implementation cfgs on:
  a median handshake of 139 to 162 ms and 175 to 441 ms per 256 KiB. Nothing on radon or xenon
  yet, and nothing separates what the cfgs cost aarch64 and riscv64.
- `script/crate-probes` may be measuring the wrong toolchain. It builds its probes under
  `target/`, inside this repository, with `RUSTUP_TOOLCHAIN` alone. Milestone 442's lane measured
  that configuration compiling `std` from the unpatched sysroot, which fails in `sys/alloc`'s
  `cfg_select!` and reads exactly like the crate under test failing. `script/crypto-probes` builds
  outside the repository for that reason; nobody has re-run the fifty of milestone 64 (enough `std` to run somebody else's crate)
  since, and the
  43/7 split in notes/crates-io-on-nife.md has not been rechecked against this.
- A toolchain FILE is not a fix for that, and the failure it produces is worse. Giving a probe
  its own `rust-toolchain.toml` naming `nife-dev` gets riscv64 and x86_64 right and still gets
  `aarch64-unknown-nife` wrong on an aarch64 host. Two architectures then agree and the third reads
  as a real finding about the crate.
