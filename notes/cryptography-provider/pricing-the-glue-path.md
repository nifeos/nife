# Pricing the glue path

*Name: provisional. After the parent's own heading.*

[`notes/cryptography-provider.md`](../cryptography-provider.md) records that calef refused `rustls-rustcrypto` and that this tree writes its own
`CryptoProvider` glue. This is the pricing done before a line of it was written, and the `rsa`
ruling that followed.

The alternative named in this milestone is to write the `CryptoProvider` glue in this tree over
primitives chosen deliberately. That does not avoid depending on the primitives, so it was
priced the same way before a line of it was written.

## The graph, measured the same way for both

`cargo tree -e normal`, normal edges only, so build scripts and proc-macro crates that run on the
host are excluded from every column, so the count is like for like (`Cargo.lock` lines would count
build dependencies too). The honest headline is that the glue path is barely smaller.

| | crates | `rsa` in the graph | alpha crate in the trust path |
|---|---|---|---|
| `rustls` + `rustls-rustcrypto` | **73** | yes | yes |
| **`cryptography_provider` as built** | **67** | yes | **no** |
| the same without `rsa`, which was built first and then ruled against | 53 | no | no |

Re-priced on 2026-09-20, after calef ruled "Take rsa". The provider is 67 crates, which is
exactly the figure this note predicted for "the glue at equal algorithm coverage" before the code
existed, and it is six fewer than the refused alpha. The whole of that six is
`rustls-rustcrypto` itself plus `base64ct`, `paste`, `pem-rfc7468`, `pkcs5` and a second, older
`rustls-webpki` (0.102.8) it pinned beside the 0.103 that `rustls` already wants. The full glue
graph is a strict subset of `rustls-rustcrypto`'s: there is nothing in it that crate does not also
pull.

So the size argument is now as weak as it can be, and the record says so rather than quietly
dropping the comparison. 67 against 73 is not why the glue was written. It was written because
the glue is the piece with the least exposure in the chain, which is DECISIONS §198 (the glue is
ours, the primitives are not).

`rsa` adds fourteen crates: `rsa`, `pkcs1`, `num-bigint-dig`, `num-integer`, `num-iter`,
`num-traits`, `libm`, `lazy_static`, `rand`, `rand_chacha`, `ppv-lite86`, `zerocopy`, `smallvec`
and `spin`.

Two of them runtime-detect SIMD, and neither is the ring-3 hazard, which was checked rather
than assumed. `ppv-lite86` gates its x86_64 module on `target_feature = "sse2"`, and `libm` gates
its x86 arch module the same way. `x86_64-unknown-nife` switches SSE2 off, so both compile to their
portable paths with no flag. Neither gets an opportunity to execute an instruction the target has
no state for. That is the opposite of `chacha20`, which selects by asking the CPU and therefore needed
`--cfg chacha20_force_soft`. Confirmed by the x86_64 leg running both real certificate chains.

The third row is where the real difference is, and it is not size. Writing the glue means
choosing which primitives are in the trust path at all, and the one worth choosing about is `rsa`.

## Quality, per primitive, read rather than recalled

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

Every license is permissive (MIT, Apache-2.0, BSD-3-Clause, ISC), so nothing here touches
DECISIONS §135 (running GPL software is aggregation, the capability boundary is what makes it so,
and packages are how it arrives).

The two audited crates are the two AEADs, and they are the ones handling every byte of every
record. The NCC Group engagement was commissioned by MobileCoin, reported publicly in February
2020, and both crates state its outcome in their own README. That is the strongest evidence in the
table and it covers the record layer.

`p256` says in its own README that it has never been independently audited. It is in the path
either way, because TLS 1.3 negotiates it and because GitHub's own certificate chain is ECDSA
P-256, so this is a fact to record rather than a choice to make.

## The one that was a choice: `rsa`, and how it was answered

calef, 2026-09-20: "Take rsa." It is in, and this section keeps the argument because the entry
it produced in `deny.toml` is the tree's first suppression and the next person will read it as
precedent.

`rsa` 0.9.10's own README says it plainly:

> This crate is vulnerable to the [Marvin Attack] which could enable private key recovery by a
> network attacker (see [RUSTSEC-2023-0071]).

Why the advisory does not describe this use. Marvin is a timing side channel in RSA PKCS#1 v1.5
decryption: it recovers a private key from an oracle that decrypts attacker-chosen
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
claim about a graph no gate looked at. Both are on that list now. That brought three licenses onto
the allow-list with their own reasons: ISC for `rustls-webpki` and `untrusted`, and Unicode-3.0 for
a proc-macro dependency that ships nothing. It also made `publish = false` necessary, because
`cargo-deny` forgives a path dependency only in a package that could not be published.

What it cost to leave out, measured rather than argued, and the reason the ruling went this
way:

| host | chain |
|---|---|
| `github.com` | ECDSA P-256 throughout, `ecdsa-with-SHA256` and `ecdsa-with-SHA384` |
| `objects.githubusercontent.com`, where a Release asset is served | **RSA 2048**, `sha256WithRSAEncryption`, Let's Encrypt |
| `ghcr.io` | **RSA 4096** leaf over a 3072-bit intermediate, Sectigo |

A client without RSA verification reaches GitHub and cannot download the file.

## §46's test, said out loud

The test is §46 (thin primitives or whole subsystems).

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

## What the pricing concluded, and what happened next

The glue path is barely smaller and meaningfully better in the one dimension calef named. At
equal coverage it is 67 crates against 73, which is nothing. It takes an abandoned alpha out of
the trust path and replaces it with code this tree can read, over primitives that are individually
maintained, widely used, and in two cases audited. It also unpins the outdated majors. If the
argument for it were crate count it would be a bad argument, and that is recorded here rather
than smoothed over.

calef ruled on both halves: `rustls-rustcrypto` refused (DECISIONS §198, the glue is ours, the
primitives are not), and then, on 2026-09-20, "Take rsa." The provider is built, the two real
chains verify on all three architectures, and the milestone's own list is finished.
