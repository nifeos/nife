//! Certificate and handshake signature verification: ECDSA over P-256 and P-384, Ed25519, and RSA.
//!
//! Each implements `rustls-pki-types`' `SignatureVerificationAlgorithm`, which `rustls-webpki` calls
//! while walking a chain and `rustls` uses for the peer's handshake signature: a pair of algorithm
//! identifiers plus one call into a primitive. RSA is taken (calef, 2026-09-20: "take rsa") because
//! the hosts that serve the bytes are RSA: `objects.githubusercontent.com` is RSA 2048 and `ghcr.io`
//! RSA 4096. RUSTSEC-2023-0071 is a timing side channel in PKCS#1 v1.5 decryption, which nothing
//! here does; `deny.toml`'s first `ignore` entry bounds that claim. Writing RSA verification here is
//! refused (notes/cryptography-provider/pricing-the-glue-path.md).

//! # The two bounds this file enforces itself
//!
//! **A modulus between 2048 and 8192 bits.** `rsa` will verify against a 512-bit key, and a chain is
//! a stranger's input. The bound is `rustls`' own ring-backed table's, which is where the names
//! `RSA_PKCS1_2048_8192_SHA256` and friends come from.
//!
//! **A PSS salt length equal to the digest length**, `rsa::pss::VerifyingKey::new`'s default. The
//! identifiers in `rustls-pki-types` (`alg_id::RSA_PSS_SHA256` and its siblings) encode exactly that
//! length, so a signature made with another arrives under a different identifier and never reaches
//! this code.

// **One import serves all three**, and that is worth a line rather than a puzzle: `p256`, `p384`
// and `ed25519-dalek` all re-export the same `signature::Verifier` trait, so bringing it into
// scope once covers every `verify` call below. Importing it three times compiles and earns three
// unused-import warnings.
use p256::ecdsa::signature::Verifier as _;
use pki_types::{AlgorithmIdentifier, InvalidSignature, SignatureVerificationAlgorithm, alg_id};
use rsa::pkcs1::DecodeRsaPublicKey as _;
use rsa::traits::PublicKeyParts as _;

/// ECDSA on P-256 with SHA-256.
#[derive(Debug)]
pub struct EcdsaP256Sha256;

/// ECDSA on P-384 with SHA-384.
#[derive(Debug)]
pub struct EcdsaP384Sha384;

/// Ed25519, RFC 8032.
#[derive(Debug)]
pub struct Ed25519;

impl SignatureVerificationAlgorithm for EcdsaP256Sha256 {
    fn public_key_alg_id(&self) -> AlgorithmIdentifier {
        alg_id::ECDSA_P256
    }

    fn signature_alg_id(&self) -> AlgorithmIdentifier {
        alg_id::ECDSA_SHA256
    }

    fn verify_signature(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), InvalidSignature> {
        // X.509 carries an ECDSA signature as a DER `SEQUENCE { r INTEGER, s INTEGER }`, not as
        // the fixed-width pair TLS uses, which is why this is `from_der` rather than `from_slice`.
        // Getting that wrong fails closed, and it fails on every certificate rather than on a
        // subtle one, which is the only comforting thing about it.
        let key = p256::ecdsa::VerifyingKey::from_sec1_bytes(public_key)
            .map_err(|_| InvalidSignature)?;
        let sig = p256::ecdsa::DerSignature::try_from(signature).map_err(|_| InvalidSignature)?;
        key.verify(message, &sig).map_err(|_| InvalidSignature)
    }
}

impl SignatureVerificationAlgorithm for EcdsaP384Sha384 {
    fn public_key_alg_id(&self) -> AlgorithmIdentifier {
        alg_id::ECDSA_P384
    }

    fn signature_alg_id(&self) -> AlgorithmIdentifier {
        alg_id::ECDSA_SHA384
    }

    fn verify_signature(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), InvalidSignature> {
        let key = p384::ecdsa::VerifyingKey::from_sec1_bytes(public_key)
            .map_err(|_| InvalidSignature)?;
        let sig = p384::ecdsa::DerSignature::try_from(signature).map_err(|_| InvalidSignature)?;
        key.verify(message, &sig).map_err(|_| InvalidSignature)
    }
}

impl SignatureVerificationAlgorithm for Ed25519 {
    fn public_key_alg_id(&self) -> AlgorithmIdentifier {
        alg_id::ED25519
    }

    fn signature_alg_id(&self) -> AlgorithmIdentifier {
        alg_id::ED25519
    }

    fn verify_signature(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), InvalidSignature> {
        // Both lengths are fixed and both conversions would panic on a slice of the wrong size, so
        // each is checked rather than unwrapped: this input is a stranger's certificate.
        let key: [u8; 32] = public_key.try_into().map_err(|_| InvalidSignature)?;
        let key =
            ed25519_dalek::VerifyingKey::from_bytes(&key).map_err(|_| InvalidSignature)?;
        let sig: [u8; 64] = signature.try_into().map_err(|_| InvalidSignature)?;
        key.verify(message, &ed25519_dalek::Signature::from_bytes(&sig))
            .map_err(|_| InvalidSignature)
    }
}

/// The modulus bounds every RSA algorithm below enforces, in **bytes**.
///
/// 2048 to 8192 bits, which is `rustls`' own ring-backed table's range and the reason its
/// algorithms are named `RSA_PKCS1_2048_8192_SHA256`. The lower bound is the load-bearing one: a
/// 1024-bit modulus is factorable by a well-resourced attacker and `rsa` itself imposes no
/// minimum, so without this line a certificate chain could be forged by presenting a small key.
const MIN_MODULUS_BYTES: usize = 2048 / 8;
const MAX_MODULUS_BYTES: usize = 8192 / 8;

