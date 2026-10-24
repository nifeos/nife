# The glue: why `cryptography_provider` is built the way it is

*Name: provisional (lane comments/929-rest-01, 2026-10-24 UTC). An appendix to
[`notes/cryptography-provider.md`](../cryptography-provider.md). It holds the design reasons behind
the crate's modules; the parent has the build table, what runs, and the refusal of
`rustls-rustcrypto`.*

## Glue, and only glue

§196 (nife carries TLS) made the provider work rather than a purchase. calef refused the one
pure-Rust option, `rustls-rustcrypto`, as low quality. That refusal is about the glue, so this crate
is glue and nothing else.

§46 (thin primitives or whole subsystems) takes cryptography rather than writing it, and every
cryptographic operation here is taken from audited crates. A `rustls` `CryptoProvider` is five
fields, and the crate selects, names and plumbs them. The case for owning it was never size. The glue
is the least-exposed piece of the chain, so it is the piece worth owning.

The provider speaks TLS 1.3 only. A package client talks to hosts that have spoken it for years.
TLS 1.2 would add a second record layer, a second key schedule and RSA key transport.

## The record layer (`aead.rs`)

This is the file with the most opportunity to be wrong, and it computes nothing. Confidentiality and
authenticity come from `aes-gcm` and `chacha20poly1305`, the two crates in the graph that carry NCC
Group's 2020 audit in their own README. What the file does is assemble the record.

`aes-gcm` offers `encrypt_in_place`, which needs a growable `aead::Buffer`, and
`encrypt_in_place_detached`, which takes a plain `&mut [u8]` and hands the tag back. `rustls`' buffers
are its own types and implement neither, so the detached form is the one that avoids copying the
record twice.

The nonce and the additional data come from `rustls` (`Nonce::new`, `make_tls13_aad`) and are used
rather than rewritten. Deriving a nonce looks like four lines and is a catastrophe when the sequence
number is encoded wrongly. `rustls` has already made those four lines part of its own API.

## RSA (`verify.rs`)

calef ruled "take rsa" on 2026-09-20, closing a fork the file had carried open for a day. Without
RSA the provider could verify `github.com`, whose chain is ECDSA P-256 throughout, but not the hosts
that serve the bytes. Why writing RSA verification here is refused, and the bounded `deny.toml`
claim, are in [pricing-the-glue-path.md](pricing-the-glue-path.md).
