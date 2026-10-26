//! Key exchange: X25519 and NIST P-256, behind `rustls`' `SupportedKxGroup`.
//!
//! Two groups, because TLS 1.3 lets the server choose and a client offering only one fails against
//! a peer configured for the other. `github.com`'s chain is ECDSA P-256 throughout
//! (notes/cryptography-provider.md), and X25519 is what almost every modern server prefers for the
//! exchange. The ephemeral secret comes from the primitive crate, not `rustls`' `SecureRandom`, as
//! `rustls` documents ("it is assumed that the cryptography library provides for this itself"). Both
//! draw through `getrandom`'s `SysRng`, which on nife is `entropy_backend` and the entropy service.
//! `UnwrapErr` panics on a failing source, which is what `OsRng` did here before: these crates take
//! an infallible generator and have no error to return.

use alloc::{boxed::Box, vec::Vec};

use getrandom::SysRng;
use getrandom::rand_core::UnwrapErr;
use p256::elliptic_curve::Generate as _;

use rustls::crypto::{ActiveKeyExchange, SharedSecret, SupportedKxGroup};
use rustls::{Error, NamedGroup, PeerMisbehaved};

/// X25519, RFC 7748.
#[derive(Debug)]
pub struct X25519;

/// NIST P-256, in TLS's `secp256r1` spelling.
#[derive(Debug)]
pub struct P256;

impl SupportedKxGroup for X25519 {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, Error> {
        let secret = x25519_dalek::EphemeralSecret::random_from_rng(&mut UnwrapErr(SysRng));
        let public = x25519_dalek::PublicKey::from(&secret);
        Ok(Box::new(ActiveX25519 {
            secret,
            public: public.as_bytes().to_vec(),
        }))
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::X25519
    }

    fn ffdhe_group(&self) -> Option<rustls::ffdhe_groups::FfdheGroup<'static>> {
        // Answered directly rather than left to the default, which `rustls` documents as
        // "extremely linker-unfriendly": its default walks a table of every finite-field group,
        // none of which this provider offers.
        None
    }
}

impl SupportedKxGroup for P256 {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, Error> {
        let secret = p256::ecdh::EphemeralSecret::generate_from_rng(&mut UnwrapErr(SysRng));
        let public = p256::Sec1Point::from(secret.public_key());
        Ok(Box::new(ActiveP256 {
            secret,
            public: public.as_bytes().to_vec(),
        }))
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::secp256r1
    }

    fn ffdhe_group(&self) -> Option<rustls::ffdhe_groups::FfdheGroup<'static>> {
        None
    }
}

struct ActiveX25519 {
    secret: x25519_dalek::EphemeralSecret,
    public: Vec<u8>,
}

impl ActiveKeyExchange for ActiveX25519 {
    fn complete(self: Box<Self>, peer: &[u8]) -> Result<SharedSecret, Error> {
        // **The length check is ours to make.** `x25519_dalek::PublicKey` is built from a fixed
        // array, so a peer share of the wrong length would otherwise panic on the conversion
        // rather than end the handshake. A peer that sends 31 bytes is misbehaving, and saying so
        // is the difference between a rejected connection and a dead process.
        let peer: [u8; 32] = peer
            .try_into()
            .map_err(|_| Error::PeerMisbehaved(PeerMisbehaved::InvalidKeyShare))?;
        let shared = self
            .secret
            .diffie_hellman(&x25519_dalek::PublicKey::from(peer));

        // **A contributory-behaviour check, which RFC 8446 section 7.4.2 requires for X25519.**
        // An all-zero shared secret means the peer sent a low-order point, and the specification
        // says to abort rather than continue with a secret an attacker chose.
        if !shared.was_contributory() {
            return Err(Error::PeerMisbehaved(PeerMisbehaved::InvalidKeyShare));
        }
        Ok(SharedSecret::from(shared.as_bytes().as_slice()))
    }

    fn pub_key(&self) -> &[u8] {
        &self.public
    }

    fn group(&self) -> NamedGroup {
        NamedGroup::X25519
    }
}

struct ActiveP256 {
    secret: p256::ecdh::EphemeralSecret,
    public: Vec<u8>,
}

impl ActiveKeyExchange for ActiveP256 {
    fn complete(self: Box<Self>, peer: &[u8]) -> Result<SharedSecret, Error> {
        // `from_sec1_bytes` rejects a point that is not on the curve and rejects the point at
        // infinity, which is the check RFC 8446 section 4.2.8.2 asks for and the reason nothing
        // here validates coordinates by hand.
        let peer = p256::PublicKey::from_sec1_bytes(peer)
            .map_err(|_| Error::PeerMisbehaved(PeerMisbehaved::InvalidKeyShare))?;
        let shared = self.secret.diffie_hellman(&peer);
        Ok(SharedSecret::from(shared.raw_secret_bytes().as_slice()))
    }

    fn pub_key(&self) -> &[u8] {
        &self.public
    }

    fn group(&self) -> NamedGroup {
        NamedGroup::secp256r1
    }
}
