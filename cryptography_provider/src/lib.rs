//! **A `rustls` crypto provider assembled in this tree, over primitives chosen deliberately**
//! (notes/cryptography-provider.md).
//!
//! Every cryptographic operation is taken from audited crates (§46 (thin primitives or whole
//! subsystems)). This crate is the glue: it selects, names and plumbs the five fields of a
//! [`rustls::crypto::CryptoProvider`] and computes nothing secret-dependent. TLS 1.3 only. On nife
//! a consuming binary also needs `use entropy_backend as _;` and
//! `cryptography_exerciser/.cargo/config.toml`'s build configuration, or the random source has no
//! backend and, on `x86_64-unknown-nife`, a primitive can execute an instruction the target lacks.
//! notes/cryptography-provider/the-glue.md has why it is built this way.

//! # The five fields
//!
//! | field | what this provides |
//! |---|---|
//! | `cipher_suites` | `TLS13_AES_128_GCM_SHA256`, `TLS13_AES_256_GCM_SHA384`, `TLS13_CHACHA20_POLY1305_SHA256` |
//! | `kx_groups` | X25519, NIST P-256 |
//! | `signature_verification_algorithms` | ECDSA P-256/SHA-256, ECDSA P-384/SHA-384, Ed25519, and RSA PKCS#1 v1.5 and PSS with SHA-256, SHA-384 and SHA-512 |
//! | `secure_random` | the entropy service, through `entropy_backend` and `getrandom` |
//! | `key_provider` | refuses: this provider loads no private keys |

//! # BUGS
//!
//! - An RSA key whose SPKI says `RSASSA-PSS` rather than `rsaEncryption`, and a PKCS#1 v1.5
//!   identifier with absent parameters, are not verified; such a chain fails closed.
//! - No TLS 1.2, no client certificates (`key_provider` refuses every key), and no FIPS claim.

#![no_std]

extern crate alloc;

use alloc::{sync::Arc, vec};

use rustls::crypto::{CryptoProvider, KeyProvider, SecureRandom, WebPkiSupportedAlgorithms};
use rustls::sign::SigningKey;
use rustls::{CipherSuite, Error, SignatureScheme, SupportedCipherSuite};

mod aead;
mod hash;
mod hmac;
mod kx;
mod verify;

mod platform;
mod provider;

use platform::*;
pub use provider::{
    TLS13_AES_128_GCM_SHA256, TLS13_AES_256_GCM_SHA384, TLS13_CHACHA20_POLY1305_SHA256, provider,
};
