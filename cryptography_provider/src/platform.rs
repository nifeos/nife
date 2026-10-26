//! **What the provider needs from nife**: randomness from the entropy service, and a key
//! provider that refuses every private key.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// `rustls`' randomness, which on nife is the entropy service.
///
/// The chain is `rustls` to here, to `getrandom`, to
/// `entropy_backend`'s hook, to `std::random::SystemRng`, to the entropy service's one endpoint,
/// to the device. Every link but `getrandom`'s is one this project owns.
#[derive(Debug)]
pub(crate) struct Entropy;

impl SecureRandom for Entropy {
    fn fill(&self, buf: &mut [u8]) -> Result<(), rustls::crypto::GetRandomFailed> {
        // `getrandom::fill` returns an error rather than panicking on a failing source, and a process that cannot get randomness should fail the handshake rather than
        // die. On nife a draw with no entropy capability granted panics inside `SystemRng`
        // regardless, which is the deliberate choice of milestone 56 (secrets, credentials, and
        // the entropy to make them safe) and not something this layer can soften.
        getrandom::fill(buf).map_err(|_| rustls::crypto::GetRandomFailed)
    }
}

/// A key provider that refuses every private key, because this provider is for a client that
/// presents no certificate.
///
/// **A refusal rather than an omission.** `rustls` requires the field, so the choice is between
/// answering honestly and pulling in PKCS#8 parsing and a signing path that nothing here would
/// ever exercise. Anything that needs client authentication needs a different provider, and will
/// find out at the call that loads the key rather than at the handshake.
#[derive(Debug)]
pub(crate) struct NoPrivateKeys;

impl KeyProvider for NoPrivateKeys {
    fn load_private_key(
        &self,
        _key_der: pki_types::PrivateKeyDer<'static>,
    ) -> Result<Arc<dyn SigningKey>, Error> {
        Err(Error::General(alloc::string::String::from(
            "this provider loads no private keys: it is for a client that presents no certificate",
        )))
    }
}