/// Decode an X.509 RSA public key and check its size.
///
/// The `subjectPublicKey` of an `rsaEncryption` SPKI is a DER `RSAPublicKey ::= SEQUENCE
/// { modulus INTEGER, publicExponent INTEGER }`, which is PKCS#1 rather than PKCS#8: the PKCS#8
/// wrapper is the SPKI itself, and webpki has already unwrapped it by the time this is called.
fn public_key(der: &[u8]) -> Result<rsa::RsaPublicKey, InvalidSignature> {
    let key = rsa::RsaPublicKey::from_pkcs1_der(der).map_err(|_| InvalidSignature)?;
    let size = key.size();
    if !(MIN_MODULUS_BYTES..=MAX_MODULUS_BYTES).contains(&size) {
        return Err(InvalidSignature);
    }
    Ok(key)
}

/// RSASSA-PKCS1-v1_5, which is what almost every certificate in the public web PKI is signed with.
///
/// The parameter is the digest, so one type serves SHA-256, SHA-384 and SHA-512 and the algorithm
/// identifiers are the only thing that differs between the three statics below.
#[derive(Debug)]
pub struct RsaPkcs1<D> {
    signature_alg_id: AlgorithmIdentifier,
    _marker: core::marker::PhantomData<fn() -> D>,
}

/// RSASSA-PSS with the digest's own length as the salt length, which is what TLS 1.3 requires for
/// a handshake signature and what modern certificate authorities increasingly issue.
#[derive(Debug)]
pub struct RsaPss<D> {
    signature_alg_id: AlgorithmIdentifier,
    _marker: core::marker::PhantomData<fn() -> D>,
}

impl<D> SignatureVerificationAlgorithm for RsaPkcs1<D>
where
    D: rsa::sha2::Digest + rsa::pkcs1::der::oid::AssociatedOid + Send + Sync + core::fmt::Debug,
{
    fn public_key_alg_id(&self) -> AlgorithmIdentifier {
        // `rsaEncryption`, which is what an RSA certificate's SPKI names whether the signature on
        // it is PKCS#1 v1.5 or PSS. A key whose SPKI says `RSASSA-PSS` instead is legal and rare,
        // and is not supported here; see this crate's `BUGS`.
        alg_id::RSA_ENCRYPTION
    }

    fn signature_alg_id(&self) -> AlgorithmIdentifier {
        self.signature_alg_id
    }

    fn verify_signature(
        &self,
        public_key_der: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), InvalidSignature> {
        let key = public_key(public_key_der)?;
        let verifying = rsa::pkcs1v15::VerifyingKey::<D>::new(key);
        let sig = rsa::pkcs1v15::Signature::try_from(signature).map_err(|_| InvalidSignature)?;
        rsa::signature::Verifier::verify(&verifying, message, &sig).map_err(|_| InvalidSignature)
    }
}

impl<D> SignatureVerificationAlgorithm for RsaPss<D>
where
    D: rsa::sha2::Digest + rsa::sha2::digest::FixedOutputReset + Send + Sync + core::fmt::Debug,
{
    fn public_key_alg_id(&self) -> AlgorithmIdentifier {
        alg_id::RSA_ENCRYPTION
    }

    fn signature_alg_id(&self) -> AlgorithmIdentifier {
        self.signature_alg_id
    }

    fn verify_signature(
        &self,
        public_key_der: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), InvalidSignature> {
        let key = public_key(public_key_der)?;
        // `new` takes the digest's output size as the salt length, which is the value the
        // algorithm identifier above encodes. See this file's header.
        let verifying = rsa::pss::VerifyingKey::<D>::new(key);
        let sig = rsa::pss::Signature::try_from(signature).map_err(|_| InvalidSignature)?;
        rsa::signature::Verifier::verify(&verifying, message, &sig).map_err(|_| InvalidSignature)
    }
}

/// RSASSA-PKCS1-v1_5 with SHA-256, the commonest signature in the public web PKI.
pub static RSA_PKCS1_SHA256: RsaPkcs1<rsa::sha2::Sha256> = RsaPkcs1 {
    signature_alg_id: alg_id::RSA_PKCS1_SHA256,
    _marker: core::marker::PhantomData,
};

/// RSASSA-PKCS1-v1_5 with SHA-384.
pub static RSA_PKCS1_SHA384: RsaPkcs1<rsa::sha2::Sha384> = RsaPkcs1 {
    signature_alg_id: alg_id::RSA_PKCS1_SHA384,
    _marker: core::marker::PhantomData,
};

/// RSASSA-PKCS1-v1_5 with SHA-512.
pub static RSA_PKCS1_SHA512: RsaPkcs1<rsa::sha2::Sha512> = RsaPkcs1 {
    signature_alg_id: alg_id::RSA_PKCS1_SHA512,
    _marker: core::marker::PhantomData,
};

/// RSASSA-PSS with SHA-256, which TLS 1.3 requires for an RSA handshake signature.
pub static RSA_PSS_SHA256: RsaPss<rsa::sha2::Sha256> = RsaPss {
    signature_alg_id: alg_id::RSA_PSS_SHA256,
    _marker: core::marker::PhantomData,
};

/// RSASSA-PSS with SHA-384.
pub static RSA_PSS_SHA384: RsaPss<rsa::sha2::Sha384> = RsaPss {
    signature_alg_id: alg_id::RSA_PSS_SHA384,
    _marker: core::marker::PhantomData,
};

/// RSASSA-PSS with SHA-512.
pub static RSA_PSS_SHA512: RsaPss<rsa::sha2::Sha512> = RsaPss {
    signature_alg_id: alg_id::RSA_PSS_SHA512,
    _marker: core::marker::PhantomData,
};
